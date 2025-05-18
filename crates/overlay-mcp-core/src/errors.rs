use axum::{body::Body, response::IntoResponse};
use http::{Response, StatusCode};
use jsonptr::PointerBuf;
use oauth2::ConfigurationError;

use crate::auth::JwtContextPointerType;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    // Normal Http Errors
    #[error("Bad Request: {0}")]
    BadRequest(#[from] Error400),

    #[error("Unauthorized: {0}")]
    Unauthorized(#[from] Error401),

    #[error("Forbidden: {0}")]
    Forbidden(#[from] Error403),

    #[error("Not Found: {0}")]
    NotFound(#[from] Error404),

    #[error("Service Unavailable: {0}")]
    ServiceUnavailable(#[from] Error503),

    // Special Errors, expected 500
    #[error("Jwt claim type error: claim `{path}` expected type `{expected_type}`, but got actual type `{actual_type}`")]
    JwtClaimTypeError {
        path: PointerBuf,
        expected_type: &'static str,
        actual_type: String,
    },

    #[error("No Token Endpoint")]
    NoTokenEndpoint,

    #[error("Session already started: {0}")]
    AlreadyStartedSession(String),

    #[error("Session already stopped: {0}")]
    AlreadyStoppedSession(String),

    #[error("Session already closed: {0}")]
    AlreadyClosedSession(String),

    #[error("Fatal error: {0}")]
    Fatal(#[from] FatalError),

    #[error("Openfga error: {0}")]
    OpenfgaError(#[from] openfga::Error),

    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("Json error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Jwt error: {0}")]
    JwtError(#[from] jsonwebtoken::errors::Error),

    #[error("Url error: {0}")]
    UrlError(#[from] url::ParseError),

    #[error("Discovery error: {0}")]
    DiscoveryError(#[from] openidconnect::DiscoveryError<oauth2::HttpClientError<reqwest::Error>>),

    #[error("SSE transport error: {0}")]
    SseTransportError(#[from] rmcp::transport::sse::SseTransportError),

    #[error("Hickory resolver error: {0}")]
    HickoryResolverError(#[from] hickory_resolver::ResolveError),

    #[error("Configuration error: {0}")]
    ConfigurationError(#[from] ConfigurationError),

    #[error("Hiqlite error: {0}")]
    HiqliteError(#[from] hiqlite::Error),

    #[error("Tokio broadcast error")]
    TokioBroadcastError,
}

#[derive(Debug, thiserror::Error)]
pub enum Error400 {
    #[error("Invalid request")]
    InvalidUrl(#[from] url::ParseError),

    #[error("Invalid header string data {0}")]
    InvalidHeaderString(http::header::HeaderName),

    #[error("'Bearer' type expected, but got {0}")]
    BearerTokenExpected(String),

    #[error("Invalid token: {0}")]
    InvalidToken(&'static str),
}

#[derive(Debug, thiserror::Error)]
pub enum Error401 {
    #[error("Authentication failed")]
    AuthenticationFailed,
}

#[derive(Debug, thiserror::Error)]
pub enum Error403 {
    #[error("Authorization failed")]
    AuthorizationFailed,
}

#[derive(Debug, thiserror::Error)]
pub enum Error404 {
    #[error("Session not found(session_id={session_id})")]
    SessionNotFound { session_id: String },
}

#[derive(Debug, thiserror::Error)]
pub enum Error503 {
    #[error("No upstream mcp server found")]
    NoUpstreamMcpServer,
}

#[derive(Debug, thiserror::Error)]
pub enum FatalError {
    #[error("Timeout")]
    Timeout,

    #[error("Token request error")]
    TokenRequestError,

    #[error("Claim path not found")]
    ClaimPathNotFound(PointerBuf),

    #[error("Unexpected claim type: {}", 0.to_string())]
    UnexpectedClaimType(JwtContextPointerType),

    #[error("Raft config error: {0}")]
    RaftUnresolvedNodeId(&'static str),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response<Body> {
        match self {
            Self::BadRequest(e) => Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(e.to_string()))
                .unwrap(),
            Self::Unauthorized(e) => Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Body::from(e.to_string()))
                .unwrap(),
            Self::Forbidden(e) => Response::builder()
                .status(StatusCode::FORBIDDEN)
                .body(Body::from(e.to_string()))
                .unwrap(),
            Self::Fatal(e) => Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(e.to_string()))
                .unwrap(),
            _ => Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap(),
        }
    }
}
