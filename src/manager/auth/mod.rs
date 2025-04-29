pub mod authenticate;
pub mod authorizer;
pub mod authorizer_constant;
pub mod authorizer_fga;
mod commons;
mod config;

use anyhow::Result;
use authenticate::AuthenticaterLayer;
use authorizer::AuthorizerLayer;
pub use commons::*;
pub use config::*;

pub async fn new_auth(auth: &AuthConfig) -> Result<(AuthenticaterLayer, AuthorizerLayer)> {
    let authenticater = AuthenticaterLayer::new(auth.get_authenticater()).await?;
    Ok((authenticater, AuthorizerLayer::new(auth).await?))
}
