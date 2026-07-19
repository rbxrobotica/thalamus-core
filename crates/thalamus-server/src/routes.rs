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
    /// Typed backend failure (additive; absent on success). Legacy clients
    /// that only read `backend_content` keep working.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_error: Option<serde_json::Value>,
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
    let identity_reachable = match &state.credential_verifier {
        Some(verifier) => verifier.probe().await,
        None => true,
    };
    let ok = audit_reachable && identity_reachable;
    let status = if !audit_reachable {
        "audit_unavailable"
    } else if !identity_reachable {
        "identity_unavailable"
    } else {
        "ready"
    };
    let body = serde_json::json!({
        "ok": ok,
        "status": status,
        "policy_loaded": true,
        "audit_reachable": audit_reachable,
        "durable_audit": state.durable_audit.is_some(),
        "identity_verifier": state.credential_verifier.is_some(),
        "identity_reachable": identity_reachable,
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
                backend_error: None,
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
                backend_error: None,
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

    // Build and audit the route envelope BEFORE the backend executes (§3
    // acceptance: the route envelope is audited for every model call).
    let route = thalamus_core::RouteEnvelope::from_envelope(envelope);
    state
        .audit_port
        .emit(thalamus_core::AuditEvent::RouteEnvelope {
            trace_id: envelope.trace_id.clone(),
            audit_id: outcome.audit_id.clone(),
            model_alias: route.model_alias.clone(),
            provider_pool: route.provider_pool.clone(),
            region: route.region.clone(),
            data_class: route.data_class.clone(),
            capability_class: route.capability_class.clone(),
            cost_class: route.cost_class.clone(),
            timeout_ms: route.timeout_ms,
            timestamp: time::OffsetDateTime::now_utc(),
        });

    // Run the synchronous BackendPort off the async runtime so a slow data
    // plane (real LLM latency) cannot starve the tokio worker. Typed backend
    // failures degrade to the legacy empty-response shape (post_call still
    // runs, /v1/call clients are not broken) and surface additively in
    // `backend_error`.
    let cancel = thalamus_core::CancelToken::new();
    let execution =
        match tokio::task::spawn_blocking(move || backend.execute(&route, &cancel)).await {
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

    let (backend_response, backend_error) = match execution {
        Ok(exec) => (
            BackendResponse {
                content: exec.content,
                tokens_used: exec.usage.total_tokens,
                latency_ms: Some(exec.latency_ms),
            },
            None,
        ),
        Err(err) => {
            tracing::error!(
                audit_id = %outcome.audit_id.0,
                code = err.code(),
                error = %crate::redact::redact(&err.to_string()),
                "backend execution failed"
            );
            (
                BackendResponse {
                    content: String::new(),
                    tokens_used: None,
                    latency_ms: None,
                },
                Some(serde_json::json!({
                    "code": err.code(),
                    "message": err.to_string(),
                })),
            )
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
        backend_error,
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
                user,
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
                    "user": user,
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
            thalamus_core::AuditEvent::RouteEnvelope {
                trace_id,
                audit_id: _,
                model_alias,
                provider_pool,
                region,
                data_class,
                capability_class,
                cost_class,
                timeout_ms,
                timestamp,
            } => AuditEventJson {
                kind: "RouteEnvelope".to_owned(),
                trace_id: trace_id.0.to_string(),
                timestamp: timestamp.to_string(),
                details: serde_json::json!({
                    "model_alias": model_alias,
                    "provider_pool": provider_pool,
                    "region": region,
                    "data_class": data_class,
                    "capability_class": capability_class,
                    "cost_class": cost_class,
                    "timeout_ms": timeout_ms,
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

// === POST /v1/call/stream — SSE streaming variant of /v1/call (§3 slice 3) ===
//
// Event sequence: `decision` first; on Allow, `chunk` events carry content
// deltas as the backend streams them, then `result` carries the post-call
// summary + usage (or a typed `backend_error`). Deny/AllowWithReview produce
// `decision` + `result` with no chunks and no backend call. Client disconnect
// cancels the backend stream mid-flight through the CancelToken.

/// POST /v1/call/stream — full round-trip with SSE streaming.
pub async fn full_call_stream(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DecideRequest>,
) -> Response {
    use axum::response::sse::{Event, KeepAlive, Sse};
    use tokio_stream::wrappers::ReceiverStream;
    use tokio_stream::StreamExt;

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
            tenant, product, ..
        }) => {
            let resp = ErrorResponse {
                error: format!(
                    "no permitted backends for tenant {} product {}",
                    tenant, product
                ),
                code: "NO_PERMITTED_BACKENDS".to_owned(),
            };
            return (StatusCode::UNPROCESSABLE_ENTITY, Json(resp)).into_response();
        }
    };

    if let Some(resp) = durable_audit_guard(&state) {
        return resp;
    }

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

    let (tx, rx) = tokio::sync::mpsc::channel::<Event>(32);
    let state_task = Arc::clone(&state);
    tokio::spawn(async move {
        stream_call_events(state_task, outcome, tx).await;
    });

    Sse::new(ReceiverStream::new(rx).map(Ok::<_, std::convert::Infallible>))
        .keep_alive(KeepAlive::default())
        .into_response()
}

async fn stream_call_events(
    state: Arc<AppState>,
    outcome: thalamus_core::PreCallOutcome,
    tx: tokio::sync::mpsc::Sender<axum::response::sse::Event>,
) {
    use axum::response::sse::Event;

    let audit_id = outcome.audit_id.0.to_string();
    let (decision_str, terminal_status) = match &outcome.decision {
        PolicyDecision::Allow => ("Allow", None),
        PolicyDecision::Deny { .. } => ("Deny", Some("Denied")),
        PolicyDecision::AllowWithReview { .. } => ("AllowWithReview", Some("NeedsHumanReview")),
    };
    let decision_event = Event::default().event("decision").data(
        serde_json::json!({
            "decision": decision_str,
            "audit_id": audit_id,
            "trace_id": outcome.trace_id.0.to_string(),
            "policy_id": outcome.policy.id,
        })
        .to_string(),
    );
    if tx.send(decision_event).await.is_err() {
        return;
    }

    // Deny / AllowWithReview: no backend call (structural enforcement).
    if let Some(status) = terminal_status {
        let _ = tx
            .send(
                Event::default().event("result").data(
                    serde_json::json!({ "status": status, "audit_id": audit_id }).to_string(),
                ),
            )
            .await;
        return;
    }

    let Some(envelope) = outcome.envelope.clone() else {
        let _ = tx
            .send(
                Event::default().event("error").data(
                    serde_json::json!({
                        "code": "INVARIANT_VIOLATION",
                        "message": "allow decision produced no envelope",
                    })
                    .to_string(),
                ),
            )
            .await;
        return;
    };
    let Some(backend) = state.backend_port.as_ref().map(Arc::clone) else {
        let _ = tx
            .send(
                Event::default().event("error").data(
                    serde_json::json!({ "code": "NO_BACKEND", "message": "no backend configured" })
                        .to_string(),
                ),
            )
            .await;
        return;
    };

    // §3 acceptance: route envelope audited before the backend executes.
    let route = thalamus_core::RouteEnvelope::from_envelope(&envelope);
    state
        .audit_port
        .emit(thalamus_core::AuditEvent::RouteEnvelope {
            trace_id: envelope.trace_id.clone(),
            audit_id: outcome.audit_id.clone(),
            model_alias: route.model_alias.clone(),
            provider_pool: route.provider_pool.clone(),
            region: route.region.clone(),
            data_class: route.data_class.clone(),
            capability_class: route.capability_class.clone(),
            cost_class: route.cost_class.clone(),
            timeout_ms: route.timeout_ms,
            timestamp: time::OffsetDateTime::now_utc(),
        });

    // Client disconnect drops the SSE receiver; the sink's failed send then
    // cancels the backend stream mid-flight through the token.
    let cancel = thalamus_core::CancelToken::new();
    let cancel_for_sink = cancel.clone();
    let tx_for_sink = tx.clone();
    let execution = tokio::task::spawn_blocking(move || {
        let mut sink = |delta: &str| {
            let event = Event::default()
                .event("chunk")
                .data(serde_json::json!({ "delta": delta }).to_string());
            if tx_for_sink.blocking_send(event).is_err() {
                cancel_for_sink.cancel();
            }
        };
        backend.execute_streaming(&route, &cancel_for_sink, &mut sink)
    })
    .await;

    let execution = match execution {
        Ok(r) => r,
        Err(_join_err) => {
            let _ = tx
                .send(
                    Event::default().event("error").data(
                        serde_json::json!({
                            "code": "BACKEND_TASK_FAILED",
                            "message": "backend execution task failed",
                        })
                        .to_string(),
                    ),
                )
                .await;
            return;
        }
    };

    match execution {
        Ok(exec) => {
            let backend_response = BackendResponse {
                content: exec.content,
                tokens_used: exec.usage.total_tokens,
                latency_ms: Some(exec.latency_ms),
            };
            // post_call is non-bypassable on the Allow path.
            let post_result = thalamus_core::post_call(
                &backend_response,
                &envelope,
                &outcome.policy,
                state.audit_port.as_ref(),
                state.eval_port.as_ref(),
                state.obs_port.as_ref(),
            );
            let _ = tx
                .send(
                    Event::default().event("result").data(
                        serde_json::json!({
                            "status": format!("{:?}", post_result.status),
                            "risk_class": format!("{:?}", post_result.risk_class),
                            "executable_by_agent": post_result.executable_by_agent,
                            "schema_valid": post_result.schema_valid,
                            "audit_id": audit_id,
                            "usage": exec.usage,
                            "latency_ms": exec.latency_ms,
                        })
                        .to_string(),
                    ),
                )
                .await;
        }
        Err(err) => {
            tracing::error!(
                audit_id = %audit_id,
                code = err.code(),
                error = %crate::redact::redact(&err.to_string()),
                "streaming backend execution failed"
            );
            let partial_usage = match &err {
                thalamus_core::BackendCallError::Timeout { partial_usage }
                | thalamus_core::BackendCallError::Cancelled { partial_usage } => {
                    Some(partial_usage.clone())
                }
                _ => None,
            };
            let _ = tx
                .send(
                    Event::default().event("error").data(
                        serde_json::json!({
                            "code": err.code(),
                            "message": err.to_string(),
                            "audit_id": audit_id,
                            "partial_usage": partial_usage,
                        })
                        .to_string(),
                    ),
                )
                .await;
        }
    }
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
    /// Required (Gate D acceptance): every governed session records the mode
    /// it was created under. `governed_llm_access` for external agents.
    pub governance_mode: String,
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
    if req.governance_mode != thalamus_core::GOVERNANCE_MODE_LLM_ACCESS
        && req.governance_mode != thalamus_core::GOVERNANCE_MODE_WORKSPACE
    {
        return typed_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "governance_mode must be governed_llm_access or governed_workspace",
            false,
        );
    }
    let input = crate::ports::sessions::NewSession {
        tenant: req.tenant,
        product: req.product,
        workflow: req.workflow,
        principal: caller.subject.clone(),
        delegation_token_id: caller.jti.clone(),
        governance_mode: req.governance_mode,
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

/// Route lease contract version (rbx.route_lease.v1): the negotiation block
/// returned on run creation so clients can compile their prompt against the
/// resolved alias/profile BEFORE submitting the call payload. The lease is
/// negotiation metadata; enforcement stays at call time (policy + 1:1 claim).
pub const ROUTE_LEASE_SCHEMA_VERSION: &str = "rbx.route_lease.v1";
/// Institutional default prompt profile when the policy pins none.
pub const DEFAULT_PROMPT_PROFILE: &str = "rbx.default.v1";
/// Advisory lease validity window; run/session state stays authoritative.
const ROUTE_LEASE_TTL_SECS: i64 = 1800;

/// Build the route lease for a freshly created run. `status` is `granted`
/// only when the policy resolves and permits the alias the call will use;
/// `no_policy` / `model_not_permitted` tell the client not to compile and
/// call (the call would be refused with the matching typed error).
fn route_lease_for_run(
    state: &AppState,
    run: &thalamus_core::RunRecord,
    session: &thalamus_core::SessionRecord,
) -> serde_json::Value {
    let request = CallRequest {
        tenant: session.tenant.clone(),
        product: session.product.clone(),
        user: session.principal.clone().unwrap_or_default(),
        workflow: session.workflow.clone(),
        intent: "route_lease".to_owned(),
        prompt: String::new(),
        requested_backend: None,
        budget_hint: None,
        run_correlated: true,
    };
    let policy = state.policy_port.resolve(&request);
    // Mirror call-time semantics: a run-pinned alias must be permitted or the
    // call will refuse (model_not_permitted); an unpinned run falls back to
    // the first permitted backend, exactly like select_backend.
    let (status, model_alias) = if policy.id == "no-match" {
        ("no_policy", run.model_alias.clone())
    } else if let Some(alias) = run.model_alias.as_deref() {
        if policy.permitted_backends.iter().any(|b| b.id == alias) {
            ("granted", Some(alias.to_owned()))
        } else {
            ("model_not_permitted", Some(alias.to_owned()))
        }
    } else if let Some(first) = policy.permitted_backends.first() {
        ("granted", Some(first.id.clone()))
    } else {
        ("no_policy", None)
    };
    let issued_at = time::OffsetDateTime::now_utc();
    let expires_at = issued_at + time::Duration::seconds(ROUTE_LEASE_TTL_SECS);
    let format = &time::format_description::well_known::Rfc3339;
    serde_json::json!({
        "schema_version": ROUTE_LEASE_SCHEMA_VERSION,
        "lease_id": Uuid::new_v4(),
        "session_id": session.session_id,
        "run_id": run.run_id,
        "status": status,
        "model_alias": model_alias,
        "prompt_profile_id": policy
            .prompt_profile
            .as_deref()
            .unwrap_or(DEFAULT_PROMPT_PROFILE),
        "capabilities": {
            "streaming": true,
            "payload_kinds": [PAYLOAD_KIND_CHAT_V1],
            "tools": true,
        },
        "context": {
            "max_tokens": policy.budget.max_tokens,
            "max_context_utilization": thalamus_core::DEFAULT_CONTEXT_UTILIZATION_LIMIT,
            "context_policy_ref": thalamus_core::DEFAULT_CONTEXT_POLICY_REF,
        },
        "policy_snapshot_id": policy.id,
        "issued_at": issued_at.format(format).ok(),
        "expires_at": expires_at.format(format).ok(),
    })
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
            // Attach the route lease (rbx.route_lease.v1) as an additive field
            // on the run record: pre-lease clients keep deserializing the
            // record unchanged. Lease lookup failures degrade to the bare
            // record rather than failing a run that was already created.
            let mut body = serde_json::to_value(&record)
                .unwrap_or_else(|_| serde_json::json!({ "run_id": record.run_id }));
            if let Ok(Some((run, session))) = state.session_store.run_with_session(&record.run_id) {
                if let Some(obj) = body.as_object_mut() {
                    obj.insert(
                        "route_lease".to_owned(),
                        route_lease_for_run(&state, &run, &session),
                    );
                }
            }
            (StatusCode::CREATED, Json(body)).into_response()
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

// === /rbx/v1 governance records (§3 slice 4) ===

#[derive(Debug, Deserialize)]
pub struct ToolDecisionRequest {
    pub session_id: String,
    #[serde(default)]
    pub run_id: Option<String>,
    pub tool: String,
    /// `allowed` | `denied`
    pub decision: String,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ApprovalRequest {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
    pub subject: String,
    /// `approved` | `rejected`
    pub decision: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct EvidenceRequest {
    #[serde(default)]
    pub run_id: Option<String>,
    pub kind: String,
    pub uri: String,
    pub content_hash: String,
}

/// `Ok(None)` = absent, `Ok(Some)` = valid, `Err(what)` = present but invalid
/// (caller responds with `invalid_id(what)`).
fn parse_optional_uuid<'a>(raw: &Option<String>, what: &'a str) -> Result<Option<Uuid>, &'a str> {
    match raw {
        None => Ok(None),
        Some(raw) => match parse_uuid(raw) {
            Some(id) => Ok(Some(id)),
            None => Err(what),
        },
    }
}

/// POST /rbx/v1/tool-decisions — record a governed tool-invocation decision.
pub async fn rbx_tool_decision(
    State(state): State<Arc<AppState>>,
    Extension(caller): Extension<VerifiedCaller>,
    Json(req): Json<ToolDecisionRequest>,
) -> Response {
    let Some(session_id) = parse_uuid(&req.session_id) else {
        return invalid_id("session");
    };
    let run_id = match parse_optional_uuid(&req.run_id, "run") {
        Ok(id) => id,
        Err(what) => return invalid_id(what),
    };
    if !matches!(req.decision.as_str(), "allowed" | "denied") {
        return typed_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "decision must be 'allowed' or 'denied'",
            false,
        );
    }
    let input = crate::ports::sessions::ToolDecision {
        session_id,
        run_id,
        tool: req.tool.clone(),
        decision: req.decision.clone(),
        metadata: req.metadata.unwrap_or_else(|| serde_json::json!({})),
    };
    match state.session_store.record_tool_decision(&input) {
        Ok(invocation_id) => {
            emit_lifecycle(
                &state,
                &session_id,
                "tool_invocation",
                &invocation_id,
                &format!("tool_{}:{}", req.decision, req.tool),
                caller.subject.as_deref(),
            );
            if let Some(resp) = durable_audit_guard(&state) {
                return resp;
            }
            (
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "invocation_id": invocation_id,
                    "decision": req.decision,
                })),
            )
                .into_response()
        }
        Err(err) if err == "unknown_session" => typed_error(
            StatusCode::NOT_FOUND,
            "unknown_session",
            "no session with this id",
            false,
        ),
        Err(err) => store_error(&err),
    }
}

/// POST /rbx/v1/approvals — record an approval. The approver is always the
/// verified caller, never taken from the body.
pub async fn rbx_approval(
    State(state): State<Arc<AppState>>,
    Extension(caller): Extension<VerifiedCaller>,
    Json(req): Json<ApprovalRequest>,
) -> Response {
    let session_id = match parse_optional_uuid(&req.session_id, "session") {
        Ok(id) => id,
        Err(what) => return invalid_id(what),
    };
    let run_id = match parse_optional_uuid(&req.run_id, "run") {
        Ok(id) => id,
        Err(what) => return invalid_id(what),
    };
    if !matches!(req.decision.as_str(), "approved" | "rejected") {
        return typed_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "decision must be 'approved' or 'rejected'",
            false,
        );
    }
    let Some(approver) = caller.subject.clone() else {
        return typed_error(
            StatusCode::FORBIDDEN,
            "policy_denied",
            "approvals require a credential with a subject",
            false,
        );
    };
    let input = crate::ports::sessions::ApprovalInput {
        session_id,
        run_id,
        subject: req.subject.clone(),
        approver: approver.clone(),
        decision: req.decision.clone(),
        reason: req.reason.clone(),
        metadata: req.metadata.unwrap_or_else(|| serde_json::json!({})),
    };
    match state.session_store.record_approval(&input) {
        Ok(approval_id) => {
            let stream = session_id.unwrap_or(approval_id);
            emit_lifecycle(
                &state,
                &stream,
                "approval",
                &approval_id,
                &format!("approval_{}", req.decision),
                Some(&approver),
            );
            if let Some(resp) = durable_audit_guard(&state) {
                return resp;
            }
            (
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "approval_id": approval_id,
                    "approver": approver,
                    "decision": req.decision,
                })),
            )
                .into_response()
        }
        Err(err) => store_error(&err),
    }
}

