use std::{collections::HashSet, future::Future, time::Duration};

use anyhow::{Context, Result};
use hickory_resolver::{
    name_server::GenericConnector, proto::runtime::TokioRuntimeProvider, Resolver,
};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::utils::urls_display::UrlsDisplay;

pub struct ResolverManager {
    resolver: Resolver<GenericConnector<TokioRuntimeProvider>>,
    cancel: CancellationToken,
}

impl ResolverManager {
    pub fn new(cancel: CancellationToken) -> Self {
        let provider = GenericConnector::<TokioRuntimeProvider>::default();
        let resolver = Resolver::builder(provider).unwrap().build();
        Self { cancel, resolver }
    }

    pub fn listen<R, F>(&self, url: Url, mut f: F) -> Result<()>
    where
        R: Future<Output = ()> + Send + 'static,
        F: FnMut(Vec<Url>) -> R + Send + 'static,
    {
        let ct = self.cancel.clone();
        let resolver = self.resolver.clone();
        tokio::spawn(async move {
            let mut last_urls = HashSet::new();
            loop {
                // Use the cloned Arc inside the spawned task
                match Self::inner_discover(&resolver, &url).await {
                    Ok(found_urls) => {
                        let found_urls_wrap = UrlsDisplay(found_urls.iter());
                        if last_urls != found_urls {
                            let urls = found_urls.iter().cloned().collect();
                            f(urls).await;
                            last_urls = found_urls.clone();
                            tracing::info!(urls = %found_urls_wrap, "upstream urls changed");
                        } else {
                            tracing::info!(urls = %found_urls_wrap, "upstream urls unchanged");
                        }
                    }
                    Err(e) => {
                        tracing::error!(url=url.as_str(), error = %e, "Failed to discover initial upstream URLs in on_change");
                    }
                }
                tokio::select! {
                    _ = ct.cancelled() => {
                        tracing::info!("resolver manager cancelled");
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_secs(15)) => {}
                }
            }
        });
        Ok(())
    }

    async fn inner_discover(
        resolver: &Resolver<GenericConnector<TokioRuntimeProvider>>,
        url: &Url,
    ) -> Result<HashSet<Url>> {
        tracing::info!(query= %url, "dns discovery query");
        let lookup_result = resolver
            .lookup_ip(url.host_str().unwrap())
            .await
            .context("failed to lookup ip")?;
        let mut result = HashSet::new();
        for found in lookup_result.iter() {
            let mut temp = url.clone();
            let found_str = found.to_string();
            temp.set_host(Some(&found_str))
                .context("failed to set host")?;
            result.insert(temp);
        }
        Ok(result)
    }
}
