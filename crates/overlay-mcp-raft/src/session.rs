use std::{borrow::Cow, sync::Arc, time::Duration};

use futures::{SinkExt, StreamExt};
use overlay_mcp_core::{
    BypassDownstream, Downstream, Error, FatalError, GeneralSession, SessionGuard, StreamGuard,
    Upstream,
};
use rmcp::{
    model::{ClientJsonRpcMessage, ServerJsonRpcMessage},
    transport::{IntoTransport, SseTransport},
    RoleClient,
};
use serde::{Deserialize, Serialize};
use tokio::{
    sync::{
        broadcast::{self, error::RecvError},
        Mutex, Notify,
    },
    time::timeout,
};
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

use crate::{RaftManagerInner, RaftSchema, RaftSchemaEvent};

#[derive(Clone)]
pub struct RaftSession {
    pub(crate) parent: Arc<RaftManagerInner>,

    pub(crate) session_id: String,
    pub(crate) upstream_url: Url,
    pub(crate) subsession_id: String,

    pub(crate) channels: Arc<RaftSessionChannels>,
    pub(crate) cancel_token: CancellationToken,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct RaftSessionData {
    pub(crate) session_id: String,
    pub(crate) upstream_url: Url,
    pub(crate) main_subsession_id: Option<String>,
}

pub struct RaftSessionChannels {
    pub(crate) upstream: Mutex<Option<Upstream>>,
    pub(crate) downstream: Mutex<Option<Downstream>>,
    pub(crate) bypass_downstream: Mutex<Option<BypassDownstream>>,
    connection: Mutex<RaftSessionConnection>,
    pub(crate) stream_notify: Notify,
}

enum RaftSessionConnection {
    Stopped {
        clt_recv: broadcast::Receiver<ClientJsonRpcMessage>,
        svr_send: broadcast::Sender<ServerJsonRpcMessage>,
    },
    Started(CancellationToken),
}

impl GeneralSession for RaftSession {
    fn session_id(&self) -> Cow<str> {
        Cow::Borrowed(&self.session_id)
    }

    fn upstream_url(&self) -> Cow<Url> {
        Cow::Borrowed(&self.upstream_url)
    }

    async fn guard_upstream(&self) -> Result<StreamGuard<Upstream>, Error> {
        if self.cancel_token.is_cancelled() {
            return Err(Error::AlreadyClosedSession(self.session_id.clone()));
        }

        let upstream = timeout(Duration::from_secs(5), async {
            loop {
                if let Some(upstream) = self.channels.upstream.lock().await.take() {
                    return upstream;
                }
                self.channels.stream_notify.notified().await;
            }
        })
        .await
        .map_err(|_| FatalError::Timeout)?;

        let session_id = self.session_id().into_owned();
        let (guard, notify) = StreamGuard::new(upstream);
        let inner = self.channels.clone();
        tokio::spawn(async move {
            let upstream = notify
                .await
                .expect("guard must be notified, but not. logic error");
            tracing::info!(session_id = session_id, "upstream returned by guard");
            inner.upstream.lock().await.replace(upstream);
        });
        Ok(guard)
    }

    async fn guard_downstream(&self) -> Result<StreamGuard<Downstream>, Error> {
        if self.cancel_token.is_cancelled() {
            return Err(Error::AlreadyClosedSession(self.session_id.clone()));
        }

        let downstream = timeout(Duration::from_secs(5), async {
            loop {
                if let Some(downstream) = self.channels.downstream.lock().await.take() {
                    return downstream;
                }
            }
        })
        .await
        .map_err(|_| FatalError::Timeout)?;

        let session_id = self.session_id().into_owned();
        let (guard, notify) = StreamGuard::new(downstream);
        let inner = self.channels.clone();
        tokio::spawn(async move {
            let downstream = notify
                .await
                .expect("guard must be notified, but not. logic error");
            tracing::info!(session_id = session_id, "downstream returned by guard");
            inner.downstream.lock().await.replace(downstream);
        });
        Ok(guard)
    }

