use axum::{
    body::Body,
    debug_handler,
    extract::{FromRequestParts, Query, Request, State},
    response::{sse::Event, Response, Sse},
    Extension,
};
use futures_util::Stream;
use http::StatusCode;

use crate::{
    authorizer::CheckAuthorizer,
    handler::AppState,
    manager::Manager,
    middleware::{HeaderMCPProtocolVersion, MCPProtocolVersion, MCPSessionId},
    utils::AnyError,
};

use super::handle_http_sse;

#[debug_handler]
pub async fn handle_get(
    State(state): State<AppState>,
    HeaderMCPProtocolVersion(version): HeaderMCPProtocolVersion,
    session_manager: Extension<Manager>,
    req: Request<Body>,
) -> Result<Sse<impl Stream<Item = Result<Event, anyhow::Error>>>, AnyError> {
    match version {
        // unspecified or v20241105 are treated the same way
        MCPProtocolVersion::Unspecified | MCPProtocolVersion::V20241105 => {
            let (mut parts, body) = req.into_parts();
            let check = CheckAuthorizer::from_request_parts(&mut parts, &state)
                .await
                .unwrap();
            handle_http_sse::handler_downstream(
                State(state),
                check,
                session_manager,
                Request::from_parts(parts, body),
            )
            .await
        }
        MCPProtocolVersion::V20250326 => Err(Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(Body::empty())
            .unwrap()
            .into()),
        MCPProtocolVersion::Unknown(version) => {
            tracing::error!(
                mcp_protocol_version = version,
                "unknown MCP protocol version"
            );
            Err(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(format!(
                    "unknown MCP protocol version {:?}",
                    version
                )))
                .unwrap()
                .into())
        }
    }
}

pub async fn handle_post(
    State(state): State<AppState>,
    HeaderMCPProtocolVersion(version): HeaderMCPProtocolVersion,
    session_id: MCPSessionId,
    session_manager: Extension<Manager>,
    req: Request<Body>,
) -> Result<Response<Body>, AnyError> {
    match version {
        // unspecified or v20241105 are treated the same way
        MCPProtocolVersion::Unspecified | MCPProtocolVersion::V20241105 => {
            let (mut parts, body) = req.into_parts();
            let check = CheckAuthorizer::from_request_parts(&mut parts, &state)
                .await
                .unwrap();
            handle_http_sse::handler_upstream(
                State(state),
                session_id,
                check,
                session_manager,
                Request::from_parts(parts, body),
            )
            .await
        }
        MCPProtocolVersion::V20250326 => {
            let (mut parts, body) = req.into_parts();
            let query =
                Query::<handle_http_sse::UpstreamQuery>::from_request_parts(&mut parts, &state)
                    .await
                    .unwrap();
            let check = CheckAuthorizer::from_request_parts(&mut parts, &state)
                .await
                .unwrap();
            handle_http_sse::handler_upstream(
                State(state),
                session_id,
                check,
                session_manager,
                Request::from_parts(parts, body),
            )
            .await
        }
        MCPProtocolVersion::Unknown(version) => {
            tracing::error!(
                mcp_protocol_version = version,
                "unknown MCP protocol version"
            );
            Err(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(format!(
                    "unknown MCP protocol version {:?}",
                    version
                )))
                .unwrap()
                .into())
        }
    }
}
