//! [`DbDriver`] implementation via `tokio-postgres` (plain TCP).

use std::{
    sync::{Arc, Mutex},
    sync::atomic::{AtomicU64, Ordering},
};

use tokio_postgres::{Client, Config, NoTls};

use crate::{
    dialect::{Dialect, PostgresDialect},
    driver::{DbDriver, EngineKind, ExecuteStats, PageRequest, QueryId, ServerInfo},
    error::DriverError,
    page::PageBuilder,
    postgres::{config::PostgresConfig, pgpass::resolve_password, types},
    ColumnMeta, Page,
};

const CURSOR: &str = "wisp_cursor";

struct CursorState {
    sql: String,
    position: u64,
}

struct ActiveQuery {
    id: QueryId,
    cancel: tokio_postgres::CancelToken,
}

struct Session {
    client: Arc<Client>,
    connection: tokio::task::JoinHandle<()>,
}

struct PgState {
    session: Option<Session>,
    server_info: Option<ServerInfo>,
    in_tx: bool,
    cursor: Option<CursorState>,
    active: Option<ActiveQuery>,
    next_query: AtomicU64,
}

impl PgState {
    fn new() -> Self {
        Self {
            session: None,
            server_info: None,
            in_tx: false,
            cursor: None,
            active: None,
            next_query: AtomicU64::new(1),
        }
    }

    fn client(&self) -> Result<Arc<Client>, DriverError> {
        self.session
            .as_ref()
            .map(|s| Arc::clone(&s.client))
            .ok_or(DriverError::NotConnected)
    }

    fn next_query_id(&self) -> QueryId {
        QueryId::new(self.next_query.fetch_add(1, Ordering::Relaxed))
    }
}

/// PostgreSQL driver (SCRAM / MD5 / trust via `tokio-postgres`; TLS in LUM-018).
pub struct PostgresDriver {
    config: PostgresConfig,
    runtime: tokio::runtime::Handle,
    state: Arc<Mutex<PgState>>,
    dialect: PostgresDialect,
}

impl PostgresDriver {
    pub fn new(config: PostgresConfig, runtime: tokio::runtime::Handle) -> Self {
        Self {
            config,
            runtime,
            state: Arc::new(Mutex::new(PgState::new())),
            dialect: PostgresDialect,
        }
    }

    fn run<F, R>(&self, fut: F) -> R
    where
        F: std::future::Future<Output = R>,
    {
        let handle = self.runtime.clone();
        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::task::block_in_place(|| handle.block_on(fut))
        } else {
            handle.block_on(fut)
        }
    }
}

impl DbDriver for PostgresDriver {
    fn connect(&mut self) -> Result<(), DriverError> {
        let config = self.config.clone();
        let state = Arc::clone(&self.state);
        self.run(connect_session(state, config))
    }

    fn close(&mut self) -> Result<(), DriverError> {
        let state = Arc::clone(&self.state);
        self.run(close_session(state))
    }

    fn ping(&mut self) -> Result<(), DriverError> {
        let state = Arc::clone(&self.state);
        self.run(async move {
            let client = state.lock().expect("pg state").client()?;
            client
                .query_one("SELECT 1", &[])
                .await
                .map(|_| ())
                .map_err(map_pg_err)
        })
    }

    fn execute(&mut self, sql: &str) -> Result<ExecuteStats, DriverError> {
        let sql = sql.to_owned();
        let state = Arc::clone(&self.state);
        self.run(async move {
            let client = {
                let mut guard = state.lock().expect("pg state");
                let client = guard.client()?;
                let query_id = guard.next_query_id();
                guard.active = Some(ActiveQuery {
                    id: query_id,
                    cancel: client.cancel_token(),
                });
                client
            };
            let affected = client.execute(sql.as_str(), &[]).await.map_err(map_pg_err)? as u64;
            state.lock().expect("pg state").active = None;
            Ok(ExecuteStats {
                rows_affected: affected,
            })
        })
    }

    fn query_paged(&mut self, sql: &str, request: PageRequest) -> Result<Page, DriverError> {
        let sql = sql.to_owned();
        let state = Arc::clone(&self.state);
        self.run(query_paged(state, sql, request))
    }

    fn cancel(&mut self, query: QueryId) -> Result<(), DriverError> {
        let state = Arc::clone(&self.state);
        self.run(cancel_query(state, query))
    }

    fn begin(&mut self) -> Result<(), DriverError> {
        let state = Arc::clone(&self.state);
        self.run(async move {
            let client = state.lock().expect("pg state").client()?;
            client.batch_execute("BEGIN").await.map_err(map_pg_err)?;
            state.lock().expect("pg state").in_tx = true;
            Ok(())
        })
    }