    async fn guard_bypass_downstream(&self) -> Result<StreamGuard<BypassDownstream>, Error> {
        if self.cancel_token.is_cancelled() {
            return Err(Error::AlreadyClosedSession(self.session_id.clone()));
        }

        let bypass_downstream = timeout(Duration::from_secs(5), async {
            loop {
                if let Some(bypass_downstream) = self.channels.bypass_downstream.lock().await.take()
                {
                    return bypass_downstream;
                }
            }
        })
        .await
        .map_err(|_| FatalError::Timeout)?;

        let session_id = self.session_id().into_owned();
        let (guard, notify) = StreamGuard::new(bypass_downstream);
        let inner = self.channels.clone();
        tokio::spawn(async move {
            let bypass_downstream = notify
                .await
                .expect("guard must be notified, but not. logic error");
            tracing::info!(
                session_id = session_id,
                "bypass_downstream returned by guard"
            );
            inner
                .bypass_downstream
                .lock()
                .await
                .replace(bypass_downstream);
        });
        Ok(guard)
    }

    async fn guard_close(&self) -> Result<SessionGuard, Error> {
        let (session_guard, session_guard_receiver) =
            SessionGuard::new(self.session_id().into_owned());
        let token = self.cancel_token.clone();
        tokio::spawn(async move {
            match session_guard_receiver.await {
                Ok(_) => {
                    token.cancel();
                }
                Err(e) => {
                    tracing::error!("session guard receiver error: {:?}", e);
                }
            }
        });
        Ok(session_guard)
    }

    async fn is_started(&self) -> bool {
        if self.cancel_token.is_cancelled() {
            return false;
        }
        self.channels.connection.lock().await.is_started()
    }

    async fn start(&self, original_request: &http::request::Parts) -> Result<(), Error> {
        if self.cancel_token.is_cancelled() {
            return Err(Error::AlreadyClosedSession(self.session_id.clone()));
        }
        let lock = timeout(
            Duration::from_secs(5),
            self.parent.raft_client.lock(self.session_id.clone()),
        )
        .await
        .map_err(|_| Error::Fatal(FatalError::Timeout))
        .and_then(|result| result.map_err(Error::HiqliteError))?;

        let mut session_data = self
            .parent
            .raft_client
            .get::<_, _, RaftSessionData>(RaftSchema::Session, &self.session_id)
            .await?
            .ok_or_else(|| Error::AlreadyClosedSession(self.session_id.clone()))?;
        if session_data.main_subsession_id.is_none() {
            session_data.main_subsession_id = Some(self.subsession_id.clone());
        }
        self.parent
            .raft_client
            .put(
                RaftSchema::Session,
                self.session_id.clone(),
                &session_data,
                None,
            )
            .await?;
        drop(lock);

        if session_data.main_subsession_id.as_ref() == Some(&self.subsession_id) {
            tracing::info!(
                session_id = self.session_id,
                subsession_id = self.subsession_id,
                "start sub session"
            );
            return self.start_sub_session().await;
        }
        tracing::info!(
            session_id = self.session_id,
            subsession_id = self.subsession_id,
            "start main session"
        );
        self.start_main_session(&reqwest::Client::new()).await
    }

    async fn stop(&self) -> Result<(), Error> {
        if self.cancel_token.is_cancelled() {
            return Err(Error::AlreadyClosedSession(self.session_id.clone()));
        }

        let lock = timeout(
            Duration::from_secs(5),
            self.parent.raft_client.lock(self.session_id.clone()),
        )
        .await
        .map_err(|_| Error::Fatal(FatalError::Timeout))
        .and_then(|result| result.map_err(Error::HiqliteError))?;

        let mut session_data = self
            .parent
            .raft_client
            .get::<_, _, RaftSessionData>(RaftSchema::Session, &self.session_id)
            .await?
            .ok_or_else(|| Error::AlreadyClosedSession(self.session_id.clone()))?;
        if session_data.main_subsession_id.as_ref() == Some(&self.subsession_id) {
            session_data.main_subsession_id = None;
            self.parent
                .raft_client
                .put(
                    RaftSchema::Session,
                    self.session_id.clone(),
                    &session_data,
                    None,
                )
                .await?;
        }
        drop(lock);

        match &*self.channels.connection.lock().await {
            RaftSessionConnection::Stopped { .. } => {
                return Err(Error::AlreadyStoppedSession(self.session_id.clone()));
            }
            RaftSessionConnection::Started(stop_ct) => {
                stop_ct.cancel();
            }
        }
        let timeout = timeout(Duration::from_secs(5), async {
            loop {
                self.channels.stream_notify.notified().await;

                match &*self.channels.connection.lock().await {
                    RaftSessionConnection::Stopped { .. } => {
                        break;
                    }
                    RaftSessionConnection::Started(_) => continue,
                }
            }
        });
        if timeout.await.is_err() {
            // if stopped, but not restored in 5 seconds
            // consider this a fatal error, but don't panic
            // because this can be delayed by the network (very unlikely)
            return Err(FatalError::Timeout.into());
        }
        Ok(())
    }

