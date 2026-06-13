#![expect(missing_docs, reason = "TODO while building")]

use crate::{config::Config, state::AppState};

pub mod api;
pub mod config;
pub mod error;
pub mod routes;
pub mod state;

pub async fn create_app(config: Config) -> axum::Router {
    let state = AppState::new(config);
    let router = routes::router(state);
    router
}
