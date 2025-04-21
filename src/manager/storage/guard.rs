use super::{ManagerTrait, StorageManager};

pub struct SessionGuard(pub String, pub StorageManager);

impl SessionGuard {
    pub fn session_id(&self) -> &str {
        &self.0
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        let session_id = self.0.clone();
        let mut manager = self.1.clone();
        tokio::spawn(async move {
            match manager.delete(session_id.clone()).await {
                Ok(_) => {
                    tracing::info!(session_id = session_id, "session deleted");
                }
                Err(err) => {
                    tracing::error!(session_id=session_id, error=?err, "failed to delete session");
                }
            };
        });
    }
}
