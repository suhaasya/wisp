//! New / edit connection sheet (LUM-016).

#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use std::{rc::Rc, sync::Arc};

use gpui::{
    div, prelude::*, px, Context, Entity, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Task, Window,
};
use tokio_util::sync::CancellationToken;
use wisp_core::{
    bridge::DbCommandPayload, validate, ConnectionFormDraft, ConnectionHub, ConnectionId,
    ConnectionTestSpec, DbEventPayload, FormEngine, FormEnvironment, FormField, FormTab, SslMode,
    SslTrustStore, SshAuthMethod,
};
use crate::{
    bridge::{db_bridge, spawn_db},
    components::{
        button::{button, ButtonVariant},
        checkbox::checkbox,
        segmented::segmented_control,
        tabs::horizontal_tabs,
        text_input::{TextInput, TextInputKind},
        toggle::toggle,
    },
    connections::{
        env::env_edge_colour,
        shell_commands::{ShellCommand, ShellCommandSender},
    },
    theme::{self, ResolvedTheme},
};

pub struct ConnectionForm {
    focus_handle: FocusHandle,
    hub: Arc<ConnectionHub>,
    commands: ShellCommandSender,
    theme: ResolvedTheme,
    tab: FormTab,
    draft: ConnectionFormDraft,
    name_input: Entity<TextInput>,
    host_input: Entity<TextInput>,
    port_input: Entity<TextInput>,
    user_input: Entity<TextInput>,
    password_input: Entity<TextInput>,
    database_input: Entity<TextInput>,
    ssl_ca_input: Entity<TextInput>,
    ssl_client_cert_input: Entity<TextInput>,
    ssl_client_key_input: Entity<TextInput>,
    ssh_host_input: Entity<TextInput>,
    ssh_port_input: Entity<TextInput>,
    ssh_user_input: Entity<TextInput>,
    ssh_password_input: Entity<TextInput>,
    ssh_identity_input: Entity<TextInput>,
    ssh_config_host_input: Entity<TextInput>,
    test_status: SharedString,
    test_running: bool,
    test_cancel: Option<CancellationToken>,
    /// Must stay alive until the bridge callback runs (dropping cancels the waiter).
    test_task: Option<Task<()>>,
    folder_open: bool,
}

impl ConnectionForm {
    pub fn new(
        cx: &mut Context<Self>,
        hub: Arc<ConnectionHub>,
        commands: ShellCommandSender,
    ) -> Self {
        let theme = theme::read_global(cx).resolved().clone();
        let draft = ConnectionFormDraft::new_connection();
        Self {
            focus_handle: cx.focus_handle(),
            hub,
            commands,
            theme: theme.clone(),
            tab: FormTab::General,
            draft: draft.clone(),
            name_input: text_field(cx, &theme, "Connection name"),
            host_input: text_field(cx, &theme, "Host"),
            port_input: text_field(cx, &theme, "5432"),
            user_input: text_field(cx, &theme, "User"),
            password_input: password_field(cx, &theme, "Password"),
            database_input: text_field(cx, &theme, "Database"),
            ssl_ca_input: text_field(cx, &theme, "Path to CA PEM"),
            ssl_client_cert_input: text_field(cx, &theme, "Path to client cert PEM"),
            ssl_client_key_input: text_field(cx, &theme, "Path to client key PEM"),
            ssh_host_input: text_field(cx, &theme, "Bastion host"),
            ssh_port_input: text_field(cx, &theme, "22"),
            ssh_user_input: text_field(cx, &theme, "SSH user"),
            ssh_password_input: password_field(cx, &theme, "SSH password"),
            ssh_identity_input: text_field(cx, &theme, "Identity file path"),
            ssh_config_host_input: text_field(cx, &theme, "OpenSSH config Host alias"),
            test_status: "Test the connection before saving.".into(),
            test_running: false,
            test_cancel: None,
            test_task: None,
            folder_open: false,
        }
    }

