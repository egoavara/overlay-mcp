use anyhow::Result;
use enum_dispatch::enum_dispatch;
use rmcp::{
    service::{RxJsonRpcMessage, TxJsonRpcMessage},
    RoleClient,
};
use tokio::sync::broadcast;
use url::Url;

use super::{ConnectionState, LocalManager, RaftManager, SessionGuard, StreamGuard};

#[enum_dispatch]
pub trait ManagerTrait: Clone + Send + Sync {
    // route
    async fn replace_route<U: Into<Url>, I: Iterator<Item = U>>(
        &self,
        routes: I,
    ) -> Result<Vec<Url>>;
    async fn route(&self) -> Result<Url>;
    // session
    async fn reload(&self, session_id: String) -> Result<Option<ConnectionState>>;
    async fn connect(&self, client: reqwest::Client) -> Result<ConnectionState>;

    async fn take_upstream(
        &self,
        session_id: String,
    ) -> Result<StreamGuard<broadcast::Sender<TxJsonRpcMessage<RoleClient>>>>;
    async fn take_downstream(
        &self,
        session_id: String,
    ) -> Result<StreamGuard<broadcast::Receiver<RxJsonRpcMessage<RoleClient>>>>;

    async fn take_bypass_downstream(
        &self,
        session_id: String,
    ) -> Result<StreamGuard<broadcast::Sender<RxJsonRpcMessage<RoleClient>>>>;

    async fn delete(&self, session_id: String) -> Result<()>;
    async fn session_guard(&self, session_id: String) -> Result<SessionGuard>;
    // close
    async fn close(&self) -> Result<()>;
}

#[derive(Clone)]
#[enum_dispatch(ManagerTrait)]
pub enum StorageManager {
    Local(LocalManager),
    Raft(RaftManager),
}
