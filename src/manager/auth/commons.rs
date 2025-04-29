use axum::{body::Body, response::Response};
use http::StatusCode;
use json_patch::jsonptr::PointerBuf;
use jsonwebtoken::TokenData;
use serde::{Deserialize, Serialize};

use crate::reqmodifier::reference::HttpPartReference;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum JwtContextPointerType {
    #[serde(rename = "string")]
    String,
    #[serde(rename = "string[]")]
    StringArray,
}

#[derive(Debug, Clone)]
pub enum Authentication {
    Both {
        apikey: String,
        apikey_from: HttpPartReference,
        jwt: TokenData<serde_json::Value>,
    },
    ApiKey {
        apikey: String,
        apikey_from: HttpPartReference,
    },
    Jwt {
        jwt: TokenData<serde_json::Value>,
    },
    NoAuth,
}

pub enum AuthorizationResult {
    Allow,
    Deny,
    Unauthorized,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JwtContext {
    pub relation: String,
    pub group: String,
    pub path: PointerBuf,
    pub r#type: JwtContextPointerType,
}

impl AuthorizationResult {
    pub fn to_err_response(&self) -> Result<(), Response<Body>> {
        match self {
            AuthorizationResult::Allow => Ok(()),
            AuthorizationResult::Deny => Err(Response::builder()
                .status(StatusCode::FORBIDDEN)
                .body(Body::empty())
                .unwrap()),
            AuthorizationResult::Unauthorized => Err(Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Body::empty())
                .unwrap()),
        }
    }
    pub fn to_is_allowed(&self) -> Result<bool, Response<Body>> {
        match self {
            AuthorizationResult::Allow => Ok(true),
            AuthorizationResult::Deny => Ok(false),
            AuthorizationResult::Unauthorized => Err(Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Body::empty())
                .unwrap()),
        }
    }
}

impl Authentication {
    pub fn apikey(&self) -> Option<(String, HttpPartReference)> {
        match self {
            Authentication::Both {
                apikey,
                apikey_from,
                ..
            }
            | Authentication::ApiKey {
                apikey,
                apikey_from,
                ..
            } => Some((apikey.clone(), apikey_from.clone())),
            _ => None,
        }
    }
}
