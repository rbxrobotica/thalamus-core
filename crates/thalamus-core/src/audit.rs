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
}
