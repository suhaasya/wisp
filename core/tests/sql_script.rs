//! Script execution, errors, and cancel (LUM-033).

use std::time::{Duration, Instant};

use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;
use wisp_core::{
    session::{SessionManager, SessionOpenSpec, SessionRuntimeConfig, ScriptStatement},
    split_statements, ConnectionEngine, ConnectionProfile, SessionError, SplitFlavor,
    StatementRunOutcome,
};
use wisp_store::ConnectionId;

fn mock_profile() -> ConnectionProfile {
    let mut profile = ConnectionProfile::new("mock", ConnectionEngine::PostgreSql);
    profile.id = ConnectionId::new_v7();
    profile.host = Some("wisp-mock".into());
    profile
}

fn open_mock(rt: &Runtime, manager: &std::sync::Arc<tokio::sync::Mutex<SessionManager>>) -> ConnectionId {
    let profile = mock_profile();
    let id = profile.id;
    let spec = SessionOpenSpec {
        profile,
        secrets: Default::default(),
    };
    rt.block_on(async {
        SessionManager::open(manager, spec, tokio::runtime::Handle::current())
            .await
            .expect("open mock");
    });
    id
}

#[test]
fn multi_statement_script_mixed_outcomes() {
    let rt = Runtime::new().expect("runtime");
    let manager = SessionManager::new(SessionRuntimeConfig {
        keep_alive: Duration::from_secs(3600),
        ..SessionRuntimeConfig::default()
    });
    let id = open_mock(&rt, &manager);
    let script = "SELECT 1;\nUPDATE t SET a=1;\nSELECT 2;";
    let ranges = split_statements(script, SplitFlavor::Standard);
    assert_eq!(ranges.len(), 3);
    let statements: Vec<_> = ranges
        .into_iter()
        .map(|r| ScriptStatement {
            byte_start: r.start,
            sql: script[r].to_string(),
        })
        .collect();
    rt.block_on(async {
        let report = SessionManager::run_script(
            &manager,
            id,
            statements,
            true,
            CancellationToken::new(),
        )
        .await
        .expect("run");
        assert!(!report.cancelled);
        assert_eq!(report.outcomes.len(), 3);
        assert!(matches!(
            report.outcomes[0],
            StatementRunOutcome::ResultSet { .. }
        ));
        assert!(matches!(
            report.outcomes[1],
            StatementRunOutcome::Message { .. }
        ));
        assert!(matches!(
            report.outcomes[2],
            StatementRunOutcome::ResultSet { .. }
        ));
        SessionManager::shutdown_all(&manager).await;
    });
}

#[test]
fn error_at_third_statement_stops_batch() {
    let rt = Runtime::new().expect("runtime");
    let manager = SessionManager::new(SessionRuntimeConfig {
        keep_alive: Duration::from_secs(3600),
        ..SessionRuntimeConfig::default()
    });
    let id = open_mock(&rt, &manager);
    let script = "SELECT 1;\nSELECT 2;\nUPDATE t SET x = 1 WISP_SYNTAX_ERROR;\nSELECT 4;\nSELECT 5;";
    let statements: Vec<_> = split_statements(script, SplitFlavor::Standard)
        .into_iter()
        .map(|r| ScriptStatement {
            byte_start: r.start,
            sql: script[r].to_string(),
        })
        .collect();
    rt.block_on(async {
        let report = SessionManager::run_script(
            &manager,
            id,
            statements,
            true,
            CancellationToken::new(),
        )
        .await
        .expect("run");
        assert_eq!(report.outcomes.len(), 3);
        assert!(matches!(
            report.outcomes[2],
            StatementRunOutcome::Failed { stmt_index: 2, .. }
        ));
        SessionManager::shutdown_all(&manager).await;
    });
}

#[test]
fn cancel_token_stops_script_before_next_statement() {
    let rt = Runtime::new().expect("runtime");
    let manager = SessionManager::new(SessionRuntimeConfig {
        keep_alive: Duration::from_secs(3600),
        ..SessionRuntimeConfig::default()
    });
    let id = open_mock(&rt, &manager);
    let cancel = CancellationToken::new();
    cancel.cancel();
    rt.block_on(async {
        let report = SessionManager::run_script(
            &manager,
            id,
            vec![
                ScriptStatement {
                    byte_start: 0,
                    sql: "SELECT 1".into(),
                },
                ScriptStatement {
                    byte_start: 10,
                    sql: "SELECT 2".into(),
                },
            ],
            true,
            cancel,
        )
        .await
        .expect("run");
        assert!(report.cancelled);
        assert!(report.outcomes.is_empty());
        SessionManager::shutdown_all(&manager).await;
    });
}

#[test]
fn safe_mode_requires_write_approval() {
    let rt = Runtime::new().expect("runtime");
    let manager = SessionManager::new(SessionRuntimeConfig {
        keep_alive: Duration::from_secs(3600),
        ..SessionRuntimeConfig::default()
    });
    let mut profile = mock_profile();
    profile.safe_mode = true;
    let id = profile.id;
    let spec = SessionOpenSpec {
        profile,
        secrets: Default::default(),
    };
    rt.block_on(async {
        SessionManager::open(&manager, spec, tokio::runtime::Handle::current())
            .await
            .expect("open");
        let err = SessionManager::run_script(
            &manager,
            id,
            vec![ScriptStatement {
                byte_start: 0,
                sql: "INSERT INTO t VALUES (1)".into(),
            }],
            false,
            CancellationToken::new(),
        )
        .await
        .expect_err("needs confirm");
        assert!(matches!(
            err,
            SessionError::WriteNeedsConfirmation { .. }
        ));
        SessionManager::shutdown_all(&manager).await;
    });
}
