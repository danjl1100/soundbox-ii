use axum::Json;

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

#[derive(serde::Serialize)]
#[serde(tag = "status")]
#[serde(rename_all = "snake_case")]
enum ResponseOut<T> {
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
