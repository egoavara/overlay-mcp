use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use crate::{
    config::RaftConfig,
    mcp::tunnel::{new_tunnel, MCPTunnelForClient, MCPTunnelForServer},
};
use anyhow::{Context, Result};
use futures::{FutureExt, SinkExt, StreamExt};
use rmcp::{
    service::{RxJsonRpcMessage, TxJsonRpcMessage},
    transport::{IntoTransport, SseTransport},
    RoleClient,
};
use serde::{Deserialize, Serialize};
use tokio::{
    sync::{
        broadcast::{self, error::RecvError},
        Mutex, Notify, RwLock,
    },
    time::timeout,
};
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

use super::{ConnectionState, ManagerTrait, SessionGuard, StreamGuard};

#[derive(Clone)]
pub struct RaftManager {
    inner: Arc<RaftManagerInner>,
    route_inner: Arc<RouteManagerInner>,
    cancel: CancellationToken,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum RaftManagerEvent {
    ReplaceRoute(Vec<Url>),
    DeleteSession(String),
    // String, ser(RxJsonRpcMessage<RoleClient>)
    DownstreamMessage(String, String),
    // String, ser(TxJsonRpcMessage<RoleClient>)
    UpstreamMessage(String, String),
}

struct RaftManagerInner {
    hiqlite: hiqlite::Client,
    sessions: RwLock<HashMap<String, Arc<NodeExclusiveRaftSession>>>,
}
struct RouteManagerInner {
    routes: RwLock<Vec<Url>>,
    current_index: AtomicUsize,
}

#[derive(Debug, Clone, Serialize, Deserialize, hiqlite::EnumIter, hiqlite::ToPrimitive)]
pub enum RaftCacheKey {
    Session,
}

struct NodeExclusiveRaftSession {
    up: Mutex<Option<broadcast::Sender<TxJsonRpcMessage<RoleClient>>>>,
    down: Mutex<Option<broadcast::Receiver<RxJsonRpcMessage<RoleClient>>>>,
    down_bypass: Mutex<Option<broadcast::Sender<RxJsonRpcMessage<RoleClient>>>>,
    notify: Notify,
    ct: CancellationToken,
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
        let hiqlite_client = client.clone();
        let inner = Arc::new(RaftManagerInner {
            hiqlite: hiqlite_client,
            sessions: RwLock::new(HashMap::new()),
        });
        let route_inner = Arc::new(RouteManagerInner {
            routes: RwLock::new(vec![]),
            current_index: AtomicUsize::new(0),
        });
        let inner_clone = inner.clone();
        let route_inner_clone = route_inner.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = child_token.cancelled() => {
                        break;
                    }
                    event = client.listen::<RaftManagerEvent>() => {
                        match event {
                            Ok(event) => {
                                let result = match event {
                                    RaftManagerEvent::ReplaceRoute(routes) => route_inner_clone.handle_replace_route(routes).await,
                                    RaftManagerEvent::DeleteSession(session_id) => inner_clone.handle_delete(session_id).await,
                                    _ => Ok(()),
                                };
                                if let Err(e) = result {
                                    tracing::error!("raft manager event listen error: {}", e);
                                }
                            }
                            Err(e) => {
                                tracing::error!("raft manager event listen error: {}", e);
                            }
                        }
                    }
                }
            }
        });
        Ok(Self {
            inner,
            route_inner,
            cancel,
        })
    }
}
impl ManagerTrait for RaftManager {
    async fn replace_route<U: Into<Url>, I: Iterator<Item = U>>(
        &self,
        routes: I,
    ) -> Result<Vec<Url>> {
        self.route_inner.replace_route(routes).await
    }

    async fn route(&self) -> Result<Url> {
        self.route_inner.route().await
    }

    async fn delete(&self, session_id: String) -> Result<()> {
        self.inner.delete(session_id).await
    }

    async fn session_guard(&self, session_id: String) -> Result<SessionGuard> {
        let inner = self.inner.clone();
        RaftManagerInner::session_guard(inner, session_id).await
    }

    async fn close(&self) -> Result<()> {
        self.cancel.cancel();
        Ok(())
    }

    async fn reload(&self, session_id: String) -> Result<Option<ConnectionState>> {
        self.inner.reload(session_id).await
    }

    async fn connect(&self, client: reqwest::Client) -> Result<ConnectionState> {
        let upstream = self.route().await?;
        self.inner
            .connect(self.cancel.child_token(), upstream, client)
            .await
    }

