use std::sync::Arc;

use anyhow::{Context, Result};
use tokio_util::sync::CancellationToken;

use crate::{
    config::{ClusterConfig, Config, UpstreamConfig},
    manager::{
        resolver::ResolverManager,
        storage::{LocalManager, ManagerTrait, RaftManager, StorageManager},
    },
};

pub async fn init_storage(config: Arc<Config>) -> Result<StorageManager> {
    let manager: StorageManager = if let ClusterConfig::Raft(raft) = &config.server.cluster {
        tracing::info!("Raft cluster: {:?}", raft);
        let manager = RaftManager::new(raft).await?;
        manager.into()
    } else {
        tracing::warn!("🚨🚨🚨 Single host mode enabled, no cluster configuration found, don't use more than one replica for service");
        let local = LocalManager::new();
        local.into()
    };
    Ok(manager)
}

pub async fn init_resolver(
    ct: CancellationToken,
    config: Arc<Config>,
    ref_manager: StorageManager,
) -> Result<ResolverManager> {
    let resolver_manager = ResolverManager::new(ct);
    match &config.upstream {
        UpstreamConfig::Static(static_config) => {
            let manager = ref_manager.clone();
            manager
                .replace_route(static_config.urls.clone().into_iter())
                .await?;
        }
        UpstreamConfig::HeadlessDiscovery(dynamic_config) => {
            resolver_manager
                .listen(dynamic_config.discovery.clone(), move |urls| {
                    let mananger = ref_manager.clone();
                    async move {
                        tracing::info!("upstream urls changed: {:?}", urls);
                        match mananger.replace_route(urls.into_iter()).await {
                            Ok(_) => {}
                            Err(err) => {
                                tracing::error!(error = ?err, "failed to replace route");
                            }
                        }
                    }
                })
                .context("failed to set upstream on_change")?;
        }
    }

    Ok(resolver_manager)
}