    pub fn open_new(&mut self, cx: &mut Context<Self>) {
        self.draft = ConnectionFormDraft::new_connection();
        self.sync_inputs_from_draft(cx);
        self.test_status = "Test the connection before saving.".into();
        self.tab = FormTab::General;
        cx.notify();
    }

    pub fn open_with_draft(&mut self, draft: ConnectionFormDraft, cx: &mut Context<Self>) {
        self.draft = draft;
        self.sync_inputs_from_draft(cx);
        self.test_status = "Test the connection before saving.".into();
        self.tab = FormTab::General;
        cx.notify();
    }

    pub fn open_edit(&mut self, id: ConnectionId, cx: &mut Context<Self>) {
        if let Some(profile) = self.hub.get(id) {
            self.draft = ConnectionFormDraft::from_profile(&profile);
            self.sync_inputs_from_draft(cx);
            self.test_status = "Test the connection before saving.".into();
            cx.notify();
        }
    }

    fn sync_inputs_from_draft(&mut self, cx: &mut Context<Self>) {
        set_input(&self.name_input, cx, &self.draft.name);
        set_input(&self.host_input, cx, &self.draft.host);
        set_input(&self.port_input, cx, &self.draft.port);
        set_input(&self.user_input, cx, &self.draft.user);
        set_input(&self.password_input, cx, &self.draft.password);
        set_input(&self.database_input, cx, &self.draft.database);
        set_input(&self.ssl_ca_input, cx, &self.draft.ssl_ca_file);
        set_input(&self.ssl_client_cert_input, cx, &self.draft.ssl_client_cert_file);
        set_input(&self.ssl_client_key_input, cx, &self.draft.ssl_client_key_file);
        set_input(&self.ssh_host_input, cx, &self.draft.ssh_host);
        set_input(&self.ssh_port_input, cx, &self.draft.ssh_port);
        set_input(&self.ssh_user_input, cx, &self.draft.ssh_user);
        set_input(&self.ssh_password_input, cx, &self.draft.ssh_password);
        set_input(&self.ssh_identity_input, cx, &self.draft.ssh_identity_file);
        set_input(&self.ssh_config_host_input, cx, &self.draft.ssh_config_host);
    }

    fn pull_draft_from_inputs(&mut self, cx: &mut Context<Self>) {
        self.draft.name = self.name_input.read(cx).content().to_string();
        self.draft.host = self.host_input.read(cx).content().to_string();
        self.draft.port = self.port_input.read(cx).content().to_string();
        self.draft.user = self.user_input.read(cx).content().to_string();
        self.draft.password = self.password_input.read(cx).content().to_string();
        self.draft.database = self.database_input.read(cx).content().to_string();
        self.draft.ssl_ca_file = self.ssl_ca_input.read(cx).content().to_string();
        self.draft.ssl_client_cert_file = self.ssl_client_cert_input.read(cx).content().to_string();
        self.draft.ssl_client_key_file = self.ssl_client_key_input.read(cx).content().to_string();
        self.draft.ssh_host = self.ssh_host_input.read(cx).content().to_string();
        self.draft.ssh_port = self.ssh_port_input.read(cx).content().to_string();
        self.draft.ssh_user = self.ssh_user_input.read(cx).content().to_string();
        self.draft.ssh_password = self.ssh_password_input.read(cx).content().to_string();
        self.draft.ssh_identity_file = self.ssh_identity_input.read(cx).content().to_string();
        self.draft.ssh_config_host = self.ssh_config_host_input.read(cx).content().to_string();
    }

    fn engine_index(&self) -> usize {
        FormEngine::SEGMENTS
            .iter()
            .position(|e| *e == self.draft.engine)
            .unwrap_or(0)
    }

    fn cancel_test(&mut self) {
        if let Some(token) = self.test_cancel.take() {
            token.cancel();
        }
        self.test_task = None;
        self.test_running = false;
    }