    async fn take_upstream(
        &self,
        session_id: String,
    ) -> Result<StreamGuard<broadcast::Sender<TxJsonRpcMessage<RoleClient>>>> {
        let manager = self.inner.clone();
        let Some(state) = self.reload(session_id.clone()).await? else {
            return Err(anyhow::anyhow!("session not found"));
        };

        let node_session = {
            let lock = manager.sessions.read().await;
            lock.get(&session_id).cloned()
        };
        let session = match node_session {
            Some(session) => session,
            None => self.inner.sub_session(session_id.clone()).await?,
        };
        let sender = session.get_locking_up().await?;
        let (guard, dropper) = StreamGuard::new(sender);
        tokio::spawn(async move {
            let sender = match dropper.await {
                Ok(sender) => sender,
                Err(err) => {
                    tracing::error!(session_id = session_id, error = ?err, "failed to returning upstream");
                    return;
                }
            };
            let lock = manager.sessions.write().await;
            let Some(session) = lock.get(&session_id) else {
                tracing::warn!(
                    session_id = session_id,
                    "session not found, it might be session closed before returning upstream"
                );
                return;
            };
            tracing::info!(session_id = session_id, "returning upstream");
            session.returning_up(sender).await;
        });
        Ok(guard)
    }

    async fn take_bypass_downstream(
        &self,
        session_id: String,
    ) -> Result<StreamGuard<broadcast::Sender<RxJsonRpcMessage<RoleClient>>>> {
        let manager = self.inner.clone();
        let Some(state) = self.reload(session_id.clone()).await? else {
            return Err(anyhow::anyhow!("session not found"));
        };

        let node_session = {
            let lock = manager.sessions.read().await;
            lock.get(&session_id).cloned()
        };
        let session = match node_session {
            Some(session) => session,
            None => self.inner.sub_session(session_id.clone()).await?,
        };
        let sender = session.get_locking_bypass_down().await?;
        let (guard, dropper) = StreamGuard::new(sender);
        tokio::spawn(async move {
            let sender = match dropper.await {
                Ok(sender) => sender,
                Err(err) => {
                    tracing::error!(session_id = session_id, error = ?err, "failed to returning bypass downstream");
                    return;
                }
            };
            let lock = manager.sessions.write().await;
            let Some(session) = lock.get(&session_id) else {
                tracing::warn!(session_id = session_id, "session not found, it might be session closed before returning bypass downstream");
                return;
            };
            tracing::info!(session_id = session_id, "returning bypass downstream");
            session.returning_bypass_down(sender).await;
        });
        Ok(guard)
    }

    async fn take_downstream(
        &self,
        session_id: String,
    ) -> Result<StreamGuard<broadcast::Receiver<RxJsonRpcMessage<RoleClient>>>> {
        let manager = self.inner.clone();
        let Some(state) = self.reload(session_id.clone()).await? else {
            return Err(anyhow::anyhow!("session not found"));
        };

        let node_session = {
            let lock = manager.sessions.read().await;
            lock.get(&session_id).cloned()
        };
        let session = match node_session {
            Some(session) => session,
            None => self.inner.sub_session(session_id.clone()).await?,
        };

        let receiver = session.get_locking_down().await?;
        let (guard, dropper) = StreamGuard::new(receiver);
        tokio::spawn(async move {
            let receiver = match dropper.await {
                Ok(receiver) => receiver,
                Err(err) => {
                    tracing::error!(session_id = session_id, error = ?err, "failed to returning upstream");
                    return;
                }
            };
            let lock = manager.sessions.write().await;
            let Some(session) = lock.get(&session_id) else {
                tracing::warn!(
                    session_id = session_id,
                    "session not found, it might be session closed before returning upstream"
                );
                return;
            };
            tracing::info!(session_id = session_id, "returning downstream");
            session.returning_down(receiver).await;
        });
        Ok(guard)
    }
}

