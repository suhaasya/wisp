//! [`DbDriver`] for MySQL and MariaDB via `mysql_async` (plain TCP).

use std::{
    sync::Arc,
    sync::atomic::{AtomicU64, Ordering},
};

use tokio::sync::Mutex;

use mysql_async::{Conn, Opts, OptsBuilder, prelude::*};

use crate::ssh_tunnel::{map_ssh_error, mysql_local_forward};
use crate::tls::mysql_ssl_opts;
use wisp_transport::SshSession;
use crate::TlsInfo;
use wisp_store::SslMode;

use crate::{
    dialect::{Dialect, MysqlDialect},
    driver::{DbDriver, EngineKind, ExecuteStats, PageRequest, QueryId, ServerInfo},
    error::DriverError,
    mysql::{config::MysqlConfig, types},
    page::PageBuilder,
    ColumnMeta, Page,
};

struct ActiveQuery {
    id: QueryId,
    connection_id: u64,
}

struct MysqlState {
    conn: Option<Conn>,
    cancel_conn: Option<Conn>,
    server_info: Option<ServerInfo>,
    in_tx: bool,
    active: Option<ActiveQuery>,
    next_query: AtomicU64,
    ssh: Option<SshSession>,
}

impl MysqlState {
    fn new() -> Self {
        Self {
            conn: None,
            cancel_conn: None,
            server_info: None,
            in_tx: false,
            active: None,
            next_query: AtomicU64::new(1),
            ssh: None,
        }
    }

    fn next_query_id(&self) -> QueryId {
        QueryId::new(self.next_query.fetch_add(1, Ordering::Relaxed))
    }
}

/// MySQL / MariaDB driver (`caching_sha2_password`, `mysql_native_password`; TLS in LUM-018).
pub struct MysqlDriver {
    config: MysqlConfig,
    runtime: tokio::runtime::Handle,
    state: Arc<Mutex<MysqlState>>,
    dialect: MysqlDialect,
}

impl MysqlDriver {
    pub fn new(config: MysqlConfig, runtime: tokio::runtime::Handle) -> Self {
        Self {
            config,
            runtime,
            state: Arc::new(Mutex::new(MysqlState::new())),
            dialect: MysqlDialect,
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

impl DbDriver for MysqlDriver {
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
            let mut guard = state.lock().await;
            let conn = guard.conn.as_mut().ok_or(DriverError::NotConnected)?;
            conn.query_drop("SELECT 1").await.map_err(map_mysql_err)?;
            Ok(())
        })
    }

    fn execute(&mut self, sql: &str) -> Result<ExecuteStats, DriverError> {
        let sql = sql.to_owned();
        let state = Arc::clone(&self.state);
        self.run(async move {
            let (connection_id, query_id) = {
                let mut guard = state.lock().await;
                let conn = guard.conn.as_mut().ok_or(DriverError::NotConnected)?;
                let connection_id: u64 = conn
                    .query_first("SELECT CONNECTION_ID()")
                    .await
                    .map_err(map_mysql_err)?
                    .ok_or_else(|| DriverError::user("MySQL error", "no connection id"))?;
                let query_id = guard.next_query_id();
                guard.active = Some(ActiveQuery {
                    id: query_id,
                    connection_id,
                });
                (connection_id, query_id)
            };
            let _ = (connection_id, query_id);
            let result = {
                let mut guard = state.lock().await;
                let conn = guard.conn.as_mut().ok_or(DriverError::NotConnected)?;
                conn.query_drop(sql.as_str()).await
            };
            state.lock().await.active = None;
            result.map_err(map_mysql_err)?;
            let affected = {
                let mut guard = state.lock().await;
                let conn = guard.conn.as_mut().ok_or(DriverError::NotConnected)?;
                conn.affected_rows()
            };
            Ok(ExecuteStats { rows_affected: affected })
        })
    }

    fn query_paged(&mut self, sql: &str, request: PageRequest) -> Result<Page, DriverError> {
        let sql = sql.to_owned();
        let config = self.config.clone();
        let state = Arc::clone(&self.state);
        self.run(query_paged(state, config, sql, request))
    }