    fn run_test(&mut self, cx: &mut Context<Self>) {
        self.pull_draft_from_inputs(cx);
        let validation = validate(&self.draft);
        if !validation.is_valid() {
            self.test_status = "Fix validation errors before testing.".into();
            cx.notify();
            return;
        }
        let port: u16 = self.draft.port.trim().parse().unwrap_or(5432);
        let spec = ConnectionTestSpec::from_form(
            self.draft.engine,
            &self.draft.host,
            port,
            &self.draft.user,
            &self.draft.password,
            &self.draft.database,
            self.draft.ssl_settings(),
            self.draft.ssh_settings(),
            &self.draft.ssh_password,
            &self.draft.ssh_key_passphrase,
        );
        self.cancel_test();
        self.test_running = true;
        self.test_status = "Testing connection…".into();
        let bridge = db_bridge(cx);
        let (_id, cancel, task) = spawn_db(cx, &bridge, DbCommandPayload::TestConnection(spec), {
            move |this, cx, result| {
                this.test_running = false;
                this.test_cancel = None;
                this.test_task = None;
                match result {
                    Ok(DbEventPayload::ConnectionTest(Ok(out))) => {
                        this.test_status = format!(
                            "Connected in {} ms — {} ({}), {}",
                            out.latency_ms, out.version, out.database, out.tls_summary
                        )
                        .into();
                    }
                    Ok(DbEventPayload::ConnectionTest(Err(err))) => {
                        this.test_status = err.to_string().into();
                    }
                    Ok(_) => {
                        this.test_status = "Unexpected test response.".into();
                    }
                    Err(err) => {
                        this.test_status = err.message().into();
                    }
                }
                cx.notify();
            }
        });
        self.test_cancel = Some(cancel);
        self.test_task = Some(task);
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>, connect: bool) {
        self.pull_draft_from_inputs(cx);
        let validation = validate(&self.draft);
        if !validation.is_valid() {
            cx.notify();
            return;
        }
        let profile = self.draft.to_profile();
        let password = (!self.draft.password.is_empty()).then_some(self.draft.password.as_str());
        match self.hub.save_profile(profile, password, self.draft.save_password) {
            Ok(id) => {
                self.commands.push(ShellCommand::FormSaved { id, connect });
            }
            Err(err) => {
                self.test_status = err.to_string().into();
            }
        }
        cx.notify();
    }
}

