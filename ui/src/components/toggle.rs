use gpui::{
    div, px, Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, Window,
};

use crate::{
    components::focus::focus_visible_ring,
    theme::ResolvedTheme,
};

pub fn toggle<V: 'static>(
    cx: &mut Context<V>,
    id: &'static str,
    label: &'static str,
    on: bool,
    theme: &ResolvedTheme,
    on_toggle: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let c = &theme.colors;
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
                .w(px(36.))
                .h(px(20.))
                .rounded_full()
                .bg(if on { c.accent } else { c.line })
                .p(px(2.))
                .child(
                    div()
                        .size(px(16.))
                        .rounded_full()
                        .bg(c.panel)
                        .ml(if on { px(16.) } else { px(0.) }),
                ),
        )
        .child(
            div()
                .text_sm()
                .text_color(c.ink1)
                .child(label),
        )
}