    fn cancel(&mut self, query: QueryId) -> Result<(), DriverError> {
        let config = self.config.clone();
        let state = Arc::clone(&self.state);
        self.run(cancel_query(state, config, query))
    }

    fn begin(&mut self) -> Result<(), DriverError> {
        let state = Arc::clone(&self.state);
        self.run(async move {
            let mut guard = state.lock().await;
            let conn = guard.conn.as_mut().ok_or(DriverError::NotConnected)?;
            conn.query_drop("START TRANSACTION")
                .await
                .map_err(map_mysql_err)?;
            guard.in_tx = true;
            Ok(())
        })
    }

    fn commit(&mut self) -> Result<(), DriverError> {
        let state = Arc::clone(&self.state);
        self.run(async move {
            let mut guard = state.lock().await;
            let conn = guard.conn.as_mut().ok_or(DriverError::NotConnected)?;
            conn.query_drop("COMMIT").await.map_err(map_mysql_err)?;
            guard.in_tx = false;
            Ok(())
        })
    }

    fn rollback(&mut self) -> Result<(), DriverError> {
        let state = Arc::clone(&self.state);
        self.run(async move {
            let mut guard = state.lock().await;
            let conn = guard.conn.as_mut().ok_or(DriverError::NotConnected)?;
            conn.query_drop("ROLLBACK").await.map_err(map_mysql_err)?;
            guard.in_tx = false;
            Ok(())
        })
    }

    fn server_info(&self) -> Result<ServerInfo, DriverError> {
        let state = Arc::clone(&self.state);
        self.run(async move {
            let guard = state.lock().await;
            guard.server_info.clone().ok_or(DriverError::NotConnected)
        })
    }

    fn dialect(&self) -> &dyn Dialect {
        &self.dialect
    }
}

async fn connect_session(state: Arc<Mutex<MysqlState>>, config: MysqlConfig) -> Result<(), DriverError> {
    if state.lock().await.conn.is_some() {
        return Ok(());
    }
    let mut connect_host = config.host.clone();
    let mut connect_port = config.port;
    let mut ssh_session = None;
    if config.ssh.enabled {
        let (local_port, session) = mysql_local_forward(
            &config.ssh,
            &config.ssh_secrets,
            config.host.as_str(),
            config.port,
        )
        .await
        .map_err(map_ssh_error)?;
        connect_host = "127.0.0.1".into();
        connect_port = local_port;
        ssh_session = Some(session);
    }

    let mut builder = OptsBuilder::default()
        .ip_or_hostname(connect_host.as_str())
        .tcp_port(connect_port)
        .user(Some(config.user.as_str()))
        .db_name(Some(config.database.as_str()));
    if let Some(password) = config.password.as_deref() {
        builder = builder.pass(Some(password));
    }
    if let Some(ssl_opts) = mysql_ssl_opts(&config.ssl) {
        builder = builder.ssl_opts(ssl_opts);
    }
    let opts: Opts = builder.into();
    let mut conn = Conn::new(opts).await.map_err(map_mysql_err)?;
    if let Some(timeout) = config.statement_timeout {
        let ms = timeout.as_millis().max(1);
        conn.query_drop(format!("SET SESSION max_execution_time = {ms}"))
            .await
            .map_err(map_mysql_err)?;
    }
    conn.query_drop("SELECT 1").await.map_err(map_mysql_err)?;
    let mut info = load_server_info(&mut conn).await?;
    if config.ssl.mode != SslMode::Disable {
        info.tls = Some(TlsInfo {
            version: "TLS".into(),
            cipher: "rustls".into(),
        });
    }
    let mut guard = state.lock().await;
    guard.server_info = Some(info);
    guard.conn = Some(conn);
    guard.ssh = ssh_session;
    Ok(())
}

async fn close_session(state: Arc<Mutex<MysqlState>>) -> Result<(), DriverError> {
    let mut guard = state.lock().await;
    guard.conn = None;
    guard.cancel_conn = None;
    guard.server_info = None;
    guard.in_tx = false;
    guard.active = None;
    guard.ssh = None;
    Ok(())
}

