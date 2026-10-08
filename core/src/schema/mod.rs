//! Schema browser model (LUM-022).

mod catalog;
mod filter;
mod mock;
mod sidebar;

pub use catalog::{SchemaCatalog, SchemaObject, SchemaObjectId, SchemaObjectKind};
pub use filter::{fuzzy_match_highlight_indices, fuzzy_match_name};
pub use mock::{demo_catalog_large_tables, demo_catalog_shop};
pub use sidebar::{
    build_sidebar_rows, PerConnectionSidebarState, SchemaSidebarPrefs, SchemaSidebarStateStore,
    SidebarRow,
};
