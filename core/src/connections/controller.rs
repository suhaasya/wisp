//! Testable connection-manager state (filter, selection, actions).

use wisp_store::ConnectionId;

use super::{
    hub::ConnectionHub,
    view::{ConnectionsView, RailSelection},
    ConnectionHubError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagerAction {
    Connect(ConnectionId),
    None,
}

#[derive(Debug, Default)]
pub struct ConnectionManagerController {
    pub rail: RailSelection,
    pub search: String,
    pub selected_index: usize,
    pub pending_delete: Option<ConnectionId>,
}

impl ConnectionManagerController {
    pub fn view(&self, hub: &ConnectionHub) -> ConnectionsView {
        let folders = hub.folders();
        let profiles = hub.profiles();
        ConnectionsView::build(&folders, &profiles, self.rail, &self.search)
    }

    pub fn select_next(&mut self, hub: &ConnectionHub) {
        let count = self.view(hub).flat_cards.len();
        if count == 0 {
            self.selected_index = 0;
        } else {
            self.selected_index = (self.selected_index + 1).min(count - 1);
        }
    }

    pub fn select_prev(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    pub fn connect_selected(&self, hub: &ConnectionHub) -> ManagerAction {
        let view = self.view(hub);
        view.flat_cards
            .get(self.selected_index)
            .map_or(ManagerAction::None, |c| ManagerAction::Connect(c.id))
    }

    pub fn delete_pending(&mut self, hub: &ConnectionHub) -> Result<(), ConnectionHubError> {
        if let Some(id) = self.pending_delete.take() {
            hub.delete(id)?;
            self.selected_index = self.selected_index.saturating_sub(1);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::connections::fuzzy_match;
    use wisp_store::{ConnectionEngine, ConnectionProfile, MockSecretStore};

    fn sample_hub() -> (ConnectionHub, tempfile::TempDir) {
        let secrets = Arc::new(MockSecretStore::new());
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("connections.toml");
        let hub = ConnectionHub::empty_at(path, secrets);
        let mut p = ConnectionProfile::new("shop_prod", ConnectionEngine::PostgreSql);
        p.host = Some("db.example.com".into());
        hub.create(p).unwrap();
        let mut q = ConnectionProfile::new("billing", ConnectionEngine::MySql);
        q.name = "billing".into();
        hub.create(q).unwrap();
        (hub, dir)
    }

    #[test]
    fn filter_narrows_cards() {
        let (hub, _dir) = sample_hub();
        let mut ctrl = ConnectionManagerController::default();
        ctrl.search = "shop".into();
        assert_eq!(ctrl.view(&hub).flat_cards.len(), 1);
        assert!(fuzzy_match("shop", &hub.profiles()[0]));
    }

    #[test]
    fn delete_removes_profile() {
        let (hub, _dir) = sample_hub();
        let id = hub.profiles()[0].id;
        let mut ctrl = ConnectionManagerController::default();
        ctrl.pending_delete = Some(id);
        ctrl.delete_pending(&hub).unwrap();
        assert_eq!(hub.profiles().len(), 1);
    }

    #[test]
    fn connect_selected_emits_session_open() {
        let (hub, _dir) = sample_hub();
        let ctrl = ConnectionManagerController::default();
        match ctrl.connect_selected(&hub) {
            ManagerAction::Connect(id) => {
                hub.touch_connect(id).unwrap();
            }
            ManagerAction::None => panic!("expected connect"),
        }
    }
}
