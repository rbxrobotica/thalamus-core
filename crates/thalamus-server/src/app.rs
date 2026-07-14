use std::sync::Arc;

use axum::extract::Request;
use axum::middleware::{self, Next};
use axum::routing::{get, post};
use axum::Router;

use thalamus_core::BackendPort;

use crate::auth;
use crate::config::ServerConfig;
use crate::ports;
use crate::ports::audit::AuditStore;
use crate::routes;

const EVAL_CHANNEL_CAPACITY: usize = 256;

/// Shared application state injected into all handlers.
pub struct AppState {
    pub policy_port: Arc<dyn thalamus_core::PolicyPort + Send + Sync>,
    pub context_port: Arc<dyn thalamus_core::ContextPort + Send + Sync>,
    pub audit_port: Arc<ports::InMemoryAuditPort>,
    pub eval_port: Arc<dyn thalamus_core::EvalPort + Send + Sync>,
    pub obs_port: Arc<dyn thalamus_core::ObservabilityPort + Send + Sync>,
    pub backend_port: Option<Arc<dyn BackendPort + Send + Sync>>,
    pub audit_store: AuditStore,
    #[allow(
        dead_code,
        reason = "used by integration tests and future eval inspection endpoint"
    )]
    pub eval_store: thalamus_eval::EvalStore,
    /// Inbound credential verifier for the gated `/rbx/v1/*` surface. `None`
    /// means `THALAMUS_RBX_API` is off and no `/rbx/v1/*` routes are mounted.
    pub credential_verifier: Option<Arc<dyn auth::CredentialVerifier + Send + Sync>>,
}

#[allow(
    dead_code,
    reason = "used by default binary build and integration tests"
)]
pub fn build(config: ServerConfig) -> Router {
    let policy_port = Arc::new(ports::ConfigPolicyPort::from_config(&config));
    let context_port = Arc::new(ports::StaticContextPort::empty());
    let audit_port = Arc::new(ports::InMemoryAuditPort::new());
    let audit_store = audit_port.store();
    let eval_port = Arc::new(ports::ChannelEvalPort::new(EVAL_CHANNEL_CAPACITY));
    let eval_store = eval_port.store().clone();
    let obs_port = Arc::new(ports::LoggingObservabilityPort);
    let backend_port: Option<Arc<dyn BackendPort + Send + Sync>> = None;

    let state = Arc::new(AppState {
        policy_port,
        context_port,
        audit_port,
        eval_port,
        obs_port,
        backend_port,
        audit_store,
        eval_store,
        credential_verifier: None,
    });

    build_router(state)
}

/// Build an app for testing with a custom backend port.
#[allow(dead_code, reason = "used by integration tests")]
pub fn build_with_backend(
    config: ServerConfig,
    backend: Arc<dyn BackendPort + Send + Sync>,
) -> Router {
    let policy_port = Arc::new(ports::ConfigPolicyPort::from_config(&config));
    let context_port = Arc::new(ports::StaticContextPort::empty());
    let audit_port = Arc::new(ports::InMemoryAuditPort::new());
    let audit_store = audit_port.store();
    let eval_port = Arc::new(ports::ChannelEvalPort::new(EVAL_CHANNEL_CAPACITY));
    let eval_store = eval_port.store().clone();
    let obs_port = Arc::new(ports::LoggingObservabilityPort);

    let state = Arc::new(AppState {
        policy_port,
        context_port,
        audit_port,
        eval_port,
        obs_port,
        backend_port: Some(backend),
        audit_store,
        eval_store,
        credential_verifier: None,
    });

    build_router(state)
}

/// Build an app for testing with custom policy and backend ports.
#[allow(dead_code, reason = "used by integration tests")]
pub fn build_with_ports(
    policy_port: Arc<dyn thalamus_core::PolicyPort + Send + Sync>,
    backend: Arc<dyn BackendPort + Send + Sync>,
) -> Router {
    let context_port = Arc::new(ports::StaticContextPort::empty());
    let audit_port = Arc::new(ports::InMemoryAuditPort::new());
    let audit_store = audit_port.store();
    let eval_port = Arc::new(ports::ChannelEvalPort::new(EVAL_CHANNEL_CAPACITY));
    let eval_store = eval_port.store().clone();
    let obs_port = Arc::new(ports::LoggingObservabilityPort);

    let state = Arc::new(AppState {
        policy_port,
        context_port,
        audit_port,
        eval_port,
        obs_port,
        backend_port: Some(backend),
        audit_store,
        eval_store,
        credential_verifier: None,
    });

    build_router(state)
}

