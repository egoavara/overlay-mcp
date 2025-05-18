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
use tokio::{
    sync::{
        broadcast::{self, error::RecvError},
        Mutex, Notify,
    },
    time::timeout,
};

use tokio_util::sync::CancellationToken;
use url::Url;

use crate::StandaloneManagerInner;

#[derive(Clone)]
pub struct StandaloneSession {
    #[allow(dead_code)]
    pub(crate) parent: Arc<StandaloneManagerInner>,
    pub(crate) inner: Arc<StandaloneSessionInner>,
}

enum StandaloneSessionConnection {
    Stopped {
        clt_recv: broadcast::Receiver<ClientJsonRpcMessage>,
        svr_send: broadcast::Sender<ServerJsonRpcMessage>,
    },
    Started(CancellationToken),
}

pub struct StandaloneSessionInner {
    pub(crate) session_id: String,
    pub(crate) upstream_url: Url,

    pub(crate) upstream: Mutex<Option<Upstream>>,
    pub(crate) downstream: Mutex<Option<Downstream>>,
    pub(crate) bypass_downstream: Mutex<Option<BypassDownstream>>,

    connection: Arc<Mutex<StandaloneSessionConnection>>,

    pub(crate) stream_notify: Notify,
    pub(crate) cancel_token: CancellationToken,
}

impl GeneralSession for StandaloneSession {
    fn session_id(&self) -> Cow<str> {
        Cow::Borrowed(&self.inner.session_id)
    }

    fn upstream_url(&self) -> Cow<Url> {
        Cow::Borrowed(&self.inner.upstream_url)
    }

