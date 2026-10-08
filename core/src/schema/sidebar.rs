//! Flatten catalog + UI prefs into rows for virtualised sidebar lists.

use std::collections::BTreeSet;

use wisp_store::ConnectionId;

use super::catalog::{SchemaCatalog, SchemaObjectId, SchemaObjectKind};
use super::filter::fuzzy_match_name;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarRowKind {
    GroupHeader,
    Object,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarRow {
    Group {
        kind: SchemaObjectKind,
        count: usize,
        expanded: bool,
    },
    Object {
        id: SchemaObjectId,
        name: String,
        object_kind: SchemaObjectKind,
    },
}

#[derive(Debug, Clone, Default)]
pub struct SchemaSidebarPrefs {
    pub schema: String,
    pub expanded: BTreeSet<SchemaObjectKind>,
}

impl SchemaSidebarPrefs {
    pub fn is_expanded(&self, kind: SchemaObjectKind) -> bool {
        self.expanded.contains(&kind)
    }

    pub fn toggle_group(&mut self, kind: SchemaObjectKind) {
        if !self.expanded.insert(kind) {
            self.expanded.remove(&kind);
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PerConnectionSidebarState {
    pub prefs: SchemaSidebarPrefs,
    pub selected_row: usize,
}

#[derive(Debug, Clone, Default)]
pub struct SchemaSidebarStateStore {
    by_connection: std::collections::HashMap<ConnectionId, PerConnectionSidebarState>,
}

impl SchemaSidebarStateStore {
    pub fn state_mut(&mut self, id: ConnectionId) -> &mut PerConnectionSidebarState {
        self.by_connection.entry(id).or_default()
    }

    pub fn state(&self, id: ConnectionId) -> PerConnectionSidebarState {
        self.by_connection.get(&id).cloned().unwrap_or_default()
    }
}

pub fn build_sidebar_rows(
    catalog: &SchemaCatalog,
    schema: &str,
    prefs: &SchemaSidebarPrefs,
    filter: &str,
) -> Vec<SidebarRow> {
    let counts = catalog.count_by_kind(schema);
    let filtering = !filter.trim().is_empty();
    let mut rows = Vec::new();

    for kind in SchemaObjectKind::ALL {
        let count = *counts.get(&kind).unwrap_or(&0);
        if count == 0 {
            continue;
        }
        let expanded = prefs.is_expanded(kind) || filtering;
        rows.push(SidebarRow::Group {
            kind,
            count,
            expanded,
        });
        if !expanded {
            continue;
        }
        let mut objects: Vec<_> = catalog
            .objects_in_schema(schema)
            .filter(|o| o.kind == kind && fuzzy_match_name(&o.name, filter))
            .map(|o| SidebarRow::Object {
                id: o.id,
                name: o.name.clone(),
                object_kind: o.kind,
            })
            .collect();
        objects.sort_by(|a, b| match (a, b) {
            (SidebarRow::Object { name: a, .. }, SidebarRow::Object { name: b, .. }) => {
                a.cmp(b)
            }
            _ => std::cmp::Ordering::Equal,
        });
        if filtering {
            let matched = objects.len();
            if let Some(SidebarRow::Group { count, .. }) = rows.last_mut() {
                *count = matched;
            }
        }
        rows.extend(objects);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::mock;

    #[test]
    fn virtual_row_count_for_large_table_group() {
        let catalog = mock::demo_catalog_large_tables(5_000);
        let mut prefs = SchemaSidebarPrefs::default();
        prefs.schema = "public".into();
        prefs.expanded.insert(SchemaObjectKind::Tables);
        let rows = build_sidebar_rows(&catalog, "public", &prefs, "");
        assert_eq!(rows.len(), 1 + 5_000);
    }

    #[test]
    fn filter_ten_k_objects_stays_fast() {
        use std::time::Instant;

        let catalog = mock::demo_catalog_large_tables(10_000);
        let mut prefs = SchemaSidebarPrefs::default();
        prefs.schema = "public".into();
        prefs.expanded.insert(SchemaObjectKind::Tables);
        let start = Instant::now();
        let rows = build_sidebar_rows(&catalog, "public", &prefs, "tbl_1234");
        let elapsed = start.elapsed();
        assert!(rows.iter().any(|r| matches!(r, SidebarRow::Object { .. })));
        assert!(
            elapsed.as_millis() < 500,
            "filter took {:?}, expected <500ms",
            elapsed
        );
    }

    #[test]
    fn filter_narrows_table_rows() {
        let catalog = mock::demo_catalog_large_tables(10_000);
        let mut prefs = SchemaSidebarPrefs::default();
        prefs.schema = "public".into();
        prefs.expanded.insert(SchemaObjectKind::Tables);
        let rows = build_sidebar_rows(&catalog, "public", &prefs, "tbl_999");
        let objects = rows
            .iter()
            .filter(|r| matches!(r, SidebarRow::Object { .. }))
            .count();
        assert!(objects >= 1 && objects < 100);
    }
}
