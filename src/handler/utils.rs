use axum::response::{IntoResponse, Response};
use http::StatusCode;

pub type AnyResult<T> = Result<T, AnyError>;
pub struct AnyError(anyhow::Error);

impl IntoResponse for AnyError {
    fn into_response(self) -> Response<axum::body::Body> {
        (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()).into_response()
    }
}

impl<T> From<T> for AnyError
where
    T: Into<anyhow::Error>,
{
    fn from(error: T) -> Self {
        AnyError(error.into())
    }
}
