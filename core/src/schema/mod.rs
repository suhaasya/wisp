//! Schema browser model (LUM-022).

mod catalog;
mod filter;
mod introspect;
mod metadata_cache;
mod mock;
mod relations;
mod sidebar;

pub use catalog::{
    catalog_disconnected, catalog_placeholder, SchemaCatalog, SchemaObject, SchemaObjectId,
    SchemaObjectKind,
};
pub use filter::{fuzzy_match_highlight_indices, fuzzy_match_name};
pub use introspect::{load_schema_catalog, SchemaLoadResult};
pub use metadata_cache::{shared_metadata_cache, SchemaMetadataCache, SharedMetadataCache, TableKey};
pub use mock::{demo_catalog_large_tables, demo_catalog_shop};
pub use relations::{
    query_for_fk_target, relations_for_table, reverse_references, ForeignKey, ReverseReference,
    TableRelationSchema,
};
pub use sidebar::{
    build_sidebar_rows, PerConnectionSidebarState, SchemaSidebarPrefs, SchemaSidebarStateStore,
    SidebarRow,
};
