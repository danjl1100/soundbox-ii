#![expect(missing_docs, reason = "TODO while building")]

use crate::{config::Config, state::AppState};
use tracing_subscriber::{
    EnvFilter, Layer as _, fmt, layer::SubscriberExt, util::SubscriberInitExt,
};

pub mod api;
pub mod config;
pub mod domain;
pub mod error;
pub mod infra;
pub mod routes;
pub mod state;

#[expect(clippy::unused_async, reason = "future capability may require async")]
pub async fn create_app(config: Config) -> axum::Router {
    let state = AppState::new(config);
    routes::router(state)
}

pub fn init_tracing(log_level: &str) {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    // Use runtime configuration rather than build profile to control
    // log format. A release build in staging should still be readable,
    // and a debug build against production data should still emit JSON.
    let json_output = std::env::var("LOG_JSON").is_ok_and(|v| v == "true" || v == "1");

    let fmt_layer = if json_output {
        fmt::layer().json().boxed()
    } else {
        fmt::layer().pretty().boxed()
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();
}
