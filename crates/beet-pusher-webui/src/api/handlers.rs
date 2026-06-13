use axum::{Json, extract::State, http::StatusCode};

use crate::{error::AppResult, state::AppState};

// TODO
#[derive(serde::Serialize)]
pub struct NounResponse {
    todo: &'static str,
}
// # Errors
// Never returns an error. Nouns are always and forever.
pub async fn list_nouns(state: State<AppState>) -> AppResult<(StatusCode, Json<NounResponse>)> {
    let todo = "nouns???";
    Ok((StatusCode::OK, Json(NounResponse { todo })))
}
