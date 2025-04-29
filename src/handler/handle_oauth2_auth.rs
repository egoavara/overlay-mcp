use anyhow::Context;
use axum::{
    extract::Query,
    response::{IntoResponse, Redirect},
    Extension,
};
use oauth2::{CsrfToken, PkceCodeChallenge, RedirectUrl, Scope};
use serde::Deserialize;

use crate::{manager::auth::authenticate::Authenticater, utils::AnyError};


#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub(crate) struct AuthParams {
    response_type: String,
    client_id: String,
    code_challenge: String,
    code_challenge_method: String,
    redirect_uri: String,
}

pub(crate) async fn handler(
    Extension(authenticater): Extension<Authenticater>,
    Query(query): Query<AuthParams>,
) -> Result<impl IntoResponse, AnyError> {
    let code_challenge = serde_json::from_value::<PkceCodeChallenge>(serde_json::json!(
        {
            "code_challenge": query.code_challenge,
            "code_challenge_method": query.code_challenge_method,
        }
    ))
    .context("code challenge")?;
    let oauth_client = authenticater
        .create_oauth_client()
        .set_redirect_uri(RedirectUrl::new(query.redirect_uri).context("redirect url")?);

    let mut auth_request = oauth_client
        .authorize_url(CsrfToken::new_random)
        .set_pkce_challenge(code_challenge);

    for scope in &authenticater.client_config.scopes {
        auth_request = auth_request.add_scope(Scope::new(scope.clone()));
    }

    let (auth_url, _csrf_token) = auth_request.url();

    Ok(Redirect::to(auth_url.as_str()))
}