    async fn close(&self) -> Result<(), Error> {
        self.parent
            .raft_client
            .delete(RaftSchema::Session, self.session_id.clone())
            .await?;
        self.parent
            .raft_client
            .notify(&RaftSchemaEvent::DeleteSession(self.session_id.clone()))
            .await?;
        Ok(())
    }
}

impl RaftSession {
    pub(crate) fn new(
        parent: Arc<RaftManagerInner>,
        session_id: String,
        upstream_url: Url,
        cancel_token: CancellationToken,
    ) -> Self {
        let (clt_send, clt_recv) = broadcast::channel::<ClientJsonRpcMessage>(16);
        let (svr_send, svr_recv) = broadcast::channel::<ServerJsonRpcMessage>(16);

        let upstream = Upstream(clt_send);
        let downstream = Downstream(svr_recv);
        let bypass_downstream = BypassDownstream(svr_send.clone());

        RaftSession {
            parent,
            session_id,
            subsession_id: Uuid::new_v4().to_string(),
            upstream_url,
            channels: Arc::new(RaftSessionChannels {
                upstream: Mutex::new(Some(upstream)),
                downstream: Mutex::new(Some(downstream)),
                bypass_downstream: Mutex::new(Some(bypass_downstream)),
                connection: Mutex::new(RaftSessionConnection::Stopped { clt_recv, svr_send }),
                stream_notify: Notify::new(),
            }),
            cancel_token,
        }
    }

