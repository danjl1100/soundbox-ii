use std::collections::HashMap;

use axum::{Json, http::StatusCode, response::IntoResponse};

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("resource not found")]
    NotFound,
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
        let (status, error_type, message, fields) = match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "not_found", self.to_string(), None),
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, "validation_error", msg, None),
            AppError::ValidationFields(fields) => (
                StatusCode::BAD_REQUEST,
                "validation_error",
                "request validation failed".to_string(),
                Some(fields),
            ),
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                self.to_string(),
                None,
            ),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "forbidden", self.to_string(), None),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "conflict", msg, None),
            AppError::Internal(err) => {
                // Log the full error chain for debugging.
                // This is the only place where the real error details are visible
                tracing::error!(error = ?err, "internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "an internal error occurred".to_string(),
                    None,
                )
            }
        };

        let body = if let Some(fields) = fields {
            serde_json::json!({
                "todo": "error JSON structure", // TODO
                "fields": fields,
                // "error": {
                //     "type": error_type,
                //     "message": message,
                //     "fields": fields,
                // }
            })
        } else {
            serde_json::json!({
                "todo": "error JSON structure" // TODO
                // "error": {
                //     "type": error_type,
                //     "message": message,
                // }
            })
        };

        (status, Json(body)).into_response()
    }
}