impl NodeExclusiveRaftSession {
    async fn get_locking_bypass_down(
        &self,
    ) -> Result<broadcast::Sender<RxJsonRpcMessage<RoleClient>>> {
        loop {
            let mut down_lock = self.down_bypass.lock().await;
            if let Some(down) = down_lock.take() {
                return Ok(down);
            }
            drop(down_lock);
            timeout(Duration::from_secs(5), self.notify.notified()).await?;
        }
    }
    async fn get_locking_down(&self) -> Result<broadcast::Receiver<RxJsonRpcMessage<RoleClient>>> {
        loop {
            let mut down_lock = self.down.lock().await;
            if let Some(down) = down_lock.take() {
                return Ok(down);
            }
            drop(down_lock);
            timeout(Duration::from_secs(5), self.notify.notified()).await?;
        }
    }
    async fn get_locking_up(&self) -> Result<broadcast::Sender<TxJsonRpcMessage<RoleClient>>> {
        loop {
            let mut up_lock = self.up.lock().await;
            if let Some(up) = up_lock.take() {
                return Ok(up);
            }
            drop(up_lock);
            timeout(Duration::from_secs(5), self.notify.notified()).await?;
        }
    }
    async fn returning_up(&self, up: broadcast::Sender<TxJsonRpcMessage<RoleClient>>) {
        let mut up_lock = self.up.lock().await;
        up_lock.replace(up);
        drop(up_lock);
        self.notify.notify_waiters();
    }
    async fn returning_down(&self, down: broadcast::Receiver<RxJsonRpcMessage<RoleClient>>) {
        let mut down_lock = self.down.lock().await;
        down_lock.replace(down);
        drop(down_lock);
        self.notify.notify_waiters();
    }
    async fn returning_bypass_down(&self, down: broadcast::Sender<RxJsonRpcMessage<RoleClient>>) {
        let mut down_lock = self.down_bypass.lock().await;
        down_lock.replace(down);
        drop(down_lock);
        self.notify.notify_waiters();
    }
}

