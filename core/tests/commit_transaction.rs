//! Transactional commit rolls back on mid-batch failure (LUM-029).

use std::time::Duration;

use tokio::runtime::Runtime;
use wisp_core::{
    session::{SessionManager, SessionOpenSpec, SessionRuntimeConfig},
    ConnectionEngine, ConnectionProfile, SessionError,
};
use wisp_store::ConnectionId;

fn mock_profile() -> ConnectionProfile {
    let mut profile = ConnectionProfile::new("mock", ConnectionEngine::PostgreSql);
    profile.id = ConnectionId::new_v7();
    profile.host = Some("wisp-mock".into());
    profile
}

#[test]
fn constraint_violation_mid_batch_rolls_back() {
    let rt = Runtime::new().expect("runtime");
    let manager = SessionManager::new(SessionRuntimeConfig {
        keep_alive: Duration::from_secs(3600),
        ..SessionRuntimeConfig::default()
    });
    let profile = mock_profile();
    let id = profile.id;
    let spec = SessionOpenSpec {
        profile,
        secrets: Default::default(),
    };
    rt.block_on(async {
        SessionManager::open(&manager, spec, tokio::runtime::Handle::current())
            .await
            .expect("open mock");
        let err = SessionManager::commit_transaction(
            &manager,
            id,
            vec![
                "UPDATE t SET a = 1 WHERE id = 1".into(),
                "UPDATE t SET a = 2 WHERE id = 2 WISP_TEST_FAIL_SECOND".into(),
            ],
            true,
        )
        .await
        .expect_err("second stmt fails");
        assert!(matches!(
            err,
            SessionError::CommitFailed { index: 1, .. }
        ));
        SessionManager::shutdown_all(&manager).await;
    });
}
