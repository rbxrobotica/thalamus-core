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
    /// When true, only run-bound calls (`/rbx/v1/runs/{run_id}/calls`) are
    /// allowed for this tenant/product/workflow: the legacy uncorrelated
    /// `/v1/call` surface is denied before any backend contact.
    #[serde(default)]
    pub require_run_correlation: bool,
    /// Prompt profile this policy's backends are compatible with, negotiated
    /// to clients in the run route lease (rbx.route_lease.v1). Clients compile
    /// their prompts against this profile before the model call; absent means
    /// the institutional default profile.
    #[serde(default)]
    pub prompt_profile: Option<String>,
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
