use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::Result;
use futures::{FutureExt, SinkExt};
use rmcp::{
    service::{RxJsonRpcMessage, TxJsonRpcMessage},
    transport::{IntoTransport, SseTransport},
    RoleClient,
};
use tokio::{
    sync::{
        broadcast::{self, error::RecvError},
        Mutex, Notify, RwLock,
    },
    time::timeout,
};
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

use crate::mcp::tunnel::{new_tunnel, MCPTunnelForClient, MCPTunnelForServer};

use super::{ConnectionState, ManagerTrait, SessionGuard, StreamGuard};

#[derive(Clone)]
pub struct LocalManager(Arc<LocalSessionManagerInner>);

struct LocalSessionManagerInner {
    upstreams: RwLock<Vec<Url>>,
    current_index: AtomicUsize,
    sessions: RwLock<HashMap<String, Arc<LocalSession>>>,
}

struct LocalSession {
    state: ConnectionState,
    up: Mutex<Option<broadcast::Sender<TxJsonRpcMessage<RoleClient>>>>,
    down: Mutex<Option<broadcast::Receiver<RxJsonRpcMessage<RoleClient>>>>,
    down_bypass: Mutex<Option<broadcast::Sender<RxJsonRpcMessage<RoleClient>>>>,
    notify: Notify,
    token: CancellationToken,
}

impl LocalManager {
    pub fn new() -> Self {
        Self(Arc::new(LocalSessionManagerInner {
            upstreams: RwLock::new(vec![]),
            current_index: AtomicUsize::new(0),
            sessions: RwLock::new(HashMap::new()),
        }))
    }
}
impl ManagerTrait for LocalManager {
    async fn replace_route<U: Into<Url>, I: Iterator<Item = U>>(
        &self,
        routes: I,
    ) -> Result<Vec<Url>> {
        let routes = routes.map(|route| route.into()).collect::<Vec<_>>();
        {
            let mut lock = self.0.upstreams.write().await;
            lock.clear();
            lock.extend(routes.clone());
        }
        Ok(routes)
    }

    async fn route(&self) -> Result<Url> {
        let index = self.0.current_index.fetch_add(1, Ordering::Relaxed);
        let lock = self.0.upstreams.read().await;
        let upstream = lock[index % lock.len()].clone();
        Ok(upstream)
    }

    async fn delete(&self, session_id: String) -> Result<()> {
        let old = self.0.sessions.write().await.remove(&session_id);

        tracing::info!(session_id = &session_id, "deleting session");
        if let Some(local) = old {
            local.token.cancel();
        }
        Ok(())
    }

    async fn session_guard(&self, session_id: String) -> Result<SessionGuard> {
        let Some(state) = self.reload(session_id).await? else {
            return Err(anyhow::anyhow!("session not found"));
        };
        let (guard, receiver) = SessionGuard::new(state.session_id);
        let manager = self.0.clone();
        tokio::spawn(async move {
            let manager = LocalManager(manager);
            let receiver = receiver;
            let Ok(session_id) = receiver.await else {
                tracing::error!("session guard receiver closed");
                return;
            };
            if let Err(err) = manager.delete(session_id).await {
                tracing::warn!(error = ?err, "failed to delete session, it might be already deleted");
            }
        });
        Ok(guard)
    }

    async fn close(&self) -> Result<()> {
        Ok(())
    }

    async fn reload(&self, session_id: String) -> Result<Option<ConnectionState>> {
        let lock = self.0.sessions.read().await;
        let session = lock.get(&session_id).cloned();
        drop(lock);

        match session {
            Some(local) => Ok(Some(local.state.clone())),
            None => Ok(None),
        }
    }
    async fn connect(&self, client: reqwest::Client) -> Result<ConnectionState> {
        let session_id = Uuid::new_v4().to_string();
        let upstream = self.route().await?;
        let transport = SseTransport::start_with_client(upstream.clone(), client).await?;
        let (for_server, for_client) = new_tunnel();
        let ct = CancellationToken::new();

        let MCPTunnelForClient {
            recv: clt_recv,
            send: clt_send,
        } = for_client;

        let MCPTunnelForServer {
            recv: svr_recv,
            send: svr_send,
        } = for_server;
        let inner = LocalSession {
            state: ConnectionState {
                session_id: session_id.clone(),
                upstream,
            },
            up: Mutex::new(Some(clt_send)),
            down: Mutex::new(Some(clt_recv)),
            down_bypass: Mutex::new(Some(svr_send.clone())),
            notify: Notify::new(),
            token: ct,
        };
        let (sink, stream) = IntoTransport::<RoleClient, _, _>::into_transport(transport);
        let sink_ct = inner.token.child_token();
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
        let stream_ct = inner.token.child_token();
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
        let state = inner.state.clone();
        self.0
            .sessions
            .write()
            .await
            .insert(session_id, Arc::new(inner));
        Ok(state)
    }

    async fn take_upstream(
        &self,
        session_id: String,
    ) -> Result<StreamGuard<broadcast::Sender<TxJsonRpcMessage<RoleClient>>>> {
        let manager = self.0.clone();
        let session = {
            let lock = self.0.sessions.read().await;
            lock.get(&session_id).cloned()
        };
        match session {
            Some(session) => {
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
                        tracing::warn!(session_id = session_id, "session not found, it might be session closed before returning upstream");
                        return;
                    };
                    tracing::info!(session_id = session_id, "returning upstream");
                    session.returning_up(sender).await;
                });
                Ok(guard)
            }
            None => Err(anyhow::anyhow!("session not found")),
        }
    }

    async fn take_bypass_downstream(
        &self,
        session_id: String,
    ) -> Result<StreamGuard<broadcast::Sender<RxJsonRpcMessage<RoleClient>>>> {
        let manager = self.0.clone();
        let session = {
            let lock = self.0.sessions.read().await;
            lock.get(&session_id).cloned()
        };
        match session {
            Some(session) => {
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
            None => Err(anyhow::anyhow!("session not found")),
        }
    }

    async fn take_downstream(
        &self,
        session_id: String,
    ) -> Result<StreamGuard<broadcast::Receiver<RxJsonRpcMessage<RoleClient>>>> {
        let manager = self.0.clone();
        let session = {
            let lock = self.0.sessions.read().await;
            lock.get(&session_id).cloned()
        };
        match session {
            Some(session) => {
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
                        tracing::warn!(session_id = session_id, "session not found, it might be session closed before returning upstream");
                        return;
                    };
                    tracing::info!(session_id = session_id, "returning downstream");
                    session.returning_down(receiver).await;
                });
                Ok(guard)
            }
            None => Err(anyhow::anyhow!("session not found")),
        }
    }
}

impl LocalSession {
    async fn get_locking_bypass_down(&self) -> Result<broadcast::Sender<RxJsonRpcMessage<RoleClient>>> {
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
