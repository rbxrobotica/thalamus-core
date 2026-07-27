use std::sync::Arc;

use axum::extract::Request;
use axum::middleware::{self, Next};
use axum::routing::{get, post};
use axum::Router;

use thalamus_core::{BackendPort, EmbeddingPort};

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
    pub audit_port: ports::audit::SharedAuditPort,
    /// Authoritative durable audit store (Phase 2). `None` = in-memory only.
    pub durable_audit: Option<ports::audit::SharedDurableAudit>,
    /// Session/run lifecycle store (Phase 3). Postgres-backed when the durable
    /// audit store is wired; in-memory otherwise.
    pub session_store: ports::sessions::SharedSessionStore,
    /// Rate limiter for /rbx/v1/* (§3 security). `None` = disabled.
    pub rate_limiter: Option<Arc<crate::rate_limit::RateLimiter>>,
    pub eval_port: Arc<dyn thalamus_core::EvalPort + Send + Sync>,
    pub obs_port: Arc<dyn thalamus_core::ObservabilityPort + Send + Sync>,
    pub backend_port: Option<Arc<dyn BackendPort + Send + Sync>>,
    pub embedding_port: Option<Arc<dyn EmbeddingPort + Send + Sync>>,
    pub audit_store: AuditStore,
    #[allow(
        dead_code,
        reason = "used by integration tests and future eval inspection endpoint"
    )]
    pub eval_store: thalamus_eval::EvalStore,
    /// Inbound credential verifier for the gated `/rbx/v1/*` surface. `None`
    /// means `THALAMUS_RBX_API` is off and no `/rbx/v1/*` routes are mounted.
    pub credential_verifier: Option<Arc<dyn auth::CredentialVerifier + Send + Sync>>,
    /// Model alias to price, so every finalized run carries a cost alongside
    /// its tokens and latency. Empty book = every run recorded `unpriced`.
    pub pricing: Arc<thalamus_core::PriceBook>,
}

