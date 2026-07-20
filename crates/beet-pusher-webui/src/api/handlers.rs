// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use std::sync::Arc;

use axum::{extract::State, http::StatusCode};

use crate::{
    api::{
        JsonOut, ResponseOut,
        extractors::ValidatedJson,
        handlers::dtos::{CreateBucketDto, CreateBucketResponse},
    },
    domain::services::{node_service::NodeService, ports::BeetPusherPipe},
    error::{AppError, AppResult, ErrorOut},
};

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(node_create_bucket),
    components(schemas(CreateBucketDto)),
    tags((name = "nodes", description = "Nodes in the bucket spigot graph"))
)]
pub(crate) struct ApiDoc;

pub async fn health_check() -> StatusCode {
    StatusCode::OK
}

mod dtos {
    use utoipa::ToSchema;
    use validator::Validate;

    #[derive(serde::Deserialize, Validate, ToSchema)]
    pub struct CreateBucketDto {
        pub parent: String,
    }

    #[derive(serde::Serialize, ToSchema)]
    pub struct CreateBucketResponse {
        pub path: String,
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/nodes/create-bucket",
    request_body = CreateBucketDto,
    responses(
        (status = 201, description = "Node created successfully", body = CreateBucketResponse),
        (status = 400, description = "Validation error", body = ResponseOut<ErrorOut>),
    ),
    tag = "node"
)]
pub(crate) async fn node_create_bucket<T: BeetPusherPipe>(
    State(nodes): State<Arc<NodeService<T>>>,
    ValidatedJson(payload): ValidatedJson<CreateBucketDto>,
) -> AppResult<(StatusCode, JsonOut<CreateBucketResponse>)> {
    let CreateBucketDto { parent } = payload;

    let parent = parent
        .parse()
        .map_err(|e| AppError::Validation(format!("invalid node path: {e}")))?;

    let path = nodes.create_bucket(parent).await?;

    Ok((
        StatusCode::CREATED,
        JsonOut::success(CreateBucketResponse {
            path: path.to_string(),
        }),
    ))
}