    fn commit(&mut self) -> Result<(), DriverError> {
        let state = Arc::clone(&self.state);
        self.run(async move {
            let client = state.lock().expect("pg state").client()?;
            client.batch_execute("COMMIT").await.map_err(map_pg_err)?;
            let mut guard = state.lock().expect("pg state");
            guard.in_tx = false;
            guard.cursor = None;
            Ok(())
        })
    }

    fn rollback(&mut self) -> Result<(), DriverError> {
        let state = Arc::clone(&self.state);
        self.run(async move {
            let client = state.lock().expect("pg state").client()?;
            client.batch_execute("ROLLBACK").await.map_err(map_pg_err)?;
            let mut guard = state.lock().expect("pg state");
            guard.in_tx = false;
            guard.cursor = None;
            Ok(())
        })
    }

    fn server_info(&self) -> Result<ServerInfo, DriverError> {
        self.state
            .lock()
            .expect("pg state")
            .server_info
            .clone()
            .ok_or(DriverError::NotConnected)
    }

    fn dialect(&self) -> &dyn Dialect {
        &self.dialect
    }
}

async fn connect_session(
    state: Arc<Mutex<PgState>>,
    config: PostgresConfig,
) -> Result<(), DriverError> {
    if state.lock().expect("pg state").session.is_some() {
        return Ok(());
    }
    let password = resolve_password(
        &config.host,
        config.port,
        &config.database,
        &config.user,
        config.password.as_deref(),
    )?;
    let cfg = build_pg_config(&config, password);
    let stream = wisp_transport::connect_tcp(&config.host, config.port)
        .await
        .map_err(|e| DriverError::user("TCP connect failed", e.to_string()))?;
    let (client, connection) = cfg
        .connect_raw(stream, NoTls)
        .await
        .map_err(|e| DriverError::user("Could not connect to PostgreSQL", e.to_string()))?;
    let join = tokio::spawn(async move {
        if let Err(err) = connection.await {
            eprintln!("postgres connection closed: {err}");
        }
    });
    let info = load_server_info(&client).await?;
    let mut guard = state.lock().expect("pg state");
    guard.server_info = Some(info);
    guard.session = Some(Session {
        client: Arc::new(client),
        connection: join,
    });
    Ok(())
}

async fn close_session(state: Arc<Mutex<PgState>>) -> Result<(), DriverError> {
    if state.lock().expect("pg state").cursor.is_some() {
        close_cursor(&state).await?;
    }
    let session = state.lock().expect("pg state").session.take();
    if let Some(session) = session {
        session.connection.abort();
    }
    let mut guard = state.lock().expect("pg state");
    guard.in_tx = false;
    guard.server_info = None;
    Ok(())
}

async fn close_cursor(state: &Arc<Mutex<PgState>>) -> Result<(), DriverError> {
    if state.lock().expect("pg state").cursor.is_none() {
        return Ok(());
    }
    let client = state.lock().expect("pg state").client()?;
    let _ = client
        .batch_execute(&format!("CLOSE {CURSOR}"))
        .await
        .map_err(map_pg_err);
    let in_tx = state.lock().expect("pg state").in_tx;
    if in_tx {
        let _ = client.batch_execute("ROLLBACK").await;
    }
    let mut guard = state.lock().expect("pg state");
    guard.in_tx = false;
    guard.cursor = None;
    Ok(())
}

async fn ensure_cursor(state: &Arc<Mutex<PgState>>, sql: &str) -> Result<(), DriverError> {
    let needs_new = state
        .lock()
        .expect("pg state")
        .cursor
        .as_ref()
        .is_none_or(|c| c.sql != sql);
    if !needs_new {
        return Ok(());
    }
    close_cursor(state).await?;
    let client = state.lock().expect("pg state").client()?;
    client
        .batch_execute("BEGIN READ ONLY")
        .await
        .map_err(map_pg_err)?;
    let declare = format!("DECLARE {CURSOR} NO SCROLL CURSOR FOR {sql}");
    client.batch_execute(&declare).await.map_err(map_pg_err)?;
    let mut guard = state.lock().expect("pg state");
    guard.in_tx = true;
    guard.cursor = Some(CursorState {
        sql: sql.to_string(),
        position: 0,
    });
    Ok(())
}

