use serde::{Deserialize, Serialize};

use crate::domain::{BackendHandle, CallRequest, PolicyDecision, RiskLevel};

// === Policy ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub id: String,
    pub tenant: String,
    pub product: String,
    pub workflow: String,
    pub permitted_backends: Vec<BackendHandle>,
    pub budget: Budget,
    pub context_grants: Vec<ContextGrant>,
    pub redaction_rules: Vec<RedactionRule>,
    pub audit_required: bool,
    pub risk_threshold: RiskLevel,
}

// === Budget ===

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Budget {
    pub max_tokens: u32,
    pub max_latency_ms: u64,
}

// === Context grant ===

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextGrant {
    pub source: String,
    pub authorized: bool,
}

// === Redaction ===

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionRule {
    pub pattern: String,
    pub action: RedactionAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RedactionAction {
    Redact,
    Block,
}

// === Policy engine trait ===

/// Evaluates a CallRequest against a resolved Policy, producing a
/// PolicyDecision. Implementations must remain pure and synchronous.
pub trait PolicyEngine {
    fn evaluate(&self, request: &CallRequest, policy: &Policy) -> PolicyDecision;
}
