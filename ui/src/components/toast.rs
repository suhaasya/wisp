use gpui::{div, IntoElement, ParentElement, SharedString, Styled};

use crate::theme::ResolvedTheme;

pub fn toast(theme: &ResolvedTheme, message: impl Into<SharedString>) -> impl IntoElement {
    let c = &theme.colors;
    div()
        .absolute()
        .bottom_4()
        .right_4()
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(c.line)
        .bg(c.raise)
        .shadow_md()
        .text_sm()
        .text_color(c.ink1)
        .child(message.into())
}
