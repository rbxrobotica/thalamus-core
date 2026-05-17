use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use thalamus_core::{
    AuditId, BackendResponse, CallRequest, PolicyDecision,
    PreCallError,
};

use crate::app::AppState;

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
        PolicyDecision::AllowWithReview { review_reason, policy_ref } => DecideResponse {
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
            // Store pre-call record for post-call correlation
            if let Some(ref envelope) = outcome.envelope {
                state.audit_store.store_pre_call_record(
                    outcome.audit_id.clone(),
                    envelope.clone(),
                    outcome.policy.clone(),
                );
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
                PolicyDecision::Deny { reason, policy_ref: pr } => {
                    ("Deny".to_owned(), None, Some(format!("{}: {}", pr, reason)))
                }
                PolicyDecision::AllowWithReview { review_reason: rr, policy_ref: pr } => {
                    ("AllowWithReview".to_owned(), Some(rr.clone()), Some(pr.clone()))
                }
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
        Err(PreCallError::NoPermittedBackend { tenant, product, policy_id: _ }) => {
            let resp = ErrorResponse {
                error: format!("no permitted backends for tenant {} product {}", tenant, product),
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

    let record = match state.audit_store.get_pre_call_record(&audit_id) {
        Some(r) => r,
        None => {
            let resp = ErrorResponse {
                error: format!("no pre-call record found for audit_id {}", req.audit_id),
                code: "UNKNOWN_AUDIT_ID".to_owned(),
            };
            return (StatusCode::NOT_FOUND, Json(resp)).into_response();
        }
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
        Err(PreCallError::NoPermittedBackend { tenant, product, policy_id }) => {
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

    // Store pre-call record for post-call correlation (Allow/AllowWithReview)
    if let Some(ref envelope) = outcome.envelope {
        state.audit_store.store_pre_call_record(
            outcome.audit_id.clone(),
            envelope.clone(),
            outcome.policy.clone(),
        );
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
        PolicyDecision::AllowWithReview { review_reason, policy_ref } => {
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
        Some(b) => b,
        None => {
            let resp = ErrorResponse {
                error: "no backend configured".to_owned(),
                code: "NO_BACKEND".to_owned(),
            };
            return (StatusCode::SERVICE_UNAVAILABLE, Json(resp)).into_response();
        }
    };

    let backend_response = backend.call(envelope);

    // post_call is non-bypassable on the Allow path
    let post_result = thalamus_core::post_call(
        &backend_response,
        envelope,
        &outcome.policy,
        state.audit_port.as_ref(),
        state.eval_port.as_ref(),
        state.obs_port.as_ref(),
    );

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

    let events = state.audit_store.get_by_audit_id(&audit_id);
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
        })
        .collect();

    let resp = AuditResponse {
        audit_id: id,
        events: event_jsons,
    };

    (StatusCode::OK, Json(resp)).into_response()
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
            thalamus_core::BackendHandle { id: b.id.clone(), backend_type: bt }
        }),
        budget_hint: req.budget_hint.as_ref().map(|h| thalamus_core::BudgetHint {
            max_tokens: h.max_tokens,
            max_latency_ms: h.max_latency_ms,
        }),
    }
}
