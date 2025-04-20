use std::{collections::HashSet, future::Future, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use url::Url;

use crate::config::UpstreamConfig;

pub struct UpstreamManager {
    upstream: Arc<UpstreamConfig>,
}

impl UpstreamConfig {
    pub fn build_manager(&self) -> UpstreamManager {
        UpstreamManager {
            upstream: Arc::new(self.clone()),
        }
    }
}

impl UpstreamManager {
    pub async fn discover(&self) -> Result<Vec<Url>> {
        let config = &*self.upstream;
        Self::inner_discover(config).await
    }

    async fn inner_discover(config: &UpstreamConfig) -> Result<Vec<Url>> {
        match config {
            UpstreamConfig::Static(static_upstream) => Ok(static_upstream.urls.clone()),
            UpstreamConfig::HeadlessDiscovery(headless_discovery_upstream) => {
                let provider = hickory_resolver::name_server::TokioConnectionProvider::default();
                let resolver = hickory_resolver::Resolver::builder(provider)
                    .unwrap()
                    .build();
                let lookup_result = resolver
                    .lookup_ip(headless_discovery_upstream.discovery.host_str().unwrap())
                    .await
                    .context("failed to lookup ip")?;
                let mut result = Vec::new();
                for found in lookup_result.iter() {
                    let mut temp = headless_discovery_upstream.discovery.clone();
                    let found_str = found.to_string();
                    temp.set_host(Some(&found_str))
                        .context("failed to set host")?;
                    result.push(temp);
                }
                let result_url = UrlVec(result);
                Ok(result_url.0)
            }
        }
    }
    pub fn on_change<R, F>(&self, mut f: F) -> Result<()>
    where
        R: Future<Output = ()> + Send + 'static,
        F: FnMut(Vec<Url>) -> R + Send + 'static,
    {
        let upstream_config_arc = self.upstream.clone(); // Clone the Arc<UpstreamConfig>
        tokio::spawn(async move {
            let mut last_urls = HashSet::new();
            loop {
                // Use the cloned Arc inside the spawned task
                match UpstreamManager::inner_discover(&upstream_config_arc).await {
                    Ok(found_urls) => {
                        let found_urls_set = HashSet::<Url>::from_iter(found_urls.clone());
                        let found_urls_wrap = UrlVec(found_urls);
                        if last_urls == found_urls_set {
                            last_urls = found_urls_set;
                            tracing::info!(urls = %found_urls_wrap, "upstream urls changed");
                            f(found_urls_wrap.0).await;
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to discover initial upstream URLs in on_change");
                    }
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
        Ok(())
    }
}

pub struct UrlVec(pub Vec<Url>);

impl std::fmt::Display for UrlVec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut ls = f.debug_list();
        for url in &self.0 {
            ls.entry(&url.to_string());
        }
        ls.finish()
    }
}