/// POST /rbx/v1/evidence — record an evidence reference (pointer + hash;
/// never the payload).
pub async fn rbx_evidence(
    State(state): State<Arc<AppState>>,
    Extension(caller): Extension<VerifiedCaller>,
    Json(req): Json<EvidenceRequest>,
) -> Response {
    let run_id = match parse_optional_uuid(&req.run_id, "run") {
        Ok(id) => id,
        Err(what) => return invalid_id(what),
    };
    if req.uri.is_empty() || req.content_hash.is_empty() {
        return typed_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "uri and content_hash are required",
            false,
        );
    }
    let input = crate::ports::sessions::EvidenceInput {
        run_id,
        kind: req.kind.clone(),
        uri: req.uri.clone(),
        content_hash: req.content_hash.clone(),
    };
    match state.session_store.record_evidence(&input) {
        Ok(evidence_id) => {
            let stream = run_id.unwrap_or(evidence_id);
            emit_lifecycle(
                &state,
                &stream,
                "evidence",
                &evidence_id,
                &format!("evidence_recorded:{}", req.kind),
                caller.subject.as_deref(),
            );
            if let Some(resp) = durable_audit_guard(&state) {
                return resp;
            }
            (
                StatusCode::CREATED,
                Json(serde_json::json!({ "evidence_id": evidence_id })),
            )
                .into_response()
        }
        Err(err) => store_error(&err),
    }
}

