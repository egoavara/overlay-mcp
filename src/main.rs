mod command;
mod config;
mod handler;
mod middleware;

use anyhow::{Context, Result};
use axum::Router;
use clap::Parser;
use command::Command;
use config::Config;
use handler::AppState;
use http::Method;
use middleware::JwtMiddlewareState;
use openidconnect::core::CoreProviderMetadata;
use openidconnect::IssuerUrl;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::DefaultOnResponse;
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, TraceLayer};
use tower_http::LatencyUnit;
use tracing::level_filters::LevelFilter;
use tracing::Level;
use tracing_subscriber::EnvFilter;

// figment 및 Config 추가
use figment::{
    providers::{Format, Json as FigmentJson},
    Figment,
};

#[tokio::main]
async fn main() -> Result<()> {
    // dotenv 파일을 이용한 환경변수 주입
    let _ = dotenvy::dotenv();
    let cli: Command = Command::parse();
    let configfile = cli.configfile.clone();

    // 설정 로드 (Figment 사용)
    let mut config_loader: Figment = Figment::new();
    if let Some(configfile) = &cli.configfile {
        config_loader = config_loader.merge(FigmentJson::file(configfile));
    }
    let config: Config = config_loader
        .merge(cli)
        .extract()
        .context("Failed to load configuration")?;

    // 로깅 필터 설정
    let env_filter = config
        .application
        .as_ref()
        .and_then(|app| app.log_filter.clone())
        .unwrap_or("warn".to_string())
        .parse::<EnvFilter>()
        .unwrap();

    tracing_subscriber::fmt().with_env_filter(env_filter).init();
    tracing::info!("{}", serde_json::to_string_pretty(&config).unwrap());

    // 상태 설정

    let config = Arc::new(config);

    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .expect("Client should build");

    // OIDC Discovery 수행 (config 사용)
    let issuer_url = IssuerUrl::new(config.oidc.issuer.clone())?;
    let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, &http_client).await?;
    // Ensure token endpoint exists before creating MCPAuthClient
    if provider_metadata.token_endpoint().is_none() {
        return Err(anyhow::anyhow!("Token endpoint not found in OIDC metadata"));
    }

    tracing::info!("OIDC Discovery 완료: URL={}", config.oidc.issuer);

    // 애플리케이션 상태 설정 (config 사용)
    let state = AppState {
        jwt_middleware: JwtMiddlewareState::load(provider_metadata, &config, &http_client).await?,
        config: config.clone(),
        reqwest: http_client,
        configfile: Arc::new(configfile),
    };

    if let Some(configfile) = &*state.configfile {
        tracing::info!("file loaded from {}", configfile.display());
    }

    // 라우터 설정
    let app = Router::new()
        .merge(handler::router())
        .layer(
            ServiceBuilder::new()
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(DefaultMakeSpan::new())
                        .on_response(
                            DefaultOnResponse::new()
                                .level(Level::INFO)
                                .latency_unit(LatencyUnit::Seconds)
                                .include_headers(true),
                        )
                        .on_request(DefaultOnRequest::new().level(Level::INFO)),
                )
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
                ),
        )
        .with_state(state);

    // 서버 주소 설정 (config 사용)
    tracing::info!("서버가 시작되었습니다: {}", config.server.addr);

    // 서버 실행
    let listener = tokio::net::TcpListener::bind(config.server.addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
