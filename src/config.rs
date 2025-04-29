use axum_client_ip::ClientIpSource;
use serde::{Deserialize, Serialize};
use serde_with::{formats::PreferOne, serde_as, OneOrMany};
use std::net::SocketAddr;
use url::Url;

use crate::{manager::auth::AuthConfig, reqmodifier::BaseModifiers};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub application: ApplicationConfig,
    pub server: ServerConfig,
    pub upstream: UpstreamConfig,
    pub auth: AuthConfig,
    pub otel: Option<OpenTelemetryConfig>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApplicationConfig {
    pub log_filter: Option<String>,
    pub ip_extract: Option<ClientIpSource>,
    pub prometheus: bool,
    pub health_check: bool,
    pub passthrough: BaseModifiers,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum UpstreamConfig {
    Static(StaticUpstream),
    HeadlessDiscovery(HeadlessDiscoveryUpstream),
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StaticUpstream {
    #[serde_as(as = "OneOrMany<_, PreferOne>")]
    pub urls: Vec<Url>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HeadlessDiscoveryUpstream {
    pub discovery: Url,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenTelemetryConfig {
    pub endpoint: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub addr: SocketAddr,
    pub hostname: Url,
    #[serde(default)]
    pub cluster: ClusterConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ClusterConfig {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "raft")]
    Raft(Box<RaftConfig>),
    // TODO: Rdbms{}
}
impl Default for ClusterConfig {
    fn default() -> Self {
        Self::None {}
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RaftConfig {
    pub id: Option<u64>,
    pub index: Option<usize>,
    pub secret: String,
    #[serde(default)]
    pub cluster: hiqlite::RaftConfig,
    pub nodes: Vec<Node>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Node {
    pub id: u64,
    pub api: String,
    pub raft: String,
    #[serde(default = "default_read_pool_size")]
    pub read_pool_size: usize,
}

fn default_read_pool_size() -> usize {
    10
}