fn page_sql(sql: &str, request: PageRequest) -> String {
    format!(
        "SELECT * FROM ({sql}) AS wisp_page_sub LIMIT {} OFFSET {}",
        request.limit, request.offset
    )
}

async fn query_paged(
    state: Arc<Mutex<MysqlState>>,
    config: MysqlConfig,
    sql: String,
    request: PageRequest,
) -> Result<Page, DriverError> {
    if request.limit == 0 {
        return Err(DriverError::InvalidPage);
    }
    let paged = page_sql(&sql, request);
    let mut guard = state.lock().await;
    let connection_id: u64 = {
        let conn = guard.conn.as_mut().ok_or(DriverError::NotConnected)?;
        conn.query_first("SELECT CONNECTION_ID()")
            .await
            .map_err(map_mysql_err)?
            .ok_or_else(|| DriverError::user("MySQL error", "no connection id"))?
    };
    let query_id = guard.next_query_id();
    guard.active = Some(ActiveQuery {
        id: query_id,
        connection_id,
    });

    let conn = guard.conn.as_mut().ok_or(DriverError::NotConnected)?;
    let mut stream = conn.query_iter(paged).await.map_err(map_mysql_err)?;
    let column_meta: Vec<ColumnMeta> = stream
        .columns_ref()
        .iter()
        .map(|c| types::column_meta(&c.name_str(), c.column_type()))
        .collect();
    let col_count = column_meta.len().max(1);

    let mut builder = PageBuilder::with_capacity(
        column_meta,
        request.limit as usize,
        request.limit as usize * col_count * 32,
    );

    let mut row_count = 0usize;
    while row_count < request.limit as usize {
        let row = stream.next().await.map_err(map_mysql_err)?;
        let Some(row) = row else {
            break;
        };
        let cells =
            types::row_to_values(&row, builder.arena_mut(), config.tinyint1_is_bool);
        builder
            .push_row(cells)
            .map_err(|_| DriverError::InvalidPage)?;
        row_count += 1;
    }

    guard.active = None;
    Ok(builder.finish())
}

async fn cancel_query(
    state: Arc<Mutex<MysqlState>>,
    config: MysqlConfig,
    query: QueryId,
) -> Result<(), DriverError> {
    let connection_id = {
        let guard = state.lock().await;
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
        active.connection_id
    };

    let kill = format!("KILL QUERY {connection_id}");
    {
        let mut guard = state.lock().await;
        if guard.cancel_conn.is_none() {
            let mut builder = OptsBuilder::default()
                .ip_or_hostname(config.host.as_str())
                .tcp_port(config.port)
                .user(Some(config.user.as_str()))
                .db_name(Some(config.database.as_str()));
            if let Some(password) = config.password.as_deref() {
                builder = builder.pass(Some(password));
            }
            let opts: Opts = builder.into();
            guard.cancel_conn = Some(Conn::new(opts).await.map_err(map_mysql_err)?);
        }
        let cancel = guard.cancel_conn.as_mut().expect("cancel conn");
        cancel.query_drop(kill.as_str()).await.map_err(map_mysql_err)?;
    }
    state.lock().await.active = None;
    Ok(())
}

async fn load_server_info(conn: &mut Conn) -> Result<ServerInfo, DriverError> {
    let row: (String, Option<String>, String) = conn
        .query_first("SELECT VERSION(), DATABASE(), CURRENT_USER()")
        .await
        .map_err(map_mysql_err)?
        .ok_or_else(|| DriverError::user("MySQL error", "version query empty"))?;
    let (version, database, user) = row;
    let engine = if version.to_ascii_lowercase().contains("mariadb") {
        EngineKind::MariaDb
    } else {
        EngineKind::MySql
    };
    Ok(ServerInfo {
        engine,
        version,
        database: database.unwrap_or_default(),
        user,
        tls: None,
    })
}

fn map_mysql_err(err: mysql_async::Error) -> DriverError {
    let message = err.to_string();
    if message.contains("Query execution was interrupted") || message.contains("1317") {
        return DriverError::Cancelled;
    }
    DriverError::user("MySQL error", message)
}
