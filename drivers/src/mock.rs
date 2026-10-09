//! In-memory driver for tests and UI harnesses (no network).

use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

use crate::{
    column::ColumnMeta,
    dialect::{Dialect, PostgresDialect},
    driver::{DbDriver, EngineKind, ExecuteStats, PageRequest, QueryId, ServerInfo},
    error::DriverError,
    page::Page,
};

static QUERY_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Simulated table data for `query_paged`.
#[derive(Debug)]
pub struct MockDriver {
    connected: bool,
    in_tx: bool,
    dialect: PostgresDialect,
    total_rows: usize,
    columns: usize,
    cancel_flags: HashMap<QueryId, AtomicBool>,
    active_query: Option<QueryId>,
    tx_stmt_index: u32,
}

impl Default for MockDriver {
    fn default() -> Self {
        Self {
            connected: false,
            in_tx: false,
            dialect: PostgresDialect,
            total_rows: 10_000,
            columns: 10,
            cancel_flags: HashMap::new(),
            active_query: None,
            tx_stmt_index: 0,
        }
    }
}

impl MockDriver {
    pub fn with_shape(mut self, total_rows: usize, columns: usize) -> Self {
        self.total_rows = total_rows;
        self.columns = columns;
        self
    }

    fn ensure_connected(&self) -> Result<(), DriverError> {
        if self.connected {
            Ok(())
        } else {
            Err(DriverError::NotConnected)
        }
    }

    fn next_query_id(&mut self) -> QueryId {
        let id = QueryId::new(QUERY_COUNTER.fetch_add(1, Ordering::Relaxed));
        self.cancel_flags.insert(id, AtomicBool::new(false));
        self.active_query = Some(id);
        id
    }
}

impl DbDriver for MockDriver {
    fn connect(&mut self) -> Result<(), DriverError> {
        self.connected = true;
        Ok(())
    }

    fn close(&mut self) -> Result<(), DriverError> {
        self.connected = false;
        self.in_tx = false;
        Ok(())
    }

    fn ping(&mut self) -> Result<(), DriverError> {
        self.ensure_connected()
    }

    fn execute(&mut self, sql: &str) -> Result<ExecuteStats, DriverError> {
        self.ensure_connected()?;
        if sql.contains("WISP_SYNTAX_ERROR") {
            return Err(DriverError::user(
                "syntax error",
                "simulated syntax error for LUM-033 test",
            ));
        }
        if self.in_tx {
            self.tx_stmt_index += 1;
            if sql.contains("WISP_TEST_FAIL_SECOND") && self.tx_stmt_index == 2 {
                return Err(DriverError::user(
                    "constraint violation",
                    "simulated failure for LUM-029 test",
                ));
            }
        }
        Ok(ExecuteStats { rows_affected: 1 })
    }

    fn query_paged(&mut self, sql: &str, request: PageRequest) -> Result<Page, DriverError> {
        self.ensure_connected()?;
        if request.limit == 0 {
            return Err(DriverError::InvalidPage);
        }
        let query_id = self.next_query_id();
        let cancel = self.cancel_flags.get(&query_id).expect("cancel slot");

        if sql.trim().to_ascii_uppercase().starts_with("EXPLAIN") {
            let plan = mock_explain_json();
            let columns = vec![ColumnMeta::new("QUERY PLAN", "json")];
            let mut builder = Page::builder(columns);
            builder
                .push_text_row(&[plan.as_str()])
                .map_err(|_| DriverError::InvalidPage)?;
            self.active_query = None;
            return Ok(builder.finish());
        }

        if sql.contains("WISP_MOCK_SLOW") {
            for _ in 0..500 {
                if cancel.load(Ordering::Relaxed) {
                    self.active_query = None;
                    return Err(DriverError::Cancelled);
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }

        let col_count = self.columns;
        let columns: Vec<_> = (0..col_count)
            .map(|c| ColumnMeta::new(format!("col_{c}"), "text"))
            .collect();
        let mut builder = Page::builder(columns);

        let start = request.offset as usize;
        let end = (start + request.limit as usize).min(self.total_rows);
        for row in start..end {
            if cancel.load(Ordering::Relaxed) {
                self.active_query = None;
                return Err(DriverError::Cancelled);
            }
            let texts: Vec<String> = (0..col_count)
                .map(|c| format!("r{row}c{c}"))
                .collect();
            let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
            builder.push_text_row(&refs).map_err(|_| DriverError::InvalidPage)?;
        }

        self.active_query = None;
        Ok(builder.finish())
    }

    fn in_flight_query(&self) -> Option<QueryId> {
        self.active_query
    }

    fn cancel(&mut self, query: QueryId) -> Result<(), DriverError> {
        if let Some(flag) = self.cancel_flags.get_mut(&query) {
            flag.store(true, Ordering::Relaxed);
            Ok(())
        } else {
            Err(DriverError::user(
                "Unknown query",
                format!("no query with id {}", query.0),
            ))
        }
    }

    fn begin(&mut self) -> Result<(), DriverError> {
        self.ensure_connected()?;
        self.in_tx = true;
        self.tx_stmt_index = 0;
        Ok(())
    }

    fn commit(&mut self) -> Result<(), DriverError> {
        self.ensure_connected()?;
        self.in_tx = false;
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), DriverError> {
        self.ensure_connected()?;
        self.in_tx = false;
        Ok(())
    }

    fn server_info(&self) -> Result<ServerInfo, DriverError> {
        self.ensure_connected()?;
        Ok(ServerInfo {
            engine: EngineKind::Mock,
            version: "mock-0.1".into(),
            database: "mock".into(),
            user: "mock".into(),
            tls: None,
        })
    }

    fn dialect(&self) -> &dyn Dialect {
        &self.dialect
    }
}

fn mock_explain_json() -> String {
    r#"{"Plan":{"Node Type":"Seq Scan","Relation Name":"users","Total Cost":12.5,"Plans":[{"Node Type":"Index Scan","Total Cost":4.2,"Actual Total Time":0.08}]}}"#.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_paged_query() {
        let mut driver = MockDriver::default().with_shape(500, 3);
        driver.connect().unwrap();
        let page = driver
            .query_paged(
                "SELECT 1",
                PageRequest {
                    limit: 300,
                    offset: 0,
                },
            )
            .unwrap();
        assert_eq!(page.row_count, 300);
        assert_eq!(page.column_count(), 3);
        assert_eq!(page.text_at(0, 0), Some("r0c0"));
    }
}