// === Helpers ===

// === /rbx/v1/runs/{run_id}/calls — run-bound governed calls (SLICE-T1, Gate D) ===
//
// The ONLY way to execute a model call on the governed surface. The server
// derives tenant/product/workflow/user from the session record and the model
// from the run record; the request body carries intent + a structured
// `chat.completions.v1` payload only. Invariant chain, in order: verified
// credential (middleware) -> run exists -> caller owns the session (404
// anti-enumeration + ownership_violation audit) -> session open + run active
// -> atomic 1:1 execution claim -> budget -> policy (deny never reaches the
// backend) -> correlated route envelope audited -> execution -> post-call ->
// run finished + usage recorded.

/// The only payload kind the governed call surface accepts in P0.
pub const PAYLOAD_KIND_CHAT_V1: &str = "chat.completions.v1";

#[derive(Debug, Deserialize)]
pub struct RunCallRequest {
    pub intent: String,
    pub payload_kind: String,
    pub payload: serde_json::Value,
}

/// Everything resolved before backend execution of a run-bound call.
struct PreparedRunCall {
    session: thalamus_core::SessionRecord,
    run_id: Uuid,
    outcome: thalamus_core::PreCallOutcome,
    envelope: thalamus_core::Envelope,
    route: thalamus_core::RouteEnvelope,
}

