use tower_http::{classify::{ServerErrorsAsFailures, SharedClassifier}, trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer}, LatencyUnit};
use tracing::Level;

pub fn trace_layer() -> TraceLayer<SharedClassifier<ServerErrorsAsFailures>>{
    TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new()
            .level(Level::INFO)
            .include_headers(true)
        )
        .on_response(
            DefaultOnResponse::new()
                .level(Level::INFO)
                .latency_unit(LatencyUnit::Millis)
                .include_headers(true),
        )
}