fn build_router(state: Arc<AppState>) -> Router {
    let credential_verifier = state.credential_verifier.clone();

    let router = Router::new()
        .route("/healthz", get(routes::healthz))
        .route("/readyz", get(routes::readyz))
        .route("/v1/decide", post(routes::decide))
        .route("/v1/pre-call", post(routes::pre_call))
        .route("/v1/post-call", post(routes::post_call))
        .route("/v1/call", post(routes::full_call))
        .route("/v1/audit/{id}", get(routes::get_audit))
        .with_state(state);

    // Gated /rbx/v1/* surface (THALAMUS_RBX_API). Mounted only when a
    // credential verifier is present. The auth middleware is scoped to this
    // sub-router, so the legacy /v1/* and /healthz routes are never affected.
    if let Some(verifier) = credential_verifier {
        let rbx = Router::new()
            .route("/rbx/v1/identity", get(routes::rbx_identity))
            .layer(middleware::from_fn(move |req: Request, next: Next| {
                let verifier = Arc::clone(&verifier);
                async move { auth::require_credential(verifier, req, next).await }
            }));
        router.merge(rbx)
    } else {
        router
    }
}

/// Build an app that also serves the gated `/rbx/v1/*` surface with the given
/// credential verifier (`THALAMUS_RBX_API=on`). Used by `main` (real
/// introspection verifier) and by integration tests (static verifier). No
/// backend adapter is wired: Phase 1 Gate A exercises identity/middleware only;
/// LLM routing through the adapters lands in Phase 3.
#[allow(
    dead_code,
    reason = "used by main when THALAMUS_RBX_API=on and by tests"
)]
pub fn build_with_rbx_api(
    config: ServerConfig,
    credential_verifier: Arc<dyn auth::CredentialVerifier + Send + Sync>,
) -> Router {
    let policy_port = Arc::new(ports::ConfigPolicyPort::from_config(&config));
    let context_port = Arc::new(ports::StaticContextPort::empty());
    let audit_port = Arc::new(ports::InMemoryAuditPort::new());
    let audit_store = audit_port.store();
    let eval_port = Arc::new(ports::ChannelEvalPort::new(EVAL_CHANNEL_CAPACITY));
    let eval_store = eval_port.store().clone();
    let obs_port = Arc::new(ports::LoggingObservabilityPort);

    let state = Arc::new(AppState {
        policy_port,
        context_port,
        audit_port,
        eval_port,
        obs_port,
        backend_port: None,
        audit_store,
        eval_store,
        credential_verifier: Some(credential_verifier),
    });

    build_router(state)
}

/// Build an app with an injected eval sink, normally backed by the TraceExporter seam.
pub fn build_with_eval_sink(
    config: ServerConfig,
    backend: Option<Arc<dyn BackendPort + Send + Sync>>,
    eval_sink: Arc<dyn thalamus_eval::EvalSink + Send + Sync>,
    content_policy: thalamus_eval::ContentPolicy,
) -> Router {
    let policy_port = Arc::new(ports::ConfigPolicyPort::from_config(&config));
    let context_port = Arc::new(ports::StaticContextPort::empty());
    let audit_port = Arc::new(ports::InMemoryAuditPort::new());
    let audit_store = audit_port.store();
    let eval_port = Arc::new(ports::ChannelEvalPort::new_with_sink(
        EVAL_CHANNEL_CAPACITY,
        eval_sink,
        content_policy,
    ));
    let eval_store = eval_port.store().clone();
    let obs_port = Arc::new(ports::LoggingObservabilityPort);

    let state = Arc::new(AppState {
        policy_port,
        context_port,
        audit_port,
        eval_port,
        obs_port,
        backend_port: backend,
        audit_store,
        eval_store,
        credential_verifier: None,
    });

    build_router(state)
}

/// Build an app for testing that returns the Router and the EvalStore handle,
/// so integration tests can inspect eval records after requests.
#[allow(dead_code, reason = "used by integration tests")]
pub fn build_with_eval_inspection(
    config: ServerConfig,
    backend: Arc<dyn BackendPort + Send + Sync>,
) -> (Router, thalamus_eval::EvalStore) {
    let policy_port = Arc::new(ports::ConfigPolicyPort::from_config(&config));
    let context_port = Arc::new(ports::StaticContextPort::empty());
    let audit_port = Arc::new(ports::InMemoryAuditPort::new());
    let audit_store = audit_port.store();
    let eval_port = Arc::new(ports::ChannelEvalPort::new(EVAL_CHANNEL_CAPACITY));
    let eval_store = eval_port.store().clone();
    let obs_port = Arc::new(ports::LoggingObservabilityPort);

    let state = Arc::new(AppState {
        policy_port,
        context_port,
        audit_port,
        eval_port,
        obs_port,
        backend_port: Some(backend),
        audit_store,
        eval_store: eval_store.clone(),
        credential_verifier: None,
    });

    (build_router(state), eval_store)
}
