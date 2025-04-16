use axum::{
    extract::{FromRef, Query, State},
    response::{IntoResponse, Redirect},
};
use oauth2::{CsrfToken, PkceCodeChallenge, RedirectUrl, Scope};
use serde::Deserialize;

use crate::middleware::JwtMiddlewareState;

use super::{utils::AnyResult, AppState};

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
    State(state): State<AppState>,
    Query(query): Query<AuthParams>,
) -> AnyResult<impl IntoResponse> {
    let code_challenge = serde_json::from_value::<PkceCodeChallenge>(serde_json::json!(
        {
            "code_challenge": query.code_challenge,
            "code_challenge_method": query.code_challenge_method,
        }
    ))?;
    let jwt_middleware = JwtMiddlewareState::from_ref(&state);
    let oauth_client = jwt_middleware
        .oauth_client
        .clone()
        .set_redirect_uri(RedirectUrl::new(query.redirect_uri)?);

    let mut auth_request = oauth_client
        .authorize_url(CsrfToken::new_random)
        .set_pkce_challenge(code_challenge);

    for scope in &jwt_middleware.client_config.scopes {
        auth_request = auth_request.add_scope(Scope::new(scope.clone()));
    }

    let (auth_url, _csrf_token) = auth_request.url();

    Ok(Redirect::to(auth_url.as_str()))
}