/// Refusal body for the governed call surface: typed error + audit id when a
/// decision was already recorded.
fn run_call_refusal(
    status: StatusCode,
    code: &str,
    message: &str,
    audit_id: Option<&str>,
) -> Response {
    let body = serde_json::json!({
        "error": { "code": code, "message": message, "retryable": false },
        "audit_id": audit_id,
    });
    (status, Json(body)).into_response()
}

/// Mark a claimed run as failed with a refusal reason (best-effort: the
/// refusal response is authoritative even if the store write fails).
fn fail_claimed_run(state: &AppState, run_id: &Uuid, reason: &str, audit_id: Option<&str>) {
    let outcome = serde_json::json!({ "refusal": reason, "audit_id": audit_id });
    if let Err(err) = state
        .session_store
        .finish_run_execution(run_id, "failed", &outcome)
    {
        tracing::error!(%run_id, error = %crate::redact::redact(&err), "failed to finalize refused run");
    }
}

/// Validate + authorize + claim + decide a run-bound call. Every refusal
/// returns the full typed response; on success the backend is ready to
/// execute within the audited route envelope.
#[allow(
    clippy::result_large_err,
    reason = "the Err is a ready axum Response returned straight to the client"
)]
fn prepare_run_call(
    state: &Arc<AppState>,
    caller: &VerifiedCaller,
    raw_run_id: &str,
    req: &RunCallRequest,
) -> Result<PreparedRunCall, Response> {
    if req.payload_kind != PAYLOAD_KIND_CHAT_V1 {
        return Err(typed_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            &format!("payload_kind must be {PAYLOAD_KIND_CHAT_V1}"),
            false,
        ));
    }
    let Some(payload) = req.payload.as_object() else {
        return Err(typed_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "payload must be an object",
            false,
        ));
    };
    if payload.contains_key("model") {
        return Err(typed_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "payload.model is not allowed: the model comes from the run",
            false,
        ));
    }
    if payload
        .get("messages")
        .and_then(|m| m.as_array())
        .is_none_or(|m| m.is_empty())
    {
        return Err(typed_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "payload.messages must be a non-empty array",
            false,
        ));
    }
    let Some(run_id) = parse_uuid(raw_run_id) else {
        return Err(invalid_id("run"));
    };
    if let Some(resp) = durable_audit_guard(state) {
        return Err(resp);
    }

    // Ownership before any state change (I2/I4): a caller must never be able
    // to claim, probe or fail another principal's run. Mismatches and unknown
    // runs are indistinguishable (404) to prevent run-id enumeration.
    let lookup = state
        .session_store
        .run_with_session(&run_id)
        .map_err(|e| store_error(&e))?;
    let Some((_, owner_session)) = lookup else {
        return Err(typed_error(
            StatusCode::NOT_FOUND,
            "not_found",
            "unknown run",
            false,
        ));
    };
    let caller_subject = caller.subject.as_deref();
    if caller_subject.is_none() || owner_session.principal.as_deref() != caller_subject {
        emit_lifecycle(
            state,
            &owner_session.session_id,
            "call",
            &run_id,
            "ownership_violation",
            caller_subject,
        );
        tracing::warn!(
            %run_id,
            session = %owner_session.session_id,
            "run-bound call refused: caller does not own the session"
        );
        return Err(typed_error(
            StatusCode::NOT_FOUND,
            "not_found",
            "unknown run",
            false,
        ));
    }

    // Atomic 1:1 execution claim (I5/I6) + budget re-check under lock (I9).
    let (run, session) =
        state
            .session_store
            .claim_run_execution(&run_id)
            .map_err(|err| match err {
                crate::ports::sessions::ClaimRunError::UnknownRun => {
                    typed_error(StatusCode::NOT_FOUND, "not_found", "unknown run", false)
                }
                crate::ports::sessions::ClaimRunError::SessionClosed => typed_error(
                    StatusCode::CONFLICT,
                    "session_closed",
                    "session is closed",
                    false,
                ),
                crate::ports::sessions::ClaimRunError::RunNotActive => typed_error(
                    StatusCode::CONFLICT,
                    "run_closed",
                    "run is not active",
                    false,
                ),
                crate::ports::sessions::ClaimRunError::AlreadyExecuted => typed_error(
                    StatusCode::CONFLICT,
                    "run_already_executed",
                    "a call was already executed on this run",
                    false,
                ),
                crate::ports::sessions::ClaimRunError::BudgetExceeded {
                    scope_type,
                    scope_ref,
                } => typed_error(
                    StatusCode::TOO_MANY_REQUESTS,
                    "budget_exceeded",
                    &format!("budget exhausted for {scope_type} {scope_ref}"),
                    false,
                ),
                crate::ports::sessions::ClaimRunError::Store(err) => store_error(&err),
            })?;

    // Policy sees the serialized payload; the model request comes from the
    // run record, never from the payload (validated above).
    let requested_backend = run
        .model_alias
        .as_ref()
        .map(|alias| thalamus_core::BackendHandle {
            id: alias.clone(),
            backend_type: thalamus_core::BackendType::Model,
        });
    let call_request = CallRequest {
        tenant: session.tenant.clone(),
        product: session.product.clone(),
        user: session.principal.clone().unwrap_or_default(),
        workflow: session.workflow.clone(),
        intent: req.intent.clone(),
        prompt: req.payload.to_string(),
        requested_backend,
        budget_hint: None,
        run_correlated: true,
    };

    let outcome = match thalamus_core::pre_call(
        &call_request,
        state.policy_port.as_ref(),
        state.context_port.as_ref(),
        state.audit_port.as_ref(),
        state.obs_port.as_ref(),
    ) {
        Ok(o) => o,
        Err(PreCallError::NoPermittedBackend { .. }) => {
            fail_claimed_run(state, &run_id, "no_permitted_backends", None);
            return Err(typed_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "no_permitted_backends",
                "policy permits no backends for this session",
                false,
            ));
        }
    };
    let audit_id = outcome.audit_id.0.to_string();

    match &outcome.decision {
        PolicyDecision::Deny { reason, .. } => {
            let reason = reason.clone();
            fail_claimed_run(state, &run_id, "policy_denied", Some(&audit_id));
            emit_lifecycle(
                state,
                &session.session_id,
                "call",
                &outcome.audit_id.0,
                "call_denied",
                session.principal.as_deref(),
            );
            return Err(run_call_refusal(
                StatusCode::FORBIDDEN,
                "policy_denied",
                &reason,
                Some(&audit_id),
            ));
        }
        PolicyDecision::AllowWithReview { review_reason, .. } => {
            let reason = review_reason.clone();
            fail_claimed_run(state, &run_id, "needs_human_review", Some(&audit_id));
            return Err(run_call_refusal(
                StatusCode::FORBIDDEN,
                "needs_human_review",
                &reason,
                Some(&audit_id),
            ));
        }
        PolicyDecision::Allow => {}
    }

    let Some(mut envelope) = outcome.envelope.clone() else {
        fail_claimed_run(state, &run_id, "invariant_violation", Some(&audit_id));
        return Err(typed_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "invariant_violation",
            "allow decision produced no envelope",
            true,
        ));
    };
    // select_backend falls back to the first permitted backend; for run-bound
    // calls the run's model must be honored or refused, never substituted.
    if let Some(alias) = run.model_alias.as_deref() {
        if envelope.backend_handle.id != alias {
            fail_claimed_run(state, &run_id, "model_not_permitted", Some(&audit_id));
            return Err(run_call_refusal(
                StatusCode::UNPROCESSABLE_ENTITY,
                "model_not_permitted",
                &format!("run model '{alias}' is not permitted by policy"),
                Some(&audit_id),
            ));
        }
    }
    envelope.chat_payload = Some(req.payload.clone());

    // Correlated pre-call record: the durable row carries session + run ids
    // so the call is joinable to its lifecycle chain (I8).
    match &state.durable_audit {
        Some(durable) => {
            let stored = durable
                .store_pre_call_record_correlated(
                    &outcome.audit_id,
                    &envelope,
                    &outcome.policy,
                    &session.session_id,
                    &run_id,
                )
                .is_ok();
            if !stored {
                fail_claimed_run(state, &run_id, "audit_unavailable", Some(&audit_id));
                return Err(audit_unavailable());
            }
        }
        None => state.audit_store.store_pre_call_record(
            outcome.audit_id.clone(),
            envelope.clone(),
            outcome.policy.clone(),
        ),
    }

    let route = thalamus_core::RouteEnvelope::from_envelope(&envelope);
    state
        .audit_port
        .emit(thalamus_core::AuditEvent::RouteEnvelope {
            trace_id: envelope.trace_id.clone(),
            audit_id: outcome.audit_id.clone(),
            model_alias: route.model_alias.clone(),
            provider_pool: route.provider_pool.clone(),
            region: route.region.clone(),
            data_class: route.data_class.clone(),
            capability_class: route.capability_class.clone(),
            cost_class: route.cost_class.clone(),
            timeout_ms: route.timeout_ms,
            timestamp: time::OffsetDateTime::now_utc(),
        });
    emit_lifecycle(
        state,
        &session.session_id,
        "call",
        &outcome.audit_id.0,
        "call_started",
        session.principal.as_deref(),
    );

    Ok(PreparedRunCall {
        session,
        run_id,
        outcome,
        envelope,
        route,
    })
}

