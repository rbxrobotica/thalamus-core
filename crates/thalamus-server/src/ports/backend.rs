use thalamus_core::{BackendPort, BackendResponse, Envelope};

/// No-op backend port: used when no backend is configured.
/// The /v1/call endpoint returns a structured 503 when this is active.
pub struct NoopBackendPort;

impl BackendPort for NoopBackendPort {
    fn call(&self, _envelope: &Envelope) -> BackendResponse {
        unreachable!("NoopBackendPort must never be called; /v1/call returns 503 before reaching it")
    }
}

/// Dev echo backend: only available under the `dev` feature flag,
/// never in default builds.
#[cfg(feature = "dev")]
pub struct EchoBackendPort;

#[cfg(feature = "dev")]
impl BackendPort for EchoBackendPort {
    fn call(&self, envelope: &Envelope) -> BackendResponse {
        tracing::info!(audit_id = %envelope.audit_id.0, "echo_backend_call");
        BackendResponse {
            content: format!("echo: {}", envelope.prompt),
            tokens_used: Some(10),
            latency_ms: Some(1),
        }
    }
}
