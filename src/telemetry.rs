use opentelemetry::trace::TraceId;
use tracing_subscriber::{prelude::*, EnvFilter, Registry};

///  Fetch an `opentelemetry::trace::TraceId` as hex through the full tracing stack
#[must_use]
pub fn get_trace_id() -> TraceId {
    use opentelemetry::trace::TraceContextExt as _; // opentelemetry::Context -> opentelemetry::trace::Span
    use tracing_opentelemetry::OpenTelemetrySpanExt as _; // tracing::Span to opentelemetry::Context

    tracing::Span::current()
        .context()
        .span()
        .span_context()
        .trace_id()
}

fn init_tracer() -> opentelemetry_sdk::trace::Tracer {
    use opentelemetry::trace::TracerProvider;
    #[cfg(feature = "telemetry")]
    use opentelemetry_otlp::SpanExporter;
    use opentelemetry_sdk::trace::SdkTracerProvider;

    #[cfg(feature = "telemetry")]
    let exporter = SpanExporter::builder().with_tonic().build().unwrap();
    let builder = SdkTracerProvider::builder();
    #[cfg(feature = "telemetry")]
    let builder = builder.with_batch_exporter(exporter);
    builder.build().tracer("addon-provider-fleet")
}

/// Initialize tracing
///
/// # Panics
///
/// This function will panic if the default tracing subscriber cannot be set.
pub fn init() {
    // Setup tracing layers
    let telemetry = tracing_opentelemetry::layer().with_tracer(init_tracer());
    let logger = tracing_subscriber::fmt::layer().compact();
    let env_filter = EnvFilter::try_from_default_env()
        .or(EnvFilter::try_new("info"))
        .unwrap();

    // Decide on layers
    let collector = Registry::default()
        .with(telemetry)
        .with(logger)
        .with(env_filter);

    // Initialize tracing
    tracing::subscriber::set_global_default(collector).unwrap();
}

#[cfg(test)]
mod test {
    // This test only works when telemetry is initialized fully
    // and requires OTEL_EXPORTER_OTLP_LOGS_ENDPOINT pointing to a valid server
    #[tokio::test]
    #[ignore = "requires a trace exporter"]
    async fn get_trace_id_returns_valid_traces() {
        use super::*;
        super::init();
        #[tracing::instrument(name = "test_span")] // need to be in an instrumented fn
        fn test_trace_id() -> TraceId {
            get_trace_id()
        }
        assert_ne!(test_trace_id(), TraceId::INVALID, "valid trace");
    }
}
