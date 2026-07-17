use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use thalamus_core::{AuditId, BackendResponse, CallRequest, PolicyDecision, PreCallError};

use crate::app::AppState;
use crate::auth::VerifiedCaller;
use axum::Extension;

// === Request / Response types ===

#[derive(Debug, Deserialize)]
pub struct DecideRequest {
    pub tenant: String,
    pub product: String,
    pub user: String,
    pub workflow: String,
    pub intent: String,
    pub prompt: String,
    pub requested_backend: Option<BackendHandleJson>,
    pub budget_hint: Option<BudgetHintJson>,
}

#[derive(Debug, Deserialize)]
pub struct BackendHandleJson {
    pub id: String,
    pub backend_type: String,
}

#[derive(Debug, Deserialize)]
pub struct BudgetHintJson {
    pub max_tokens: Option<u32>,
    pub max_latency_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct PostCallRequest {
    pub audit_id: String,
    pub content: String,
    pub tokens_used: Option<u32>,
    pub latency_ms: Option<u64>,
}

// === Response types ===

#[derive(Debug, Serialize)]
pub struct DecideResponse {
    pub decision: String,
    pub policy_id: String,
    pub reason: Option<String>,
    pub review_reason: Option<String>,
    pub policy_ref: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PreCallResponse {
    pub decision: String,
    pub trace_id: String,
    pub audit_id: String,
    pub policy_id: String,
    pub envelope: Option<EnvelopeJson>,
    pub review_reason: Option<String>,
    pub policy_ref: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EnvelopeJson {
    pub trace_id: String,
    pub audit_id: String,
    pub backend_handle_id: String,
    pub prompt: String,
    pub policy_ref: String,
    pub budget_max_tokens: u32,
    pub budget_max_latency_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct PostCallResponse {
    pub status: String,
    pub risk_class: String,
    pub executable_by_agent: bool,
    pub schema_valid: bool,
    pub audit_id: String,
}

#[derive(Debug, Serialize)]
pub struct FullCallResponse {
    pub decision: String,
    pub post_call: PostCallResponse,
    pub backend_content: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuditResponse {
    pub audit_id: String,
    pub events: Vec<AuditEventJson>,
}

#[derive(Debug, Serialize)]
pub struct AuditEventJson {
    pub kind: String,
    pub trace_id: String,
    pub timestamp: String,
    pub details: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

// === Handlers ===

/// GET /healthz - liveness probe.
pub async fn healthz() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({ "ok": true, "status": "ok" })),
    )
}

/// GET /readyz - readiness probe.
///
/// When the durable audit store is wired (Phase 2, `THALAMUS_DATABASE_URL`),
/// readiness requires a live Jaguar probe plus a healthy emit path — the
/// pilot must not serve traffic without authoritative audit writes. Without
/// a durable store, in-memory semantics apply and the service is ready as
/// soon as it is wired.
pub async fn readyz(State(state): State<Arc<AppState>>) -> Response {
    let backend_configured = state.backend_port.is_some();
    let audit_reachable = match &state.durable_audit {
        Some(durable) => durable.probe() && durable.healthy(),
        None => true,
    };
    let ok = audit_reachable;
    let body = serde_json::json!({
        "ok": ok,
        "status": if ok { "ready" } else { "audit_unavailable" },
        "policy_loaded": true,
        "audit_reachable": audit_reachable,
        "durable_audit": state.durable_audit.is_some(),
        "backend_configured": backend_configured,
    });
    let code = if ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (code, Json(body)).into_response()
}

/// Structured 503 for a configured-but-unavailable authoritative audit store.
fn audit_unavailable() -> Response {
    let resp = ErrorResponse {
        error: "authoritative audit store unavailable".to_owned(),
        code: "AUDIT_UNAVAILABLE".to_owned(),
    };
    (StatusCode::SERVICE_UNAVAILABLE, Json(resp)).into_response()
}

/// Fail closed after flow emits: if the durable store is wired but the emit
/// path is unhealthy, the response must not pretend the call was audited.
fn durable_audit_guard(state: &AppState) -> Option<Response> {
    match &state.durable_audit {
        Some(durable) if !durable.healthy() => Some(audit_unavailable()),
        _ => None,
    }
}

/// GET /rbx/v1/identity - gated by the credential middleware. Returns the
/// validated caller, proving the presented opaque Thalamus session credential
/// was accepted (ADR-0101). The `/rbx/v1/sessions` and `/rbx/v1/runs` routes
/// land in Phase 3 behind the same `THALAMUS_RBX_API` flag.
pub async fn rbx_identity(Extension(caller): Extension<VerifiedCaller>) -> impl IntoResponse {
    (StatusCode::OK, Json(caller))
}

/// POST /v1/decide — policy decision only, no backend call.
pub async fn decide(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DecideRequest>,
) -> impl IntoResponse {
    let call_request = decide_to_call_request(&req);
    let policy = state.policy_port.resolve(&call_request);
    let decision = state.policy_port.evaluate(&call_request, &policy);

    let resp = match &decision {
        PolicyDecision::Allow => DecideResponse {
            decision: "Allow".to_owned(),
            policy_id: policy.id.clone(),
            reason: None,
            review_reason: None,
            policy_ref: None,
        },
        PolicyDecision::Deny { reason, policy_ref } => DecideResponse {
            decision: "Deny".to_owned(),
            policy_id: policy.id.clone(),
            reason: Some(reason.clone()),
            review_reason: None,
            policy_ref: Some(policy_ref.clone()),
        },
        PolicyDecision::AllowWithReview {
            review_reason,
            policy_ref,
        } => DecideResponse {
            decision: "AllowWithReview".to_owned(),
            policy_id: policy.id.clone(),
            reason: None,
            review_reason: Some(review_reason.clone()),
            policy_ref: Some(policy_ref.clone()),
        },
    };

    (StatusCode::OK, Json(resp))
}

/// POST /v1/pre-call — pre-call phase: decision + envelope (when allowed).
pub async fn pre_call(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DecideRequest>,
) -> Response {
    let call_request = decide_to_call_request(&req);

    let outcome = thalamus_core::pre_call(
        &call_request,
        state.policy_port.as_ref(),
        state.context_port.as_ref(),
        state.audit_port.as_ref(),
        state.obs_port.as_ref(),
    );

    match outcome {
        Ok(outcome) => {
            if let Some(resp) = durable_audit_guard(&state) {
                return resp;
            }

            // Store pre-call record for post-call correlation (durable when
            // the Phase 2 store is wired; in-memory otherwise)
            if let Some(ref envelope) = outcome.envelope {
                match &state.durable_audit {
                    Some(durable) => {
                        if durable
                            .store_pre_call_record(&outcome.audit_id, envelope, &outcome.policy)
                            .is_err()
                        {
                            return audit_unavailable();
                        }
                    }
                    None => state.audit_store.store_pre_call_record(
                        outcome.audit_id.clone(),
                        envelope.clone(),
                        outcome.policy.clone(),
                    ),
                }
            }

            let envelope_json = outcome.envelope.as_ref().map(|e| EnvelopeJson {
                trace_id: e.trace_id.0.to_string(),
                audit_id: e.audit_id.0.to_string(),
                backend_handle_id: e.backend_handle.id.clone(),
                prompt: e.prompt.clone(),
                policy_ref: e.policy_ref.clone(),
                budget_max_tokens: e.budget.max_tokens,
                budget_max_latency_ms: e.budget.max_latency_ms,
            });

            let (decision_str, review_reason, policy_ref) = match &outcome.decision {
                PolicyDecision::Allow => ("Allow".to_owned(), None, None),
                PolicyDecision::Deny {
                    reason,
                    policy_ref: pr,
                } => ("Deny".to_owned(), None, Some(format!("{}: {}", pr, reason))),
                PolicyDecision::AllowWithReview {
                    review_reason: rr,
                    policy_ref: pr,
                } => (
                    "AllowWithReview".to_owned(),
                    Some(rr.clone()),
                    Some(pr.clone()),
                ),
            };

            let resp = PreCallResponse {
                decision: decision_str,
                trace_id: outcome.trace_id.0.to_string(),
                audit_id: outcome.audit_id.0.to_string(),
                policy_id: outcome.policy.id.clone(),
                envelope: envelope_json,
                review_reason,
                policy_ref,
            };

            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(PreCallError::NoPermittedBackend {
            tenant,
            product,
            policy_id: _,
        }) => {
            let resp = ErrorResponse {
                error: format!(
                    "no permitted backends for tenant {} product {}",
                    tenant, product
                ),
                code: "NO_PERMITTED_BACKENDS".to_owned(),
            };
            (StatusCode::UNPROCESSABLE_ENTITY, Json(resp)).into_response()
        }
    }
}

/// POST /v1/post-call — validate an externally-executed response.
///
/// Correlates policy/budget from the in-memory audit store by audit_id.
/// The caller supplies only audit_id and the response data; the envelope
/// and policy come from the pre-call record stored during /v1/pre-call
/// or /v1/call. Unknown audit_id => structured 4xx.
pub async fn post_call(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PostCallRequest>,
) -> Response {
    let audit_id = match Uuid::parse_str(&req.audit_id) {
        Ok(u) => AuditId(u),
        Err(_) => {
            let resp = ErrorResponse {
                error: "invalid audit id".to_owned(),
                code: "INVALID_AUDIT_ID".to_owned(),
            };
            return (StatusCode::BAD_REQUEST, Json(resp)).into_response();
        }
    };

    let record = match &state.durable_audit {
        Some(durable) => match durable.get_pre_call_record(&audit_id) {
            Err(_) => return audit_unavailable(),
            Ok(Some(r)) => r,
            Ok(None) => {
                let resp = ErrorResponse {
                    error: format!("no pre-call record found for audit_id {}", req.audit_id),
                    code: "UNKNOWN_AUDIT_ID".to_owned(),
                };
                return (StatusCode::NOT_FOUND, Json(resp)).into_response();
            }
        },
        None => match state.audit_store.get_pre_call_record(&audit_id) {
            Some(r) => r,
            None => {
                let resp = ErrorResponse {
                    error: format!("no pre-call record found for audit_id {}", req.audit_id),
                    code: "UNKNOWN_AUDIT_ID".to_owned(),
                };
                return (StatusCode::NOT_FOUND, Json(resp)).into_response();
            }
        },
    };

    let response = BackendResponse {
        content: req.content,
        tokens_used: req.tokens_used,
        latency_ms: req.latency_ms,
    };

    let result = thalamus_core::post_call(
        &response,
        &record.envelope,
        &record.policy,
        state.audit_port.as_ref(),
        state.eval_port.as_ref(),
        state.obs_port.as_ref(),
    );

    if let Some(resp) = durable_audit_guard(&state) {
        return resp;
    }

    let resp = PostCallResponse {
        status: format!("{:?}", result.status),
        risk_class: format!("{:?}", result.risk_class),
        executable_by_agent: result.executable_by_agent,
        schema_valid: result.schema_valid,
        audit_id: audit_id.0.to_string(),
    };

    (StatusCode::OK, Json(resp)).into_response()
}

/// POST /v1/call — full round-trip: pre_call -> BackendPort -> post_call.
///
/// Structural enforcement:
/// - Deny => no BackendPort call, structured deny response
/// - AllowWithReview => no BackendPort call, NeedsHumanReview + review id
/// - Allow => BackendPort call, post_call always runs, PostCallResult returned
/// - No backend configured => structured 503
/// - Empty permitted_backends on Allow => typed 4xx, no panic
pub async fn full_call(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DecideRequest>,
) -> Response {
    let call_request = decide_to_call_request(&req);

    let outcome = match thalamus_core::pre_call(
        &call_request,
        state.policy_port.as_ref(),
        state.context_port.as_ref(),
        state.audit_port.as_ref(),
        state.obs_port.as_ref(),
    ) {
        Ok(o) => o,
        Err(PreCallError::NoPermittedBackend {
            tenant,
            product,
            policy_id,
        }) => {
            let resp = ErrorResponse {
                error: format!(
                    "no permitted backends for tenant {} product {} (policy {})",
                    tenant, product, policy_id
                ),
                code: "NO_PERMITTED_BACKENDS".to_owned(),
            };
            return (StatusCode::UNPROCESSABLE_ENTITY, Json(resp)).into_response();
        }
    };

    if let Some(resp) = durable_audit_guard(&state) {
        return resp;
    }

    // Store pre-call record for post-call correlation (Allow/AllowWithReview)
    if let Some(ref envelope) = outcome.envelope {
        match &state.durable_audit {
            Some(durable) => {
                if durable
                    .store_pre_call_record(&outcome.audit_id, envelope, &outcome.policy)
                    .is_err()
                {
                    return audit_unavailable();
                }
            }
            None => state.audit_store.store_pre_call_record(
                outcome.audit_id.clone(),
                envelope.clone(),
                outcome.policy.clone(),
            ),
        }
    }

    match &outcome.decision {
        PolicyDecision::Deny { reason, policy_ref } => {
            // NO backend call on Deny
            let post_resp = PostCallResponse {
                status: "Denied".to_owned(),
                risk_class: "N/A".to_owned(),
                executable_by_agent: false,
                schema_valid: false,
                audit_id: outcome.audit_id.0.to_string(),
            };
            let resp = FullCallResponse {
                decision: format!("Deny: {} (ref: {})", reason, policy_ref),
                post_call: post_resp,
                backend_content: None,
            };
            return (StatusCode::OK, Json(resp)).into_response();
        }
        PolicyDecision::AllowWithReview {
            review_reason,
            policy_ref,
        } => {
            // NO backend call on AllowWithReview
            let post_resp = PostCallResponse {
                status: "NeedsHumanReview".to_owned(),
                risk_class: "N/A".to_owned(),
                executable_by_agent: false,
                schema_valid: false,
                audit_id: outcome.audit_id.0.to_string(),
            };
            let resp = FullCallResponse {
                decision: format!(
                    "AllowWithReview: {} (ref: {}, review_id: {})",
                    review_reason, policy_ref, outcome.audit_id.0
                ),
                post_call: post_resp,
                backend_content: None,
            };
            return (StatusCode::OK, Json(resp)).into_response();
        }
        PolicyDecision::Allow => {}
    }

    // Allow path: backend call + mandatory post_call
    let envelope = match outcome.envelope.as_ref() {
        Some(e) => e,
        None => {
            tracing::error!(
                audit_id = %outcome.audit_id.0,
                "Allow decision produced no envelope — invariant violation"
            );
            let resp = ErrorResponse {
                error: "allow decision produced no envelope".to_owned(),
                code: "INVARIANT_VIOLATION".to_owned(),
            };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(resp)).into_response();
        }
    };

    let backend = match state.backend_port.as_ref() {
        Some(b) => std::sync::Arc::clone(b),
        None => {
            let resp = ErrorResponse {
                error: "no backend configured".to_owned(),
                code: "NO_BACKEND".to_owned(),
            };
            return (StatusCode::SERVICE_UNAVAILABLE, Json(resp)).into_response();
        }
    };

    // Run the synchronous BackendPort off the async runtime so a slow data
    // plane (real LLM latency) cannot starve the tokio worker.
    let envelope_for_backend = envelope.clone();
    let backend_response =
        match tokio::task::spawn_blocking(move || backend.call(&envelope_for_backend)).await {
            Ok(r) => r,
            Err(_join_err) => {
                tracing::error!(
                    audit_id = %outcome.audit_id.0,
                    "backend blocking task panicked"
                );
                let resp = ErrorResponse {
                    error: "backend execution task failed".to_owned(),
                    code: "BACKEND_TASK_FAILED".to_owned(),
                };
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(resp)).into_response();
            }
        };

    // post_call is non-bypassable on the Allow path
    let post_result = thalamus_core::post_call(
        &backend_response,
        envelope,
        &outcome.policy,
        state.audit_port.as_ref(),
        state.eval_port.as_ref(),
        state.obs_port.as_ref(),
    );

    if let Some(resp) = durable_audit_guard(&state) {
        return resp;
    }

    let post_resp = PostCallResponse {
        status: format!("{:?}", post_result.status),
        risk_class: format!("{:?}", post_result.risk_class),
        executable_by_agent: post_result.executable_by_agent,
        schema_valid: post_result.schema_valid,
        audit_id: outcome.audit_id.0.to_string(),
    };

    let resp = FullCallResponse {
        decision: "Allow".to_owned(),
        post_call: post_resp,
        backend_content: Some(backend_response.content),
    };

    (StatusCode::OK, Json(resp)).into_response()
}

/// GET /v1/audit/{id} — retrieve audit events for an audit_id.
pub async fn get_audit(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let audit_id = match Uuid::parse_str(&id) {
        Ok(u) => AuditId(u),
        Err(_) => {
            let resp = ErrorResponse {
                error: "invalid audit id".to_owned(),
                code: "INVALID_AUDIT_ID".to_owned(),
            };
            return (StatusCode::BAD_REQUEST, Json(resp)).into_response();
        }
    };

    let events = match &state.durable_audit {
        Some(durable) => match durable.events_by_audit_id(&audit_id) {
            Ok(events) => events,
            Err(_) => return audit_unavailable(),
        },
        None => state.audit_store.get_by_audit_id(&audit_id),
    };
    let event_jsons: Vec<AuditEventJson> = events
        .iter()
        .map(|e| match e {
            thalamus_core::AuditEvent::PreCallDecision {
                trace_id,
                audit_id: _,
                tenant,
                product,
                workflow,
                policy_ref,
                decision,
                backend,
                timestamp,
            } => AuditEventJson {
                kind: "PreCallDecision".to_owned(),
                trace_id: trace_id.0.to_string(),
                timestamp: timestamp.to_string(),
                details: serde_json::json!({
                    "tenant": tenant,
                    "product": product,
                    "workflow": workflow,
                    "policy_ref": policy_ref,
                    "decision": decision,
                    "backend": backend.as_ref().map(|b| &b.id),
                }),
            },
            thalamus_core::AuditEvent::PostCallOutcome {
                trace_id,
                audit_id: _,
                status,
                risk_class,
                executable_by_agent,
                schema_valid,
                timestamp,
            } => AuditEventJson {
                kind: "PostCallOutcome".to_owned(),
                trace_id: trace_id.0.to_string(),
                timestamp: timestamp.to_string(),
                details: serde_json::json!({
                    "status": status,
                    "risk_class": format!("{:?}", risk_class),
                    "executable_by_agent": executable_by_agent,
                    "schema_valid": schema_valid,
                }),
            },
            thalamus_core::AuditEvent::Lifecycle {
                trace_id,
                audit_id: _,
                entity_type,
                entity_id,
                action,
                principal,
                timestamp,
            } => AuditEventJson {
                kind: "Lifecycle".to_owned(),
                trace_id: trace_id.0.to_string(),
                timestamp: timestamp.to_string(),
                details: serde_json::json!({
                    "entity_type": entity_type,
                    "entity_id": entity_id,
                    "action": action,
                    "principal": principal,
                }),
            },
        })
        .collect();

    let resp = AuditResponse {
        audit_id: id,
        events: event_jsons,
    };

    (StatusCode::OK, Json(resp)).into_response()
}

// === /rbx/v1 session/run lifecycle (master plan §3, slice 1) ===
//
// All handlers run behind the credential middleware: the VerifiedCaller
// extension is always present, and a session/run is never created for an
// unverified caller. Principal and delegation token id come from the verified
// credential, never from the request body.

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub tenant: String,
    pub product: String,
    pub workflow: String,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct CreateRunRequest {
    #[serde(default)]
    pub model_alias: Option<String>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

/// P0 typed-error response (standard section 11 shape).
fn typed_error(status: StatusCode, code: &str, message: &str, retryable: bool) -> Response {
    let body = serde_json::json!({
        "error": { "code": code, "message": message, "retryable": retryable }
    });
    (status, Json(body)).into_response()
}

fn store_error(message: &str) -> Response {
    typed_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "store_unavailable",
        message,
        true,
    )
}

/// Emit a lifecycle audit event. The audit stream is the session id, so one
/// session's whole lifecycle forms a single hash chain in the durable store.
fn emit_lifecycle(
    state: &AppState,
    session_id: &Uuid,
    entity_type: &str,
    entity_id: &Uuid,
    action: &str,
    principal: Option<&str>,
) {
    state.audit_port.emit(thalamus_core::AuditEvent::Lifecycle {
        trace_id: thalamus_core::TraceId(Uuid::new_v4()),
        audit_id: AuditId(*session_id),
        entity_type: entity_type.to_owned(),
        entity_id: entity_id.to_string(),
        action: action.to_owned(),
        principal: principal.map(str::to_owned),
        timestamp: time::OffsetDateTime::now_utc(),
    });
}

/// POST /rbx/v1/sessions — create a governed session for the verified caller.
pub async fn rbx_create_session(
    State(state): State<Arc<AppState>>,
    Extension(caller): Extension<VerifiedCaller>,
    Json(req): Json<CreateSessionRequest>,
) -> Response {
    let input = crate::ports::sessions::NewSession {
        tenant: req.tenant,
        product: req.product,
        workflow: req.workflow,
        principal: caller.subject.clone(),
        delegation_token_id: caller.jti.clone(),
        idempotency_key: req.idempotency_key,
    };
    match state.session_store.create_session(&input) {
        Ok(record) => {
            emit_lifecycle(
                &state,
                &record.session_id,
                "session",
                &record.session_id,
                "session_created",
                record.principal.as_deref(),
            );
            if let Some(resp) = durable_audit_guard(&state) {
                return resp;
            }
            (StatusCode::CREATED, Json(record)).into_response()
        }
        Err(err) => store_error(&err),
    }
}

fn parse_uuid(raw: &str) -> Option<Uuid> {
    Uuid::parse_str(raw).ok()
}

fn invalid_id(what: &str) -> Response {
    typed_error(
        StatusCode::BAD_REQUEST,
        "invalid_request",
        &format!("invalid {what} id"),
        false,
    )
}

/// POST /rbx/v1/sessions/{session_id}/runs — refused when the session is
/// unknown/closed or a governing budget is exhausted (§3 acceptance:
/// budget-exceeded blocks new runs).
pub async fn rbx_create_run(
    State(state): State<Arc<AppState>>,
    Extension(caller): Extension<VerifiedCaller>,
    Path(session_id): Path<String>,
    body: Option<Json<CreateRunRequest>>,
) -> Response {
    let Some(session_id) = parse_uuid(&session_id) else {
        return invalid_id("session");
    };
    let req = body.map(|Json(b)| b).unwrap_or_default();
    match state.session_store.create_run(
        &session_id,
        req.model_alias.as_deref(),
        req.idempotency_key.as_deref(),
    ) {
        Ok(record) => {
            emit_lifecycle(
                &state,
                &session_id,
                "run",
                &record.run_id,
                "run_created",
                caller.subject.as_deref(),
            );
            if let Some(resp) = durable_audit_guard(&state) {
                return resp;
            }
            (StatusCode::CREATED, Json(record)).into_response()
        }
        Err(crate::ports::sessions::CreateRunError::UnknownSession) => typed_error(
            StatusCode::NOT_FOUND,
            "unknown_session",
            "no session with this id",
            false,
        ),
        Err(crate::ports::sessions::CreateRunError::SessionClosed) => {
            emit_lifecycle(
                &state,
                &session_id,
                "run",
                &session_id,
                "run_refused_session_closed",
                caller.subject.as_deref(),
            );
            typed_error(
                StatusCode::CONFLICT,
                "session_closed",
                "session is closed",
                false,
            )
        }
        Err(crate::ports::sessions::CreateRunError::BudgetExceeded {
            scope_type,
            scope_ref,
        }) => {
            emit_lifecycle(
                &state,
                &session_id,
                "run",
                &session_id,
                "run_refused_budget_exceeded",
                caller.subject.as_deref(),
            );
            typed_error(
                StatusCode::TOO_MANY_REQUESTS,
                "budget_exceeded",
                &format!("budget exhausted for {scope_type} {scope_ref}"),
                false,
            )
        }
        Err(crate::ports::sessions::CreateRunError::Store(err)) => store_error(&err),
    }
}

/// POST /rbx/v1/sessions/{session_id}/close — idempotent.
pub async fn rbx_close_session(
    State(state): State<Arc<AppState>>,
    Extension(caller): Extension<VerifiedCaller>,
    Path(session_id): Path<String>,
) -> Response {
    let Some(session_id) = parse_uuid(&session_id) else {
        return invalid_id("session");
    };
    match state.session_store.close_session(&session_id) {
        Ok(Some(record)) => {
            emit_lifecycle(
                &state,
                &session_id,
                "session",
                &session_id,
                "session_closed",
                caller.subject.as_deref(),
            );
            if let Some(resp) = durable_audit_guard(&state) {
                return resp;
            }
            (StatusCode::OK, Json(record)).into_response()
        }
        Ok(None) => typed_error(
            StatusCode::NOT_FOUND,
            "unknown_session",
            "no session with this id",
            false,
        ),
        Err(err) => store_error(&err),
    }
}

/// POST /rbx/v1/runs/{run_id}/cancel — idempotent for finished runs.
pub async fn rbx_cancel_run(
    State(state): State<Arc<AppState>>,
    Extension(caller): Extension<VerifiedCaller>,
    Path(run_id): Path<String>,
) -> Response {
    let Some(run_id) = parse_uuid(&run_id) else {
        return invalid_id("run");
    };
    match state.session_store.cancel_run(&run_id) {
        Ok(Some(record)) => {
            emit_lifecycle(
                &state,
                &record.session_id,
                "run",
                &run_id,
                "run_cancelled",
                caller.subject.as_deref(),
            );
            if let Some(resp) = durable_audit_guard(&state) {
                return resp;
            }
            (StatusCode::OK, Json(record)).into_response()
        }
        Ok(None) => typed_error(
            StatusCode::NOT_FOUND,
            "unknown_run",
            "no run with this id",
            false,
        ),
        Err(err) => store_error(&err),
    }
}

/// GET /rbx/v1/sessions/{session_id}/limits — governing budgets + context
/// policy (initial 70% utilization policy per §3 acceptance).
pub async fn rbx_session_limits(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Response {
    let Some(session_id) = parse_uuid(&session_id) else {
        return invalid_id("session");
    };
    match state.session_store.session_limits(&session_id) {
        Ok(Some(limits)) => (StatusCode::OK, Json(limits)).into_response(),
        Ok(None) => typed_error(
            StatusCode::NOT_FOUND,
            "unknown_session",
            "no session with this id",
            false,
        ),
        Err(err) => store_error(&err),
    }
}

// === Helpers ===

fn decide_to_call_request(req: &DecideRequest) -> CallRequest {
    CallRequest {
        tenant: req.tenant.clone(),
        product: req.product.clone(),
        user: req.user.clone(),
        workflow: req.workflow.clone(),
        intent: req.intent.clone(),
        prompt: req.prompt.clone(),
        requested_backend: req.requested_backend.as_ref().map(|b| {
            let bt = match b.backend_type.as_str() {
                "Model" => thalamus_core::BackendType::Model,
                "Tool" => thalamus_core::BackendType::Tool,
                "McpServer" => thalamus_core::BackendType::McpServer,
                "A2AAgent" => thalamus_core::BackendType::A2AAgent,
                other => thalamus_core::BackendType::Custom(other.to_owned()),
            };
            thalamus_core::BackendHandle {
                id: b.id.clone(),
                backend_type: bt,
            }
        }),
        budget_hint: req.budget_hint.as_ref().map(|h| thalamus_core::BudgetHint {
            max_tokens: h.max_tokens,
            max_latency_ms: h.max_latency_ms,
        }),
    }
}
