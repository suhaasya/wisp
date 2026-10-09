//! Live schema introspection (LUM-021).

mod mysql;
mod page_rows;
mod postgres;

use std::collections::HashMap;

use wisp_drivers::{ColumnMeta, DbDriver, DriverError};
use wisp_store::{ConnectionEngine, ConnectionProfile};

use super::catalog::SchemaCatalog;
use super::metadata_cache::TableKey;
use super::mock::demo_catalog_shop;

pub use page_rows::fetch_text_rows;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaLoadResult {
    pub catalog: SchemaCatalog,
    pub columns: HashMap<TableKey, Vec<ColumnMeta>>,
}

pub fn load_schema_catalog(
    driver: &mut dyn DbDriver,
    engine: ConnectionEngine,
    profile: &ConnectionProfile,
) -> Result<SchemaLoadResult, DriverError> {
    if profile.host.as_deref() == Some("wisp-mock") {
        return Ok(mock_catalog(profile));
    }
    let label = profile.name.clone();
    let (catalog, columns) = match engine {
        ConnectionEngine::PostgreSql => postgres::load_schema(driver, profile, &label)?,
        ConnectionEngine::MySql | ConnectionEngine::MariaDb => {
            mysql::load_schema(driver, profile, &label)?
        }
    };
    Ok(SchemaLoadResult { catalog, columns })
}

fn mock_catalog(profile: &ConnectionProfile) -> SchemaLoadResult {
    let mut catalog = demo_catalog_shop();
    catalog.connection_label = profile.name.clone();
    if let Some(db) = &profile.database {
        catalog.database = db.clone();
    }
    SchemaLoadResult {
        catalog,
        columns: HashMap::new(),
    }
}
