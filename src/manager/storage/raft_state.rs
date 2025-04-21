use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::{ContextV7, Timestamp};

use crate::config::RaftConfig;

use super::{ConnectionState, ConnectionStateCreate, ManagerTrait};

#[derive(Clone)]
pub struct RaftManager {
    hiqlite: hiqlite::Client,
    route_counter: Arc<AtomicUsize>,
    routes: Arc<RwLock<Vec<Url>>>,
    context_v7: Arc<Mutex<ContextV7>>,
    cancel: CancellationToken,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum RaftManagerEvent {
    ReplaceRoute(Vec<Url>),
}

#[derive(Debug, Clone, Serialize, Deserialize, hiqlite::EnumIter, hiqlite::ToPrimitive)]
pub enum RaftCacheKey {
    Session,
}

impl RaftManager {
    pub async fn new(config: &RaftConfig) -> Result<Self> {
        let id = match (config.id, config.index) {
            (Some(id), None) => id,
            (None, Some(index)) => {
                config
                    .nodes
                    .get(index)
                    .ok_or(anyhow::anyhow!("raft index out of bounds"))?
                    .id
            }
            (Some(_), Some(_)) => {
                return Err(anyhow::anyhow!(
                    "raft id and index cannot be set at the same time"
                ));
            }
            (None, None) => {
                return Err(anyhow::anyhow!("raft id or index must be set"));
            }
        };
        let config = hiqlite::NodeConfig {
            node_id: id,
            raft_config: config.cluster.clone(),
            secret_raft: config.secret.clone(),
            secret_api: config.secret.clone(),
            nodes: config
                .nodes
                .iter()
                .map(|node| hiqlite::Node {
                    id: node.id,
                    addr_raft: node.raft.clone(),
                    addr_api: node.api.clone(),
                })
                .collect(),
            ..Default::default()
        };
        let client = hiqlite::start_node_with_cache::<RaftCacheKey>(config)
            .await
            .context("raft mode start failed")?;
        let cancel = CancellationToken::new();
        let child_token = cancel.child_token();
        let routes = Arc::new(RwLock::new(vec![]));
        let hiqlite_client = client.clone();
        let listener_routes = routes.clone();
        tokio::spawn(async move {
            'main: loop {
                tokio::select! {
                    _ = child_token.cancelled() => {
                        break 'main;
                    }
                    event = hiqlite_client.listen::<RaftManagerEvent>() => {
                        match event {
                            Ok(RaftManagerEvent::ReplaceRoute(replace_routes)) => {
                                let mut routes_target = listener_routes.write().await;
                                routes_target.clear();
                                routes_target.extend(replace_routes);
                                tracing::info!(route= ?routes_target, "raft manager event listen replace route",);
                            }
                            Err(e) => {
                                tracing::error!("raft manager event listen error: {}", e);
                            }
                        }
                    }
                }
            }
        });
        let context_v7 = Arc::new(Mutex::new(ContextV7::new()));
        Ok(Self {
            hiqlite: client,
            route_counter: Arc::new(AtomicUsize::new(0)),
            routes,
            context_v7,
            cancel,
        })
    }
}

impl ManagerTrait for RaftManager {
    async fn replace_route<U: Into<Url>, I: Iterator<Item = U>>(
        &mut self,
        routes: I,
    ) -> Result<Vec<Url>> {
        let routes = routes.map(|route| route.into()).collect::<Vec<_>>();
        self.hiqlite
            .notify(&RaftManagerEvent::ReplaceRoute(routes.clone()))
            .await?;
        Ok(routes)
    }

    async fn route(&self) -> Result<Url> {
        let index = self.route_counter.fetch_add(1, Ordering::Relaxed);
        let routes = self.routes.read().await;
        Ok(routes[index % routes.len()].clone())
    }

    async fn create(&mut self, data: ConnectionStateCreate) -> Result<ConnectionState> {
        let session_id = {
            let context = self.context_v7.lock().await;
            uuid::Uuid::new_v7(Timestamp::now(&*context)).to_string()
        };
        let connection_state = ConnectionState::new(data, session_id);
        self.hiqlite
            .put(
                RaftCacheKey::Session,
                connection_state.session_id.clone(),
                &connection_state,
                None,
            )
            .await?;
        Ok(connection_state)
    }

    async fn get(&self, session_id: String) -> Result<Option<ConnectionState>> {
        let a = self
            .hiqlite
            .get::<_, _, ConnectionState>(RaftCacheKey::Session, session_id)
            .await
            .context("raft get session failed")?;
        Ok(a)
    }

    async fn delete(&mut self, session_id: String) -> Result<()> {
        self.hiqlite
            .delete(RaftCacheKey::Session, session_id)
            .await?;
        Ok(())
    }

    async fn patch<Patcher: FnOnce(&mut ConnectionState) -> Result<()>>(
        &mut self,
        session_id: String,
        patcher: Patcher,
    ) -> Result<Option<ConnectionState>> {
        let lock = self.hiqlite.lock(session_id.clone()).await?;
        let connection_state = self
            .hiqlite
            .get::<_, _, ConnectionState>(RaftCacheKey::Session, session_id.as_str())
            .await
            .context("raft get session failed")?;
        let Some(mut connection_state) = connection_state else {
            return Ok(None);
        };
        patcher(&mut connection_state)?;
        self.hiqlite
            .put(
                RaftCacheKey::Session,
                session_id.clone(),
                &connection_state,
                None,
            )
            .await?;
        drop(lock);
        Ok(Some(connection_state))
    }

    async fn close(&self) -> Result<()> {
        self.cancel.cancel();
        self.hiqlite.shutdown().await?;
        Ok(())
    }
}
