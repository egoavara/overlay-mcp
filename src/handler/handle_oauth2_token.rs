use std::borrow::Cow;

use anyhow::Context;
use axum::{
    extract::State,
    Extension, Form, Json,
};
use oauth2::{basic::BasicTokenResponse, AuthorizationCode, PkceCodeVerifier, RedirectUrl};
use serde::Deserialize;

use crate::{manager::auth::authenticate::Authenticater, utils::AnyError};

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
    Extension(authenticater): Extension<Authenticater>,
    Form(query): Form<TokenForm>,
) -> Result<Json<BasicTokenResponse>, AnyError> {
    let redirect_url = RedirectUrl::new(query.redirect_uri).context("redirect url")?;
    let oauth_client = authenticater.create_oauth_client();

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
