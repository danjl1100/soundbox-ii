use axum::{extract::State, http::StatusCode};

use crate::{
    api::{
        JsonOut,
        extractors::ValidatedJson,
        handlers::dtos::{CreateBucketDto, CreateBucketResponse},
    },
    error::AppResult,
    state::AppState,
};

pub async fn health_check() -> StatusCode {
    StatusCode::OK
}

mod dtos {
    pub(super) use crate::domain::models::NodePath;
    use validator::Validate;

    #[derive(serde::Deserialize, Validate)]
    pub struct CreateBucketDto {
        parent: NodePath,
    }

    #[derive(serde::Serialize)]
    pub struct CreateBucketResponse {
        pub path: NodePath,
    }
}
#[axum::debug_handler]
pub async fn node_create_bucket(
    state: State<AppState>,
    ValidatedJson(payload): ValidatedJson<CreateBucketDto>,
) -> AppResult<(StatusCode, JsonOut<CreateBucketResponse>)> {
    let path = ".0".parse().expect("valid constant"); // TODO
    Ok((
        StatusCode::OK,
        JsonOut::success(CreateBucketResponse { path }),
    ))
}
