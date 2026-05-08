//! AccessKit projection helpers.
//!
//! Currently provides `node_id_from_global` — a deterministic mapping
//! from `GlobalElementId` to `accesskit::NodeId`. Same `GlobalElementId`
//! always hashes to the same `NodeId`, which gives platform a11y
//! adapters a stable identifier for an element across frames as long
//! as the element's path through the element tree is stable.
//!
//! Future additions: focus-handle ↔ NodeId mapping, role helpers, and
//! whatever else the framework integration needs.

use crate::GlobalElementId;
use std::hash::{Hash, Hasher};

/// The synthetic root NodeId under which the framework attaches every
/// element-projected accessibility node. AccessKit requires a single
/// root per tree, but gpui's element trees can have multiple top-level
/// containers; collecting them all under a fixed root is the simplest
/// correct shape and lets [`Element::accessibility`] callers stay
/// ignorant of tree-roots.
pub(crate) const A11Y_ROOT_ID: accesskit::NodeId = accesskit::NodeId(0);

/// Derive a stable `accesskit::NodeId` from a `GlobalElementId`. Hashes
/// every `ElementId` segment in the path; the result is deterministic
/// for the same path across frames and across processes.
///
/// Distinct paths produce distinct ids with very high probability
/// (collision is bounded by the 64-bit hash output). Elements whose
/// path isn't stable across rebuilds — e.g. anonymous children of a
/// dynamic list — should override `Element::accessibility` to return
/// a domain-keyed `NodeId` directly rather than relying on this
/// derivation.
pub fn node_id_from_global(id: &GlobalElementId) -> accesskit::NodeId {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for segment in id.0.iter() {
        segment.hash(&mut hasher);
    }
    accesskit::NodeId::from(hasher.finish())
}

/// Build the full per-frame tree map from the per-element collected
/// nodes. Wraps every collected node under the synthetic
/// [`A11Y_ROOT_ID`] root so the AccessKit invariant ("every node is
/// either the root or a child of one in the tree") holds. The
/// returned `HashMap` is what we diff against next frame's full
/// tree.
pub(crate) fn full_tree(
    collected: Vec<(accesskit::NodeId, accesskit::Node)>,
) -> std::collections::HashMap<accesskit::NodeId, accesskit::Node> {
    let child_ids: Vec<accesskit::NodeId> = collected.iter().map(|(id, _)| *id).collect();
    let mut root = accesskit::Node::new(accesskit::Role::Window);
    root.set_children(child_ids);

    let mut map = std::collections::HashMap::with_capacity(collected.len() + 1);
    map.insert(A11Y_ROOT_ID, root);
    for (id, node) in collected {
        map.insert(id, node);
    }
    map
}

/// Compute a `TreeUpdate` containing only the nodes that differ from
/// `last`. On `declare_tree` the update includes every node in
/// `current` (AccessKit requires a complete tree on initialization);
/// otherwise emits only added/changed nodes.
///
/// Removal is implicit: a node that was in `last` but not in `current`
/// is already absent from its parent's `children` list (parents are
/// rebuilt every frame from the current collected set), so the
/// parent's updated entry is in the diff and AccessKit treats the
/// dropped child as removed per its standard semantics.
pub(crate) fn diff_tree_update(
    current: &std::collections::HashMap<accesskit::NodeId, accesskit::Node>,
    last: &std::collections::HashMap<accesskit::NodeId, accesskit::Node>,
    declare_tree: bool,
) -> accesskit::TreeUpdate {
    let nodes: Vec<(accesskit::NodeId, accesskit::Node)> = if declare_tree {
        current.iter().map(|(id, n)| (*id, n.clone())).collect()
    } else {
        current
            .iter()
            .filter(|(id, n)| last.get(id) != Some(*n))
            .map(|(id, n)| (*id, n.clone()))
            .collect()
    };

    accesskit::TreeUpdate {
        nodes,
        tree: if declare_tree {
            Some(accesskit::Tree::new(A11Y_ROOT_ID))
        } else {
            None
        },
        tree_id: accesskit::TreeId::ROOT,
        // Focus mapping is a follow-up; for now point at the root.
        focus: A11Y_ROOT_ID,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ElementId, GlobalElementId};
    use std::sync::Arc;

    fn make_global(segments: Vec<ElementId>) -> GlobalElementId {
        GlobalElementId(Arc::from(segments.as_slice()))
    }

    #[test]
    fn node_id_is_deterministic() {
        let path = make_global(vec![
            ElementId::Name("root".into()),
            ElementId::Integer(42),
            ElementId::Name("child".into()),
        ]);
        let a = node_id_from_global(&path);
        let b = node_id_from_global(&path);
        assert_eq!(
            a, b,
            "the same GlobalElementId must hash to the same NodeId"
        );
    }

    #[test]
    fn distinct_paths_have_distinct_ids() {
        let p1 = make_global(vec![ElementId::Name("button".into())]);
        let p2 = make_global(vec![ElementId::Name("link".into())]);
        let p3 = make_global(vec![
            ElementId::Name("button".into()),
            ElementId::Integer(0),
        ]);
        assert_ne!(node_id_from_global(&p1), node_id_from_global(&p2));
        assert_ne!(node_id_from_global(&p1), node_id_from_global(&p3));
        assert_ne!(node_id_from_global(&p2), node_id_from_global(&p3));
    }

    #[test]
    fn first_emission_includes_full_tree() {
        let mut button = accesskit::Node::new(accesskit::Role::Button);
        button.set_label("Hi");
        let collected = vec![(accesskit::NodeId::from(42u64), button)];
        let current = full_tree(collected);

        let update = diff_tree_update(&current, &std::collections::HashMap::new(), true);
        assert!(
            update.tree.is_some(),
            "first emission must declare tree (TreeUpdate::tree required by AccessKit on init)"
        );
        assert_eq!(update.tree.as_ref().unwrap().root, A11Y_ROOT_ID);
        // Synthetic root + the one pushed child.
        assert_eq!(update.nodes.len(), 2);
        assert_eq!(update.focus, A11Y_ROOT_ID);
    }

    #[test]
    fn subsequent_emission_skips_tree_field() {
        let current = full_tree(vec![]);
        let update = diff_tree_update(&current, &current, false);
        assert!(update.tree.is_none());
    }

    #[test]
    fn unchanged_emission_is_empty_diff() {
        let mut button = accesskit::Node::new(accesskit::Role::Button);
        button.set_label("Hi");
        let collected = vec![(accesskit::NodeId::from(42u64), button)];
        let current = full_tree(collected);
        let update = diff_tree_update(&current, &current, false);
        assert!(
            update.nodes.is_empty(),
            "diff against an identical previous tree must be empty"
        );
    }

    #[test]
    fn changed_node_only_in_diff() {
        let id = accesskit::NodeId::from(42u64);
        let mut button_v1 = accesskit::Node::new(accesskit::Role::Button);
        button_v1.set_label("Hi");
        let mut button_v2 = accesskit::Node::new(accesskit::Role::Button);
        button_v2.set_label("Bye"); // label changed
        let last = full_tree(vec![(id, button_v1)]);
        let current = full_tree(vec![(id, button_v2)]);

        let update = diff_tree_update(&current, &last, false);
        // Root's children list is the same (just [42]) → root unchanged.
        // Only the button (id=42) differs by label.
        assert_eq!(update.nodes.len(), 1);
        assert_eq!(update.nodes[0].0, id);
        assert_eq!(update.nodes[0].1.label(), Some("Bye"));
    }
}
