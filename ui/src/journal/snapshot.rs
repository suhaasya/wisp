//! Build crash journal snapshots from workspace UI state.

use gpui::App;
use wisp_core::{
    JournalTab, JournalTabKind, JournalWorkspace, JournalWriter, WindowJournal,
};

use crate::workspace::{WorkspaceTab, WorkspaceTabKind, WorkspaceView};

pub fn collect_window_journal(
    workspace: &WorkspaceView,
    journal: &JournalWriter,
    cx: &App,
) -> WindowJournal {
    let workspace_body = workspace.connection_id().map(|connection_id| {
        let tabs = workspace
            .journal_tabs()
            .iter()
            .map(|tab| journal_tab_from_workspace(tab.clone(), workspace, cx))
            .collect();
        JournalWorkspace {
            connection_id: connection_id.0,
            active_tab: workspace.journal_active_tab(),
            next_tab_id: workspace.journal_next_tab_id(),
            tabs,
        }
    });
    WindowJournal::new(journal.window_id(), workspace_body)
}

fn journal_tab_from_workspace(
    tab: WorkspaceTab,
    workspace: &WorkspaceView,
    cx: &App,
) -> JournalTab {
    let tab_id = tab.id;
    match tab.kind {
        WorkspaceTabKind::Query => {
            let sql = workspace
                .query_editor(tab_id)
                .map(|ed| ed.read(cx).editor_text())
                .unwrap_or_default();
            JournalTab {
                tab_id,
                title: tab.title.to_string(),
                kind: JournalTabKind::Query,
                pinned: tab.pinned,
                query_sql: Some(sql),
                staged_grid: None,
            }
        }
        WorkspaceTabKind::TableData => {
            let staged = workspace
                .table_grid(tab_id)
                .map(|grid| grid.read(cx).staged_snapshot());
            JournalTab {
                tab_id,
                title: tab.title.to_string(),
                kind: JournalTabKind::TableData,
                pinned: tab.pinned,
                query_sql: None,
                staged_grid: staged,
            }
        }
        WorkspaceTabKind::TableStructure => JournalTab {
            tab_id,
            title: tab.title.to_string(),
            kind: JournalTabKind::TableStructure,
            pinned: tab.pinned,
            query_sql: None,
            staged_grid: None,
        },
    }
}
