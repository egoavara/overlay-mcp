use std::{collections::HashMap, sync::Arc};

use overlay_mcp_core::{
    server::RaftConfig, BaseModifiers, Config, Error, FatalError, GeneralSession,
    GeneralSessionManager,
};
use tokio::sync::{broadcast, RwLock};
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

use crate::{RaftSchema, RaftSchemaEvent, RaftSession, RaftSessionData};

#[derive(Clone)]
pub struct RaftManager {
    inner: Arc<RaftManagerInner>,
}

pub(crate) struct RaftManagerInner {
    pub(crate) raft_client: hiqlite::Client,
    pub(crate) event_send: broadcast::Sender<RaftSchemaEvent>,
    pub(crate) sessions: RwLock<HashMap<String, RaftSession>>,
    pub(crate) cancel_token: CancellationToken,
    pub(crate) passthrough: BaseModifiers,
}

impl RaftManager {
    pub async fn new(
        cancel_token: CancellationToken,
        config: &RaftConfig,
        passthrough: BaseModifiers,
    ) -> Result<Self, Error> {
        let node_id = match (&config.id, &config.index) {
            (Some(id), None) => *id,
            (None, Some(index)) => config.nodes[*index].id,
            (Some(_), Some(_)) => {
                return Err(Error::Fatal(FatalError::RaftUnresolvedNodeId(
                    "both id and index are set, only one should be set",
                )));
            }
            (None, None) => {
                return Err(Error::Fatal(FatalError::RaftUnresolvedNodeId(
                    "both id and index are not set, one should be set",
                )));
            }
        };
        let current_node =
            config
                .nodes
                .iter()
                .find(|node| node.id == node_id)
                .ok_or(Error::Fatal(FatalError::RaftUnresolvedNodeId(
                    "node not found",
                )))?;
        let config = hiqlite::NodeConfig {
            node_id,
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
            read_pool_size: current_node.read_pool_size,
            ..Default::default()
        };
        tracing::info!("raft config: {:?}", config);
        let client = hiqlite::start_node_with_cache::<RaftSchema>(config).await?;
        let (send, _) = broadcast::channel(16);
        let inner = Arc::new(RaftManagerInner {
            raft_client: client,
            event_send: send.clone(),
            sessions: RwLock::new(HashMap::new()),
            cancel_token,
            passthrough,
        });
        let event_clt = inner.raft_client.clone();
        let event_cancel = inner.cancel_token.clone();
        let event_inner = inner.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = event_cancel.cancelled() => {
                        break;
                    }
                    event = event_clt.listen::<RaftSchemaEvent>() => {
                        match event {
                            Ok(RaftSchemaEvent::DeleteSession(id)) => {
                                tracing::debug!("delete session: {:?}", id);
                                let deleted_session = event_inner.sessions.write().await.remove(&id);
                                if let Some(session) = deleted_session {
                                    session.cancel_token.cancel();
                                }
                            }
                            Ok(event) => {
                                tracing::debug!("other event: {:?}", event);
                                match send.send(event) {
                                    Ok(_) => {}
                                    Err(_) => {
                                        // no active listener error, it was intentional design
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!("error listening for event: {:?}", e);
                            }
                        }
                    }
                }
            }
        });
        Ok(Self { inner })
    }
}

impl GeneralSessionManager for RaftManager {
    type Session = RaftSession;

    async fn create(&self, upstream_url: Url) -> Result<Self::Session, Error> {
        let session_data = RaftSessionData {
            session_id: Uuid::new_v4().to_string(),
            upstream_url,
            main_subsession_id: None,
        };

        let session = RaftSession::new(
            self.inner.clone(),
            session_data.session_id.clone(),
            session_data.upstream_url.clone(),
            self.inner.cancel_token.child_token(),
        );

        self.inner
            .raft_client
            .put(
                RaftSchema::Session,
                session_data.session_id.clone(),
                &session_data,
                None,
            )
            .await?;

        self.inner
            .sessions
            .write()
            .await
            .insert(session.session_id().into_owned(), session.clone());

        Ok(session)
    }

    async fn find(&self, session_id: &str) -> Result<Option<Self::Session>, Error> {
        let Some(session_data) = self
            .inner
            .raft_client
            .get::<_, _, RaftSessionData>(RaftSchema::Session, session_id)
            .await?
        else {
            return Ok(None);
        };

        let session = {
            let lock = self.inner.sessions.read().await;
            lock.get(session_id).cloned()
        };

        if let Some(session) = session {
            Ok(Some(session))
        } else {
            let mut lock = self.inner.sessions.write().await;
            let session = lock
                .entry(session_data.session_id.clone())
                .or_insert_with(|| {
                    RaftSession::new(
                        self.inner.clone(),
                        session_data.session_id.clone(),
                        session_data.upstream_url.clone(),
                        self.inner.cancel_token.child_token(),
                    )
                })
                .clone();
            Ok(Some(session))
        }
    }
}
