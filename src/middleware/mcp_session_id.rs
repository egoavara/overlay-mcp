use axum::{
    body::Body,
    extract::{FromRequestParts, OptionalFromRequestParts},
    response::Response,
};
use http::{request::Parts, StatusCode};

#[allow(dead_code)]
pub struct MCPSessionId(pub String);

impl MCPSessionId {
    fn extract_from_header(header: &Parts) -> Result<Option<Self>, Response<Body>> {
        let header = header.headers.get("MCP-Session-Id");
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
    fn extract_from_query(parts: &Parts) -> Result<Option<Self>, Response<Body>> {
        let Some(query) = parts.uri.query() else {
            return Ok(None);
        };
        let Some((_, session_id)) =
            form_urlencoded::parse(query.as_bytes()).find(|(key, _)| key == "session_id")
        else {
            return Ok(None);
        };
        Ok(Some(Self(session_id.to_string())))
    }
}

impl<S> FromRequestParts<S> for MCPSessionId
where
    S: Send + Sync,
{
    type Rejection = Response<Body>;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        <Self as OptionalFromRequestParts<S>>::from_request_parts(parts, state)
            .await?
            .ok_or_else(|| {
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::from("MCP-Session-Id is required"))
                    .unwrap()
            })
    }
}

impl<S> OptionalFromRequestParts<S> for MCPSessionId
where
    S: Send + Sync,
{
    type Rejection = Response<Body>;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Option<Self>, Self::Rejection> {
        let value_from_header = Self::extract_from_header(parts)?;
        if let Some(value) = value_from_header {
            return Ok(Some(value));
        }
        let value_from_query = Self::extract_from_query(parts)?;
        if let Some(value) = value_from_query {
            return Ok(Some(value));
        }
        Ok(None)
    }
}
