use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionState {
    pub session_id: String,
    pub upstream: Url,
    pub upstream_session_id: String,
    pub connected_at: DateTime<Utc>,
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
            connected_at: Utc::now(),
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
