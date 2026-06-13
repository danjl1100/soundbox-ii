use std::sync::Arc;

use crate::config::Config;
use axum::extract::FromRef;

#[derive(Clone, FromRef)]
pub struct AppState {
    pub config: Arc<Config>,
    // TODO: beet_pusher_service (generic across where it puts the Json
    // messages, std pipes or in-memory if tests)
}
impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
}
