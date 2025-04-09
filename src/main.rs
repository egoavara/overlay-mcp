mod mcp;
mod oauth2_wellknown;

use anyhow::{Context, Result};
use axum::{
    body::Body,
    debug_handler,
    extract::{Query, State},
    http::{Request, Response, StatusCode, Uri},
    response::{IntoResponse, Redirect},
    routing::{get, post, put},
    Form, Json, Router,
};
use chrono::{Duration, Utc};
use clap::Parser;
use http::{HeaderName, Method};
use hyper_util::{
    client::legacy::connect::HttpConnector,
    rt::{TokioExecutor, TokioIo},
};
use oauth2::{
    basic::{BasicTokenResponse, BasicTokenType},
    EmptyExtraTokenFields, PkceCodeChallenge, PkceCodeVerifier, StandardTokenResponse,
};
use openidconnect::core::CoreProviderMetadata;
use openidconnect::IssuerUrl;
use rand::{distr::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{fmt::format, net::SocketAddr, str::FromStr};
use tokio::net::TcpStream;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// 자기 자산의 호스트명
    #[arg(long)]
    host: String,

    /// 서버 포트
    #[arg(long, default_value = "9090")]
    port: u16,

    /// 프록시 대상 주소
    #[arg(long)]
    proxy: String,

    /// OpenID Connect Issuer URL (e.g., https://accounts.google.com)
    #[arg(short, long)]
    openid_connect: String,

    /// OAuth2 Client ID
    #[arg(long)]
    client_id: String,

    /// OAuth2 Client Secret
    #[arg(long)]
    client_secret: String,

    /// OAuth2 Scopes (can be specified multiple times)
    #[arg(long, required = true)] // Require at least one scope
    scope: Vec<String>,
}

#[derive(Clone)]
struct AppState {
    args: Arc<Args>,
    auth_client: Arc<mcp::MCPAuthClient>,
    upstream_uri: Uri,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 로깅 초기화
    tracing_subscriber::fmt::init();

    // 커맨드 라인 인자 파싱
    let args = Args::parse();
    let args = Arc::new(args);

    let http_client = reqwest::ClientBuilder::new()
        // Following redirects opens the client up to SSRF vulnerabilities.
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build");

    // OIDC Discovery 수행
    let issuer_url = IssuerUrl::new(args.openid_connect.clone())?;
    let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, &http_client).await?;
    // Ensure token endpoint exists before creating MCPAuthClient
    if provider_metadata.token_endpoint().is_none() {
        return Err(anyhow::anyhow!("Token endpoint not found in OIDC metadata"));
    }

    tracing::info!("OIDC Discovery 완료: URL={}", args.openid_connect);

    // MCP 인증 클라이언트 초기화 (CoreProviderMetadata 객체 전달)
    let auth_client = Arc::new(
        mcp::MCPAuthClient::new(
            provider_metadata, // Pass the whole metadata object
            args.client_id.clone(),
            args.client_secret.clone(),
            args.scope.clone(),
        )
        .context("Failed to create MCPAuthClient")?,
    );

    // 애플리케이션 상태 설정
    let state = AppState {
        auth_client,
        args: args.clone(),
        upstream_uri: Uri::from_str(&args.proxy).unwrap(),
    };

    // 라우터 설정
    let app = Router::new()
        .route(
            "/.well-known/oauth-authorization-server",
            get(handle_well_known),
        )
        .route("/sse", get(handle_sse))
        .route("/oauth2/callback", get(handle_callback))
        .route("/oauth2/client/register", post(handle_client_register))
        .route("/oauth2/auth", get(handle_auth))
        .route("/oauth2/token", post(handle_token))
        .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()))
        .layer(
            CorsLayer::new()
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::DELETE,
                    Method::OPTIONS,
                ])
                .allow_origin(Any),
        )
        .with_state(state);

    // 서버 주소 설정
    let addr = SocketAddr::from(([0, 0, 0, 0], args.port));
    tracing::info!("서버가 시작되었습니다: {}", addr);

    // 서버 실행
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

struct AnyError(anyhow::Error);

impl IntoResponse for AnyError {
    fn into_response(self) -> Response<axum::body::Body> {
        (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()).into_response()
    }
}

impl From<anyhow::Error> for AnyError {
    fn from(error: anyhow::Error) -> Self {
        AnyError(error)
    }
}

async fn handle_sse(
    state: State<AppState>,
    req: Request<Body>,
) -> Result<Response<Body>, AnyError> {
    let (mut parts, body) = req.into_parts();
    let _ = match state.auth_client.handle_mcp_auth(&parts).await {
        Ok(token) => token,
        Err(response) => return Ok(response),
    };
    let mut target_part = state.upstream_uri.clone().into_parts();
    target_part.path_and_query = parts.uri.path_and_query().cloned();
    parts.uri = Uri::from_parts(target_part).unwrap();

    let req = Request::from_parts(parts, body);
    let client = hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
        .build(HttpConnector::new());
    Ok(client
        .request(req)
        .await
        .context("failed to request")?
        .into_response())
}