impl Render for ConnectionForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.theme = theme::read_global(cx).resolved().clone();
        self.pull_draft_from_inputs(cx);
        let validation = validate(&self.draft);
        let c = &self.theme.colors;
        let folders = self.hub.folders();
        let folder_labels: Vec<String> = std::iter::once("None".to_string())
            .chain(folders.iter().map(|f| f.name.clone()))
            .collect();
        let folder_selected = self
            .draft
            .folder_id
            .and_then(|id| folders.iter().position(|f| f.id == id).map(|i| i + 1))
            .unwrap_or(0);

        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(c.canvas)
            .p_5()
            .track_focus(&self.focus_handle)
            .child(
                div()
                    .w(px(640.))
                    .max_w_full()
                    .flex()
                    .flex_col()
                    .bg(c.panel)
                    .border_1()
                    .border_color(c.line)
                    .rounded_lg()
                    .shadow_lg()
                    .overflow_hidden()
                    .child(
                        div()
                            .px_5()
                            .pt_4()
                            .pb_3()
                            .border_b_1()
                            .border_color(c.line2)
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(if self.draft.id.is_some() {
                                        "Edit connection"
                                    } else {
                                        "New connection"
                                    }),
                            )
                            .child(
                                div()
                                    .mt_3()
                                    .child(segmented_control(
                                        cx,
                                        "engine-segment",
                                        &["PostgreSQL", "MySQL", "MariaDB"],
                                        self.engine_index(),
                                        &self.theme,
                                        |this, index, _, cx| {
                                            this.draft.set_engine(FormEngine::SEGMENTS[index]);
                                            this.sync_inputs_from_draft(cx);
                                            cx.notify();
                                        },
                                    )),
                            ),
                    )
                    .child(horizontal_tabs(
                        cx,
                        "conn-form-tabs",
                        &["General", "SSH", "SSL", "Advanced"],
                        match self.tab {
                            FormTab::General => 0,
                            FormTab::Ssh => 1,
                            FormTab::Ssl => 2,
                            FormTab::Advanced => 3,
                        },
                        &self.theme,
                        |this, index, _, cx| {
                            this.tab = match index {
                                1 => FormTab::Ssh,
                                2 => FormTab::Ssl,
                                3 => FormTab::Advanced,
                                _ => FormTab::General,
                            };
                            cx.notify();
                        },
                    ))
                    .child(
                        div()
                            .px_5()
                            .py_4()
                            .child(match self.tab {
                                FormTab::General => general_tab(
                                    cx,
                                    &self.theme,
                                    &validation,
                                    &self.name_input,
                                    &self.host_input,
                                    &self.port_input,
                                    &self.user_input,
                                    &self.password_input,
                                    &self.database_input,
                                    &folder_labels,
                                    folder_selected,
                                    self.folder_open,
                                    &mut self.draft,
                                    Rc::new(|this, open, cx| {
                                        this.folder_open = open;
                                        cx.notify();
                                    }),
                                    Rc::from(
                                        folders
                                            .iter()
                                            .map(|f| f.id)
                                            .collect::<Vec<_>>(),
                                    ),
                                )
                                .into_any_element(),
                                FormTab::Advanced => advanced_tab(c, &self.draft).into_any_element(),
                                FormTab::Ssl => ssl_tab(
                                    cx,
                                    &self.theme,
                                    &mut self.draft,
                                    &self.host_input,
                                    &self.ssl_ca_input,
                                    &self.ssl_client_cert_input,
                                    &self.ssl_client_key_input,
                                )
                                .into_any_element(),
                                FormTab::Ssh => ssh_tab(
                                    cx,
                                    &self.theme,
                                    &mut self.draft,
                                    &self.ssh_host_input,
                                    &self.ssh_port_input,
                                    &self.ssh_user_input,
                                    &self.ssh_password_input,
                                    &self.ssh_identity_input,
                                    &self.ssh_config_host_input,
                                )
                                .into_any_element(),
                            }),
                    )
                    .child(
                        div()
                            .px_5()
                            .py_3()
                            .border_t_1()
                            .border_color(c.line2)
                            .bg(c.raise)
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .text_color(if self.test_running {
                                        c.ink2
                                    } else if self.test_status.contains("Connected") {
                                        c.accent
                                    } else {
                                        c.ink3
                                    })
                                    .child(self.test_status.clone()),
                            )
                            .child(button(
                                cx,
                                "conn-test",
                                if self.test_running {
                                    "Cancel test"
                                } else {
                                    "Test"
                                },
                                ButtonVariant::Default,
                                &self.theme,
                                |this, _, cx| {
                                    if this.test_running {
                                        this.cancel_test();
                                        this.test_status = "Test cancelled.".into();
                                    } else {
                                        this.run_test(cx);
                                    }
                                    cx.notify();
                                },
                            ))
                            .child(button(
                                cx,
                                "conn-cancel",
                                "Cancel",
                                ButtonVariant::Ghost,
                                &self.theme,
                                |this, _, _| {
                                    this.cancel_test();
                                    this.commands.push(ShellCommand::FormCancelled);
                                },
                            ))
                            .child(button(
                                cx,
                                "conn-save",
                                "Save",
                                ButtonVariant::Default,
                                &self.theme,
                                |this, _, cx| this.save(cx, false),
                            ))
                            .child(button(
                                cx,
                                "conn-save-connect",
                                "Save and connect",
                                ButtonVariant::Primary,
                                &self.theme,
                                |this, _, cx| this.save(cx, true),
                            )),
                    ),
            )
    }
}

