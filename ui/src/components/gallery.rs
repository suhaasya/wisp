//! Debug component gallery (`ui-gallery` feature).

use gpui::{
    div, prelude::*, px, Context, Entity, FocusHandle, Focusable, ParentElement, Render, Styled,
    Window,
};

use crate::{
    components::{
        button::{button, icon_button, ButtonVariant},
        checkbox, overlay,
        segmented::segmented_control,
        select,
        tabs::horizontal_tabs,
        text_input::{TextInput, TextInputKind},
        toast, toggle, tooltip,
    },
    theme::{self, ResolvedTheme},
};

pub struct ComponentGallery {
    focus_handle: FocusHandle,
    theme: ResolvedTheme,
    tab: usize,
    segmented: usize,
    select_index: usize,
    select_open: bool,
    checkbox: bool,
    toggle_on: bool,
    modal_open: bool,
    sheet_open: bool,
    show_toast: bool,
    show_tooltip: bool,
    list_index: usize,
    plain_input: Entity<TextInput>,
    password_input: Entity<TextInput>,
    mono_input: Entity<TextInput>,
}

impl ComponentGallery {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let theme = theme::read_global(cx).resolved().clone();
        Self {
            focus_handle: cx.focus_handle(),
            theme: theme.clone(),
            tab: 0,
            segmented: 0,
            select_index: 0,
            select_open: false,
            checkbox: true,
            toggle_on: false,
            modal_open: false,
            sheet_open: false,
            show_toast: true,
            show_tooltip: false,
            list_index: 0,
            plain_input: cx.new(|cx| {
                TextInput::new(cx, "Plain text…", TextInputKind::SingleLine, theme.clone())
            }),
            password_input: cx.new(|cx| {
                TextInput::new(cx, "Password…", TextInputKind::Password, theme.clone())
            }),
            mono_input: cx.new(|cx| {
                TextInput::new(cx, "host:5432", TextInputKind::Mono, theme.clone())
            }),
        }
    }

    fn refresh_theme(&mut self, cx: &mut Context<Self>) {
        self.theme = theme::read_global(cx).resolved().clone();
        for input in [&self.plain_input, &self.password_input, &self.mono_input] {
            input.update(cx, |input, cx| {
                input.set_theme(self.theme.clone());
                cx.notify();
            });
        }
    }
}

