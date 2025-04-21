use anyhow::Result;
use enum_dispatch::enum_dispatch;
use url::Url;

use super::{ConnectionState, ConnectionStateCreate, ConnectionStateUpdate, LocalManager, RaftManager, SessionGuard};

#[enum_dispatch]
pub trait ManagerTrait {
    async fn replace_route<U: Into<Url>, I: Iterator<Item = U>>(
        &mut self,
        routes: I,
    ) -> Result<Vec<Url>>;
    async fn route(&self) -> Result<Url>;
    async fn create(&mut self, data: ConnectionStateCreate) -> Result<ConnectionState>;
    async fn get(&self, session_id: String) -> Result<Option<ConnectionState>>;
    async fn delete(&mut self, session_id: String) -> Result<()>;
    async fn update(
        &mut self,
        session_id: String,
        data: ConnectionStateUpdate,
    ) -> Result<Option<ConnectionState>> {
        self.patch(session_id, |session_data| {
            if let Some(upstream) = data.upstream {
                session_data.upstream = upstream;
            }
            if let Some(upstream_session_id) = data.upstream_session_id {
                session_data.upstream_session_id = upstream_session_id;
            }
            if let Some(last_accessed_at) = data.last_accessed_at {
                session_data.last_accessed_at = last_accessed_at;
            }
            Ok(())
        })
        .await
    }
    async fn patch<Patcher: FnOnce(&mut ConnectionState) -> Result<()>>(
        &mut self,
        session_id: String,
        patcher: Patcher,
    ) -> Result<Option<ConnectionState>>;

    async fn close(&self) -> Result<()>;
}

#[derive(Clone)]
#[enum_dispatch(ManagerTrait)]
pub enum StorageManager {
    Local(LocalManager),
    Raft(RaftManager),
}

impl StorageManager {
    pub fn guard(&self, session_id: String) -> SessionGuard {
        SessionGuard(session_id, self.clone())
    }
}
