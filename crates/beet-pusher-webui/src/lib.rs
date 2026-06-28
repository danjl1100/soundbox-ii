// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
#![expect(missing_docs, reason = "TODO while building")]

use crate::{config::Config, domain::services::ports::BeetPusherPipe, state::AppState};
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
pub async fn create_app(config: Config, pipe: impl BeetPusherPipe) -> axum::Router {
    let state = AppState::new(config, pipe);
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
        fmt::layer().json().with_writer(std::io::stderr).boxed()
    } else {
        fmt::layer()
            // multi-line [`tracing_subscriber::Pretty`] is too verbose...
            // .pretty()
            .with_writer(std::io::stderr)
            .boxed()
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();
}
