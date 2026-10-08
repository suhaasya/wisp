use std::rc::Rc;

use gpui::{
    div, Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, Window,
};

use crate::{
    components::focus::focus_visible_ring,
    theme::ResolvedTheme,
};

/// Vertical list with arrow-key navigation (`on_move` receives new index).
pub fn keyboard_list<V: 'static>(
    cx: &mut Context<V>,
    id: &'static str,
    items: &[&'static str],
    selected: usize,
    theme: &ResolvedTheme,
    on_select: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let c = &theme.colors;
    let item_count = items.len();
    let on_select = Rc::new(on_select);
    div()
        .id(id)
        .flex()
        .flex_col()
        .children(items.iter().enumerate().map(|(index, label)| {
            let active = index == selected;
            let click = on_select.clone();
            let keys = on_select.clone();
            div()
                .id((id, index as u32))
                .px_2()
                .h(theme.density.row_height())
                .flex()
                .items_center()
                .tab_index(0)
                .focus_visible(focus_visible_ring(c))
                .cursor_pointer()
                .bg(if active { c.accent_soft } else { c.panel })
                .text_color(if active { c.ink1 } else { c.ink2 })
                .text_sm()
                .child(*label)
                .on_click(cx.listener(move |view, _, window, cx| {
                    click(view, index, window, cx);
                }))
                .on_key_down(cx.listener(
                    move |view, event: &gpui::KeyDownEvent, window, cx| {
                        let key = event.keystroke.key.as_str();
                        let next = match key {
                            "up" | "k" if index > 0 => Some(index - 1),
                            "down" | "j" if index + 1 < item_count => Some(index + 1),
                            _ => None,
                        };
                        if let Some(i) = next {
                            keys(view, i, window, cx);
                        }
                    },
                ))
        }))
}
