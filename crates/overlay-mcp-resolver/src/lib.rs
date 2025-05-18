mod discovery_resolver;
mod static_resolver;

use axum::http;
pub use discovery_resolver::*;
use overlay_mcp_core::{Config, Error, GeneralResolver, UpstreamConfig};
pub use static_resolver::*;

use tokio_util::sync::CancellationToken;
use url::Url;

#[derive(Clone)]
pub enum Resolver {
    Static(StaticResolver),
    Discovery(DiscoveryResolver),
}

impl Resolver {
    pub fn new(ct: CancellationToken, config: &Config) -> Result<Self, Error> {
        match &config.upstream {
            UpstreamConfig::Static(static_upstream) => {
                Ok(Self::Static(StaticResolver::new(static_upstream)))
            }
            UpstreamConfig::Discovery(headless_discovery_upstream) => {
                DiscoveryResolver::new(ct, headless_discovery_upstream).map(Self::Discovery)
            }
        }
    }
}

impl GeneralResolver for Resolver {
    async fn resolve(&self, target: &http::request::Parts) -> Result<Url, Error> {
        match self {
            Self::Static(resolver) => resolver.resolve(target).await,
            Self::Discovery(resolver) => resolver.resolve(target).await,
        }
    }
}
