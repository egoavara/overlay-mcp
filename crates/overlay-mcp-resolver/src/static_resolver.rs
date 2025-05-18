use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use axum::http;
use overlay_mcp_core::{upstream::StaticUpstream, Error, Error503, GeneralResolver};
use tokio::sync::RwLock;
use url::Url;

#[derive(Clone)]
pub struct StaticResolver(pub(crate) Arc<RwLock<InnerStaticResolver>>);

impl StaticResolver {
    pub fn new(config: &StaticUpstream) -> Self {
        Self(Arc::new(RwLock::new(InnerStaticResolver {
            counter: AtomicUsize::new(0),
            urls: config.urls.clone(),
        })))
    }
}

pub struct InnerStaticResolver {
    counter: AtomicUsize,
    urls: Vec<Url>,
}

impl GeneralResolver for StaticResolver {
    async fn resolve(&self, _target: &http::request::Parts) -> Result<Url, Error> {
        let resolver = self.0.read().await;
        if resolver.urls.is_empty() {
            return Err(Error::ServiceUnavailable(Error503::NoUpstreamMcpServer));
        }
        let index = resolver.counter.fetch_add(1, Ordering::SeqCst) % resolver.urls.len();
        Ok(resolver.urls[index].clone())
    }
}
