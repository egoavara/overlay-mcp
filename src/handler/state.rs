use std::sync::Arc;

use axum::extract::FromRef;
use reqwest::Client;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub(crate) reqwest: Client,
    pub(crate) config: Arc<Config>,
}

impl FromRef<AppState> for Client {
    fn from_ref(input: &AppState) -> Self {
        input.reqwest.clone()
    }
}
