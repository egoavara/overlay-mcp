mod authorizer;
mod command;
mod config;
mod config_loader;
mod fga;
mod handler;
mod middleware;
mod utils;

use anyhow::{Context, Result};
use authorizer::AuthorizerEngine;
use axum::routing::get;
use axum_client_ip::ClientIpSource;
use axum_health::Health;
use axum_prometheus::PrometheusMetricLayer;
use clap::Parser;
use command::Command;
use config::Config;
use handler::AppState;
use middleware::{trace_layer, ApikeyExtractorState, JwtMiddlewareState};
use std::{net::SocketAddr, sync::Arc};
use tower_http::cors::CorsLayer;
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
    if let Some(configfile) = &configfile {
        config_loader = config_loader.merge(FigmentJson::file(configfile));
    }
    let config: Config = config_loader
        .merge(cli)
        .extract()
        .context("Failed to load configuration")?;

    // 로깅 필터 설정
    let env_filter = config
        .application
        .log_filter
        .as_ref()
        .cloned()
        .unwrap_or("info".to_string())
        .parse::<EnvFilter>()
        .unwrap();

    tracing_subscriber::fmt()
        .pretty()
        .with_env_filter(env_filter)
        .init();
    tracing::info!("{}", serde_json::to_string_pretty(&config).unwrap());

    // 상태 설정

    let config = Arc::new(config);

    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .expect("Client should build");

    let (issuer, oauth_client, valiator_set, client_config) = config.idp.load(&http_client).await?;

    let authorizer = AuthorizerEngine::new(config.authorizer.clone()).await;
    let api_key_extractor = ApikeyExtractorState::load(config.application.apikey.clone()).await?;
    // 애플리케이션 상태 설정 (config 사용)
    let state = AppState {
        jwt_middleware: JwtMiddlewareState::new(issuer, oauth_client, valiator_set, client_config)
            .map_err(|err| {
                tracing::error!("Failed to create JwtMiddlewareState: {}", err);
                err
            })?,
        api_key_extractor,
        authorizer,
        config: config.clone(),
        reqwest: http_client,
        configfile: Arc::new(configfile),
    };

    if let Some(configfile) = &*state.configfile {
        tracing::info!("file loaded from {}", configfile.display());
    }

    // 라우터 설정
    let mut router = handler::router().with_state(state).layer(trace_layer());

    if config.application.health_check {
        let health = Health::builder().build();
        router = router
            .route("/.meta/health", get(axum_health::health))
            .layer(health);
    }

    if config.application.prometheus {
        tracing::info!("Enable Prometheus metrics");
        let (prometheus_layer, prometheus_metrics) = PrometheusMetricLayer::pair();
        router = router
            .route(
                "/.meta/metrics",
                get(move || async move { prometheus_metrics.render() }),
            )
            .layer(prometheus_layer);
    }
    router = router.layer(
        config
            .application
            .ip_extract
            .clone()
            .unwrap_or(ClientIpSource::ConnectInfo)
            .into_extension(),
    );
    router = router.layer(CorsLayer::permissive());
    // 서버 주소 설정 (config 사용)
    tracing::info!("Server started at: {}", config.server.addr);

    // 서버 실행
    let listener = tokio::net::TcpListener::bind(config.server.addr).await?;
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
