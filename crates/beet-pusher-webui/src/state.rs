use std::sync::Arc;

use crate::config::Config;
use axum::extract::FromRef;

#[derive(Clone, FromRef)]
pub struct AppState {
    pub config: Arc<Config>,
    // TODO: beet_pusher_service (generic across where it puts the Json
    // messages, std pipes or in-memory if tests)
    // pub beet_pusher_pipe: BeetPusherPipe<T>,
}
impl AppState {
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
}
