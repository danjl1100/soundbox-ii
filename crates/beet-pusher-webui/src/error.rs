// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use std::collections::HashMap;

use axum::{http::StatusCode, response::IntoResponse};
use utoipa::ToSchema;

use crate::{api::JsonOut, domain::services::node_service::CreateBucketError};

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Validation(String),
    #[error("validation failed")]
    ValidationFields(HashMap<String, Vec<String>>),
    #[error("authentication required")]
    Unauthorized,
    #[error("insufficient permissions")]
    Forbidden,
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Internal(#[from] eyre::Error),
}

#[derive(serde::Serialize, ToSchema)]
pub struct ErrorOut {
    r#type: &'static str,
    message: String,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    fields: Option<serde_json::Value>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let err = |r#type: &'static str, message: String| ErrorOut {
            r#type,
            message,
            fields: None,
        };

        let (status, err_value) = match self {
            AppError::Validation(message) => {
                (StatusCode::BAD_REQUEST, err("validation_error", message))
            }
            AppError::ValidationFields(fields) => {
                let mut err_map = err("validation_error", "request validation failed".to_string());
                err_map.fields = Some(serde_json::json!(fields));
                (StatusCode::BAD_REQUEST, err_map)
            }
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                err("unauthorized", self.to_string()),
            ),
            AppError::Forbidden => (StatusCode::FORBIDDEN, err("forbidden", self.to_string())),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, err("conflict", msg)),
            AppError::Internal(error) => {
                // Log the full error chain for debugging.
                // This is the only place where the real error details are visible
                tracing::error!(?error, "internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    err("internal_error", "an internal error occurred".to_string()),
                )
            }
        };

        (status, JsonOut::fail(err_value)).into_response()
    }
}

impl From<CreateBucketError> for AppError {
    fn from(value: CreateBucketError) -> Self {
        Self::Internal(value.into())
    }
}
