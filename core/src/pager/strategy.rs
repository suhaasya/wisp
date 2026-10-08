//! How pages are fetched from the server.

use wisp_drivers::Value;

/// Stable keyset sort: `(sort columns…, primary key)` ascending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeysetPlan {
    pub order_columns: Vec<String>,
    pub primary_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchStrategyKind {
    Keyset,
    ServerCursor,
    Offset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchStrategy {
    Keyset(KeysetPlan),
    ServerCursor,
    Offset,
}

impl FetchStrategy {
    pub fn kind(&self) -> FetchStrategyKind {
        match self {
            Self::Keyset(_) => FetchStrategyKind::Keyset,
            Self::ServerCursor => FetchStrategyKind::ServerCursor,
            Self::Offset => FetchStrategyKind::Offset,
        }
    }
}

/// Last-row cursor for the next keyset page.
#[derive(Debug, Clone, PartialEq)]
pub struct KeysetCursor {
    pub values: Vec<Value>,
}
