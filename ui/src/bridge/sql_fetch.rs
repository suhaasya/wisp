//! Blocking session SQL page fetch via the DB bridge (LUM-033 / grid browse).

use std::sync::{Arc, Mutex};

use wisp_core::{
    bridge::{DbBridge, DbCommandPayload, DbEventPayload},
    ConnectionId, DriverError, Page, SessionSqlFetcher,
};

pub struct BridgeSqlFetcher {
    pub bridge: Arc<DbBridge>,
    pub connection_id: ConnectionId,
    pub sql: String,
}

impl SessionSqlFetcher for BridgeSqlFetcher {
    fn fetch_page(&mut self, offset: u64, limit: u32) -> Result<Page, DriverError> {
        let (_, _, done_rx) = self.bridge.submit(DbCommandPayload::SessionQueryPage {
            id: self.connection_id,
            sql: self.sql.clone(),
            offset,
            limit,
        });
        let event = done_rx
            .recv_timeout(std::time::Duration::from_secs(120))
            .map_err(|_| DriverError::user("Query page timeout", "bridge recv"))?;
        match event.result() {
            Ok(DbEventPayload::SessionQueryPage(Ok(snapshot))) => snapshot.into_page(),
            Ok(DbEventPayload::SessionQueryPage(Err(err))) => {
                Err(DriverError::user("Query failed", &err.to_string()))
            }
            _ => Err(DriverError::user(
                "Unexpected bridge response",
                "session query page",
            )),
        }
    }
}

pub fn shared_sql_fetcher(
    bridge: Arc<DbBridge>,
    connection_id: ConnectionId,
    sql: String,
) -> Arc<Mutex<dyn SessionSqlFetcher>> {
    Arc::new(Mutex::new(BridgeSqlFetcher {
        bridge,
        connection_id,
        sql,
    }))
}