async fn query_paged(
    state: Arc<Mutex<PgState>>,
    sql: String,
    request: PageRequest,
) -> Result<Page, DriverError> {
    if request.limit == 0 {
        return Err(DriverError::InvalidPage);
    }
    {
        let mut guard = state.lock().expect("pg state");
        let client = guard.client()?;
        let query_id = guard.next_query_id();
        guard.active = Some(ActiveQuery {
            id: query_id,
            cancel: client.cancel_token(),
        });
    }

    ensure_cursor(&state, &sql).await?;

    let (client, fetch_sql, skip) = {
        let mut guard = state.lock().expect("pg state");
        let client = guard.client()?;
        let cursor = guard.cursor.as_mut().expect("cursor");
        let skip = if request.offset > cursor.position {
            Some((request.offset - cursor.position, client.clone()))
        } else {
            None
        };
        let fetch_sql = format!("FETCH FORWARD {} FROM {CURSOR}", request.limit);
        (client, fetch_sql, skip)
    };

    if let Some((skip, client)) = skip {
        let move_sql = format!("MOVE FORWARD {skip} IN {CURSOR}");
        client
            .execute(move_sql.as_str(), &[])
            .await
            .map_err(map_pg_err)?;
        state.lock().expect("pg state").cursor.as_mut().expect("cursor").position +=
            skip;
    }

    let rows = client
        .query(fetch_sql.as_str(), &[])
        .await
        .map_err(map_pg_err)?;
    {
        let mut guard = state.lock().expect("pg state");
        guard.cursor.as_mut().expect("cursor").position += rows.len() as u64;
        guard.active = None;
    }

    if rows.is_empty() {
        let columns = describe_columns(&client, &sql).await?;
        return Ok(Page::builder(columns).finish());
    }

    let columns: Vec<ColumnMeta> = rows[0]
        .columns()
        .iter()
        .map(|col| types::column_meta(col.name(), col.type_()))
        .collect();
    let mut builder = PageBuilder::with_capacity(
        columns,
        rows.len(),
        rows.len() * rows[0].len() * 32,
    );
    for row in &rows {
        let cells = types::row_to_values(row, builder.arena_mut());
        builder
            .push_row(cells)
            .map_err(|_| DriverError::InvalidPage)?;
    }
    Ok(builder.finish())
}

async fn cancel_query(state: Arc<Mutex<PgState>>, query: QueryId) -> Result<(), DriverError> {
    let cancel = {
        let guard = state.lock().expect("pg state");
        let Some(active) = guard.active.as_ref() else {
            return Err(DriverError::user(
                "No running query",
                format!("query id {}", query.0),
            ));
        };
        if active.id != query {
            return Err(DriverError::user(
                "Unknown query",
                format!("active id {}", active.id.0),
            ));
        }
        active.cancel.clone()
    };
    cancel
        .cancel_query(NoTls)
        .await
        .map_err(|e| DriverError::user("Cancel failed", e.to_string()))?;
    state.lock().expect("pg state").active = None;
    Ok(())
}

async fn describe_columns(
    client: &Arc<Client>,
    sql: &str,
) -> Result<Vec<ColumnMeta>, DriverError> {
    let stmt = client.prepare(sql).await.map_err(map_pg_err)?;
    Ok(stmt
        .columns()
        .iter()
        .map(|col| types::column_meta(col.name(), col.type_()))
        .collect())
}

fn build_pg_config(config: &PostgresConfig, password: Option<String>) -> Config {
    let mut cfg = Config::new();
    cfg.host(config.host.as_str());
    cfg.port(config.port);
    cfg.user(&config.user);
    cfg.dbname(&config.database);
    cfg.application_name(&config.application_name);
    if let Some(password) = password {
        cfg.password(password);
    }
    if let Some(timeout) = config.statement_timeout {
        let ms = timeout.as_millis();
        cfg.options(format!("-c statement_timeout={ms}"));
    }
    cfg
}

async fn load_server_info(client: &Client) -> Result<ServerInfo, DriverError> {
    let row = client
        .query_one("SELECT version(), current_database(), current_user", &[])
        .await
        .map_err(map_pg_err)?;
    Ok(ServerInfo {
        engine: EngineKind::PostgreSql,
        version: row.get(0),
        database: row.get(1),
        user: row.get(2),
    })
}

fn map_pg_err(err: tokio_postgres::Error) -> DriverError {
    if err.is_closed() {
        return DriverError::NotConnected;
    }
    if let Some(db) = err.as_db_error() {
        if db.code().code() == "57014" {
            return DriverError::Cancelled;
        }
    }
    DriverError::user("PostgreSQL error", err.to_string())
}
