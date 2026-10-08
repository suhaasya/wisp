use std::rc::Rc;

use gpui::{
    div, Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, Window,
};

use crate::{
    components::focus::focus_visible_ring,
    theme::ResolvedTheme,
};

pub fn horizontal_tabs<V: 'static>(
    cx: &mut Context<V>,
    id: &'static str,
    tabs: &[&'static str],
    selected: usize,
    theme: &ResolvedTheme,
    on_select: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let c = &theme.colors;
    let on_select = Rc::new(on_select);
    div()
        .id(id)
        .flex()
        .gap_1()
        .border_b_1()
        .border_color(c.line2)
        .children(tabs.iter().enumerate().map(|(index, label)| {
            let active = index == selected;
            let on_select = on_select.clone();
            div()
                .id((id, index))
                .px_3()
                .h(theme.density.row_height())
                .flex()
                .items_center()
                .tab_index(0)
                .focus_visible(focus_visible_ring(c))
                .cursor_pointer()
                .border_b_2()
                .border_color(if active { c.accent } else { c.panel })
                .text_color(if active { c.ink1 } else { c.ink2 })
                .text_sm()
                .child(*label)
                .on_click(cx.listener(move |view, _, window, cx| {
                    on_select(view, index, window, cx);
                }))
        }))
}
