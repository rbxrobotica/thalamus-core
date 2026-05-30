use std::sync::Arc;

use serde::Serialize;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceExporterSelection {
    RbxObservabilityHttp,
    #[cfg(feature = "langfuse")]
    Langfuse,
    #[cfg(not(feature = "langfuse"))]
    None,
}

pub fn select_trace_exporter(rbx_observability_url: Option<&str>) -> TraceExporterSelection {
    if rbx_observability_url.is_some_and(|url| !url.trim().is_empty()) {
        return TraceExporterSelection::RbxObservabilityHttp;
    }

    #[cfg(feature = "langfuse")]
    {
        TraceExporterSelection::Langfuse
    }

    #[cfg(not(feature = "langfuse"))]
    {
        TraceExporterSelection::None
    }
}

pub fn trace_exporter_sink_from_env() -> Option<Arc<dyn EvalSink + Send + Sync>> {
    let exporter: Arc<dyn TraceExporter + Send + Sync> =
        match select_trace_exporter(std::env::var("RBX_OBSERVABILITY_URL").ok().as_deref()) {
            TraceExporterSelection::RbxObservabilityHttp => {
                Arc::new(HttpTraceExporter::from_env()?)
            }
            #[cfg(feature = "langfuse")]
            TraceExporterSelection::Langfuse => Arc::new(LangfuseTraceExporter::from_env()),
            #[cfg(not(feature = "langfuse"))]
            TraceExporterSelection::None => return None,
        };

    Some(Arc::new(TraceExporterEvalSink::new(exporter)))
}

#[derive(Debug, Clone)]
pub struct HttpTraceExporter {
    endpoint: String,
    token: Option<String>,
    timeout_ms: u64,
}

#[derive(Debug, Serialize)]
struct TracePayload {
    trace_id: Option<String>,
    spans: Vec<Span>,
}

#[derive(Debug, Serialize)]
struct Span {
    span_id: String,
    name: String,
    start_time: String,
    end_time: Option<String>,
    attributes: serde_json::Value,
    events: Vec<SpanEvent>,
}

#[derive(Debug, Serialize)]
struct SpanEvent {
    name: String,
    timestamp: String,
    attributes: serde_json::Value,
}

impl HttpTraceExporter {
    pub fn new(endpoint: String, token: Option<String>, timeout_ms: u64) -> Option<Self> {
        let endpoint = endpoint.trim().trim_end_matches('/').to_owned();
        if endpoint.is_empty() {
            return None;
        }

        Some(Self {
            endpoint,
            token: token.and_then(|token| {
                let token = token.trim().to_owned();
                (!token.is_empty()).then_some(token)
            }),
            timeout_ms,
        })
    }

    pub fn from_env() -> Option<Self> {
        Self::new(
            std::env::var("RBX_OBSERVABILITY_URL").ok()?,
            std::env::var("RBX_OBSERVABILITY_TOKEN").ok(),
            std::env::var("RBX_OBSERVABILITY_TIMEOUT_MS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(2_000),
        )
    }

    fn traces_url(&self) -> String {
        format!("{}/v1/traces", self.endpoint)
    }

    fn build_payload(submission: &EvalSubmission) -> TracePayload {
        let record = &submission.record;
        let attributes = serde_json::json!({
            "eval_ref": record.eval_ref,
            "schema_valid": record.schema_valid,
            "risk_class": record.risk_class,
            "policy_id": record.policy_id,
            "citation_check": record.citation_check,
            "hallucination_signals": record.hallucination_signals,
            "response_metadata": {
                "content_len": record.response_metadata.content_len,
                "tokens_used": record.response_metadata.tokens_used,
                "latency_ms": record.response_metadata.latency_ms,
            },
            "audit_id": record.audit_id,
            "authorized_content": submission.authorized_content,
        });

        TracePayload {
            trace_id: record.trace_id.clone(),
            spans: vec![Span {
                span_id: record.eval_ref.clone(),
                name: format!("eval:{}", record.policy_id),
                start_time: record.created_at.clone(),
                end_time: None,
                attributes,
                events: Vec::new(),
            }],
        }
    }

    fn post(&self, payload: &TracePayload) -> Result<(), String> {
        let timeout = std::time::Duration::from_millis(self.timeout_ms);
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .build();
        let agent: ureq::Agent = config.into();
        let url = self.traces_url();
        let mut request = agent.post(&url).header("Content-Type", "application/json");

        if let Some(token) = &self.token {
            request = request.header("Authorization", &format!("Bearer {token}"));
        }

        request
            .send_json(payload)
            .map(|_| ())
            .map_err(|e| format!("rbx-observability POST: {e}"))
    }
}

impl TraceExporter for HttpTraceExporter {
    fn export(&self, submission: &EvalSubmission) {
        let payload = Self::build_payload(submission);
        if let Err(e) = self.post(&payload) {
            tracing::warn!(eval_ref = %submission.record.eval_ref, error = %e, "rbx-observability trace export failed (best-effort, dropped)");
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use thalamus_eval::{CitationCheckOutcome, EvalRecord, EvalSubmission, ResponseMetadata};

    #[test]
    fn selects_http_exporter_when_rbx_observability_url_is_set() {
        assert_eq!(
            select_trace_exporter(Some("http://localhost:8080")),
            TraceExporterSelection::RbxObservabilityHttp
        );
    }

    #[test]
    fn ignores_empty_rbx_observability_url() {
        assert_ne!(
            select_trace_exporter(Some("   ")),
            TraceExporterSelection::RbxObservabilityHttp
        );
    }

    #[test]
    fn http_exporter_normalizes_endpoint_and_token() {
        let exporter = HttpTraceExporter::new(
            " http://localhost:8080/ ".to_owned(),
            Some(" token ".to_owned()),
            100,
        )
        .unwrap();

        assert_eq!(exporter.traces_url(), "http://localhost:8080/v1/traces");
        assert_eq!(exporter.token.as_deref(), Some("token"));
    }

    #[test]
    fn http_payload_matches_observability_trace_contract() {
        let submission = EvalSubmission {
            record: EvalRecord {
                eval_ref: "eval-1".to_owned(),
                schema_valid: true,
                citation_check: CitationCheckOutcome::NotRequired,
                hallucination_signals: vec!["signal-a".to_owned()],
                risk_class: "Low".to_owned(),
                response_metadata: ResponseMetadata {
                    content_len: 42,
                    tokens_used: Some(10),
                    latency_ms: Some(25),
                },
                trace_id: Some("trace-1".to_owned()),
                audit_id: Some("audit-1".to_owned()),
                policy_id: "policy-a".to_owned(),
                created_at: "2026-05-30T00:00:00Z".to_owned(),
            },
            authorized_content: None,
        };

        let payload = serde_json::to_value(HttpTraceExporter::build_payload(&submission)).unwrap();
        assert_eq!(payload["trace_id"], "trace-1");
        assert_eq!(payload["spans"][0]["span_id"], "eval-1");
        assert_eq!(payload["spans"][0]["name"], "eval:policy-a");
        assert_eq!(payload["spans"][0]["start_time"], "2026-05-30T00:00:00Z");
        assert_eq!(payload["spans"][0]["events"].as_array().unwrap().len(), 0);
        assert_eq!(payload["spans"][0]["attributes"]["risk_class"], "Low");
    }
}
