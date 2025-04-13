use std::{net::IpAddr, str::FromStr};

use futures_util::Stream;
use http::{uri::PathAndQuery, HeaderMap, HeaderName, HeaderValue, Method};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
fn x(){
    
}

pub trait AuthorizerComponent {
    fn whitelist<'a>(&self, request: &'a AuthorizerRequest) -> impl Stream<Item = AuthorizerResponseAllow>;
    fn blacklist<'a>(&self, request: &'a AuthorizerRequest) -> impl Stream<Item = AuthorizerResponseDeny>;
}

#[serde_as]
#[derive(Debug, Clone)]
pub struct AuthorizerRequest {
    pub ip: IpAddr,
    pub method: Method,
    pub path: PathAndQuery,
    pub headers: HeaderMap,
    pub jwt: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub enum AuthorizerResponse {
    Allow(AuthorizerResponseAllow),
    Deny(AuthorizerResponseDeny),
}

#[derive(Debug, Clone)]
pub struct AuthorizerResponseAllow {
    pub authorizer: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AuthorizerResponseDeny {
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

