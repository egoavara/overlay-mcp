use axum::{
    body::Body,
    response::{IntoResponse, Response},
};
use http::StatusCode;

pub enum AnyError {
    Anyhow(anyhow::Error),
    Response(Response<Body>),
}

impl From<anyhow::Error> for AnyError {
    fn from(error: anyhow::Error) -> Self {
        AnyError::Anyhow(error)
    }
}

impl From<Response<Body>> for AnyError {
    fn from(response: Response<Body>) -> Self {
        AnyError::Response(response)
    }
}

impl IntoResponse for AnyError {
    fn into_response(self) -> Response {
        match self {
            AnyError::Anyhow(error) => {
                tracing::error!(error = ?error);
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::empty())
                    .unwrap()
            }
            AnyError::Response(response) => response,
        }
    }
}
