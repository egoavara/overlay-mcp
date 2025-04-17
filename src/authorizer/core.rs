use std::net::IpAddr;

use http::{uri::PathAndQuery, HeaderMap, Method};
use serde_with::serde_as;

#[serde_as]
#[derive(Debug, Clone)]
pub struct AuthorizerRequest {
    pub ip: IpAddr,
    pub method: Method,
    pub path: PathAndQuery,
    #[allow(dead_code)]
    pub headers: HeaderMap,
    pub jwt: Option<serde_json::Value>,
    pub apikey: Option<String>,
}

#[derive(Debug, Clone)]
pub enum AuthorizerResponse {
    #[allow(dead_code)]
    Allow(AuthorizerResponseAllow),
    Deny(AuthorizerResponseDeny),
}

#[derive(Debug, Clone)]
pub struct AuthorizerResponseAllow {
    #[allow(dead_code)]
    pub authorizer: String,
    #[allow(dead_code)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AuthorizerResponseDeny {
    #[allow(dead_code)]
    pub authorizer: String,
    pub reason: Option<String>,
}

impl From<AuthorizerResponseAllow> for AuthorizerResponse {
    fn from(value: AuthorizerResponseAllow) -> Self {
        AuthorizerResponse::Allow(value)
    }
}

impl From<AuthorizerResponseDeny> for AuthorizerResponse {
    fn from(value: AuthorizerResponseDeny) -> Self {
        AuthorizerResponse::Deny(value)
    }
}