impl RaftManagerInner {
    async fn connect(
        &self,
        ct: CancellationToken,
        upstream: Url,
        client: reqwest::Client,
    ) -> Result<ConnectionState> {
        let session_id = Uuid::new_v4().to_string();
        let transport = SseTransport::start_with_client(upstream.clone(), client).await?;
        let state = ConnectionState {
            session_id: session_id.clone(),
            upstream,
        };
        self.hiqlite
            .put(RaftCacheKey::Session, session_id.clone(), &state, None)
            .await?;
        let (svr_send, svr_recv) = self.main_session(session_id).await?;
        let (sink, stream) = IntoTransport::<RoleClient, _, _>::into_transport(transport);
        let sink_ct = ct.child_token();
        tokio::spawn(async move {
            let mut sink = sink;
            let mut recv = svr_recv;
            loop {
                tokio::select! {
                    msg = recv.recv() => {
                        match msg {
                            Ok(msg) => {
                                tracing::info!("to server message: {:?}", msg);
                                match sink.send(msg).await {
                                    Ok(_) => {}
                                    Err(e) => {
                                        tracing::error!("send error: {:?}", e);
                                    }
                                }
                            }
                            Err(RecvError::Lagged(_)) => {}
                            Err(RecvError::Closed) => {
                                tracing::error!("server -> overlay -> client stream closed");
                                break;
                            }
                        }
                    }
                    _ = sink_ct.cancelled() => {
                        break;
                    }
                }
            }
        });
        let stream_ct = ct.child_token();
        tokio::spawn(async move {
            let mut stream = stream;
            let send = svr_send;
            loop {
                tokio::select! {
                    msg = stream.next() => {
                        match msg {
                            Some(msg) => {
                                tracing::info!("to client message: {:?}", msg);
                                match send.send(msg) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        tracing::error!("send error: {:?}", e);
                                    }
                                }
                            }
                            None => {
                                tracing::info!("client -> overlay -> server stream closed");
                                break;
                            }
                        }
                    }
                    _ = stream_ct.cancelled() => {
                        break;
                    }
                }
            }
        });

        Ok(state)
    }

    async fn sub_session(&self, session_id: String) -> Result<Arc<NodeExclusiveRaftSession>> {
        let ct = CancellationToken::new();
        let (for_server, for_client) = new_tunnel();

        let MCPTunnelForClient {
            recv: clt_recv,
            send: clt_send,
        } = for_client;

        let MCPTunnelForServer {
            recv: svr_recv,
            send: svr_send,
        } = for_server;

        let listener = self.hiqlite.clone();
        let child_token = ct.child_token();
        let current_session_id = session_id.clone();
        let downstream_sender = svr_send.clone();
        let inner = NodeExclusiveRaftSession {
            up: Mutex::new(Some(clt_send)),
            down: Mutex::new(Some(clt_recv)),
            down_bypass: Mutex::new(Some(svr_send.clone())),
            notify: Notify::new(),
            ct: ct.clone(),
        };
        let session_del_ct = ct.clone();
        // cause other node may have same session id, so need to listen all node's session event
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = child_token.cancelled() => {
                        break;
                    }
                    event = listener.listen::<RaftManagerEvent>() => {
                        match event {
                            Ok(event) => {
                                match event {
                                    RaftManagerEvent::DownstreamMessage(session_id, message) => {
                                        if session_id == current_session_id {
                                            // send error can be ignored, because local session not exist, but other node may exist
                                            let _ = downstream_sender.send(serde_json::from_str::<RxJsonRpcMessage<RoleClient>>(message.as_str()).unwrap());
                                        }
                                    }
                                    RaftManagerEvent::UpstreamMessage(_, _) => {
                                        // if currrent session is not main session
                                        // upstream message will be ignored
                                    },
                                    RaftManagerEvent::DeleteSession(session_id) => {
                                        if session_id == current_session_id {
                                            session_del_ct.cancel();
                                            break;
                                        }
                                    },
                                    _ => {}
                                }
                            }
                            Err(e) => {
                                tracing::error!("raft manager event listen error: {}", e);
                            }
                        }
                    }
                }
            }
        });
        let raft_broadcaster = self.hiqlite.clone();
        let child_token = ct.child_token();
        let current_session_id = session_id.clone();
        tokio::spawn(async move {
            let mut stream = svr_recv;
            loop {
                tokio::select! {
                    _ = child_token.cancelled() => {
                        break;
                    }
                    msg = stream.recv() => {
                        match msg {
                            Ok(msg) => {
                                tracing::info!("to client message: {:?}", msg);
                                match raft_broadcaster.notify(&RaftManagerEvent::UpstreamMessage(current_session_id.clone(), serde_json::to_string(&msg).unwrap())).await {
                                    Ok(_) => {}
                                    Err(e) => {
                                        tracing::error!("raft manager event notify error: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!("raft manager event listen error: {}", e);
                            }
                        }
                    }
                }
            }
        });
        let session = Arc::new(inner);
        self.sessions
            .write()
            .await
            .insert(session_id, session.clone());
        Ok(session)
    }

    async fn main_session(
        &self,
        session_id: String,
    ) -> Result<(
        broadcast::Sender<RxJsonRpcMessage<RoleClient>>,
        broadcast::Receiver<TxJsonRpcMessage<RoleClient>>,
    )> {
        let ct = CancellationToken::new();
        let (for_server, for_client) = new_tunnel();

        let MCPTunnelForClient {
            recv: clt_recv,
            send: clt_send,
        } = for_client;

        let MCPTunnelForServer {
            recv: svr_recv,
            send: svr_send,
        } = for_server;

        let listener = self.hiqlite.clone();
        let child_token = ct.child_token();
        let current_session_id = session_id.clone();
        let downstream_sender = svr_send.clone();
        let upstream_sender = clt_send.clone();
        let inner = NodeExclusiveRaftSession {
            up: Mutex::new(Some(clt_send)),
            down: Mutex::new(Some(clt_recv)),
            down_bypass: Mutex::new(Some(svr_send.clone())),
            notify: Notify::new(),
            ct: ct.clone(),
        };
        // cause other node may have same session id, so need to listen all node's session event
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = child_token.cancelled() => {
                        break;
                    }
                    event = listener.listen::<RaftManagerEvent>() => {
                        match event {
                            Ok(event) => {
                                match event {
                                    RaftManagerEvent::DownstreamMessage(session_id, message) => {
                                        if session_id == current_session_id {
                                            // send error can be ignored, because local session not exist, but other node may exist
                                            let _ = downstream_sender.send(serde_json::from_str::<RxJsonRpcMessage<RoleClient>>(message.as_str()).unwrap());
                                        }
                                    }
                                    RaftManagerEvent::UpstreamMessage(session_id, message) => {
                                        if session_id == current_session_id {
                                            // send error can be ignored, because local session not exist, but other node may exist
                                            let _ = upstream_sender.send(serde_json::from_str::<TxJsonRpcMessage<RoleClient>>(message.as_str()).unwrap());
                                        }
                                    },
                                    _ => {}
                                }
                            }
                            Err(e) => {
                                tracing::error!("raft manager event listen error: {}", e);
                            }
                        }
                    }
                }
            }
        });
        self.sessions
            .write()
            .await
            .insert(session_id, Arc::new(inner));
        Ok((svr_send, svr_recv))
    }

    async fn delete(&self, session_id: String) -> Result<()> {
        self.hiqlite
            .delete(RaftCacheKey::Session, session_id.clone())
            .await?;
        // delete order is important,
        // if raft data remain, request with expired session id will make empty, but never delete session data
        // so delete raft data first, then delete local session, then notify other node to delete other's session
        let mut lock = self.sessions.write().await;
        lock.remove(&session_id);
        drop(lock);
        self.hiqlite
            .notify(&RaftManagerEvent::DeleteSession(session_id))
            .await?;
        Ok(())
    }

    async fn reload(&self, session_id: String) -> Result<Option<ConnectionState>> {
        let session = self
            .hiqlite
            .get::<_, _, ConnectionState>(RaftCacheKey::Session, session_id)
            .await?;
        Ok(session)
    }

    async fn session_guard(
        this: Arc<RaftManagerInner>,
        session_id: String,
    ) -> Result<SessionGuard> {
        let Some(state) = this.reload(session_id).await? else {
            return Err(anyhow::anyhow!("session not found"));
        };
        let (guard, receiver) = SessionGuard::new(state.session_id);
        tokio::spawn(async move {
            let receiver = receiver;
            let Ok(session_id) = receiver.await else {
                tracing::error!("session guard receiver closed");
                return;
            };
            if let Err(err) = this.delete(session_id).await {
                tracing::warn!(error = ?err, "failed to delete session, it might be already deleted");
            }
        });
        Ok(guard)
    }

    async fn handle_delete(&self, session_id: String) -> Result<()> {
        let mut lock = self.sessions.write().await;
        lock.remove(&session_id);
        Ok(())
    }
}

