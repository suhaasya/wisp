use std::time::Duration;

/// Per-session runtime options (LUM-020).
#[derive(Debug, Clone)]
pub struct SessionRuntimeConfig {
    pub keep_alive: Duration,
    pub reconnect_max_backoff: Duration,
}

impl Default for SessionRuntimeConfig {
    fn default() -> Self {
        Self {
            keep_alive: Duration::from_secs(60),
            reconnect_max_backoff: Duration::from_secs(30),
        }
    }
}
