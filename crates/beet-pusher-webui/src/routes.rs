use axum::{
    Router,
    routing::{get, post},
};

use crate::{
    api::handlers::{health_check, node_create_bucket},
    state::AppState,
};

pub fn router(state: AppState) -> Router {
    Router::new()
        .nest("/api/v1", api_routes())
        .route("/health", get(health_check))
        .with_state(state)
}

fn api_routes() -> Router<AppState> {
    Router::new().nest("/nodes", node_routes())
}

fn node_routes() -> Router<AppState> {
    Router::new().route("/create-bucket", post(node_create_bucket))
}
