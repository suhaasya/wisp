use gpui::{div, IntoElement, ParentElement, Styled};

use crate::theme::ResolvedTheme;

/// Static helper text shown adjacent to a control (full hover tooltips need deferred layout).
pub fn tooltip_hint(theme: &ResolvedTheme, text: &'static str) -> impl IntoElement {
    let c = &theme.colors;
    div()
        .absolute()
        .top_full()
        .mt_1()
        .px_2()
        .py_1()
        .rounded_sm()
        .border_1()
        .border_color(c.line)
        .bg(c.panel)
        .text_xs()
        .text_color(c.ink2)
        .child(text)
}
