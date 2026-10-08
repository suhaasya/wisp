//! Docker MySQL / MariaDB integration (`WISP_MYSQL_IT=1`, see `docker-compose.bench.yml`).

use std::{
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use tokio::runtime::Runtime;
use wisp_drivers::{
    DbDriver, EngineKind, MysqlConfig, MysqlDriver, PageRequest, QueryId, Value,
};

fn rt() -> Runtime {
    Runtime::new().expect("tokio runtime")
}

#[test]
fn mysql_connect_and_types() {
    if std::env::var_os("WISP_MYSQL_IT").is_none() {
        eprintln!("skipping mysql integration (set WISP_MYSQL_IT=1)");
        return;
    }
    let rt = rt();
    let mut driver = MysqlDriver::new(MysqlConfig::from_env(), rt.handle().clone());
    driver.connect().expect("connect");
    let info = driver.server_info().expect("info");
    assert!(
        matches!(info.engine, EngineKind::MySql | EngineKind::MariaDb),
        "engine: {:?}",
        info.engine
    );
    assert!(!info.version.is_empty());

    driver
        .execute(
            "CREATE TEMPORARY TABLE wisp_it (
                id INT PRIMARY KEY,
                flag TINYINT(1),
                price DECIMAL(10,2),
                body TEXT,
                payload BLOB,
                meta JSON,
                kind ENUM('a','b')
            )",
        )
        .expect("create");

    driver
        .execute(
            "INSERT INTO wisp_it VALUES
            (1, 1, 12.34, 'hello', X'deadbeef', '{\"k\":1}', 'a')",
        )
        .expect("insert");

    let page = driver
        .query_paged(
            "SELECT id, flag, price, body, payload, meta, kind FROM wisp_it",
            PageRequest {
                limit: 10,
                offset: 0,
            },
        )
        .expect("page");
    assert_eq!(page.row_count, 1);
    assert!(matches!(page.value(0, 0), Some(Value::Int(1))));
    assert!(matches!(page.value(0, 1), Some(Value::Bool(true))));
    driver.close().expect("close");
}

#[test]
fn mysql_paging_stream_budget() {
    if std::env::var_os("WISP_MYSQL_IT").is_none() {
        return;
    }
    let rt = rt();
    let mut driver = MysqlDriver::new(MysqlConfig::from_env(), rt.handle().clone());
    driver.connect().expect("connect");
    driver
        .execute("CREATE TEMPORARY TABLE wisp_page (id INT PRIMARY KEY)")
        .expect("create");
    driver
        .execute(
            "INSERT INTO wisp_page
             WITH RECURSIVE seq AS (
               SELECT 0 AS id
               UNION ALL
               SELECT id + 1 FROM seq WHERE id < 4999
             )
             SELECT id FROM seq",
        )
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
    driver.close().expect("close");
}

#[test]
fn mysql_kill_query_cancel() {
    if std::env::var_os("WISP_MYSQL_IT").is_none() {
        return;
    }
    let rt = rt();
    let mut driver = MysqlDriver::new(MysqlConfig::from_env(), rt.handle().clone());
    driver.connect().expect("connect");

    let shared = Arc::new(Mutex::new(driver));
    let worker = {
        let shared = Arc::clone(&shared);
        thread::spawn(move || {
            let mut driver = shared.lock().expect("lock");
            let _ = driver.execute("SELECT SLEEP(30)");
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