/// Finalize an executed run-bound call: post-call validation, run state,
/// budget usage and the lifecycle audit trail.
fn finalize_run_call(
    state: &Arc<AppState>,
    prepared: &PreparedRunCall,
    execution: &Result<thalamus_core::BackendExecution, thalamus_core::BackendCallError>,
) -> Option<thalamus_core::PostCallResult> {
    let audit_id = prepared.outcome.audit_id.0.to_string();
    match execution {
        Ok(exec) => {
            let backend_response = BackendResponse {
                content: exec.content.clone(),
                tokens_used: exec.usage.total_tokens,
                latency_ms: Some(exec.latency_ms),
            };
            let post_result = thalamus_core::post_call(
                &backend_response,
                &prepared.envelope,
                &prepared.outcome.policy,
                state.audit_port.as_ref(),
                state.eval_port.as_ref(),
                state.obs_port.as_ref(),
            );
            let outcome_meta = serde_json::json!({
                "audit_id": audit_id,
                "usage": exec.usage,
                "latency_ms": exec.latency_ms,
                "post_call_status": format!("{:?}", post_result.status),
            });
            if let Err(err) = state.session_store.finish_run_execution(
                &prepared.run_id,
                "completed",
                &outcome_meta,
            ) {
                tracing::error!(run_id = %prepared.run_id, error = %crate::redact::redact(&err), "failed to finalize run");
            }
            if let Some(tokens) = exec.usage.total_tokens {
                if let Err(err) = state
                    .session_store
                    .record_usage(&prepared.session.session_id, i64::from(tokens))
                {
                    tracing::error!(
                        session = %prepared.session.session_id,
                        error = %crate::redact::redact(&err),
                        "failed to record usage"
                    );
                }
            }
            emit_lifecycle(
                state,
                &prepared.session.session_id,
                "call",
                &prepared.outcome.audit_id.0,
                "call_completed",
                prepared.session.principal.as_deref(),
            );
            Some(post_result)
        }
        Err(err) => {
            let cancelled = matches!(err, thalamus_core::BackendCallError::Cancelled { .. });
            let final_status = if cancelled { "cancelled" } else { "failed" };
            let partial_usage = match err {
                thalamus_core::BackendCallError::Timeout { partial_usage }
                | thalamus_core::BackendCallError::Cancelled { partial_usage } => {
                    Some(partial_usage.clone())
                }
                _ => None,
            };
            let outcome_meta = serde_json::json!({
                "audit_id": audit_id,
                "backend_error": err.code(),
                "partial_usage": partial_usage,
            });
            if let Err(store_err) = state.session_store.finish_run_execution(
                &prepared.run_id,
                final_status,
                &outcome_meta,
            ) {
                tracing::error!(run_id = %prepared.run_id, error = %crate::redact::redact(&store_err), "failed to finalize run");
            }
            if let Some(tokens) = partial_usage.as_ref().and_then(|u| u.total_tokens) {
                let _ = state
                    .session_store
                    .record_usage(&prepared.session.session_id, i64::from(tokens));
            }
            emit_lifecycle(
                state,
                &prepared.session.session_id,
                "call",
                &prepared.outcome.audit_id.0,
                if cancelled {
                    "call_cancelled"
                } else {
                    "call_failed"
                },
                prepared.session.principal.as_deref(),
            );
            None
        }
    }
}

