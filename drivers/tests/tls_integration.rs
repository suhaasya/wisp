//! TLS mode integration against Docker Postgres/MySQL with self-signed certs.
//!
//! Run locally: `WISP_TLS_IT=1 cargo test -p wisp-drivers --test tls_integration`
//! (requires `docker compose -f drivers/docker/tls-compose.yml up` — fixtures TBD).

#[test]
fn tls_integration_placeholder() {
    if std::env::var("WISP_TLS_IT").ok().as_deref() != Some("1") {
        return;
    }
    panic!("TLS docker fixtures not wired yet — add certs + compose for LUM-018 follow-up");
}