fn general_tab(
    cx: &mut Context<ConnectionForm>,
    theme: &ResolvedTheme,
    validation: &wisp_core::FormValidation,
    name: &Entity<TextInput>,
    host: &Entity<TextInput>,
    port: &Entity<TextInput>,
    user: &Entity<TextInput>,
    password: &Entity<TextInput>,
    database: &Entity<TextInput>,
    folder_labels: &[String],
    folder_selected: usize,
    folder_open: bool,
    draft: &mut ConnectionFormDraft,
    on_folder_open: Rc<dyn Fn(&mut ConnectionForm, bool, &mut Context<ConnectionForm>)>,
    folder_ids: Rc<[ConnectionId]>,
) -> impl IntoElement {
    let c = &theme.colors;
    let folder_name = folder_labels
        .get(folder_selected)
        .cloned()
        .unwrap_or_else(|| "None".into());
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(field(
            "Name",
            validation.field_errors.get(&FormField::Name),
            name.clone(),
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(c.ink3)
                        .child("Folder"),
                )
                .child(
                    div()
                        .id("folder-select")
                        .cursor_pointer()
                        .px_2()
                        .h(theme.density.row_height())
                        .flex()
                        .items_center()
                        .justify_between()
                        .rounded_md()
                        .border_1()
                        .border_color(c.line)
                        .bg(c.panel)
                        .on_click({
                            let open = on_folder_open.clone();
                            cx.listener(move |this, _, _, cx| {
                                open(this, !folder_open, cx);
                            })
                        })
                        .child(folder_name)
                        .child(
                            div()
                                .text_xs()
                                .text_color(c.ink3)
                                .child("▾"),
                        ),
                )
                .when(folder_open, |el| {
                    el.child(
                        div()
                            .flex()
                            .flex_col()
                            .border_1()
                            .border_color(c.line)
                            .rounded_md()
                            .bg(c.panel)
                            .children(folder_labels.iter().enumerate().map(|(i, label)| {
                                let folder_ids = folder_ids.clone();
                                div()
                                    .id(SharedString::from(format!("folder-opt-{i}")))
                                    .px_2()
                                    .py_1p5()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(c.raise))
                                    .child(label.clone())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.draft.folder_id = if i == 0 {
                                            None
                                        } else {
                                            folder_ids.get(i - 1).copied()
                                        };
                                        this.folder_open = false;
                                        cx.notify();
                                    }))
                            })),
                    )
                }),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .child(field(
                            "Host",
                            validation.field_errors.get(&FormField::Host),
                            host.clone(),
                        )),
                )
                .child(
                    div()
                        .w(px(100.))
                        .child(field(
                            "Port",
                            validation.field_errors.get(&FormField::Port),
                            port.clone(),
                        )),
                ),
        )
        .child(field(
            "User",
            validation.field_errors.get(&FormField::User),
            user.clone(),
        ))
        .child(field(
            "Password",
            None,
            password.clone(),
        ))
        .child(checkbox(
            cx,
            "save-password",
            "Save password",
            draft.save_password,
            theme,
            |this, _, cx| {
                this.draft.save_password = !this.draft.save_password;
                cx.notify();
            },
        ))
        .child(field(
            "Database",
            validation.field_errors.get(&FormField::Database),
            database.clone(),
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .text_color(c.ink3)
                        .child("Environment"),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .children(FormEnvironment::ALL.iter().map(|env| {
                            let active = draft.environment == *env;
                            let dot = env_edge_colour(
                                &env.to_tag(),
                                &c.env,
                            );
                            div()
                                .id(SharedString::from(format!("env-{}", env.label())))
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_2()
                                .py_1()
                                .rounded_full()
                                .border_1()
                                .border_color(if active { dot } else { c.line })
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.draft.set_environment(*env);
                                    cx.notify();
                                }))
                                .child(div().size(px(8.)).rounded_full().bg(dot))
                                .child(
                                    div()
                                        .text_xs()
                                        .child(env.label()),
                                )
                        })),
                ),
        )
        .child(checkbox(
            cx,
            "read-only",
            "Read-only",
            draft.read_only,
            theme,
            |this, _, cx| {
                this.draft.read_only = !this.draft.read_only;
                cx.notify();
            },
        ))
        .child(toggle(
            cx,
            "safe-mode",
            "Confirm every write",
            draft.safe_mode,
            theme,
            |this, _, cx| {
                this.draft.safe_mode = !this.draft.safe_mode;
                cx.notify();
            },
        ))
}

