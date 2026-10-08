//! Saved connection profiles: filtering, grouping, and persistence hub (LUM-014 / LUM-015).

mod controller;
mod filter;
mod hub;
mod view;

pub use controller::{ConnectionManagerController, ManagerAction};
pub use filter::fuzzy_match;
pub use hub::{ConnectionHub, ConnectionHubError};
pub use view::{
    card_from_profile, ConnectionBadge, ConnectionCardView, ConnectionEngineKind,
    ConnectionGroup, ConnectionsView, EnvFilter, RailSelection,
};

pub use wisp_store::{
    ConnectionEngine, ConnectionFolder, ConnectionId, ConnectionProfile, ConnectionsLoadError,
    EnvironmentTag, SslMode, TransportKind,
};
