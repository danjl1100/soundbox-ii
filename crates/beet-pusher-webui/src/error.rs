// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use std::collections::HashMap;

use axum::{http::StatusCode, response::IntoResponse};

use crate::{api::JsonOut, domain::services::node_service::CreateBucketError};

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    // #[error("resource not found")]
    // NotFound,
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

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let err = |error_type: &'static str, message: String| {
            let mut obj = serde_json::Map::new();
            obj.insert("type".to_string(), error_type.into());
            obj.insert("message".to_string(), message.into());
            obj
        };

        let (status, err_value) = match self {
            // AppError::NotFound => (
            //     StatusCode::NOT_FOUND,
            //     err("not_found", self.to_string()),
            // ),
            AppError::Validation(message) => {
                (StatusCode::BAD_REQUEST, err("validation_error", message))
            }
            AppError::ValidationFields(fields) => {
                let mut err_map = err("validation_error", "request validation failed".to_string());
                err_map.insert("fields".to_string(), serde_json::json!(fields));
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
