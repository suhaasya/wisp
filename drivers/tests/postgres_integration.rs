//! Docker PostgreSQL integration (`WISP_PG_IT=1`, see `docker-compose.bench.yml`).

use std::{
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use tokio::runtime::Runtime;
use wisp_drivers::{
    DbDriver, EngineKind, PageRequest, PostgresConfig, PostgresDriver, QueryId, Value,
};

fn rt() -> Runtime {
    Runtime::new().expect("tokio runtime")
}

#[test]
fn postgres_connect_and_types() {
    if std::env::var_os("WISP_PG_IT").is_none() {
        eprintln!("skipping postgres integration (set WISP_PG_IT=1)");
        return;
    }
    let rt = rt();
    let mut driver = PostgresDriver::new(PostgresConfig::from_env(), rt.handle().clone());
    driver.connect().expect("connect");
    let info = driver.server_info().expect("info");
    assert_eq!(info.engine, EngineKind::PostgreSql);
    assert!(!info.version.is_empty());
    assert!(!info.user.is_empty());

    driver
        .execute(
            "CREATE TEMP TABLE wisp_it (
                id int4, big int8, flag bool, price numeric, body text,
                payload bytea, meta jsonb, uid uuid
            )",
        )
        .expect("create temp");

    driver
        .execute(
            "INSERT INTO wisp_it VALUES (
                1, 9223372036854775807, true, 12.34, 'hello',
                '\\xdeadbeef', '{\"a\":1}', '550e8400-e29b-41d4-a716-446655440000'
            )",
        )
        .expect("insert");

    let page = driver
        .query_paged(
            "SELECT * FROM wisp_it ORDER BY id",
            PageRequest {
                limit: 10,
                offset: 0,
            },
        )
        .expect("page");
    assert_eq!(page.row_count, 1);
    assert_eq!(page.column_count(), 8);
    assert!(matches!(page.value(0, 0), Some(Value::Int(1))));
    assert!(matches!(page.value(0, 2), Some(Value::Bool(true))));
    driver.close().expect("close");
}

#[test]
fn postgres_paging_does_not_load_all_rows() {
    if std::env::var_os("WISP_PG_IT").is_none() {
        return;
    }
    let rt = rt();
    let mut driver = PostgresDriver::new(PostgresConfig::from_env(), rt.handle().clone());
    driver.connect().expect("connect");
    driver
        .execute("CREATE TEMP TABLE wisp_page AS SELECT generate_series(0, 4999) AS id")
        .expect("seed");

    let page = driver
        .query_paged(
            "SELECT id FROM wisp_page ORDER BY id",
            PageRequest {
                limit: 300,
                offset: 4700,
            },
        )
        .expect("page");
    assert_eq!(page.row_count, 300);
    assert_eq!(page.text_at(0, 0), Some("4700"));
    driver.close().expect("close");
}

#[test]
fn postgres_cancel_pg_sleep() {
    if std::env::var_os("WISP_PG_IT").is_none() {
        return;
    }
    let rt = rt();
    let mut driver = PostgresDriver::new(PostgresConfig::from_env(), rt.handle().clone());
    driver.connect().expect("connect");

    let shared = Arc::new(Mutex::new(driver));
    let worker = {
        let shared = Arc::clone(&shared);
        thread::spawn(move || {
            let mut driver = shared.lock().expect("lock");
            let _ = driver.execute("SELECT pg_sleep(30)");
        })
    };

    thread::sleep(Duration::from_millis(100));
    let start = Instant::now();
    shared
        .lock()
        .expect("lock")
        .cancel(QueryId::new(1))
        .expect("cancel");
    assert!(
        start.elapsed() < Duration::from_millis(500),
        "cancel took {:?}",
        start.elapsed()
    );
    let _ = worker.join();
}
