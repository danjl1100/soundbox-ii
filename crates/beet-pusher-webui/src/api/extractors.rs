use std::collections::HashMap;

use axum::{Json, extract::FromRequest};
use validator::Validate;

use crate::error::AppError;

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
                        .map(|m| m.to_string())
                        .unwrap_or_else(|| format!("{field} is invalid"))
                })
                .collect();
            (field.to_string(), messages)
        })
        .collect()
}
