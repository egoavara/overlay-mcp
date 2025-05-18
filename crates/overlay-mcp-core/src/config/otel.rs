use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenTelemetryConfig {
    pub endpoint: String,
} 