impl Render for ComponentGallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        self.refresh_theme(cx);
        let c = &self.theme.colors;
        let list_items = ["Connections", "Schemas", "Tables", "Columns"];
        let select_options = ["Local", "Dev", "Staging", "Production"];

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(c.canvas)
            .text_color(c.ink1)
            .text_size(self.theme.ui_font_size)
            .font(self.theme.typography.gpui_ui_font())
            .track_focus(&self.focus_handle(cx))
            .child(
                div()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(c.line2)
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Component gallery"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(c.ink3)
                            .child("Visual review surface for LUM-007 (not shipped in release)."),
                    ),
            )
            .child(
                div()
                    .id("gallery-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_6()
                    .child(section(
                        "Buttons",
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(button(
                                cx,
                                "btn-default",
                                "Default",
                                ButtonVariant::Default,
                                &self.theme,
                                |_, _, _| {},
                            ))
                            .child(button(
                                cx,
                                "btn-primary",
                                "Primary",
                                ButtonVariant::Primary,
                                &self.theme,
                                |_, _, _| {},
                            ))
                            .child(button(
                                cx,
                                "btn-warning",
                                "Warning",
                                ButtonVariant::Warning,
                                &self.theme,
                                |_, _, _| {},
                            ))
                            .child(button(
                                cx,
                                "btn-ghost",
                                "Ghost",
                                ButtonVariant::Ghost,
                                &self.theme,
                                |_, _, _| {},
                            ))
                            .child(icon_button(
                                cx,
                                "btn-icon",
                                "⋯",
                                ButtonVariant::Default,
                                &self.theme,
                                |_, _, _| {},
                            )),
                    ))
                    .child(section(
                        "Text inputs",
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .w(px(360.))
                            .child(self.plain_input.clone())
                            .child(self.password_input.clone())
                            .child(self.mono_input.clone()),
                    ))
                    .child(section(
                        "Select",
                        div().w(px(280.)).child(select::select(
                            cx,
                            "gallery-select",
                            "Environment",
                            &select_options,
                            self.select_index,
                            self.select_open,
                            &self.theme,
                            |this, _, _| {
                                this.select_open = !this.select_open;
                            },
                            |this, index, _, cx| {
                                this.select_index = index;
                                this.select_open = false;
                                cx.notify();
                            },
                        )),
                    ))
                    .child(section(
                        "Segmented control",
                        segmented_control(
                            cx,
                            "gallery-segmented",
                            &["Grid", "Form", "SQL"],
                            self.segmented,
                            &self.theme,
                            |this, index, _, cx| {
                                this.segmented = index;
                                cx.notify();
                            },
                        ),
                    ))
                    .child(section(
                        "Checkbox & toggle",
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(checkbox::checkbox(
                                cx,
                                "gallery-checkbox",
                                "Remember password",
                                self.checkbox,
                                &self.theme,
                                |this, _, cx| {
                                    this.checkbox = !this.checkbox;
                                    cx.notify();
                                },
                            ))
                            .child(toggle::toggle(
                                cx,
                                "gallery-toggle",
                                "Use SSL",
                                self.toggle_on,
                                &self.theme,
                                |this, _, cx| {
                                    this.toggle_on = !this.toggle_on;
                                    cx.notify();
                                },
                            )),
                    ))
                    .child(section(
                        "Tabs",
                        horizontal_tabs(
                            cx,
                            "gallery-tabs",
                            &["Overview", "Columns", "Indexes"],
                            self.tab,
                            &self.theme,
                            |this, index, _, cx| {
                                this.tab = index;
                                cx.notify();
                            },
                        ),
                    ))
                    .child(section(
                        "List",
                        div().w(px(240.)).child(crate::components::list::keyboard_list(
                            cx,
                            "gallery-list",
                            &list_items,
                            self.list_index,
                            &self.theme,
                            |this, index, _, cx| {
                                this.list_index = index;
                                cx.notify();
                            },
                        )),
                    ))
                    .child(section(
                        "Overlays",
                        div()
                            .flex()
                            .gap_2()
                            .child(button(
                                cx,
                                "open-modal",
                                "Open modal",
                                ButtonVariant::Default,
                                &self.theme,
                                |this, _, cx| {
                                    this.modal_open = true;
                                    cx.notify();
                                },
                            ))
                            .child(button(
                                cx,
                                "open-sheet",
                                "Open sheet",
                                ButtonVariant::Default,
                                &self.theme,
                                |this, _, cx| {
                                    this.sheet_open = true;
                                    cx.notify();
                                },
                            )),
                    ))
                    .child(section(
                        "Tooltip",
                        div()
                            .child(
                                button(
                                    cx,
                                    "tooltip-target",
                                    "Hover target",
                                    ButtonVariant::Ghost,
                                    &self.theme,
                                    |this, _, cx| {
                                        this.show_tooltip = !this.show_tooltip;
                                        cx.notify();
                                    },
                                ),
                            )
                            .children(if self.show_tooltip {
                                Some(tooltip::tooltip_hint(&self.theme, "Example tooltip copy"))
                            } else {
                                None
                            }),
                    )),
            )
            .children(if self.modal_open {
                Some(overlay::modal_centered(overlay::modal_panel(
                        &self.theme,
                        "Example modal",
                        &self.focus_handle,
                        div()
                            .text_sm()
                            .text_color(c.ink2)
                            .child("Modal body with focus trap on the panel."),
                        overlay::overlay_close(cx, &self.theme, |this, _, cx| {
                            this.modal_open = false;
                            cx.notify();
                        }),
                    ),
                ))
            } else {
                None
            })
            .children(if self.sheet_open {
                Some(overlay::modal_sheet(overlay::modal_panel(
                        &self.theme,
                        "Example sheet",
                        &self.focus_handle,
                        div()
                            .text_sm()
                            .text_color(c.ink2)
                            .child("Bottom sheet variant."),
                        overlay::overlay_close(cx, &self.theme, |this, _, cx| {
                            this.sheet_open = false;
                            cx.notify();
                        }),
                    ),
                ))
            } else {
                None
            })
            .children(if self.show_toast {
                Some(toast::toast(
                    &self.theme,
                    "Saved connection settings",
                ))
            } else {
                None
            })
    }
}

impl Focusable for ComponentGallery {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn section(title: &'static str, body: impl gpui::IntoElement) -> impl gpui::IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(body)
}