/// POST /rbx/v1/runs/{run_id}/calls — non-streaming run-bound governed call.
pub async fn rbx_run_call(
    State(state): State<Arc<AppState>>,
    Extension(caller): Extension<VerifiedCaller>,
    Path(run_id): Path<String>,
    Json(req): Json<RunCallRequest>,
) -> Response {
    let prepared = match prepare_run_call(&state, &caller, &run_id, &req) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    let backend = match state.backend_port.as_ref() {
        Some(b) => Arc::clone(b),
        None => {
            fail_claimed_run(&state, &prepared.run_id, "no_backend", None);
            return typed_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "no_backend",
                "no backend configured",
                true,
            );
        }
    };

    let route = prepared.route.clone();
    let cancel = thalamus_core::CancelToken::new();
    let execution =
        match tokio::task::spawn_blocking(move || backend.execute(&route, &cancel)).await {
            Ok(r) => r,
            Err(_join_err) => {
                fail_claimed_run(&state, &prepared.run_id, "backend_task_failed", None);
                return typed_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "backend_task_failed",
                    "backend execution task failed",
                    true,
                );
            }
        };

    let post_result = finalize_run_call(&state, &prepared, &execution);
    if let Some(resp) = durable_audit_guard(&state) {
        return resp;
    }

    let audit_id = prepared.outcome.audit_id.0.to_string();
    match execution {
        Ok(exec) => {
            let post = post_result.expect("post_call ran on success");
            let body = serde_json::json!({
                "decision": "Allow",
                "audit_id": audit_id,
                "trace_id": prepared.outcome.trace_id.0.to_string(),
                "session_id": prepared.session.session_id,
                "run_id": prepared.run_id,
                "content": exec.content,
                "finish_reason": exec.backend_metadata.get("finish_reason"),
                "message": exec.backend_metadata.get("message"),
                "usage": exec.usage,
                "latency_ms": exec.latency_ms,
                "post_call": {
                    "status": format!("{:?}", post.status),
                    "risk_class": format!("{:?}", post.risk_class),
                    "executable_by_agent": post.executable_by_agent,
                    "schema_valid": post.schema_valid,
                },
            });
            (StatusCode::OK, Json(body)).into_response()
        }
        Err(err) => {
            tracing::error!(
                audit_id = %audit_id,
                code = err.code(),
                error = %crate::redact::redact(&err.to_string()),
                "run-bound backend execution failed"
            );
            run_call_refusal(
                StatusCode::BAD_GATEWAY,
                err.code(),
                &err.to_string(),
                Some(&audit_id),
            )
        }
    }
}

