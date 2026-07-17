use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::domain::{AuditId, BackendHandle, RiskLevel, TraceId};

/// Audit events emitted during pre-call and post-call phases.
/// These are the first-class audit records — never optional when policy
/// mandates audit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEvent {
    PreCallDecision {
        trace_id: TraceId,
        audit_id: AuditId,
        tenant: String,
        product: String,
        workflow: String,
        policy_ref: String,
        decision: String,
        backend: Option<BackendHandle>,
        timestamp: OffsetDateTime,
    },
    PostCallOutcome {
        trace_id: TraceId,
        audit_id: AuditId,
        status: String,
        risk_class: RiskLevel,
        executable_by_agent: bool,
        schema_valid: bool,
        timestamp: OffsetDateTime,
    },
    /// Route envelope selected for a model call (master plan §3 acceptance:
    /// the route envelope is audited for every model call, before the
    /// backend executes).
    RouteEnvelope {
        trace_id: TraceId,
        audit_id: AuditId,
        model_alias: String,
        provider_pool: Vec<String>,
        region: Option<String>,
        data_class: Option<String>,
        capability_class: Option<String>,
        cost_class: Option<String>,
        timeout_ms: u64,
        timestamp: OffsetDateTime,
    },
    /// Session/run lifecycle transition (master plan §3): session created or
    /// closed, run created or cancelled, run refused (budget/closed-session).
    /// The audit stream is the session id, so a session's full lifecycle is
    /// one hash chain.
    Lifecycle {
        trace_id: TraceId,
        audit_id: AuditId,
        entity_type: String,
        entity_id: String,
        action: String,
        principal: Option<String>,
        timestamp: OffsetDateTime,
    },
}
