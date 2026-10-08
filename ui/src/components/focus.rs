//! Shared focus-ring styling for interactive components.

use gpui::{px, StyleRefinement, Styled};

use crate::theme::ResolvedColors;

/// Visible focus ring using theme `focus` colour (WCAG-friendly indicator).
pub fn focus_visible_ring(
    colors: &ResolvedColors,
) -> impl FnOnce(StyleRefinement) -> StyleRefinement {
    let focus = colors.focus;
    move |style| style.border(px(2.)).border_color(focus)
}
