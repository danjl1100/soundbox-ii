use axum::{Router, routing::get};

use crate::{api::handlers::list_nouns, state::AppState};

pub fn router(state: AppState) -> Router {
    Router::new()
        .nest("/api/v1", api_routes())
        .with_state(state)
}

fn api_routes() -> Router<AppState> {
    Router::new().nest("/nouns", noun_routes())
}

fn noun_routes() -> Router<AppState> {
    Router::new().route("/", get(list_nouns))
}
