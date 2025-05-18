use std::{collections::HashMap, sync::Arc};

use overlay_mcp_core::{BaseModifiers, Error, GeneralSessionManager};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

use crate::StandaloneSession;

#[derive(Clone)]
pub struct StandaloneManager {
    inner: Arc<StandaloneManagerInner>,
}

pub(crate) struct StandaloneManagerInner {
    pub(crate) passthrough: BaseModifiers,
    pub(crate) sessions: RwLock<HashMap<String, StandaloneSession>>,
    pub(crate) cancel_token: CancellationToken,
}

impl StandaloneManager {
    pub fn new(cancel_token: CancellationToken, passthrough: BaseModifiers) -> Self {
        Self {
            inner: Arc::new(StandaloneManagerInner {
                passthrough,
                sessions: RwLock::new(HashMap::new()),
                cancel_token,
            }),
        }
    }
}

impl GeneralSessionManager for StandaloneManager {
    type Session = StandaloneSession;

    async fn create(&self, upstream_url: Url) -> Result<Self::Session, Error> {
        let session_id = Uuid::new_v4().to_string();
        let close_token = self.inner.cancel_token.child_token();
        let session = StandaloneSession::new(
            self.inner.clone(),
            session_id.clone(),
            upstream_url,
            close_token,
        );

        self.inner
            .sessions
            .write()
            .await
            .insert(session_id, session.clone());

        Ok(session)
    }

    async fn find(
        &self,
        session_id: &str,
    ) -> Result<Option<Self::Session>, overlay_mcp_core::Error> {
        let session = self.inner.sessions.read().await.get(session_id).cloned();
        if let Some(session) = session {
            Ok(Some(session))
        } else {
            Ok(None)
        }
    }
}
