mod authorizer;
mod command;
mod config;
mod config_loader;
mod config_upstream;
mod fga;
mod handler;
mod init;
mod manager;
mod middleware;
mod utils;

use anyhow::{Context, Result};
use authorizer::AuthorizerEngine;
use axum::{routing::get, Extension};
use axum_client_ip::ClientIpSource;
use axum_health::Health;
use axum_prometheus::PrometheusMetricLayer;
use clap::Parser;
use command::{Cli, SubcommandRun, Subcommands};
use config::Config;
use handler::AppState;
use init::{init_storage, init_resolver};
use manager::storage::{ManagerTrait, StorageManager};
use middleware::{trace_layer, ApikeyExtractorState, JwtMiddlewareState};
use std::{net::SocketAddr, sync::Arc};
use tokio::signal::{self};
use tokio_util::sync::CancellationToken;
use tower_http::cors::CorsLayer;
use tracing_subscriber::EnvFilter;

// figment 및 Config 추가
use figment::{
    providers::{Format, Json as FigmentJson},
    Figment,
};

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    let cli: Cli = Cli::parse();
    match &cli.subcommand {
        Subcommands::Run(run) => main_run(run).await,
    }
}

async fn main_run(cli: &SubcommandRun) -> Result<()> {
    let configfile = cli.configfile.clone().map(FigmentJson::file);
    // 설정 로드 (Figment 사용)
    let config: Config = Figment::new()
        .merge(SubcommandRun::figment_default())
        .merge(configfile.unwrap_or(FigmentJson::string("{}")))
        .merge(cli.figment_merge())
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
    let cancel = CancellationToken::new();
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
    };
    let storage_manager = init_storage(config.clone()).await?;
    let _ = init_resolver(cancel.child_token(), config.clone(), storage_manager.clone()).await?;
    // 라우터 설정
    let mut router = handler::router().with_state(state).layer(trace_layer());
    router = router.layer(Extension(storage_manager.clone()));

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
    .with_graceful_shutdown(shutdown_signal(cancel, storage_manager))
    .await?;
    Ok(())
}

async fn shutdown_signal(cancel: CancellationToken, manager: StorageManager) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutting down...");
    cancel.cancel();
    if let Err(e) = manager.close().await {
        tracing::error!("failed to close manager: {}", e);
    }
}
