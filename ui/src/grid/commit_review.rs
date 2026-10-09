//! Review SQL dialog before commit (LUM-029).

use gpui::{
    div, prelude::*, px, Context, Entity, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, SharedString, Styled,
};

use wisp_core::CommitStatement;

use crate::components::{
    overlay::{self, overlay_close},
    text_input::{TextInput, TextInputKind},
};
use crate::theme::ResolvedTheme;

use super::data_grid::DataGrid;

pub struct CommitReviewState {
    pub open: bool,
    pub statements: Vec<CommitStatement>,
    pub sql_lines: Vec<String>,
    pub connection_label: SharedString,
    pub environment_label: SharedString,
    pub requires_delete_confirm: bool,
    pub confirm_input: Entity<TextInput>,
    pub committing: bool,
    pub error: Option<(usize, String)>,
}

impl CommitReviewState {
    pub fn new(cx: &mut Context<DataGrid>, theme: &ResolvedTheme) -> Self {
        Self {
            open: false,
            statements: Vec::new(),
            sql_lines: Vec::new(),
            connection_label: "—".into(),
            environment_label: "—".into(),
            requires_delete_confirm: false,
            confirm_input: cx.new(|cx| {
                TextInput::new(
                    cx,
                    "Type table name…",
                    TextInputKind::SingleLine,
                    theme.clone(),
                )
            }),
            committing: false,
            error: None,
        }
    }
}

pub fn render_commit_review(
    cx: &mut Context<DataGrid>,
    theme: &ResolvedTheme,
    state: &CommitReviewState,
    focus: &FocusHandle,
) -> impl IntoElement {
    if !state.open {
        return div().into_any_element();
    }
    let c = &theme.colors;
    let count = state.sql_lines.len();
    let conn = state.connection_label.clone();
    let env = state.environment_label.clone();
    let lines = state.sql_lines.clone();
    let needs_confirm = state.requires_delete_confirm;
    let confirm = state.confirm_input.clone();
    let error = state.error.clone();

    overlay::modal_centered(overlay::modal_panel(
        theme,
        "Review SQL",
        focus,
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .text_color(c.ink2.clone())
                    .child(format!(
                        "{count} statement{} · {conn} · {env}",
                        if count == 1 { "" } else { "s" }
                    )),
            )
            .child(
                div()
                    .id("commit-sql-list")
                    .max_h(px(280.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_2()
                    .rounded_md()
                    .border_1()
                    .border_color(c.line.clone())
                    .bg(c.panel.clone())
                    .children(lines.iter().enumerate().map(|(i, line)| {
                        div()
                            .text_xs()
                            .font(theme.typography.gpui_mono_font())
                            .text_color(c.ink1.clone())
                            .child(format!("{}. {line}", i + 1))
                            .into_any_element()
                    })),
            )
            .when(needs_confirm, |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(c.staged.deleted.clone())
                                .child("Production delete: type the table name to confirm"),
                        )
                        .child(confirm),
                )
            })
            .when(error.is_some(), |el| {
                let (ix, msg) = error.unwrap();
                el.child(
                    div()
                        .text_xs()
                        .text_color(c.staged.deleted.clone())
                        .child(format!("Statement {} failed: {msg}", ix + 1)),
                )
            })
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .pt_2()
                    .child(
                        div()
                            .id("commit-keep-editing")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .text_sm()
                            .cursor_pointer()
                            .border_1()
                            .border_color(c.line.clone())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.commit_review.open = false;
                                this.commit_review.error = None;
                                cx.notify();
                            }))
                            .child("Keep editing"),
                    )
                    .child(
                        div()
                            .id("commit-run")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .text_sm()
                            .cursor_pointer()
                            .bg(c.accent.clone())
                            .text_color(c.on_accent.clone())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.run_commit(cx, false);
                            }))
                            .child("Commit"),
                    ),
            ),
        overlay_close(cx, theme, |this, _, cx| {
            this.commit_review.open = false;
            this.commit_review.error = None;
            cx.notify();
        }),
    ))
    .into_any_element()
}
