use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use anyhow::Result;
use chrono::Utc;
use tokio::sync::{Mutex, RwLock};
use url::Url;
use uuid::{ContextV7, Timestamp, Uuid};

use super::{ConnectionState, ConnectionStateCreate, ManagerTrait};

#[derive(Debug, Clone)]
pub struct LocalManager(Arc<LocalSessionManagerInner>);

#[derive(Debug)]
struct LocalSessionManagerInner {
    upstreams: RwLock<Vec<Url>>,
    current_index: AtomicUsize,
    context: Mutex<ContextV7>,
    sessions: RwLock<HashMap<String, ConnectionState>>,
}

impl LocalManager {
    pub fn new() -> Self {
        Self(Arc::new(LocalSessionManagerInner {
            upstreams: RwLock::new(vec![]),
            current_index: AtomicUsize::new(0),
            context: Mutex::new(ContextV7::new()),
            sessions: RwLock::new(HashMap::new()),
        }))
    }
}
impl ManagerTrait for LocalManager {
    async fn replace_route<U: Into<Url>, I: Iterator<Item = U>>(
        &mut self,
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

    async fn create(&mut self, data: ConnectionStateCreate) -> Result<ConnectionState> {
        let data = {
            let context = self.0.context.lock().await;
            ConnectionState::new(data, Uuid::new_v7(Timestamp::now(&*context)).to_string())
        };
        self.0
            .sessions
            .write()
            .await
            .insert(data.session_id.clone(), data.clone());
        Ok(data)
    }

    async fn get(&self, session_id: String) -> Result<Option<ConnectionState>> {
        Ok(self.0.sessions.read().await.get(&session_id).cloned())
    }

    async fn delete(&mut self, session_id: String) -> Result<()> {
        self.0.sessions.write().await.remove(&session_id);
        Ok(())
    }

    async fn patch<Patcher: FnOnce(&mut ConnectionState) -> anyhow::Result<()>>(
        &mut self,
        session_id: String,
        patcher: Patcher,
    ) -> anyhow::Result<Option<ConnectionState>> {
        let reader = self.0.sessions.write().await;
        let session = reader.get(&session_id).cloned();
        if let Some(mut session) = session {
            patcher(&mut session)?;
            session.last_accessed_at = Utc::now();
            self.0
                .sessions
                .write()
                .await
                .insert(session_id, session.clone());
            Ok(Some(session))
        } else {
            Ok(None)
        }
    }

    async fn close(&self) -> Result<()> {
        Ok(())
    }
}
