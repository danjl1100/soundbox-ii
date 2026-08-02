// Copyright (C) 2021-2026  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details
//! Extract information from requests

use std::collections::HashMap;

use axum::{Json, extract::FromRequest};
use validator::Validate;

use crate::error::AppError;

/// Like [`Json`] except with [`Validate::validate`] run as well
pub struct ValidatedJson<T>(pub T);
impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: serde::de::DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(|e| AppError::Validation(format!("invalid JSON: {e}")))?;

        value
            .validate()
            .map_err(|e| AppError::ValidationFields(format_validation_errors(&e)))?;

        Ok(Self(value))
    }
}

fn format_validation_errors(errors: &validator::ValidationErrors) -> HashMap<String, Vec<String>> {
    errors
        .field_errors()
        .iter()
        .map(|(field, errs)| {
            let messages = errs
                .iter()
                .map(|e| {
                    e.message
                        .as_ref()
                        .map_or_else(|| format!("{field} is invalid"), ToString::to_string)
                })
                .collect();
            (field.to_string(), messages)
        })
        .collect()
}
