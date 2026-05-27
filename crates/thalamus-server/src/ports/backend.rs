/// Dev echo backend: only available under the `dev` feature flag,
/// never in default builds.
#[cfg(feature = "dev")]
#[allow(dead_code)]
pub struct EchoBackendPort;

#[cfg(feature = "dev")]
mod dev_impl {
    use thalamus_core::{BackendPort, BackendResponse, Envelope};

    use super::EchoBackendPort;

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
}
