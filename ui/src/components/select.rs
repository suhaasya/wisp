use std::rc::Rc;

use gpui::{
    div, Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, Window,
};

use crate::{
    components::{focus::focus_visible_ring, list::keyboard_list},
    theme::ResolvedTheme,
};

#[allow(clippy::too_many_arguments)]
pub fn select<V: 'static>(
    cx: &mut Context<V>,
    id: &'static str,
    label: &'static str,
    options: &[&'static str],
    selected: usize,
    open: bool,
    theme: &ResolvedTheme,
    on_open: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
    on_select: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let c = &theme.colors;
    let on_open = Rc::new(on_open);
    let on_select = Rc::new(on_select);
    let current = options.get(selected).copied().unwrap_or("—");
    div()
        .id(id)
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_xs()
                .text_color(c.ink3)
                .child(label),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .id(id)
                        .flex()
                        .items_center()
                        .justify_between()
                        .h(theme.density.row_height())
                        .px_2()
                        .rounded_md()
                        .border_1()
                        .border_color(c.line)
                        .bg(c.panel)
                        .tab_index(0)
                        .focus_visible(focus_visible_ring(c))
                        .cursor_pointer()
                        .on_click({
                            let on_open = on_open.clone();
                            cx.listener(move |view, _, window, cx| (on_open)(view, window, cx))
                        })
                        .child(
                            div()
                                .text_sm()
                                .text_color(c.ink1)
                                .child(current),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(c.ink3)
                                .child("▾"),
                        ),
                )
                .children(if open {
                    Some(
                        div()
                            .mt_1()
                            .border_1()
                            .border_color(c.line)
                            .rounded_md()
                            .bg(c.panel)
                            .overflow_hidden()
                            .child(keyboard_list(
                                cx,
                                "wisp-select-list",
                                options,
                                selected,
                                theme,
                                move |view, index, window, cx| {
                                    (on_select)(view, index, window, cx);
                                },
                            )),
                    )
                } else {
                    None
                }),
        )
}
