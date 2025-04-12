use clap::Parser;
use redact::serde::redact_secret;
use redact::Secret;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::default::Default;
use std::net::{IpAddr, SocketAddr};
use url::Url;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub server: ServerConfig,
    pub oidc: OpenIDConnectConfig,
    pub storage: Option<Authorization>,
    pub otel: Option<OpenTelemetryConfig>,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Authorization {
    Static(StaticAuthorization),
    Fga(FgaAuthorization),
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct StaticAuthorization {
    #[serde(default)]
    pub ip_whitelist: Vec<IpAddr>,
    #[serde(default)]
    pub ip_blacklist: Vec<IpAddr>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FgaAuthorization {
    pub uri: Url,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

#[derive(Parser, Debug, Clone, Deserialize, Serialize)]
pub struct OpenIDConnectConfig {
    pub issuer: String,
    pub client_id: String,
    #[serde(serialize_with = "redact::serde::redact_secret")]
    pub client_secret: Secret<String>,
    pub scopes: Vec<String>,
}
