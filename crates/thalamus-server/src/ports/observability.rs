use thalamus_core::{ObservabilityPort, TraceId};

/// Logging observability stub: emits a trace span as a log line.
pub struct LoggingObservabilityPort;

impl ObservabilityPort for LoggingObservabilityPort {
    fn span(&self, name: &str, trace_id: &TraceId) {
        tracing::info!(span = name, trace_id = %trace_id.0, "obs_span");
    }
}
