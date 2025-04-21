use std::{convert::Infallible, str::FromStr};

use axum::{
    body::Body,
    extract::{FromRequestParts, OptionalFromRequestParts},
    response::Response,
};
use http::{request::Parts, StatusCode};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum MCPProtocolVersion {
    #[serde(rename = "2024-11-05")]
    V20241105,

    #[serde(rename = "2025-03-26")]
    V20250326,

    #[serde(skip)]
    Unknown(String),

    #[serde(skip)]
    Unspecified,
}

impl FromStr for MCPProtocolVersion {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "2024-11-05" => Ok(Self::V20241105),
            "2025-03-26" => Ok(Self::V20250326),
            _ => Ok(Self::Unknown(s.to_string())),
        }
    }
}

impl MCPProtocolVersion {
    pub fn as_header_value(&self) -> Option<&'static str> {
        match self {
            Self::V20241105 => Some("2024-11-05"),
            Self::V20250326 => Some("2025-03-26"),
            Self::Unknown(_) => None,
            Self::Unspecified => None,
        }
    }
}

pub struct HeaderMCPProtocolVersion(pub MCPProtocolVersion);

impl<S> FromRequestParts<S> for HeaderMCPProtocolVersion
where
    S: Send + Sync,
{
    type Rejection = Response<Body>;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let header = parts.headers.get("MCP-Protocol-Version");
        match header {
            Some(header) => match header.to_str() {
                Ok(s) => {
                    let mcp_protocol_version = MCPProtocolVersion::from_str(s).unwrap();
                    Ok(Self(mcp_protocol_version))
                }
                Err(err) => {
                    tracing::error!(error = ?err, "invalid MCP-Protocol-Version non-ascii header");
                    Err(Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(Body::from("invalid MCP-Protocol-Version non-ascii header"))
                        .unwrap())
                }
            },
            None => Ok(HeaderMCPProtocolVersion(MCPProtocolVersion::Unspecified)),
        }
    }
}
impl<S> OptionalFromRequestParts<S> for HeaderMCPProtocolVersion
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Option<Self>, Self::Rejection> {
        let header = parts.headers.get("MCP-Protocol-Version");
        match header {
            Some(header) => match header.to_str() {
                Ok(s) => {
                    let mcp_protocol_version = MCPProtocolVersion::from_str(s).unwrap();
                    Ok(Some(Self(mcp_protocol_version)))
                }
                Err(_) => Ok(None),
            },
            None => Ok(None),
        }
    }
}
