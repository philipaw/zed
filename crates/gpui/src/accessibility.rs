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
}
