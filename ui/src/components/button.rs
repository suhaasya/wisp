//! Buttons and icon buttons.

use gpui::{
    div, Context, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window,
};

use crate::{
    components::focus::focus_visible_ring,
    theme::{ResolvedColors, ResolvedTheme},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Default,
    Primary,
    Warning,
    Ghost,
}

pub fn button<V: 'static>(
    cx: &mut Context<V>,
    id: &'static str,
    label: &'static str,
    variant: ButtonVariant,
    theme: &ResolvedTheme,
    on_click: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    styled_button(cx, id, label, variant, theme, false, on_click)
}

pub fn icon_button<V: 'static>(
    cx: &mut Context<V>,
    id: &'static str,
    label: &'static str,
    variant: ButtonVariant,
    theme: &ResolvedTheme,
    on_click: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    styled_button(cx, id, label, variant, theme, true, on_click)
}

fn styled_button<V: 'static>(
    cx: &mut Context<V>,
    id: &'static str,
    label: &'static str,
    variant: ButtonVariant,
    theme: &ResolvedTheme,
    icon: bool,
    on_click: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let c = &theme.colors;
    let (bg, fg, border) = button_colors(variant, c);
    let row_h = theme.density.row_height();
    let mut el = div()
        .id(id)
        .tab_index(0)
        .focus_visible(focus_visible_ring(c))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .h(row_h);
    if icon {
        el = el.w(row_h).min_w(row_h).text_sm();
    } else {
        el = el.px_3();
    }
    el
        .rounded_md()
        .border_1()
        .border_color(border)
        .bg(bg)
        .text_color(fg)
        .text_size(theme.ui_font_size)
        .font(theme.typography.gpui_ui_font())
        .child(label)
        .hover(|s| {
            s.bg(match variant {
                ButtonVariant::Ghost => c.raise,
                ButtonVariant::Primary => c.accent,
                _ => c.raise,
            })
        })
        .on_click(cx.listener(move |view, _, window, cx| on_click(view, window, cx)))
}

fn button_colors(
    variant: ButtonVariant,
    c: &ResolvedColors,
) -> (gpui::Rgba, gpui::Rgba, gpui::Rgba) {
    match variant {
        ButtonVariant::Default => (c.panel, c.ink1, c.line),
        ButtonVariant::Primary => (c.accent, c.on_accent, c.accent),
        ButtonVariant::Warning => (c.staged.deleted, c.ink1, c.staged.deleted),
        ButtonVariant::Ghost => (c.sidebar, c.ink2, c.line2),
    }
}

/// Small square icon affordance (text label stands in for an icon in M0).
pub struct IconButton {
    focus_handle: FocusHandle,
    label: &'static str,
    variant: ButtonVariant,
}

impl IconButton {
    pub fn new(cx: &mut Context<Self>, label: &'static str, variant: ButtonVariant) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            label,
            variant,
        }
    }
}

impl gpui::Render for IconButton {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = crate::theme::read_global(cx).resolved().clone();
        let c = &theme.colors;
        let (bg, fg, border) = button_colors(self.variant, c);
        div()
            .id("wisp-icon-button")
            .track_focus(&self.focus_handle(cx))
            .tab_index(0)
            .focus_visible(focus_visible_ring(c))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .size(theme.density.row_height())
            .rounded_md()
            .border_1()
            .border_color(border)
            .bg(bg)
            .text_color(fg)
            .child(self.label)
    }
}

impl Focusable for IconButton {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
