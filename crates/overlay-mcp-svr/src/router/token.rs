use std::borrow::Cow;

use axum::{Extension, Form, Json};
use oauth2::{basic::BasicTokenResponse, AuthorizationCode, PkceCodeVerifier, RedirectUrl};
use overlay_mcp_auth::Authn;
use overlay_mcp_core::{Error, FatalError, GeneralAuthn};
use serde::Deserialize;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Body {
    grant_type: String,
    client_id: String,
    code: String,
    code_verifier: String,
    redirect_uri: String,
}

pub type Response = BasicTokenResponse;

pub async fn handler(
    Extension(client): Extension<reqwest::Client>,
    Extension(authn): Extension<Authn>,
    Form(query): Form<Body>,
) -> Result<Json<Response>, Error> {
    let redirect_url = RedirectUrl::new(query.redirect_uri).expect("redirect url");
    let oauth_client = authn.create_oauth_client();

    let token_request = oauth_client
        .exchange_code(AuthorizationCode::new(query.code))?
        .set_pkce_verifier(PkceCodeVerifier::new(query.code_verifier.to_owned()))
        .set_redirect_uri(Cow::Borrowed(&redirect_url));

    let token_response = token_request.request_async(&client).await.map_err(|err| {
        tracing::error!(error = ?err, "token request error");
        FatalError::TokenRequestError
    })?;

    Ok(Json(token_response))
}
