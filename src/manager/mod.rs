mod local_session;
mod raft_session;

use anyhow::Result;
use chrono::{DateTime, Utc};
use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};
use url::Url;

pub use local_session::*;
pub use raft_session::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionState {
    pub session_id: String,
    pub upstream: Url,
    pub upstream_session_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStateCreate {
    pub upstream: Url,
    pub upstream_session_id: String,
}

impl ConnectionState {
    pub fn new(create: ConnectionStateCreate, session_id: String) -> Self {
        Self {
            session_id,
            upstream: create.upstream,
            upstream_session_id: create.upstream_session_id,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_accessed_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStateUpdate {
    pub upstream: Option<Url>,
    pub upstream_session_id: Option<String>,
    pub last_accessed_at: Option<DateTime<Utc>>,
}

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
pub enum Manager {
    Local(local_session::LocalManager),
    Raft(raft_session::RaftManager),
}