/// Wire envelope version for the governed SSE stream (Gate 0 item 7). Every
/// SSE event carries its `event_seq` in the native SSE `id` field (dedup and
/// ordering key is `run_id` + `event_seq`); `decision`/`result`/`error` also
/// embed `schema_version`, `event_id` and `event_seq` in their data. `chunk`
/// data stays a verbatim `chat.completion.chunk` object (pre-envelope
/// consumers keep working); its sequencing lives on the `id` line only.
pub const STREAM_WIRE_SCHEMA_VERSION: &str = "rbx.modelstream.wire.v1";

/// Stamp the wire envelope onto a non-verbatim SSE event payload.
fn envelope_event(
    kind: &str,
    seq: u64,
    mut payload: serde_json::Value,
) -> axum::response::sse::Event {
    if let Some(obj) = payload.as_object_mut() {
        obj.insert(
            "schema_version".to_owned(),
            serde_json::json!(STREAM_WIRE_SCHEMA_VERSION),
        );
        obj.insert("event_id".to_owned(), serde_json::json!(Uuid::new_v4()));
        obj.insert("event_seq".to_owned(), serde_json::json!(seq));
    }
    axum::response::sse::Event::default()
        .event(kind)
        .id(seq.to_string())
        .data(payload.to_string())
}