fn advanced_tab(c: &theme::ResolvedColors, draft: &ConnectionFormDraft) -> impl IntoElement {
    if draft.extra_options.is_empty() {
        return div()
            .py_8()
            .text_sm()
            .text_color(c.ink3)
            .child("No extra driver options from the pasted URL.")
            .into_any_element();
    }
    div()
        .flex()
        .flex_col()
        .gap_2()
        .children(draft.extra_options.iter().map(|(key, value)| {
            div()
                .flex()
                .gap_2()
                .text_sm()
                .child(
                    div()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(format!("{key}=")),
                )
                .child(div().text_color(c.ink2).child(value.clone()))
                .into_any_element()
        }))
        .into_any_element()
}

fn ssl_tab(
    cx: &mut Context<ConnectionForm>,
    theme: &ResolvedTheme,
    draft: &mut ConnectionFormDraft,
    host_input: &Entity<TextInput>,
    ssl_ca_input: &Entity<TextInput>,
    ssl_client_cert_input: &Entity<TextInput>,
    ssl_client_key_input: &Entity<TextInput>,
) -> impl IntoElement {
    let c = &theme.colors;
    let host = host_input.read(cx).content().to_string();
    let warn_insecure = draft.ssl_mode == SslMode::Disable && !host.trim().is_empty()
        && !is_local_host_for_ui(host.trim());

    div()
        .flex()
        .flex_col()
        .gap_4()
        .children(if warn_insecure {
            vec![div()
                .px_3()
                .py_2()
                .rounded_md()
                .bg(gpui::rgb(0xFFF4E5))
                .text_sm()
                .text_color(gpui::rgb(0x8A5A00))
                .child(
                    "TLS is disabled for a non-local host. Traffic is not encrypted on the network.",
                )
                .into_any_element()]
        } else {
            vec![]
        })
        .child(
            div()
                .text_xs()
                .text_color(c.ink3)
                .child("Mode"),
        )
        .child(ssl_mode_picker(cx, theme, draft))
        .child(
            div()
                .text_xs()
                .text_color(c.ink3)
                .child("Trust store"),
        )
        .child(ssl_trust_picker(cx, theme, draft))
        .child(field("CA certificate file", None, ssl_ca_input.clone()))
        .child(field("Client certificate", None, ssl_client_cert_input.clone()))
        .child(field("Client private key", None, ssl_client_key_input.clone()))
}

fn is_local_host_for_ui(host: &str) -> bool {
    let host = host.trim_matches(['[', ']']);
    host.eq_ignore_ascii_case("localhost")
        || host == "127.0.0.1"
        || host == "::1"
        || host.ends_with(".local")
}

fn ssl_mode_picker(
    cx: &mut Context<ConnectionForm>,
    theme: &ResolvedTheme,
    draft: &ConnectionFormDraft,
) -> impl IntoElement {
    const LABELS: [&str; 5] = ["Disable", "Prefer", "Require", "Verify CA", "Verify full"];
    const MODES: [SslMode; 5] = [
        SslMode::Disable,
        SslMode::Prefer,
        SslMode::Require,
        SslMode::VerifyCa,
        SslMode::VerifyFull,
    ];
    let selected = MODES
        .iter()
        .position(|m| *m == draft.ssl_mode)
        .unwrap_or(0);
    segmented_control(cx, "ssl-mode", &LABELS, selected, theme, move |this, index, _, cx| {
        this.draft.ssl_mode = MODES[index];
        cx.notify();
    })
}

