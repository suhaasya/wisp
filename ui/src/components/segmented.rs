use std::rc::Rc;

use gpui::{
    div, Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, Window,
};

use crate::{
    components::focus::focus_visible_ring,
    theme::ResolvedTheme,
};

pub fn segmented_control<V: 'static>(
    cx: &mut Context<V>,
    id: &'static str,
    options: &[&'static str],
    selected: usize,
    theme: &ResolvedTheme,
    on_select: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let c = &theme.colors;
    let on_select = Rc::new(on_select);
    div()
        .id(id)
        .flex()
        .rounded_md()
        .border_1()
        .border_color(c.line)
        .overflow_hidden()
        .children(options.iter().enumerate().map(|(index, label)| {
            let active = index == selected;
            let on_select = on_select.clone();
            div()
                .id((id, index))
                .flex_1()
                .px_3()
                .h(theme.density.row_height())
                .flex()
                .items_center()
                .justify_center()
                .tab_index(0)
                .focus_visible(focus_visible_ring(c))
                .cursor_pointer()
                .bg(if active { c.panel } else { c.sidebar })
                .text_color(if active { c.ink1 } else { c.ink2 })
                .text_sm()
                .child(*label)
                .on_click(cx.listener(move |view, _, window, cx| {
                    on_select(view, index, window, cx);
                }))
        }))
}
