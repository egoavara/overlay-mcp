use std::borrow::Cow;

use anyhow::Context;
use axum::{
    extract::{FromRef, State},
    Form, Json,
};
use oauth2::{basic::BasicTokenResponse, AuthorizationCode, PkceCodeVerifier, RedirectUrl};
use serde::Deserialize;

use crate::{middleware::JwtMiddlewareState, utils::AnyError};

use super::AppState;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct TokenForm {
    grant_type: String,
    client_id: String,
    code: String,
    code_verifier: String,
    redirect_uri: String,
}

pub(crate) async fn handler(
    State(state): State<AppState>,
    Form(query): Form<TokenForm>,
) -> Result<Json<BasicTokenResponse>, AnyError> {
    let redirect_url = RedirectUrl::new(query.redirect_uri).context("redirect url")?;
    let jwt_middleware = JwtMiddlewareState::from_ref(&state);
    let oauth_client = jwt_middleware.oauth_client.clone();

    let token_request = oauth_client
        .exchange_code(AuthorizationCode::new(query.code))
        .context("token request")?
        .set_pkce_verifier(PkceCodeVerifier::new(query.code_verifier.to_owned()))
        .set_redirect_uri(Cow::Borrowed(&redirect_url));

    let token_response = token_request
        .request_async(&state.reqwest)
        .await
        .context("token response")?;

    Ok(Json(token_response))
}
