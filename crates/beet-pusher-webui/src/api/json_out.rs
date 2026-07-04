// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
use axum::Json;
use utoipa::ToSchema;

/// Encapsulation of the final json output format
pub struct JsonOut<T>(Json<ResponseOut<T>>);
impl<T> JsonOut<T> {
    /// All went well, and (usually) some data was returned.
    pub fn success(data: T) -> Self {
        Self(Json(ResponseOut::Success { data }))
    }
    /// An error occurred in processing the request (data submitted,
    /// pre-condition, or other exceptional error case)
    pub fn fail(error: T) -> Self {
        Self(Json(ResponseOut::Fail { error }))
    }
}

#[derive(serde::Serialize, ToSchema)]
#[serde(tag = "status")]
#[serde(rename_all = "snake_case")]
pub enum ResponseOut<T> {
    Success { data: T },
    Fail { error: T },
}

impl<T> axum::response::IntoResponse for JsonOut<T>
where
    T: serde::Serialize,
{
    fn into_response(self) -> axum::response::Response {
        let Self(inner) = self;
        inner.into_response()
    }
}
