use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use axum::http;
use overlay_mcp_core::{
    upstream::DiscoveryUpstream,
    Error, Error503, GeneralResolver,
};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use url::Url;

#[derive(Clone)]
pub struct DiscoveryResolver(pub(crate) Arc<RwLock<InnerDiscoveryResolver>>);

impl DiscoveryResolver {
    pub fn new(cancel_token: CancellationToken, config: &DiscoveryUpstream) -> Result<Self, Error> {
        let resolver = hickory_resolver::Resolver::builder_tokio()?.build();
        let inner = Arc::new(RwLock::new(InnerDiscoveryResolver {
            resolver: resolver.clone(),
            token: cancel_token.clone(),
            discovery: config.discovery.clone(),
            counter: AtomicUsize::new(0),
            found_urls: vec![],
        }));
        let inner_clone = inner.clone();
        tokio::spawn(async move {
            let inner = inner_clone;
            let resolver = resolver;
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            let discovery_target = {
                let lock = inner.read().await;
                lock.discovery.clone()
            };

            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        break;
                    }
                    _ = interval.tick() => {}
                }
                tracing::debug!("discovery resolver reload");
                let lookup_result = match resolver
                    .lookup_ip(discovery_target.host_str().unwrap())
                    .await
                {
                    Ok(lookup_result) => lookup_result,
                    Err(e) => {
                        tracing::error!(
                                url = %discovery_target,
                                error = %e,
                                "failed to lookup ip for discovery target"
                        );
                        continue;
                    }
                };

                let mut result = Vec::new();
                for found in lookup_result.iter() {
                    let mut temp = discovery_target.clone();
                    let found_str = found.to_string();
                    if let Err(err) = temp.set_host(Some(&found_str)) {
                        tracing::error!(
                            url = %discovery_target,
                            error = %err,
                            "failed to set host for discovery target"
                        );
                        continue;
                    }
                    result.push(temp);
                }
                let mut lock = inner.write().await;
                lock.counter.store(0, Ordering::Relaxed);
                lock.found_urls.clear();
                lock.found_urls.extend(result);
                drop(lock);
            }
            tracing::info!("discovery resolver stopped");
        });
        Ok(Self(inner))
    }
}

pub struct InnerDiscoveryResolver {
    #[allow(dead_code)]
    resolver: hickory_resolver::TokioResolver,
    #[allow(dead_code)]
    token: CancellationToken,
    discovery: Url,
    counter: AtomicUsize,
    found_urls: Vec<Url>,
}

impl GeneralResolver for DiscoveryResolver {
    async fn resolve(&self, _target: &http::request::Parts) -> Result<Url, Error> {
        let resolver = self.0.read().await;
        if resolver.found_urls.is_empty() {
            return Err(Error::ServiceUnavailable(Error503::NoUpstreamMcpServer));
        }
        let index = resolver.counter.fetch_add(1, Ordering::SeqCst) % resolver.found_urls.len();
        Ok(resolver.found_urls[index].clone())
    }
}
