use crate::audit::AuditEvent;
use crate::domain::{BackendResponse, CallRequest, ContextEntry, Envelope, PolicyDecision};
use crate::policy::{ContextGrant, Policy};

/// Stable port: backend execution. Implemented by data-plane adapters.
/// Never a domain dependency.
pub trait BackendPort {
    fn call(&self, envelope: &Envelope) -> BackendResponse;
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
