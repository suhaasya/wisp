//! Table data query: filters + sort for a single grid tab.

use super::filter::FilterModel;
use super::sort::SortModel;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableDataQuery {
    pub schema: String,
    pub table: String,
    pub filter: FilterModel,
    pub sort: SortModel,
}

impl Default for TableDataQuery {
    fn default() -> Self {
        Self {
            schema: "public".into(),
            table: String::new(),
            filter: FilterModel::default(),
            sort: SortModel::default(),
        }
    }
}

impl TableDataQuery {
    pub fn for_table(table: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            ..Default::default()
        }
    }
}
