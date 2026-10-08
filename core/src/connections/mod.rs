//! Saved connection profiles: filtering, grouping, and persistence hub (LUM-014 / LUM-015).

mod controller;
mod filter;
mod form;
mod hub;
mod test;
mod test_error;
mod url_parse;
mod view;

pub use controller::{ConnectionManagerController, ManagerAction};
pub use filter::fuzzy_match;
pub use form::{
    validate, ConnectionFormDraft, FormEngine, FormEnvironment, FormField, FormTab, FormValidation,
};
pub use hub::{ConnectionHub, ConnectionHubError};
pub use test::{run_connection_test, ConnectionTestOutcome, ConnectionTestSpec};
pub use test_error::ConnectionTestError;
pub use url_parse::{parse_connection_paste, provider_tls_default, UrlParseError};
pub use view::{
    card_from_profile, ConnectionBadge, ConnectionCardView, ConnectionEngineKind,
    ConnectionGroup, ConnectionsView, EnvFilter, RailSelection,
};

pub use wisp_store::{
    ConnectionEngine, ConnectionFolder, ConnectionId, ConnectionProfile, ConnectionsLoadError,
    EnvironmentTag, SslMode, SslSettings, SslTrustStore, SshAuthMethod, TransportKind,
};
