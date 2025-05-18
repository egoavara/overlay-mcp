use serde::{Deserialize, Serialize};
use serde_with::{formats::PreferOne, serde_as, OneOrMany};
use url::Url;

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum UpstreamConfig {
    Static(StaticUpstream),
    Discovery(DiscoveryUpstream),
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StaticUpstream {
    #[serde_as(as = "OneOrMany<_, PreferOne>")]
    pub urls: Vec<Url>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DiscoveryUpstream {
    pub discovery: Url,
} 