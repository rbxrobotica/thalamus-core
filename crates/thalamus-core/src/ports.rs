use crate::audit::AuditEvent;
use crate::domain::{BackendResponse, CallRequest, ContextEntry, Envelope, PolicyDecision};
use crate::policy::{ContextGrant, Policy};
use crate::routing::{
    BackendCallError, BackendExecution, BackendUsage, CancelToken, RouteEnvelope,
};

/// Stable port: backend execution. Implemented by data-plane adapters.
/// Never a domain dependency.
pub trait BackendPort {
    fn call(&self, envelope: &Envelope) -> BackendResponse;

    /// Execute within a [`RouteEnvelope`] (§3): typed errors, usage, backend
    /// metadata, timeout and cooperative cancellation. Adapters must refuse
    /// execution that would cross any envelope constraint.
    ///
    /// The default implementation bridges legacy adapters through [`call`]:
    /// no constraint validation, no mid-flight cancellation, empty content
    /// mapped to `Unavailable` (the legacy failure signature).
    fn execute(
        &self,
        route: &RouteEnvelope,
        cancel: &CancelToken,
    ) -> Result<BackendExecution, BackendCallError> {
        if cancel.is_cancelled() {
            return Err(BackendCallError::Cancelled {
                partial_usage: BackendUsage::default(),
            });
        }
        let response = self.call(&route.envelope);
        if cancel.is_cancelled() {
            return Err(BackendCallError::Cancelled {
                partial_usage: BackendUsage {
                    total_tokens: response.tokens_used,
                    ..Default::default()
                },
            });
        }
        if response.content.is_empty() {
            return Err(BackendCallError::Unavailable {
                detail: "legacy adapter returned empty content".to_owned(),
            });
        }
        Ok(BackendExecution {
            usage: BackendUsage {
                total_tokens: response.tokens_used,
                ..Default::default()
            },
            latency_ms: response.latency_ms.unwrap_or_default(),
            backend_metadata: serde_json::json!({ "adapter": "legacy-bridge" }),
            content: response.content,
        })
    }

    /// Streaming execution (§3): content deltas are delivered through `sink`
    /// as they arrive; the final [`BackendExecution`] carries the full
    /// content and usage. Adapters must check `cancel` between chunks and
    /// return [`BackendCallError::Cancelled`] with partial usage mid-flight.
    ///
    /// The default implementation bridges non-streaming adapters: one
    /// [`execute`] call, whole content delivered as a single chunk.
    fn execute_streaming(
        &self,
        route: &RouteEnvelope,
        cancel: &CancelToken,
        sink: &mut dyn FnMut(&str),
    ) -> Result<BackendExecution, BackendCallError> {
        let execution = self.execute(route, cancel)?;
        sink(&execution.content);
        Ok(execution)
    }

    /// Chat-structured streaming execution for run-bound calls: each backend
    /// chunk is delivered verbatim as an OpenAI-compatible
    /// `chat.completion.chunk` JSON object (content deltas, tool_call
    /// argument deltas, finish_reason, usage), so a compatibility facade can
    /// forward them without re-synthesis.
    ///
    /// The default implementation bridges text-only adapters: content deltas
    /// from [`execute_streaming`] are wrapped into minimal chunk objects.
    fn execute_streaming_chat(
        &self,
        route: &RouteEnvelope,
        cancel: &CancelToken,
        on_chunk: &mut dyn FnMut(&serde_json::Value),
    ) -> Result<BackendExecution, BackendCallError> {
        let mut forward = |delta: &str| {
            on_chunk(&serde_json::json!({
                "choices": [{ "index": 0, "delta": { "content": delta }, "finish_reason": null }]
            }));
        };
        self.execute_streaming(route, cancel, &mut forward)
    }
}

/// Stable port: authorized context retrieval.
pub trait ContextPort {
    fn fetch(&self, grants: &[ContextGrant]) -> Vec<ContextEntry>;
}

/// Stable port: policy resolution and evaluation.
pub trait PolicyPort {
    fn resolve(&self, request: &CallRequest) -> Policy;
    fn evaluate(&self, request: &CallRequest, policy: &Policy) -> PolicyDecision;
}

/// Stable port: audit event persistence.
pub trait AuditPort {
    fn emit(&self, event: AuditEvent);
}

/// Stable port: automatic evaluation submission.
pub trait EvalPort {
    fn submit(&self, response: &BackendResponse, policy: &Policy) -> String;
}

/// Stable port: observability (spans, metrics).
pub trait ObservabilityPort {
    fn span(&self, name: &str, trace_id: &crate::domain::TraceId);
}
