//! SSH bastion integration (`WISP_SSH_IT=1`, `docker-compose.ssh-it.yml`).

use tokio::runtime::Runtime;
use wisp_drivers::{DbDriver, PostgresConfig, PostgresDriver, MysqlConfig, MysqlDriver};

fn rt() -> Runtime {
    Runtime::new().expect("tokio runtime")
}

#[test]
fn postgres_via_ssh_password() {
    if std::env::var_os("WISP_SSH_IT").is_none() {
        eprintln!("skipping ssh integration (set WISP_SSH_IT=1)");
        return;
    }
    let rt = rt();
    let mut driver = PostgresDriver::new(PostgresConfig::from_env(), rt.handle().clone());
    driver.connect().expect("connect through ssh");
    driver.ping().expect("ping");
    driver.close().ok();
}

#[test]
fn mysql_via_ssh_password() {
    if std::env::var_os("WISP_SSH_IT").is_none() {
        return;
    }
    let rt = rt();
    let mut driver = MysqlDriver::new(MysqlConfig::from_env(), rt.handle().clone());
    driver.connect().expect("connect through ssh");
    driver.ping().expect("ping");
    driver.close().ok();
}