fn ssl_trust_picker(
    cx: &mut Context<ConnectionForm>,
    theme: &ResolvedTheme,
    draft: &ConnectionFormDraft,
) -> impl IntoElement {
    let selected = match draft.ssl_trust {
        SslTrustStore::System => 0,
        SslTrustStore::CustomCa => 1,
    };
    segmented_control(
        cx,
        "ssl-trust",
        &["System roots", "Custom CA"],
        selected,
        theme,
        move |this, index, _, cx| {
            this.draft.ssl_trust = if index == 0 {
                SslTrustStore::System
            } else {
                SslTrustStore::CustomCa
            };
            cx.notify();
        },
    )
}

fn ssh_tab(
    cx: &mut Context<ConnectionForm>,
    theme: &ResolvedTheme,
    draft: &mut ConnectionFormDraft,
    ssh_host: &Entity<TextInput>,
    ssh_port: &Entity<TextInput>,
    ssh_user: &Entity<TextInput>,
    ssh_password: &Entity<TextInput>,
    ssh_identity: &Entity<TextInput>,
    ssh_config_host: &Entity<TextInput>,
) -> impl IntoElement {
    let c = &theme.colors;
    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(checkbox(
            cx,
            "ssh-enabled",
            "Connect through an SSH bastion",
            draft.ssh_enabled,
            theme,
            |this, _, cx| {
                this.draft.ssh_enabled = !this.draft.ssh_enabled;
                cx.notify();
            },
        ))
        .child(field("Bastion host", None, ssh_host.clone()))
        .child(field("Bastion port", None, ssh_port.clone()))
        .child(field("SSH user", None, ssh_user.clone()))
        .child(ssh_auth_picker(cx, theme, draft))
        .child(checkbox(
            cx,
            "ssh-agent",
            "Use ssh-agent",
            draft.ssh_use_agent,
            theme,
            |this, _, cx| {
                this.draft.ssh_use_agent = !this.draft.ssh_use_agent;
                cx.notify();
            },
        ))
        .child(field("Identity file", None, ssh_identity.clone()))
        .child(field("SSH password (password auth)", None, ssh_password.clone()))
        .child(field("Config Host alias", None, ssh_config_host.clone()))
        .child(
            div()
                .text_xs()
                .text_color(c.ink3)
                .child("Unknown bastion host keys require trust on first use (fingerprint prompt in a future build). CI/tests set WISP_SSH_TOFU=1."),
        )
}

fn ssh_auth_picker(
    cx: &mut Context<ConnectionForm>,
    theme: &ResolvedTheme,
    draft: &ConnectionFormDraft,
) -> impl IntoElement {
    const MODES: [SshAuthMethod; 3] = [
        SshAuthMethod::Agent,
        SshAuthMethod::Password,
        SshAuthMethod::PublicKey,
    ];
    const LABELS: [&str; 3] = ["Agent", "Password", "Public key"];
    let selected = MODES
        .iter()
        .position(|m| *m == draft.ssh_auth)
        .unwrap_or(0);
    segmented_control(cx, "ssh-auth", &LABELS, selected, theme, move |this, index, _, cx| {
        this.draft.ssh_auth = MODES[index];
        cx.notify();
    })
}

fn field(
    label: &'static str,
    error: Option<&String>,
    input: Entity<TextInput>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_xs()
                .text_color(gpui::rgb(0x6E7788))
                .child(label),
        )
        .child(input)
        .children(error.map(|message| {
            div()
                .text_xs()
                .text_color(gpui::rgb(0xCF4136))
                .child(message.clone())
        }))
}

fn text_field(cx: &mut Context<ConnectionForm>, theme: &ResolvedTheme, placeholder: &str) -> Entity<TextInput> {
    cx.new(|cx| TextInput::new(cx, placeholder, TextInputKind::SingleLine, theme.clone()))
}

fn password_field(cx: &mut Context<ConnectionForm>, theme: &ResolvedTheme, placeholder: &str) -> Entity<TextInput> {
    cx.new(|cx| TextInput::new(cx, placeholder, TextInputKind::Password, theme.clone()))
}

fn set_input(input: &Entity<TextInput>, cx: &mut Context<ConnectionForm>, value: &str) {
    input.update(cx, |field, cx| {
        field.set_content(value);
        cx.notify();
    });
}