async fn handle_callback(
    State(state): State<AppState>,
    req: Request<axum::body::Body>,
) -> impl IntoResponse {
    // TODO: 콜백 처리 로직 구현 (e.g., exchange code for token)
    // Example: Extract code and state from query parameters
    // let query = req.uri().query().unwrap_or("");
    // let params: HashMap<String, String> = url::form_urlencoded::parse(query.as_bytes())
    //     .into_owned()
    //     .collect();
    // let code = params.get("code");
    // let state_param = params.get("state"); // Remember to verify the state
    // let pkce_verifier = ...; // Need to retrieve the PKCE verifier associated with the state

    // if let (Some(code), Some(verifier)) = (code, pkce_verifier) {
    //     let redirect_uri = ...; // Construct redirect_uri again or retrieve from state
    //     match state.auth_client.exchange_code(code, verifier, redirect_uri).await {
    //         Ok(token_response) => {
    //             // Store the token, perhaps set a session cookie
    //             tracing::info!("Token acquired: {:?}", token_response.access_token());
    //             // Redirect user back to original destination or show success page
    //         }
    //         Err(e) => {
    //             tracing::error!("Failed to exchange code: {}", e);
    //             // Show error page
    //         }
    //     }
    // }

    Response::builder()
        .status(StatusCode::OK)
        .body(axum::body::Body::from("콜백 처리됨 (구현 필요)"))
        .unwrap()
}

#[derive(Debug, Deserialize)]
struct AuthParams {
    response_type: String,
    client_id: String,
    code_challenge: String,
    code_challenge_method: String,
    redirect_uri: String,
}

async fn handle_auth(
    State(state): State<AppState>,
    Query(query): Query<AuthParams>,
    req: Request<axum::body::Body>,
) -> Result<impl IntoResponse, AnyError> {
    let code_challenge = serde_json::from_value::<PkceCodeChallenge>(serde_json::json!(
        {
            "code_challenge": query.code_challenge,
            "code_challenge_method": query.code_challenge_method,
        }
    ))
    .unwrap();
    let auth_url = state
        .auth_client
        .get_auth_url(code_challenge, query.redirect_uri)
        .context("Failed to get auth url")?;
    Ok(Redirect::to(&auth_url))
}

#[derive(Debug, Deserialize)]
struct TokenForm {
    grant_type: String,
    client_id: String,
    code: String,
    code_verifier: String,
    redirect_uri: String,
}

async fn handle_token(
    State(state): State<AppState>,
    Form(query): Form<TokenForm>,
) -> Result<Json<BasicTokenResponse>, AnyError> {
    let client = reqwest::Client::new();

    let token_response = state
        .auth_client
        .exchange_code(
            &client,
            &query.code,
            PkceCodeVerifier::new(query.code_verifier.to_owned()),
            query.redirect_uri,
        )
        .await
        .map_err(|x| {
            tracing::error!("Failed to exchange code: {}", x);
            x
        })
        .context("Failed to exchange code")?;

    Ok(Json(token_response))
}

#[derive(Debug, Deserialize)]
struct ClientRegisterRequest {
    redirect_uris: Vec<String>,
    token_endpoint_auth_method: String,
    grant_types: Vec<String>,
    response_types: Vec<String>,
    client_name: String,
    client_uri: String,
}

#[derive(Debug, Serialize)]
struct ClientRegisterResponse {
    client_id: String,
    client_secret: String,
    redirect_uris: Vec<String>,
    client_id_issued_at: i64,
    client_secret_expires_at: i64,
}

async fn handle_client_register(
    State(_): State<AppState>,
    Json(value): Json<ClientRegisterRequest>,
) -> Json<ClientRegisterResponse> {
    let raw_client_secret = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .collect::<Vec<_>>();
    let client_secret = String::from_utf8_lossy(&raw_client_secret).to_string();
    tracing::info!("Client register: {:?}", value);

    let now = Utc::now();
    let issued_at = now.timestamp();
    let expires_at = (now + Duration::hours(1)).timestamp();

    Json(ClientRegisterResponse {
        client_id: uuid::Uuid::new_v4().to_string(),
        client_secret,
        redirect_uris: value.redirect_uris,
        client_id_issued_at: issued_at,
        client_secret_expires_at: expires_at,
    })
}

async fn handle_well_known(State(state): State<AppState>) -> impl IntoResponse {
    Json(oauth2_wellknown::WellKnownResponse {
        issuer: state.auth_client.provider_metadata.issuer().to_string(),
        authorization_endpoint: format!("{}/oauth2/auth", state.args.host),
        token_endpoint: format!("{}/oauth2/token", state.args.host),
        response_types_supported: vec!["code".to_string()],
        code_challenge_methods_supported: vec!["S256".to_string()],
        token_endpoint_auth_methods_supported: vec!["client_secret_post".to_string()],
        grant_types_supported: vec![
            "authorization_code".to_string(),
            "refresh_token".to_string(),
        ],
        registration_endpoint: format!("{}/oauth2/client/register", state.args.host),
    })
}
