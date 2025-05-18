use axum_client_ip::ClientIpSource;
use serde::{Deserialize, Serialize};

use super::reqmodifier::BaseModifiers;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApplicationConfig {
    pub log_filter: Option<String>,
    pub ip_extract: Option<ClientIpSource>,
    pub prometheus: bool,
    pub health_check: bool,
    pub passthrough: BaseModifiers,
} 