    async fn guard_upstream(&self) -> Result<StreamGuard<Upstream>, Error> {
        if self.inner.cancel_token.is_cancelled() {
            return Err(Error::AlreadyClosedSession(self.inner.session_id.clone()));
        }

        let upstream = timeout(Duration::from_secs(5), async {
            loop {
                if let Some(upstream) = self.inner.upstream.lock().await.take() {
                    return upstream;
                }
                self.inner.stream_notify.notified().await;
            }
        })
        .await
        .map_err(|_| FatalError::Timeout)?;

        let (guard, notify) = StreamGuard::new(upstream);
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let upstream = notify
                .await
                .expect("guard must be notified, but not. logic error");
            tracing::info!(session_id = inner.session_id, "upstream returned by guard");
            inner.upstream.lock().await.replace(upstream);
        });
        Ok(guard)
    }

    async fn guard_downstream(&self) -> Result<StreamGuard<Downstream>, Error> {
        if self.inner.cancel_token.is_cancelled() {
            return Err(Error::AlreadyClosedSession(self.inner.session_id.clone()));
        }

        let downstream = timeout(Duration::from_secs(5), async {
            loop {
                if let Some(downstream) = self.inner.downstream.lock().await.take() {
                    return downstream;
                }
            }
        })
        .await
        .map_err(|_| FatalError::Timeout)?;

        let (guard, notify) = StreamGuard::new(downstream);
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let downstream = notify
                .await
                .expect("guard must be notified, but not. logic error");
            tracing::info!(
                session_id = inner.session_id,
                "downstream returned by guard"
            );
            inner.downstream.lock().await.replace(downstream);
        });
        Ok(guard)
    }

    async fn guard_bypass_downstream(&self) -> Result<StreamGuard<BypassDownstream>, Error> {
        if self.inner.cancel_token.is_cancelled() {
            return Err(Error::AlreadyClosedSession(self.inner.session_id.clone()));
        }

        let bypass_downstream = timeout(Duration::from_secs(5), async {
            loop {
                if let Some(bypass_downstream) = self.inner.bypass_downstream.lock().await.take() {
                    return bypass_downstream;
                }
            }
        })
        .await
        .map_err(|_| FatalError::Timeout)?;

        let (guard, notify) = StreamGuard::new(bypass_downstream);
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let bypass_downstream = notify
                .await
                .expect("guard must be notified, but not. logic error");
            tracing::info!(
                session_id = inner.session_id,
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
        if self.inner.cancel_token.is_cancelled() {
            return Err(Error::AlreadyClosedSession(self.inner.session_id.clone()));
        }
        let (guard, notify) = SessionGuard::new(self.inner.session_id.clone());
        let ct = self.inner.cancel_token.clone();
        tokio::spawn(async move {
            let session_id = notify
                .await
                .expect("guard must be notified, but not. logic error");
            ct.cancel();
            tracing::info!(session_id, "session closed by guard");
        });

        Ok(guard)
    }

    async fn is_started(&self) -> bool {
        if self.inner.cancel_token.is_cancelled() {
            return false;
        }
        self.inner.connection.lock().await.is_started()
    }

    async fn start(&self, original_request: &http::request::Parts) -> Result<(), Error> {
        if self.inner.cancel_token.is_cancelled() {
            return Err(Error::AlreadyClosedSession(self.inner.session_id.clone()));
        }

        let (clt_recv, svr_send, stop_ct) = self
            .inner
            .connection
            .lock()
            .await
            .take(&self.inner.cancel_token)
            .ok_or(Error::AlreadyStartedSession(self.inner.session_id.clone()))?;

        let transport = SseTransport::start_with_client(
            self.inner.upstream_url.clone(),
            reqwest::Client::new(),
        )
        .await?;
        let (transport_sink, transport_stream) =
            IntoTransport::<RoleClient, _, _>::into_transport(transport);
        let returning_chan = self.inner.connection.clone();
        tokio::spawn(async move {
            let stop_ct = stop_ct;
            let transport_sink = transport_sink;
            let transport_stream = transport_stream;
            tokio::pin!(transport_sink);
            tokio::pin!(transport_stream);
            let mut recv = clt_recv;
            let send = svr_send;
            loop {
                tokio::select! {
                    msg = transport_stream.next() => {
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
            returning_chan.lock().await.restore(recv, send);
        });

        Ok(())
    }

    async fn stop(&self) -> Result<(), Error> {
        if self.inner.cancel_token.is_cancelled() {
            return Err(Error::AlreadyClosedSession(self.inner.session_id.clone()));
        }

        match &*self.inner.connection.lock().await {
            StandaloneSessionConnection::Stopped { .. } => {
                return Err(Error::AlreadyStoppedSession(self.inner.session_id.clone()));
            }
            StandaloneSessionConnection::Started(stop_ct) => {
                stop_ct.cancel();
            }
        }
        let timeout = timeout(Duration::from_secs(5), async {
            loop {
                self.inner.stream_notify.notified().await;

                match &*self.inner.connection.lock().await {
                    StandaloneSessionConnection::Stopped { .. } => {
                        break;
                    }
                    StandaloneSessionConnection::Started(_) => continue,
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
        if self.inner.cancel_token.is_cancelled() {
            return Err(Error::AlreadyClosedSession(self.inner.session_id.clone()));
        }

        self.inner.cancel_token.cancel();
        // remove session from parent handled by cancel_token
        Ok(())
    }
}

impl StandaloneSession {
    pub(crate) fn new(
        parent: Arc<StandaloneManagerInner>,
        session_id: String,
        upstream_url: Url,
        cancel_token: CancellationToken,
    ) -> Self {
        let (clt_send, clt_recv) = broadcast::channel::<ClientJsonRpcMessage>(16);
        let (svr_send, svr_recv) = broadcast::channel::<ServerJsonRpcMessage>(16);

        let upstream = Upstream(clt_send);
        let downstream = Downstream(svr_recv);
        let bypass_downstream = BypassDownstream(svr_send.clone());

        // resource cleanup
        let stop_parent = parent.clone();
        let stop_ct = cancel_token.clone();
        let stop_session_id = session_id.clone();
        tokio::spawn(async move {
            stop_ct.cancelled().await;
            stop_parent.sessions.write().await.remove(&stop_session_id);
        });

        StandaloneSession {
            parent,
            inner: Arc::new(StandaloneSessionInner {
                session_id,
                upstream_url,
                upstream: Mutex::new(Some(upstream)),
                downstream: Mutex::new(Some(downstream)),
                bypass_downstream: Mutex::new(Some(bypass_downstream)),
                connection: Arc::new(Mutex::new(StandaloneSessionConnection::Stopped {
                    clt_recv,
                    svr_send,
                })),
                stream_notify: Notify::new(),
                cancel_token,
            }),
        }
    }
}
impl StandaloneSessionConnection {
    pub fn is_started(&self) -> bool {
        matches!(self, StandaloneSessionConnection::Started(_))
    }

    pub fn take(
        &mut self,
        ct: &CancellationToken,
    ) -> Option<(
        broadcast::Receiver<ClientJsonRpcMessage>,
        broadcast::Sender<ServerJsonRpcMessage>,
        CancellationToken,
    )> {
        if let StandaloneSessionConnection::Started(_) = self {
            return None;
        }

        let stop_ct = ct.child_token();
        let temp = std::mem::replace(self, StandaloneSessionConnection::Started(stop_ct.clone()));
        match temp {
            StandaloneSessionConnection::Stopped { clt_recv, svr_send } => {
                Some((clt_recv, svr_send, stop_ct))
            }
            StandaloneSessionConnection::Started(_) => {
                unreachable!("this should never happen")
            }
        }
    }

    pub fn restore(
        &mut self,
        clt_recv: broadcast::Receiver<ClientJsonRpcMessage>,
        svr_send: broadcast::Sender<ServerJsonRpcMessage>,
    ) {
        if let StandaloneSessionConnection::Stopped { .. } = self {
            return;
        }

        let old = std::mem::replace(
            self,
            StandaloneSessionConnection::Stopped { clt_recv, svr_send },
        );
        match old {
            StandaloneSessionConnection::Stopped { .. } => {
                unreachable!("this should never happen")
            }
            StandaloneSessionConnection::Started(stop_ct) if !stop_ct.is_cancelled() => {
                stop_ct.cancel();
            }
            StandaloneSessionConnection::Started(_) => {}
        }
    }
}
