use gpui::{
    div, px, Context, FocusHandle, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled,
};

use crate::{
    components::focus::focus_visible_ring,
    theme::ResolvedTheme,
};

pub fn modal_panel(
    theme: &ResolvedTheme,
    title: &'static str,
    focus: &FocusHandle,
    body: impl IntoElement,
    close_button: impl IntoElement,
) -> impl IntoElement {
    let c = &theme.colors;
    div()
        .flex()
        .flex_col()
        .w(px(480.))
        .max_w_full()
        .bg(c.panel)
        .border_1()
        .border_color(c.line)
        .rounded_lg()
        .shadow_lg()
        .track_focus(focus)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px_4()
                .h(theme.density.row_height())
                .border_b_1()
                .border_color(c.line2)
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(c.ink1)
                        .child(title),
                )
                .child(close_button),
        )
        .child(div().p_4().child(body))
}

pub fn modal_centered(panel: impl IntoElement) -> impl IntoElement {
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::hsla(0., 0., 0., 0.35))
        .child(panel)
}

pub fn modal_sheet(panel: impl IntoElement) -> impl IntoElement {
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_end()
        .pb_4()
        .bg(gpui::hsla(0., 0., 0., 0.35))
        .child(div().w_full().flex().justify_center().child(panel))
}

pub fn overlay_close<V: 'static>(
    cx: &mut Context<V>,
    theme: &ResolvedTheme,
    on_dismiss: impl Fn(&mut V, &mut gpui::Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let c = &theme.colors;
    div()
        .id("modal-dismiss")
        .tab_index(0)
        .focus_visible(focus_visible_ring(c))
        .cursor_pointer()
        .text_sm()
        .text_color(c.ink2)
        .child("Close")
        .on_click(cx.listener(move |view, _, window, cx| on_dismiss(view, window, cx)))
}
