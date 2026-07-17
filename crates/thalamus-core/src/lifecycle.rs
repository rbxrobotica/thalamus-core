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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: Uuid,
    pub tenant: String,
    pub product: String,
    pub workflow: String,
    pub principal: Option<String>,
    pub delegation_token_id: Option<String>,
    pub status: SessionStatus,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub run_id: Uuid,
    pub session_id: Uuid,
    pub status: RunStatus,
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
