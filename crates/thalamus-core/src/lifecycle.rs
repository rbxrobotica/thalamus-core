//! Session / run lifecycle domain types (execution master plan §3).
//!
//! A `Session` is the governed unit of agent access: created only after
//! credential validation, carrying tenant/product/workflow and the principal
//! it was delegated to. `Run`s belong to a session and are refused when the
//! session is closed or a governing budget is exhausted.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Open,
    Closed,
}

/// Governance modes a session can be created under (ADR-0403 / master plan
/// §7): external agents get governed LLM access, never governed workspace
/// claims. Recorded immutably at session creation and audited.
pub const GOVERNANCE_MODE_LLM_ACCESS: &str = "governed_llm_access";
pub const GOVERNANCE_MODE_WORKSPACE: &str = "governed_workspace";

pub fn default_governance_mode() -> String {
    GOVERNANCE_MODE_LLM_ACCESS.to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: Uuid,
    pub tenant: String,
    pub product: String,
    pub workflow: String,
    pub principal: Option<String>,
    pub delegation_token_id: Option<String>,
    pub status: SessionStatus,
    /// Immutable after creation; `governed_llm_access` for Bridge sessions.
    #[serde(default = "default_governance_mode")]
    pub governance_mode: String,
    pub retention_class: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Started,
    Completed,
    Failed,
    Cancelled,
}

/// Execution claim states for the 1:1 run <-> call invariant: a run starts
/// `pending`, is atomically claimed to `executing` by the governed call
/// surface, and ends `executed`. A second call on the same run is refused.
pub const RUN_EXECUTION_PENDING: &str = "pending";
pub const RUN_EXECUTION_EXECUTING: &str = "executing";
pub const RUN_EXECUTION_EXECUTED: &str = "executed";

pub fn default_run_execution_state() -> String {
    RUN_EXECUTION_PENDING.to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub run_id: Uuid,
    pub session_id: Uuid,
    pub status: RunStatus,
    #[serde(default = "default_run_execution_state")]
    pub execution_state: String,
    pub model_alias: Option<String>,
    pub backend_id: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub finished_at: Option<OffsetDateTime>,
    pub metadata: serde_json::Value,
}

/// One governing budget line, as exposed by `GET /rbx/v1/sessions/{id}/limits`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetLine {
    pub scope_type: String,
    pub scope_ref: String,
    pub period: String,
    pub max_tokens: Option<i64>,
    pub consumed_tokens: i64,
    pub exhausted: bool,
}

/// Limits summary for a session: governing budgets plus the context policy.
/// The initial context policy is the fixed 70% utilization threshold from the
/// master plan §3 acceptance (`context_policy_ref` selects richer policies
/// later).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLimits {
    pub session_id: Uuid,
    pub status: SessionStatus,
    pub budgets: Vec<BudgetLine>,
    pub context_policy_ref: String,
    pub context_utilization_limit: f64,
}

pub const DEFAULT_CONTEXT_POLICY_REF: &str = "context-utilization-70";
pub const DEFAULT_CONTEXT_UTILIZATION_LIMIT: f64 = 0.7;
