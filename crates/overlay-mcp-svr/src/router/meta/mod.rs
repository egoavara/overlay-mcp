use axum::{routing::get, Router};
use axum_health::Health;
use axum_prometheus::PrometheusMetricLayer;
use overlay_mcp_core::Config;

pub fn router(config: &Config) -> Router<Config> {
    let mut router = Router::new();
    if config.application.health_check {
        let health = Health::builder().build();
        router = router
            .route("/health", get(axum_health::health))
            .layer(health);
    }

    if config.application.prometheus {
        tracing::info!("Enable Prometheus metrics");
        let (prometheus_layer, prometheus_metrics) = PrometheusMetricLayer::pair();
        router = router
            .route(
                "/metrics",
                get(move || async move { prometheus_metrics.render() }),
            )
            .layer(prometheus_layer);
    }
    router
}
