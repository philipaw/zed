use refineable::Refineable as _;

use crate::{
    App, Background, Corners, Element, ElementId, GlobalElementId, Hsla, InspectorElementId,
    IntoElement, Pixels, Style, StyleRefinement, Styled, Window, fill, transparent_black,
};

/// Construct a glass panel element (gem Track G). Emits a [`crate::GlassPanel`]
/// scene primitive covering the element's layout bounds. The renderer splits
/// the frame at the first glass panel — content painted before it becomes
/// blur-able backdrop, the panel and everything after render on top. In G2 the
/// panel draws its solid mapping (`background` as a plain quad); the blur and
/// glass-optics composite land in later sittings.
///
/// Size and position it like any element (it implements [`Styled`]); the
/// visual panel parameters are set with the builder methods, not the style.
pub fn glass_panel(background: impl Into<Background>) -> GlassPanelElement {
    GlassPanelElement {
        style: StyleRefinement::default(),
        background: background.into(),
        border_color: transparent_black(),
        corner_radii: Corners::default(),
        strong: false,
    }
}

/// A glass panel element; see [`glass_panel`].
pub struct GlassPanelElement {
    style: StyleRefinement,
    background: Background,
    border_color: Hsla,
    corner_radii: Corners<Pixels>,
    strong: bool,
}

impl GlassPanelElement {
    /// Rounded-rect corner radii of the panel.
    pub fn corner_radii(mut self, corner_radii: impl Into<Corners<Pixels>>) -> Self {
        self.corner_radii = corner_radii.into();
        self
    }

    /// Border color used by the solid mapping (and the G4 rim tint).
    pub fn border_color(mut self, border_color: impl Into<Hsla>) -> Self {
        self.border_color = border_color.into();
        self
    }

    /// Select the `FILL_STRONG` mapping (PlayerDock / edge toolbars).
    pub fn strong(mut self, strong: bool) -> Self {
        self.strong = strong;
        self
    }
}

impl IntoElement for GlassPanelElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for GlassPanelElement {
    type RequestLayoutState = Style;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (crate::LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.refine(&self.style);
        let layout_id = window.request_layout(style.clone(), [], cx);
        (layout_id, style)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: crate::Bounds<Pixels>,
        _request_layout: &mut Style,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: crate::Bounds<Pixels>,
        _style: &mut Style,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let _ = cx;
        window.paint_glass_panel(
            fill(bounds, self.background)
                .corner_radii(self.corner_radii)
                .border_color(self.border_color),
            self.strong,
        );
    }
}

impl Styled for GlassPanelElement {
    fn style(&mut self) -> &mut crate::StyleRefinement {
        &mut self.style
    }
}