impl RouteManagerInner {
    async fn handle_replace_route(&self, routes: Vec<Url>) -> Result<()> {
        let mut routes_target = self.routes.write().await;
        routes_target.clear();
        routes_target.extend(routes);
        tracing::info!(route= ?routes_target, "raft manager event listen replace route");
        Ok(())
    }

    async fn replace_route<U: Into<Url>, I: Iterator<Item = U>>(
        &self,
        routes: I,
    ) -> Result<Vec<Url>> {
        let routes = routes.map(|route| route.into()).collect::<Vec<_>>();
        {
            let mut lock = self.routes.write().await;
            lock.clear();
            lock.extend(routes.clone());
        }
        Ok(routes)
    }

    async fn route(&self) -> Result<Url> {
        let index = self.current_index.fetch_add(1, Ordering::Relaxed);
        let lock = self.routes.read().await;
        let upstream = lock[index % lock.len()].clone();
        Ok(upstream)
    }
}

// impl ManagerTrait for RaftManager {
//     async fn replace_route<U: Into<Url>, I: Iterator<Item = U>>(
//         &mut self,
//         routes: I,
//     ) -> Result<Vec<Url>> {
//         let routes = routes.map(|route| route.into()).collect::<Vec<_>>();
//         self.hiqlite
//             .notify(&RaftManagerEvent::ReplaceRoute(routes.clone()))
//             .await?;
//         Ok(routes)
//     }

//     async fn route(&self) -> Result<Url> {
//         let index = self.route_counter.fetch_add(1, Ordering::Relaxed);
//         let routes = self.routes.read().await;
//         Ok(routes[index % routes.len()].clone())
//     }

//     async fn get(&self, session_id: String) -> Result<Option<ConnectionState>> {
//         let a = self
//             .hiqlite
//             .get::<_, _, ConnectionState>(RaftCacheKey::Session, session_id)
//             .await
//             .context("raft get session failed")?;
//         Ok(a)
//     }

//     async fn delete(&mut self, session_id: String) -> Result<()> {
//         self.hiqlite
//             .delete(RaftCacheKey::Session, session_id)
//             .await?;
//         Ok(())
//     }

//     async fn close(&self) -> Result<()> {
//         self.cancel.cancel();
//         self.hiqlite.shutdown().await?;
//         Ok(())
//     }

//     async fn connect(&mut self,data:ConnectionStateCreate) -> Result<ConnectionState> {
//         let session_id = {
//             let context = self.context_v7.lock().await;
//             uuid::Uuid::new_v7(Timestamp::now(&*context)).to_string()
//         };
//         let connection_state = ConnectionState::new(data, session_id);
//         self.hiqlite
//             .put(
//                 RaftCacheKey::Session,
//                 connection_state.session_id.clone(),
//                 &connection_state,
//                 None,
//             )
//             .await?;
//         Ok(connection_state)
//     }
// }
