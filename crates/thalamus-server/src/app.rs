use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};

use thalamus_core::BackendPort;

use crate::config::ServerConfig;
use crate::ports;
use crate::ports::audit::AuditStore;
use crate::routes;

/// Shared application state injected into all handlers.
pub struct AppState {
    pub policy_port: Arc<dyn thalamus_core::PolicyPort + Send + Sync>,
    pub context_port: Arc<dyn thalamus_core::ContextPort + Send + Sync>,
    pub audit_port: Arc<ports::InMemoryAuditPort>,
    pub eval_port: Arc<dyn thalamus_core::EvalPort + Send + Sync>,
    pub obs_port: Arc<dyn thalamus_core::ObservabilityPort + Send + Sync>,
    pub backend_port: Option<Arc<dyn BackendPort + Send + Sync>>,
    pub audit_store: AuditStore,
}

pub fn build(config: ServerConfig) -> Router {
    let policy_port = Arc::new(ports::ConfigPolicyPort::from_config(&config));
    let context_port = Arc::new(ports::StaticContextPort::empty());
    let audit_port = Arc::new(ports::InMemoryAuditPort::new());
    let audit_store = audit_port.store();
    let eval_port = Arc::new(ports::LoggingEvalPort);
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
    let eval_port = Arc::new(ports::LoggingEvalPort);
    let obs_port = Arc::new(ports::LoggingObservabilityPort);

    let state = Arc::new(AppState {
        policy_port,
        context_port,
        audit_port,
        eval_port,
        obs_port,
        backend_port: Some(backend),
        audit_store,
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
    let eval_port = Arc::new(ports::LoggingEvalPort);
    let obs_port = Arc::new(ports::LoggingObservabilityPort);

    let state = Arc::new(AppState {
        policy_port,
        context_port,
        audit_port,
        eval_port,
        obs_port,
        backend_port: Some(backend),
        audit_store,
    });

    build_router(state)
}

fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/decide", post(routes::decide))
        .route("/v1/pre-call", post(routes::pre_call))
        .route("/v1/post-call", post(routes::post_call))
        .route("/v1/call", post(routes::full_call))
        .route("/v1/audit/{id}", get(routes::get_audit))
        .with_state(state)
}
