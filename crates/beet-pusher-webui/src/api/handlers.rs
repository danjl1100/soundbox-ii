// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use std::sync::Arc;

use axum::{extract::State, http::StatusCode};

use crate::{
    api::{
        JsonOut,
        extractors::ValidatedJson,
        handlers::dtos::{CreateBucketDto, CreateBucketResponse},
    },
    domain::services::{node_service::NodeService, ports::BeetPusherPipe},
    error::AppResult,
};

pub async fn health_check() -> StatusCode {
    StatusCode::OK
}

mod dtos {
    use crate::domain::models::NodePath;
    use validator::Validate;

    #[derive(serde::Deserialize, Validate)]
    pub struct CreateBucketDto {
        pub parent: NodePath,
    }

    #[derive(serde::Serialize)]
    pub struct CreateBucketResponse {
        pub path: NodePath,
    }
}
pub(crate) async fn node_create_bucket<T: BeetPusherPipe>(
    State(nodes): State<Arc<NodeService<T>>>,
    ValidatedJson(payload): ValidatedJson<CreateBucketDto>,
) -> AppResult<(StatusCode, JsonOut<CreateBucketResponse>)> {
    let CreateBucketDto { parent } = payload;

    let path = nodes.create_bucket(parent).await?;

    Ok((
        StatusCode::OK,
        JsonOut::success(CreateBucketResponse { path }),
    ))
}
