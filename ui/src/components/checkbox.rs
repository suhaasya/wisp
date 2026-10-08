use gpui::{
    div, px, Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, Window,
};

use crate::{
    components::focus::focus_visible_ring,
    theme::ResolvedTheme,
};

pub fn checkbox<V: 'static>(
    cx: &mut Context<V>,
    id: &'static str,
    label: &'static str,
    checked: bool,
    theme: &ResolvedTheme,
    on_toggle: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let c = &theme.colors;
    let mark = if checked { "✓" } else { "" };
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_2()
        .tab_index(0)
        .focus_visible(focus_visible_ring(c))
        .cursor_pointer()
        .on_click(cx.listener(move |view, _, window, cx| on_toggle(view, window, cx)))
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(16.))
                .rounded_sm()
                .border_1()
                .border_color(if checked { c.accent } else { c.line })
                .bg(if checked { c.accent_soft } else { c.panel })
                .text_xs()
                .text_color(c.accent)
                .child(mark),
        )
        .child(
            div()
                .text_sm()
                .text_color(c.ink1)
                .child(label),
        )
}
