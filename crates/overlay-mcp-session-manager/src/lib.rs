use overlay_mcp_core::{
    server::ClusterConfig, BypassDownstream, Config, Downstream, Error, GeneralSession,
    GeneralSessionManager, SessionGuard, StreamGuard, Upstream,
};
use overlay_mcp_raft::{RaftManager, RaftSession};
use overlay_mcp_standalone::{StandaloneManager, StandaloneSession};
use tokio_util::sync::CancellationToken;
use url::Url;

#[derive(Clone)]
pub enum SessionManager {
    Standalone(StandaloneManager),
    Raft(RaftManager),
}

#[derive(Clone)]
pub enum Session {
    Standalone(StandaloneSession),
    Raft(RaftSession),
}

impl SessionManager {
    pub async fn new(cancel_token: CancellationToken, config: &Config) -> Result<Self, Error> {
        match &config.server.cluster {
            ClusterConfig::None => Ok(Self::Standalone(StandaloneManager::new(
                cancel_token,
                config.application.passthrough.clone(),
            ))),
            ClusterConfig::Raft(raft_config) => {
                let raft_manager = RaftManager::new(
                    cancel_token,
                    raft_config,
                    config.application.passthrough.clone(),
                )
                .await?;
                Ok(Self::Raft(raft_manager))
            }
        }
    }
}

impl GeneralSessionManager for SessionManager {
    type Session = Session;

    async fn create(&self, upstream_url: Url) -> Result<Self::Session, Error> {
        match self {
            Self::Standalone(standalone_manager) => standalone_manager
                .create(upstream_url)
                .await
                .map(Session::Standalone),
            Self::Raft(raft_manager) => raft_manager.create(upstream_url).await.map(Session::Raft),
        }
    }

    async fn find(&self, session_id: &str) -> Result<Option<Self::Session>, Error> {
        match self {
            Self::Standalone(standalone_manager) => standalone_manager
                .find(session_id)
                .await
                .map(|session| session.map(Session::Standalone)),
            Self::Raft(raft_manager) => raft_manager
                .find(session_id)
                .await
                .map(|session| session.map(Session::Raft)),
        }
    }
}

impl GeneralSession for Session {
    fn session_id(&self) -> std::borrow::Cow<str> {
        match self {
            Self::Standalone(session) => session.session_id(),
            Self::Raft(session) => session.session_id(),
        }
    }

    fn upstream_url(&self) -> std::borrow::Cow<Url> {
        match self {
            Self::Standalone(session) => session.upstream_url(),
            Self::Raft(session) => session.upstream_url(),
        }
    }

    async fn guard_upstream(&self) -> Result<StreamGuard<Upstream>, Error> {
        match self {
            Self::Standalone(session) => session.guard_upstream().await,
            Self::Raft(session) => session.guard_upstream().await,
        }
    }

    async fn guard_downstream(&self) -> Result<StreamGuard<Downstream>, Error> {
        match self {
            Self::Standalone(session) => session.guard_downstream().await,
            Self::Raft(session) => session.guard_downstream().await,
        }
    }

    async fn guard_bypass_downstream(&self) -> Result<StreamGuard<BypassDownstream>, Error> {
        match self {
            Self::Standalone(session) => session.guard_bypass_downstream().await,
            Self::Raft(session) => session.guard_bypass_downstream().await,
        }
    }

    async fn guard_close(&self) -> Result<SessionGuard, Error> {
        match self {
            Self::Standalone(session) => session.guard_close().await,
            Self::Raft(session) => session.guard_close().await,
        }
    }

    async fn is_started(&self) -> bool {
        match self {
            Self::Standalone(session) => session.is_started().await,
            Self::Raft(session) => session.is_started().await,
        }
    }

    async fn start(&self, original_request: &http::request::Parts) -> Result<(), Error> {
        match self {
            Self::Standalone(session) => session.start(original_request).await,
            Self::Raft(session) => session.start(original_request).await,
        }
    }

    async fn stop(&self) -> Result<(), Error> {
        match self {
            Self::Standalone(session) => session.stop().await,
            Self::Raft(session) => session.stop().await,
        }
    }

    async fn close(&self) -> Result<(), Error> {
        match self {
            Self::Standalone(session) => session.close().await,
            Self::Raft(session) => session.close().await,
        }
    }
}