    async fn start_main_session(&self, client: &reqwest::Client) -> Result<(), Error> {
        if self.cancel_token.is_cancelled() {
            return Err(Error::AlreadyClosedSession(self.session_id.clone()));
        }

        let (clt_recv, svr_send, stop_ct) = self
            .channels
            .connection
            .lock()
            .await
            .take(&self.cancel_token)
            .ok_or(Error::AlreadyStartedSession(self.session_id.clone()))?;

        // let remote_event = self.parent.event_send.subscribe();
        let raft_client = self.parent.raft_client.clone();
        let remote_event_recv = self.parent.event_send.subscribe();
        let transport =
            SseTransport::start_with_client(self.upstream_url.clone(), client.clone()).await?;
        let (transport_sink, transport_stream) =
            IntoTransport::<RoleClient, _, _>::into_transport(transport);
        let returning_chan = self.channels.clone();
        let my_session_id = self.session_id.clone();
        tokio::spawn(async move {
            let stop_ct = stop_ct;
            let transport_sink = transport_sink;
            let transport_stream = transport_stream;
            tokio::pin!(transport_sink);
            tokio::pin!(transport_stream);
            let mut recv = clt_recv;
            let send = svr_send;
            let raft_client = raft_client;
            let mut remote_event_recv = remote_event_recv;
            let my_session_id = my_session_id;
            loop {
                tokio::select! {
                    msg = remote_event_recv.recv() => {
                        match msg {
                            Ok(RaftSchemaEvent::NotifyToMainSession(event)) if event.session_id == my_session_id => {
                                tracing::info!("my session event: {:?}", event);
                                match transport_sink.send(event.to_client_json_rpc_message()).await {
                                    Ok(_) => {}
                                    Err(e) => {
                                        tracing::error!("send error: {:?}", e);
                                    }
                                }
                            }
                            Ok(msg) => {
                                tracing::info!("other session event: {:?}", msg);
                            }
                            Err(e) => {
                                tracing::error!("error listening for client message: {:?}", e);
                            }
                        }
                    }
                    msg = transport_stream.next() => {
                        match msg {
                            Some(msg) => {
                                tracing::info!("to client message: {:?}", msg);
                                let event = RaftSchemaEvent::notify_to_sub_session(my_session_id.clone(), &msg);
                                match raft_client.notify(&event).await {
                                    Ok(_) => {}
                                    Err(e) => {
                                        tracing::error!("send error: {:?}", e);
                                    }
                                }
                                if send.send(msg).is_ok() {}
                            }
                            None => {
                                tracing::info!("client -> overlay -> server stream closed");
                                break;
                            }
                        }
                    }
                    msg = recv.recv() => {
                        match msg {
                            Ok(msg) => {
                                tracing::info!("to server message: {:?}", msg);
                                match transport_sink.send(msg).await {
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
                    _ = stop_ct.cancelled() => {
                        tracing::info!("overlay stream closed");
                        break;
                    }
                }
            }
            returning_chan.connection.lock().await.restore(recv, send);
        });

        Ok(())
    }

    async fn start_sub_session(&self) -> Result<(), Error> {
        if self.cancel_token.is_cancelled() {
            return Err(Error::AlreadyClosedSession(self.session_id.clone()));
        }

        let (clt_recv, svr_send, stop_ct) = self
            .channels
            .connection
            .lock()
            .await
            .take(&self.cancel_token)
            .ok_or(Error::AlreadyStartedSession(self.session_id.clone()))?;

        let session_id = self.session_id().into_owned();
        let raft_client = self.parent.raft_client.clone();
        let remote_event_recv = self.parent.event_send.subscribe();
        let returning_chan = self.channels.clone();
        tokio::spawn(async move {
            let stop_ct = stop_ct;
            let mut remote_event_recv = remote_event_recv;
            let raft_client = raft_client;
            let mut recv = clt_recv;
            let send = svr_send;
            let my_session_id = session_id.clone();
            loop {
                tokio::select! {
                    msg = remote_event_recv.recv() => {
                        match msg {
                            Ok(RaftSchemaEvent::NotifyToSubSession(event)) if event.session_id == my_session_id => {
                                tracing::info!("to client message: {:?}", event);
                                match send.send(event.to_server_json_rpc_message()) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        tracing::error!("send error: {:?}", e);
                                    }
                                }
                            }
                            Ok(_) => {}
                            Err(e) => {
                                tracing::error!("error listening for client message: {:?}", e);
                            }
                        }
                    }
                    msg = recv.recv() => {
                        tracing::info!("to main session message: {:?}", msg);
                        match msg {
                            Ok(msg) => {
                                let event = RaftSchemaEvent::notify_to_main_session(my_session_id.clone(), &msg);
                                match raft_client.notify(&event).await {
                                    Ok(_) => {
                                        tracing::info!("to main session message: {} {:?}", my_session_id, msg);
                                    }
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
                    _ = stop_ct.cancelled() => {
                        tracing::info!("overlay stream closed");
                        break;
                    }
                }
            }
            returning_chan.connection.lock().await.restore(recv, send);
        });

        Ok(())
    }
}

impl RaftSessionConnection {
    pub fn is_started(&self) -> bool {
        matches!(self, RaftSessionConnection::Started(_))
    }

    pub fn take(
        &mut self,
        ct: &CancellationToken,
    ) -> Option<(
        broadcast::Receiver<ClientJsonRpcMessage>,
        broadcast::Sender<ServerJsonRpcMessage>,
        CancellationToken,
    )> {
        if let RaftSessionConnection::Started(_) = self {
            return None;
        }

        let stop_ct = ct.child_token();
        let temp = std::mem::replace(self, RaftSessionConnection::Started(stop_ct.clone()));
        match temp {
            RaftSessionConnection::Stopped { clt_recv, svr_send } => {
                Some((clt_recv, svr_send, stop_ct))
            }
            RaftSessionConnection::Started(_) => {
                unreachable!("this should never happen")
            }
        }
    }

    pub fn restore(
        &mut self,
        clt_recv: broadcast::Receiver<ClientJsonRpcMessage>,
        svr_send: broadcast::Sender<ServerJsonRpcMessage>,
    ) {
        if let RaftSessionConnection::Stopped { .. } = self {
            return;
        }

        let old = std::mem::replace(self, RaftSessionConnection::Stopped { clt_recv, svr_send });
        match old {
            RaftSessionConnection::Stopped { .. } => {
                unreachable!("this should never happen")
            }
            RaftSessionConnection::Started(stop_ct) if !stop_ct.is_cancelled() => {
                stop_ct.cancel();
            }
            RaftSessionConnection::Started(_) => {}
        }
    }
}
