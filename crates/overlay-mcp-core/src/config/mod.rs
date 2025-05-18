pub mod application;
pub mod auth;
pub mod otel;
pub mod reqmodifier;
pub mod server;
pub mod upstream;

use serde::{Deserialize, Serialize};

pub use application::ApplicationConfig;
pub use auth::AuthConfig;
pub use otel::OpenTelemetryConfig;
pub use reqmodifier::BaseModifiers;
pub use server::ServerConfig;
pub use upstream::UpstreamConfig;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub application: ApplicationConfig,
    pub server: ServerConfig,
    pub upstream: UpstreamConfig,
    pub auth: AuthConfig,
    pub otel: Option<OpenTelemetryConfig>,
} 