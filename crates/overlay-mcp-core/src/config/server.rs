use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use url::Url;

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
    pub cluster: hiqlite::RaftConfig, // Consider moving hiqlite specific parts to overlay-mcp-raft crate later
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
