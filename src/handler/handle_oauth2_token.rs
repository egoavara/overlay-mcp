use std::borrow::Cow;

use axum::{extract::State, Form, Json};
use oauth2::{basic::BasicTokenResponse, AuthorizationCode, PkceCodeVerifier, RedirectUrl};
use serde::Deserialize;

use super::{utils::AnyResult, AppState};

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
) -> AnyResult<Json<BasicTokenResponse>> {
    let redirect_url = RedirectUrl::new(query.redirect_uri)?;
    let oauth_client = state.get_oauth_client();

    let token_request = oauth_client
        .exchange_code(AuthorizationCode::new(query.code))?
        .set_pkce_verifier(PkceCodeVerifier::new(query.code_verifier.to_owned()))
        .set_redirect_uri(Cow::Borrowed(&redirect_url));

    let token_response = token_request.request_async(&state.reqwest).await?;

    Ok(Json(token_response))
}
