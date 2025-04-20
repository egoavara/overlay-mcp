
use axum::{
    body::Body,
    extract::{FromRequestParts, OptionalFromRequestParts},
    response::Response,
};
use http::{request::Parts, StatusCode};

pub struct HeaderMCPSessionId(pub String);

impl<S> FromRequestParts<S> for HeaderMCPSessionId
where
    S: Send + Sync,
{
    type Rejection = Response<Body>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // OptionalFromRequestParts 트레잇의 from_request_parts를 명시적으로 호출합니다.
        let value: Option<Self> =
            <HeaderMCPSessionId as OptionalFromRequestParts<S>>::from_request_parts(parts, state)
                .await?;
        match value {
            Some(value) => Ok(value),
            None => {
                tracing::error!("MCP-Session-Id header is required");
                Err(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from("MCP-Session-Id header is required"))
                    .unwrap())
            }
        }
    }
}
impl<S> OptionalFromRequestParts<S> for HeaderMCPSessionId
where
    S: Send + Sync,
{
    type Rejection = Response<Body>;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Option<Self>, Self::Rejection> {
        let header = parts.headers.get("MCP-Session-Id");
        match header {
            Some(header) => match header.to_str() {
                Ok(s) => Ok(Some(Self(s.to_string()))),
                Err(err) => {
                    tracing::error!(error = ?err, "invalid MCP-Session-Id non-ascii header");
                    Err(Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(Body::from("invalid MCP-Session-Id non-ascii header"))
                        .unwrap())
                }
            },
            None => Ok(None),
        }
    }
}
