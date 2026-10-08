//! Session manager behaviour (LUM-020).

use std::time::Duration;

use tokio::runtime::Runtime;
use wisp_core::{
    session::{SessionManager, SessionOpenSpec, SessionRuntimeConfig},
    ConnectionEngine, ConnectionProfile,
};
use wisp_store::ConnectionId;

fn mock_profile(read_only: bool) -> ConnectionProfile {
    let mut profile = ConnectionProfile::new("mock", ConnectionEngine::PostgreSql);
    profile.id = ConnectionId::new_v7();
    profile.host = Some("wisp-mock".into());
    profile.read_only = read_only;
    profile
}

#[test]
fn read_only_blocks_writes_before_driver() {
    let rt = Runtime::new().expect("runtime");
    let manager = SessionManager::new(SessionRuntimeConfig {
        keep_alive: Duration::from_secs(3600),
        ..SessionRuntimeConfig::default()
    });
    let profile = mock_profile(true);
    let id = profile.id;
    let spec = SessionOpenSpec {
        profile,
        secrets: Default::default(),
    };
    rt.block_on(async {
        SessionManager::open(&manager, spec, tokio::runtime::Handle::current())
            .await
            .expect("open mock");
        let err = SessionManager::execute(
            &manager,
            id,
            "INSERT INTO t VALUES (1)",
            false,
        )
        .await
        .expect_err("write blocked");
        assert!(matches!(
            err,
            wisp_core::SessionError::ReadOnlyWriteBlocked
        ));
        SessionManager::shutdown_all(&manager).await;
    });
}
