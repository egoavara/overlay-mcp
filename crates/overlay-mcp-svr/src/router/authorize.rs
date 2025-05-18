use axum::{
    extract::Query,
    response::{IntoResponse, Redirect},
    Extension,
};
use oauth2::{CsrfToken, PkceCodeChallenge, RedirectUrl};
use overlay_mcp_auth::Authn;
use overlay_mcp_core::{Error, Error400, GeneralAuthn};
use serde::Deserialize;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Params {
    response_type: String,
    client_id: String,
    code_challenge: String,
    code_challenge_method: String,
    redirect_uri: String,
}

pub async fn handler(
    Extension(authenticater): Extension<Authn>,
    Query(query): Query<Params>,
) -> Result<impl IntoResponse, Error> {
    let code_challenge = serde_json::from_value::<PkceCodeChallenge>(serde_json::json!(
        {
            "code_challenge": query.code_challenge,
            "code_challenge_method": query.code_challenge_method,
        }
    ))
    .expect("must be success");
    let redirect_uri =
        RedirectUrl::new(query.redirect_uri).map_err(Error400::InvalidUrl)?;

    let (auth_url, _csrf_token) = authenticater
        .create_oauth_client()
        .set_redirect_uri(redirect_uri)
        .authorize_url(CsrfToken::new_random)
        .set_pkce_challenge(code_challenge)
        .add_scopes(authenticater.scopes())
        .url();

    Ok(Redirect::to(auth_url.as_str()))
}