/// POST /rbx/v1/runs/{run_id}/calls/stream — SSE run-bound governed call.
/// All refusals happen BEFORE the stream starts (plain typed JSON responses);
/// only an allowed, claimed call opens the SSE. Event sequence:
/// `decision` -> `chunk`* (verbatim chat.completion.chunk objects) ->
/// `result` (or `error`). Client disconnect cancels the backend mid-flight
/// and finalizes the run as cancelled.
pub async fn rbx_run_call_stream(
    State(state): State<Arc<AppState>>,
    Extension(caller): Extension<VerifiedCaller>,
    Path(run_id): Path<String>,
    Json(req): Json<RunCallRequest>,
) -> Response {
    use axum::response::sse::{Event, KeepAlive, Sse};
    use tokio_stream::wrappers::ReceiverStream;
    use tokio_stream::StreamExt;

    let prepared = match prepare_run_call(&state, &caller, &run_id, &req) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    let Some(backend) = state.backend_port.as_ref().map(Arc::clone) else {
        fail_claimed_run(&state, &prepared.run_id, "no_backend", None);
        return typed_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "no_backend",
            "no backend configured",
            true,
        );
    };

    let (tx, rx) = tokio::sync::mpsc::channel::<Event>(32);
    let state_task = Arc::clone(&state);
    tokio::spawn(async move {
        let audit_id = prepared.outcome.audit_id.0.to_string();
        // Monotonic per-stream sequence: decision=1, then chunks, then the
        // terminal event. Dedup/ordering key on the wire is run_id+event_seq.
        let seq = Arc::new(std::sync::atomic::AtomicU64::new(1));
        let decision_event = envelope_event(
            "decision",
            seq.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
            serde_json::json!({
                "decision": "Allow",
                "audit_id": audit_id,
                "trace_id": prepared.outcome.trace_id.0.to_string(),
                "session_id": prepared.session.session_id,
                "run_id": prepared.run_id,
                "policy_id": prepared.outcome.policy.id,
            }),
        );
        if tx.send(decision_event).await.is_err() {
            // Client vanished before execution: release the claim as
            // cancelled without touching the backend.
            let execution = Err(thalamus_core::BackendCallError::Cancelled {
                partial_usage: thalamus_core::BackendUsage::default(),
            });
            finalize_run_call(&state_task, &prepared, &execution);
            return;
        }

        let route = prepared.route.clone();
        let cancel = thalamus_core::CancelToken::new();
        let cancel_for_sink = cancel.clone();
        let tx_for_sink = tx.clone();
        let seq_for_sink = Arc::clone(&seq);
        let execution = tokio::task::spawn_blocking(move || {
            let mut on_chunk = |chunk: &serde_json::Value| {
                // Chunk data stays verbatim; sequencing rides the SSE id line.
                let event = Event::default()
                    .event("chunk")
                    .id(seq_for_sink
                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                        .to_string())
                    .data(chunk.to_string());
                if tx_for_sink.blocking_send(event).is_err() {
                    cancel_for_sink.cancel();
                }
            };
            backend.execute_streaming_chat(&route, &cancel_for_sink, &mut on_chunk)
        })
        .await;

        let execution = match execution {
            Ok(r) => r,
            Err(_join_err) => {
                fail_claimed_run(&state_task, &prepared.run_id, "backend_task_failed", None);
                let _ = tx
                    .send(envelope_event(
                        "error",
                        seq.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
                        serde_json::json!({
                            "code": "BACKEND_TASK_FAILED",
                            "message": "backend execution task failed",
                            "audit_id": audit_id,
                        }),
                    ))
                    .await;
                return;
            }
        };

        let post_result = finalize_run_call(&state_task, &prepared, &execution);
        match execution {
            Ok(exec) => {
                let post = post_result.expect("post_call ran on success");
                let _ = tx
                    .send(envelope_event(
                        "result",
                        seq.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
                        serde_json::json!({
                            "status": format!("{:?}", post.status),
                            "risk_class": format!("{:?}", post.risk_class),
                            "executable_by_agent": post.executable_by_agent,
                            "schema_valid": post.schema_valid,
                            "audit_id": audit_id,
                            "run_id": prepared.run_id,
                            "finish_reason": exec.backend_metadata.get("finish_reason"),
                            "usage": exec.usage,
                            "latency_ms": exec.latency_ms,
                        }),
                    ))
                    .await;
            }
            Err(err) => {
                tracing::error!(
                    audit_id = %audit_id,
                    code = err.code(),
                    error = %crate::redact::redact(&err.to_string()),
                    "run-bound streaming execution failed"
                );
                let partial_usage = match &err {
                    thalamus_core::BackendCallError::Timeout { partial_usage }
                    | thalamus_core::BackendCallError::Cancelled { partial_usage } => {
                        Some(partial_usage.clone())
                    }
                    _ => None,
                };
                let _ = tx
                    .send(envelope_event(
                        "error",
                        seq.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
                        serde_json::json!({
                            "code": err.code(),
                            "message": err.to_string(),
                            "audit_id": audit_id,
                            "run_id": prepared.run_id,
                            "partial_usage": partial_usage,
                        }),
                    ))
                    .await;
            }
        }
    });

    Sse::new(ReceiverStream::new(rx).map(Ok::<_, std::convert::Infallible>))
        .keep_alive(KeepAlive::default())
        .into_response()
}

fn decide_to_call_request(req: &DecideRequest) -> CallRequest {
    CallRequest {
        run_correlated: false,
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
