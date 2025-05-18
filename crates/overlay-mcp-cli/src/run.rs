use anyhow::{Context, Result};
use figment::{
    providers::{Format, Json as FigmentJson},
    Figment,
};
use overlay_mcp_core::Config;
use overlay_mcp_svr::router;
use serde_json::{json, Value};
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

use crate::utils::clean_json;

use super::command::SubcommandRun;

pub async fn run(cli: &SubcommandRun) -> Result<()> {
    let configfile = cli.configfile.clone().map(FigmentJson::file);
    let config: Config = Figment::new()
        .merge(configfile.unwrap_or(FigmentJson::string("{}")))
        .merge(figment_merge(cli)) // Use the standalone function
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
    tracing::info!("Server started at: {}", config.server.addr);
    let cancel = CancellationToken::new();
    let listener = tokio::net::TcpListener::bind(config.server.addr).await?;
    let app = router::router(cancel.clone(), config).await?;

    // 서버 실행
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(cancel))
        .await?;

    Ok(())
}

async fn shutdown_signal(cancel: CancellationToken) {
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
}

fn figment_merge(cli: &SubcommandRun) -> figment::providers::Serialized<figment::value::Value> {
    let mut cluster = serde_json::Map::new();
    if let Some(raft_id) = cli.raft_id {
        cluster.insert("type".to_string(), json!("raft"));
        cluster.insert("id".to_string(), json!(raft_id));
    }
    if let Some(raft_index) = cli.raft_index {
        cluster.insert("index".to_string(), json!(raft_index));
    }

    let mut server_config = json!({
        "addr": cli.addr,
        "hostname": cli.hostname,
    });
    if !cluster.is_empty() {
        server_config["cluster"] = Value::Object(cluster);
    }

    let mut upstream_config = json!(null);
    if let Some(ref url) = cli.upstream {
        // Assuming default upstream config is Static
        upstream_config = json!({ "urls": [url] });
    }

    // TODO: Refine auth config mapping based on actual AuthConfig structure
    let auth_config = json!({
        "authn": {
            "jwt": {
                "issuer": cli.issuer,
                "client": {
                    "id": cli.client_id,
                    "secret": cli.client_secret,
                    "scopes": cli.scopes,
                }
            }
        }
    });

    let result = json!({
        "application": {
            "log_filter": cli.log_filter,
            "prometheus": cli.prometheus,
            "health_check": cli.health_check,
            // passthrough and ip_extract need separate handling if configurable via CLI
        },
        "server": server_config,
        "upstream": upstream_config,
        "auth": auth_config,
        "otel": {
            "endpoint": cli.endpoint,
        }
    });

    let figment_value: figment::value::Value = serde_json::from_value(clean_json(result)).unwrap();
    figment::providers::Serialized::from(figment_value, figment::Profile::Default)
}
