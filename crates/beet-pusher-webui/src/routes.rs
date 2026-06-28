// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use axum::{
    Router,
    routing::{get, post},
};

use crate::{
    api::handlers::{health_check, node_create_bucket},
    domain::services::ports::BeetPusherPipe,
    state::AppState,
};

pub fn router<T: BeetPusherPipe>(state: AppState<T>) -> Router {
    Router::new()
        .nest("/api/v1", api_routes())
        .route("/health", get(health_check))
        .with_state(state)
}

fn api_routes<T: BeetPusherPipe>() -> Router<AppState<T>> {
    Router::new().nest("/nodes", node_routes())
}

fn node_routes<T: BeetPusherPipe>() -> Router<AppState<T>> {
    Router::new().route("/create-bucket", post(node_create_bucket))
}
