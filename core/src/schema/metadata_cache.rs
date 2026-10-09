//! Shared schema metadata for sidebar + SQL completion (LUM-021 / LUM-032).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use wisp_drivers::ColumnMeta;

use super::catalog::{SchemaCatalog, SchemaObjectKind};
use super::relations::relations_for_table;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct TableKey {
    pub schema: String,
    pub name: String,
}

/// Catalog plus lazily loaded column lists (single shared instance per connection UI).
#[derive(Debug)]
pub struct SchemaMetadataCache {
    catalog: SchemaCatalog,
    columns: HashMap<TableKey, Vec<ColumnMeta>>,
}

impl SchemaMetadataCache {
    pub fn new(catalog: SchemaCatalog) -> Self {
        Self {
            catalog,
            columns: HashMap::new(),
        }
    }

    pub fn catalog(&self) -> &SchemaCatalog {
        &self.catalog
    }

    pub fn set_catalog(&mut self, catalog: SchemaCatalog) {
        self.catalog = catalog;
        self.columns.clear();
    }

    pub fn apply_schema_load(&mut self, catalog: SchemaCatalog, columns: HashMap<TableKey, Vec<ColumnMeta>>) {
        self.catalog = catalog;
        self.columns = columns;
    }

    pub fn default_schema(&self) -> &str {
        self.catalog
            .schemas
            .first()
            .map(String::as_str)
            .unwrap_or("public")
    }

    pub fn table_names(&self, schema: &str) -> Vec<String> {
        self.catalog
            .objects_in_schema(schema)
            .filter(|o| matches!(o.kind, SchemaObjectKind::Tables | SchemaObjectKind::Views))
            .map(|o| o.name.clone())
            .collect()
    }

    pub fn function_names(&self, schema: &str) -> Vec<String> {
        self.catalog
            .objects_in_schema(schema)
            .filter(|o| o.kind == SchemaObjectKind::Functions)
            .map(|o| o.name.clone())
            .collect()
    }

    /// Load columns on first reference; uses introspection cache when populated (LUM-021).
    pub fn ensure_columns(&mut self, schema: &str, table: &str) -> &[ColumnMeta] {
        let key = TableKey {
            schema: schema.into(),
            name: table.into(),
        };
        if !self.columns.contains_key(&key) {
            self.columns
                .entry(key.clone())
                .or_insert_with(|| mock_columns_for_table(table));
        }
        &self.columns[&key]
    }

    pub fn columns_loaded(&self, schema: &str, table: &str) -> bool {
        self.columns.contains_key(&TableKey {
            schema: schema.into(),
            name: table.into(),
        })
    }
}

pub type SharedMetadataCache = Rc<RefCell<SchemaMetadataCache>>;

pub fn shared_metadata_cache(catalog: SchemaCatalog) -> SharedMetadataCache {
    Rc::new(RefCell::new(SchemaMetadataCache::new(catalog)))
}

fn mock_columns_for_table(table: &str) -> Vec<ColumnMeta> {
    let mut cols = vec![
        ColumnMeta::new("id", "bigint"),
        ColumnMeta::new("created_at", "timestamptz"),
    ];
    if let Some(rel) = relations_for_table(table) {
        for (name, fk) in rel.foreign_keys {
            let ty = format!("bigint /* → {}.{} */", fk.target_table, fk.columns[0].referenced_column);
            cols.push(ColumnMeta::new(name, ty));
        }
    }
    match table {
        "customers" => {
            cols.push(ColumnMeta::new("email", "text"));
            cols.push(ColumnMeta::new("name", "text"));
        }
        "orders" => cols.push(ColumnMeta::new("status", "text")),
        "products" => {
            cols.push(ColumnMeta::new("sku", "text"));
            cols.push(ColumnMeta::new("price", "numeric"));
        }
        "order_items" => cols.push(ColumnMeta::new("quantity", "integer")),
        _ => cols.push(ColumnMeta::new("name", "text")),
    }
    cols
}
