use std::sync::Arc;

use thalamus_eval::{EvalSink, EvalSubmission};

/// Seam for exporting Thalamus trace/eval records to an external observability owner.
///
/// ADR-0010/ADR-0300 move the Langfuse concern to rbx-observability. This seam
/// preserves the current Langfuse behavior until Thalamus can export over HTTP
/// to that service.
pub trait TraceExporter {
    fn export(&self, submission: &EvalSubmission);
}

#[allow(dead_code)]
pub struct TraceExporterEvalSink {
    exporter: Arc<dyn TraceExporter + Send + Sync>,
}

#[allow(dead_code)]
impl TraceExporterEvalSink {
    pub fn new(exporter: Arc<dyn TraceExporter + Send + Sync>) -> Self {
        Self { exporter }
    }
}

impl EvalSink for TraceExporterEvalSink {
    fn accept(&self, submission: &EvalSubmission) {
        self.exporter.export(submission);
    }
}

#[cfg(feature = "langfuse")]
pub struct LangfuseTraceExporter {
    sink: thalamus_langfuse_adapter::LangfuseSink,
}

#[cfg(feature = "langfuse")]
impl LangfuseTraceExporter {
    pub fn from_env() -> Self {
        let config = thalamus_langfuse_adapter::config::LangfuseConfig::from_env();
        Self {
            sink: thalamus_langfuse_adapter::LangfuseSink::new(config),
        }
    }
}

#[cfg(feature = "langfuse")]
impl TraceExporter for LangfuseTraceExporter {
    fn export(&self, submission: &EvalSubmission) {
        self.sink.accept(submission);
    }
}
