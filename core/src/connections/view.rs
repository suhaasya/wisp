//! View models for the connection manager screen.

use wisp_store::{
    ConnectionEngine, ConnectionFolder, ConnectionId, ConnectionProfile, EnvironmentTag, SslMode,
};

use super::filter::fuzzy_match;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RailSelection {
    #[default]
    All,
    Folder(ConnectionId),
    Environment(EnvFilter),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnvFilter {
    Production,
    Staging,
    Development,
    Local,
}

impl EnvFilter {
    pub const ALL: [EnvFilter; 4] = [
        EnvFilter::Production,
        EnvFilter::Staging,
        EnvFilter::Development,
        EnvFilter::Local,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Production => "Production",
            Self::Staging => "Staging",
            Self::Development => "Development",
            Self::Local => "Local",
        }
    }

    pub fn matches_tag(self, tag: &EnvironmentTag) -> bool {
        match (self, tag) {
            (Self::Production, EnvironmentTag::Production) => true,
            (Self::Staging, EnvironmentTag::Staging) => true,
            (Self::Development, EnvironmentTag::Development) => true,
            (Self::Local, EnvironmentTag::Custom(label)) => {
                label.eq_ignore_ascii_case("local")
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionEngineKind {
    PostgreSql,
    MySql,
    MariaDb,
}

impl From<ConnectionEngine> for ConnectionEngineKind {
    fn from(value: ConnectionEngine) -> Self {
        match value {
            ConnectionEngine::PostgreSql => Self::PostgreSql,
            ConnectionEngine::MySql => Self::MySql,
            ConnectionEngine::MariaDb => Self::MariaDb,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionBadge {
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct ConnectionCardView {
    pub id: ConnectionId,
    pub name: String,
    pub host_line: String,
    pub engine: ConnectionEngineKind,
    pub env_tag: EnvironmentTag,
    pub env_label: String,
    pub badges: Vec<ConnectionBadge>,
    pub last_used_label: Option<String>,
    pub folder_id: Option<ConnectionId>,
    pub folder_name: Option<String>,
    pub read_only: bool,
    pub edge_colour: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConnectionGroup {
    pub title: String,
    pub cards: Vec<ConnectionCardView>,
}

#[derive(Debug, Clone)]
pub struct ConnectionsView {
    pub total_profiles: usize,
    pub all_count: usize,
    pub folder_rows: Vec<(ConnectionId, String, u32, usize)>,
    pub env_counts: Vec<(EnvFilter, usize)>,
    pub groups: Vec<ConnectionGroup>,
    pub flat_cards: Vec<ConnectionCardView>,
}

impl ConnectionsView {
    pub fn build(
        folders: &[ConnectionFolder],
        profiles: &[ConnectionProfile],
        rail: RailSelection,
        search: &str,
    ) -> Self {
        let filtered: Vec<_> = profiles
            .iter()
            .filter(|p| rail_matches(rail, p))
            .filter(|p| fuzzy_match(search, p))
            .cloned()
            .collect();

        let folder_name = |id: Option<ConnectionId>| {
            id.and_then(|id| {
                folders
                    .iter()
                    .find(|f| f.id == id)
                    .map(|f| f.name.clone())
            })
        };

        let cards: Vec<ConnectionCardView> = filtered
            .iter()
            .map(|p| card_from_profile(p, folder_name(p.folder_id)))
            .collect();

        let mut folder_rows: Vec<_> = folders
            .iter()
            .map(|f| {
                let count = profiles
                    .iter()
                    .filter(|p| p.folder_id == Some(f.id))
                    .count();
                (f.id, f.name.clone(), f.order, count)
            })
            .collect();
        folder_rows.sort_by_key(|(_, _, order, _)| *order);

        let env_counts: Vec<_> = EnvFilter::ALL
            .iter()
            .map(|env| {
                let count = profiles
                    .iter()
                    .filter(|p| env.matches_tag(&p.env_tag))
                    .count();
                (*env, count)
            })
            .collect();

        let groups = group_cards(&cards, folders, rail);

        Self {
            total_profiles: profiles.len(),
            all_count: profiles.len(),
            folder_rows,
            env_counts,
            flat_cards: cards,
            groups,
        }
    }
}

fn rail_matches(rail: RailSelection, profile: &ConnectionProfile) -> bool {
    match rail {
        RailSelection::All => true,
        RailSelection::Folder(id) => profile.folder_id == Some(id),
        RailSelection::Environment(env) => env.matches_tag(&profile.env_tag),
    }
}

fn group_cards(
    cards: &[ConnectionCardView],
    folders: &[ConnectionFolder],
    rail: RailSelection,
) -> Vec<ConnectionGroup> {
    if matches!(rail, RailSelection::Folder(_)) || cards.is_empty() {
        return vec![ConnectionGroup {
            title: String::new(),
            cards: cards.to_vec(),
        }];
    }

    let mut sorted_folders: Vec<_> = folders.iter().collect();
    sorted_folders.sort_by_key(|f| f.order);

    let mut groups = Vec::new();
    let mut unfiled = Vec::new();

    for folder in sorted_folders {
        let mut group_cards: Vec<_> = cards
            .iter()
            .filter(|c| c.folder_id == Some(folder.id))
            .cloned()
            .collect();
        if group_cards.is_empty() {
            continue;
        }
        group_cards.sort_by(|a, b| a.name.cmp(&b.name));
        groups.push(ConnectionGroup {
            title: folder.name.clone(),
            cards: group_cards,
        });
    }

    for card in cards {
        if card.folder_id.is_none() {
            unfiled.push(card.clone());
        }
    }
    if !unfiled.is_empty() {
        unfiled.sort_by(|a, b| a.name.cmp(&b.name));
        groups.push(ConnectionGroup {
            title: "Unfiled".into(),
            cards: unfiled,
        });
    }

    if groups.is_empty() && !cards.is_empty() {
        groups.push(ConnectionGroup {
            title: String::new(),
            cards: cards.to_vec(),
        });
    }

    groups
}

pub fn card_from_profile(
    profile: &ConnectionProfile,
    folder_name: Option<String>,
) -> ConnectionCardView {
    ConnectionCardView {
        id: profile.id,
        name: profile.name.clone(),
        host_line: format_host(profile),
        engine: profile.engine.into(),
        env_tag: profile.env_tag.clone(),
        env_label: env_display(&profile.env_tag),
        badges: profile_tags(profile)
            .into_iter()
            .map(|label| ConnectionBadge { label })
            .collect(),
        last_used_label: profile
            .last_used_unix
            .map(format_last_used),
        folder_id: profile.folder_id,
        folder_name,
        read_only: profile.read_only,
        edge_colour: profile.colour.clone(),
    }
}

pub fn profile_tags(profile: &ConnectionProfile) -> Vec<String> {
    let mut tags = Vec::new();
    if profile.ssh.enabled {
        let host = profile
            .ssh
            .host
            .as_deref()
            .unwrap_or("bastion");
        tags.push(format!("SSH via {host}"));
    }
    if profile.ssl.mode != SslMode::Disable {
        tags.push(format!("TLS {}", ssl_label(profile.ssl.mode)));
    }
    if profile.read_only {
        tags.push("Read-only".into());
    }
    if profile.safe_mode {
        tags.push("Safe mode".into());
    }
    tags
}

fn ssl_label(mode: SslMode) -> &'static str {
    match mode {
        SslMode::Disable => "disable",
        SslMode::Prefer => "prefer",
        SslMode::Require => "require",
        SslMode::VerifyCa => "verify-ca",
        SslMode::VerifyFull => "verify-full",
    }
}

fn env_display(tag: &EnvironmentTag) -> String {
    match tag {
        EnvironmentTag::Development => "Development".into(),
        EnvironmentTag::Staging => "Staging".into(),
        EnvironmentTag::Production => "Production".into(),
        EnvironmentTag::Custom(s) => s.clone(),
    }
}

fn format_host(profile: &ConnectionProfile) -> String {
    match (&profile.host, profile.port) {
        (Some(host), Some(port)) => format!("{host}:{port}"),
        (Some(host), None) => host.clone(),
        (None, Some(port)) => format!("localhost:{port}"),
        (None, None) => "—".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wisp_store::ConnectionEngine;

    #[test]
    fn builds_view_for_two_hundred_profiles() {
        let profiles: Vec<_> = (0..200)
            .map(|i| ConnectionProfile::new(format!("conn-{i}"), ConnectionEngine::PostgreSql))
            .collect();
        let view = ConnectionsView::build(&[], &profiles, RailSelection::All, "");
        assert_eq!(view.flat_cards.len(), 200);
    }
}

fn format_last_used(unix: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let delta = (now - unix).max(0);
    if delta < 3600 {
        let mins = (delta / 60).max(1);
        format!("{mins} m ago")
    } else if delta < 86_400 {
        let hours = delta / 3600;
        format!("{hours} h ago")
    } else {
        let days = delta / 86_400;
        format!("{days} d ago")
    }
}