#[allow(
    dead_code,
    reason = "used by default binary build and integration tests"
)]
pub fn build(config: ServerConfig) -> Router {
    let policy_port = Arc::new(ports::ConfigPolicyPort::from_config(&config));
    let context_port = Arc::new(ports::StaticContextPort::empty());
    let (audit_port, audit_store, durable_audit, session_store) = ports::audit::audit_wiring();
    let eval_port = Arc::new(ports::ChannelEvalPort::new(EVAL_CHANNEL_CAPACITY));
    let eval_store = eval_port.store().clone();
    let obs_port = Arc::new(ports::LoggingObservabilityPort);
    let backend_port: Option<Arc<dyn BackendPort + Send + Sync>> = None;

    let state = Arc::new(AppState {
        policy_port,
        durable_audit,
        session_store,
        context_port,
        audit_port,
        eval_port,
        obs_port,
        backend_port,
        embedding_port: None,
        audit_store,
        eval_store,
        credential_verifier: None,
        rate_limiter: default_rate_limiter(),
        pricing: default_price_book(),
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
    let (audit_port, audit_store, durable_audit, session_store) = ports::audit::audit_wiring();
    let eval_port = Arc::new(ports::ChannelEvalPort::new(EVAL_CHANNEL_CAPACITY));
    let eval_store = eval_port.store().clone();
    let obs_port = Arc::new(ports::LoggingObservabilityPort);

    let state = Arc::new(AppState {
        policy_port,
        durable_audit,
        session_store,
        context_port,
        audit_port,
        eval_port,
        obs_port,
        backend_port: Some(backend),
        embedding_port: None,
        audit_store,
        eval_store,
        credential_verifier: None,
        rate_limiter: default_rate_limiter(),
        pricing: default_price_book(),
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
    let (audit_port, audit_store, durable_audit, session_store) = ports::audit::audit_wiring();
    let eval_port = Arc::new(ports::ChannelEvalPort::new(EVAL_CHANNEL_CAPACITY));
    let eval_store = eval_port.store().clone();
    let obs_port = Arc::new(ports::LoggingObservabilityPort);

    let state = Arc::new(AppState {
        policy_port,
        durable_audit,
        session_store,
        context_port,
        audit_port,
        eval_port,
        obs_port,
        backend_port: Some(backend),
        embedding_port: None,
        audit_store,
        eval_store,
        credential_verifier: None,
        rate_limiter: default_rate_limiter(),
        pricing: default_price_book(),
    });

    build_router(state)
}

/// Body limit for the governed authenticated surface (master plan §3 security).
const RBX_BODY_LIMIT_BYTES: usize = 256 * 1024;

/// Price book from `THALAMUS_MODEL_PRICES`. A malformed book aborts the boot:
/// running with prices silently dropped would write an audit trail whose cost
/// column is wrong rather than absent.
fn default_price_book() -> Arc<thalamus_core::PriceBook> {
    match thalamus_core::PriceBook::from_env() {
        Ok(book) => Arc::new(book),
        Err(err) => {
            eprintln!("invalid THALAMUS_MODEL_PRICES: {err}");
            std::process::exit(1);
        }
    }
}

fn default_rate_limiter() -> Option<Arc<crate::rate_limit::RateLimiter>> {
    crate::rate_limit::RateLimiter::from_env().map(Arc::new)
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
        .route("/v1/call/stream", post(routes::full_call_stream))
        .route("/v1/audit/{id}", get(routes::get_audit))
        .with_state(state.clone());

    // Gated authenticated surface (THALAMUS_RBX_API): `/rbx/v1/*` plus the
    // governed `/v1/embeddings` contract. Mounted only when a credential
    // verifier is present. The auth middleware is scoped to this sub-router,
    // so other legacy `/v1/*` and health routes are never affected.
    // Credential validation runs before every handler: a session/run is never
    // created for an unverified caller.
    if let Some(verifier) = credential_verifier {
        let rbx = Router::new()
            .route("/rbx/v1/identity", get(routes::rbx_identity))
            .route("/rbx/v1/sessions", post(routes::rbx_create_session))
            .route(
                "/rbx/v1/sessions/{session_id}/runs",
                post(routes::rbx_create_run),
            )
            .route(
                "/rbx/v1/sessions/{session_id}/close",
                post(routes::rbx_close_session),
            )
            .route(
                "/rbx/v1/sessions/{session_id}/limits",
                get(routes::rbx_session_limits),
            )
            .route("/rbx/v1/runs/{run_id}/cancel", post(routes::rbx_cancel_run))
            .route("/rbx/v1/runs/{run_id}/calls", post(routes::rbx_run_call))
            .route(
                "/rbx/v1/runs/{run_id}/calls/stream",
                post(routes::rbx_run_call_stream),
            )
            .route("/rbx/v1/tool-decisions", post(routes::rbx_tool_decision))
            .route("/rbx/v1/approvals", post(routes::rbx_approval))
            .route("/rbx/v1/evidence", post(routes::rbx_evidence))
            .route("/v1/embeddings", post(routes::embeddings))
            // Innermost: rate limiting sees the VerifiedCaller inserted by
            // the (outer) credential middleware.
            .layer(middleware::from_fn({
                let limiter = state.rate_limiter.clone();
                move |req: Request, next: Next| {
                    let limiter = limiter.clone();
                    async move { crate::rate_limit::enforce(limiter, req, next).await }
                }
            }))
            .layer(middleware::from_fn(move |req: Request, next: Next| {
                let verifier = Arc::clone(&verifier);
                async move { auth::require_credential(verifier, req, next).await }
            }))
            .layer(axum::extract::DefaultBodyLimit::max(RBX_BODY_LIMIT_BYTES))
            .with_state(state);
        router.merge(rbx)
    } else {
        router
    }
}

/// Build an app that also serves the gated `/rbx/v1/*` and `/v1/embeddings`
/// surfaces with the given credential verifier (`THALAMUS_RBX_API=on`). Used by `main` (real
/// introspection verifier) and by integration tests (static verifier). The
/// backend adapter is wired by the caller so the legacy /v1/call surface
/// keeps working when the governed surface is enabled.
#[allow(
    dead_code,
    reason = "used by main when THALAMUS_RBX_API=on and by tests"
)]
pub fn build_with_rbx_api(
    config: ServerConfig,
    credential_verifier: Arc<dyn auth::CredentialVerifier + Send + Sync>,
    backend_port: Option<Arc<dyn BackendPort + Send + Sync>>,
) -> Router {
    build_with_rbx_api_and_embedding(config, credential_verifier, backend_port, None)
}

/// Build the governed API with independently injected chat and embedding
/// execution seams. A caller may wire either port without exposing a provider
/// type to the server.
pub fn build_with_rbx_api_and_embedding(
    config: ServerConfig,
    credential_verifier: Arc<dyn auth::CredentialVerifier + Send + Sync>,
    backend_port: Option<Arc<dyn BackendPort + Send + Sync>>,
    embedding_port: Option<Arc<dyn EmbeddingPort + Send + Sync>>,
) -> Router {
    let policy_port = Arc::new(ports::ConfigPolicyPort::from_config(&config));
    let context_port = Arc::new(ports::StaticContextPort::empty());
    let (audit_port, audit_store, durable_audit, session_store) = ports::audit::audit_wiring();
    let eval_port = Arc::new(ports::ChannelEvalPort::new(EVAL_CHANNEL_CAPACITY));
    let eval_store = eval_port.store().clone();
    let obs_port = Arc::new(ports::LoggingObservabilityPort);

    let state = Arc::new(AppState {
        policy_port,
        durable_audit,
        session_store,
        context_port,
        audit_port,
        eval_port,
        obs_port,
        backend_port,
        embedding_port,
        audit_store,
        eval_store,
        credential_verifier: Some(credential_verifier),
        rate_limiter: default_rate_limiter(),
        pricing: default_price_book(),
    });

    build_router(state)
}

/// Build an app serving `/rbx/v1/*` with an injected session store. Used by
/// integration tests to seed budgets on an [`ports::sessions::InMemorySessionStore`].
#[allow(dead_code, reason = "used by integration tests")]
pub fn build_with_rbx_api_and_sessions(
    config: ServerConfig,
    credential_verifier: Arc<dyn auth::CredentialVerifier + Send + Sync>,
    session_store: ports::sessions::SharedSessionStore,
) -> Router {
    let policy_port = Arc::new(ports::ConfigPolicyPort::from_config(&config));
    let context_port = Arc::new(ports::StaticContextPort::empty());
    let (audit_port, audit_store, durable_audit, _default_sessions) = ports::audit::audit_wiring();
    let eval_port = Arc::new(ports::ChannelEvalPort::new(EVAL_CHANNEL_CAPACITY));
    let eval_store = eval_port.store().clone();
    let obs_port = Arc::new(ports::LoggingObservabilityPort);

    let state = Arc::new(AppState {
        policy_port,
        durable_audit,
        session_store,
        context_port,
        audit_port,
        eval_port,
        obs_port,
        backend_port: None,
        embedding_port: None,
        audit_store,
        eval_store,
        credential_verifier: Some(credential_verifier),
        rate_limiter: default_rate_limiter(),
        pricing: default_price_book(),
    });

    build_router(state)
}

/// Build an app serving `/rbx/v1/*` with injected session store AND backend
/// port (run-bound governed call tests, SLICE-T1).
#[allow(dead_code, reason = "used by integration tests")]
pub fn build_with_rbx_api_sessions_backend(
    config: ServerConfig,
    credential_verifier: Arc<dyn auth::CredentialVerifier + Send + Sync>,
    session_store: ports::sessions::SharedSessionStore,
    backend: Arc<dyn thalamus_core::BackendPort + Send + Sync>,
) -> Router {
    let policy_port = Arc::new(ports::ConfigPolicyPort::from_config(&config));
    let context_port = Arc::new(ports::StaticContextPort::empty());
    let (audit_port, audit_store, durable_audit, _default_sessions) = ports::audit::audit_wiring();
    let eval_port = Arc::new(ports::ChannelEvalPort::new(EVAL_CHANNEL_CAPACITY));
    let eval_store = eval_port.store().clone();
    let obs_port = Arc::new(ports::LoggingObservabilityPort);

    let state = Arc::new(AppState {
        policy_port,
        durable_audit,
        session_store,
        context_port,
        audit_port,
        eval_port,
        obs_port,
        backend_port: Some(backend),
        embedding_port: None,
        audit_store,
        eval_store,
        credential_verifier: Some(credential_verifier),
        rate_limiter: default_rate_limiter(),
        pricing: default_price_book(),
    });

    build_router(state)
}

/// Build an app serving `/rbx/v1/*` with injected session store AND rate
/// limiter (integration tests need deterministic limits without env races).
#[allow(dead_code, reason = "used by integration tests")]
pub fn build_with_rbx_api_sessions_limiter(
    config: ServerConfig,
    credential_verifier: Arc<dyn auth::CredentialVerifier + Send + Sync>,
    session_store: ports::sessions::SharedSessionStore,
    rate_limiter: Option<Arc<crate::rate_limit::RateLimiter>>,
) -> Router {
    let policy_port = Arc::new(ports::ConfigPolicyPort::from_config(&config));
    let context_port = Arc::new(ports::StaticContextPort::empty());
    let (audit_port, audit_store, durable_audit, _default_sessions) = ports::audit::audit_wiring();
    let eval_port = Arc::new(ports::ChannelEvalPort::new(EVAL_CHANNEL_CAPACITY));
    let eval_store = eval_port.store().clone();
    let obs_port = Arc::new(ports::LoggingObservabilityPort);

    let state = Arc::new(AppState {
        policy_port,
        durable_audit,
        session_store,
        context_port,
        audit_port,
        eval_port,
        obs_port,
        backend_port: None,
        embedding_port: None,
        audit_store,
        eval_store,
        credential_verifier: Some(credential_verifier),
        rate_limiter,
        pricing: default_price_book(),
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
    let (audit_port, audit_store, durable_audit, session_store) = ports::audit::audit_wiring();
    let eval_port = Arc::new(ports::ChannelEvalPort::new_with_sink(
        EVAL_CHANNEL_CAPACITY,
        eval_sink,
        content_policy,
    ));
    let eval_store = eval_port.store().clone();
    let obs_port = Arc::new(ports::LoggingObservabilityPort);

    let state = Arc::new(AppState {
        policy_port,
        durable_audit,
        session_store,
        context_port,
        audit_port,
        eval_port,
        obs_port,
        backend_port: backend,
        embedding_port: None,
        audit_store,
        eval_store,
        credential_verifier: None,
        rate_limiter: default_rate_limiter(),
        pricing: default_price_book(),
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
    let (audit_port, audit_store, durable_audit, session_store) = ports::audit::audit_wiring();
    let eval_port = Arc::new(ports::ChannelEvalPort::new(EVAL_CHANNEL_CAPACITY));
    let eval_store = eval_port.store().clone();
    let obs_port = Arc::new(ports::LoggingObservabilityPort);

    let state = Arc::new(AppState {
        policy_port,
        durable_audit,
        session_store,
        context_port,
        audit_port,
        eval_port,
        obs_port,
        backend_port: Some(backend),
        embedding_port: None,
        audit_store,
        eval_store: eval_store.clone(),
        credential_verifier: None,
        rate_limiter: default_rate_limiter(),
        pricing: default_price_book(),
    });

    (build_router(state), eval_store)
}
