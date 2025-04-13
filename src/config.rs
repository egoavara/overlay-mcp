use axum_client_ip::ClientIpSource;
use clap::Parser;
use redact::Secret;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::default::Default;
use std::net::{IpAddr, SocketAddr};
use url::Url;

use crate::authorizer::{Authorizer, AuthorizerComponent};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub application: ApplicationConfig,
    pub server: ServerConfig,
    pub oidc: OpenIDConnectConfig,
    pub authorizer: Option<Authorizer>,
    pub otel: Option<OpenTelemetryConfig>,
}

#[derive(Parser, Debug, Clone, Deserialize, Serialize)]
pub struct ApplicationConfig {
    pub log_filter: Option<String>,
    pub ip_extract: Option<ClientIpSource>,
    pub prometheus: bool,
    pub health_check: bool,
}

#[derive(Parser, Debug, Clone, Deserialize, Serialize)]
pub struct OpenTelemetryConfig {
    pub endpoint: String,
}

#[derive(Parser, Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub addr: SocketAddr,
    pub hostname: Url,
    pub upstream: Url,
}

#[derive(Parser, Debug, Clone, Deserialize, Serialize)]
pub struct OpenIDConnectConfig {
    pub issuer: String,
    pub client_id: String,
    #[serde(serialize_with = "redact::serde::redact_secret")]
    pub client_secret: Secret<String>,
    pub scopes: Vec<String>,
}