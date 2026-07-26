use serde::{Deserialize, Serialize};
use uuid::Uuid;

// === Risk classification ===

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Prohibited,
}

// === Policy decision ===

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyDecision {
    Allow,
    Deny {
        reason: String,
        policy_ref: String,
    },
    AllowWithReview {
        review_reason: String,
        policy_ref: String,
    },
}

// === Post-call status ===

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PostCallStatus {
    Valid,
    Invalid,
    NeedsHumanReview,
}

// === Trace and audit IDs ===

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AuditId(pub Uuid);

// === Backend handle (opaque) ===

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BackendHandle {
    pub id: String,
    pub backend_type: BackendType,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BackendType {
    Model,
    Tool,
    McpServer,
    A2AAgent,
    Custom(String),
}

// === Call request ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallRequest {
    pub tenant: String,
    pub product: String,
    pub user: String,
    pub workflow: String,
    pub intent: String,
    pub prompt: String,
    pub requested_backend: Option<BackendHandle>,
    pub budget_hint: Option<BudgetHint>,
    /// True only when the call arrived through the run-bound governed surface
    /// (`/rbx/v1/runs/{run_id}/calls`), set by the server, never by callers.
    /// Policies with `require_run_correlation` deny uncorrelated calls.
    #[serde(default)]
    pub run_correlated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetHint {
    pub max_tokens: Option<u32>,
    pub max_latency_ms: Option<u64>,
}

// === Envelope (built prompt for backend) ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub trace_id: TraceId,
    pub audit_id: AuditId,
    pub backend_handle: BackendHandle,
    pub prompt: String,
    pub authorized_context: Vec<ContextEntry>,
    pub redaction_applied: bool,
    pub policy_ref: String,
    pub budget: crate::policy::Budget,
    /// Structured chat payload (`chat.completions.v1`: messages, tools,
    /// tool_choice, max_tokens) for run-bound calls. When present, adapters
    /// execute it instead of wrapping `prompt` in a single user message;
    /// `prompt` then holds the serialized payload for policy scanning and
    /// post-call validation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_payload: Option<serde_json::Value>,
}

// === Context ===

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextEntry {
    pub source: String,
    pub content: String,
}

// === Post-call result ===

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PostCallResult {
    pub status: PostCallStatus,
    pub risk_class: RiskLevel,
    pub executable_by_agent: bool,
    pub strategos_event: Option<StrategosEvent>,
    pub schema_valid: bool,
    pub hallucination_signals: Vec<String>,
    pub citation_check: CitationCheck,
}

// === Citation check ===

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CitationCheck {
    NotRequired,
    Passed,
    Failed { reason: String },
}

// === Strategos event ===

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrategosEvent {
    pub event_type: String,
    pub summary: String,
    pub audit_id: AuditId,
    pub trace_id: TraceId,
}

// === Backend response (raw, from data plane) ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendResponse {
    pub content: String,
    pub tokens_used: Option<u32>,
    pub latency_ms: Option<u64>,
}

/// A governed embedding request. The model alias is resolved only by the
/// data-plane adapter; callers never see provider model names or credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    pub model_alias: String,
    pub input: Vec<String>,
    pub trace_id: TraceId,
    pub audit_id: AuditId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub model_alias: String,
    pub vectors: Vec<Vec<f32>>,
    pub provider_metadata: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmbeddingError {
    InvalidRequest { detail: String },
    Unavailable { detail: String },
    MalformedResponse { detail: String },
}

impl std::fmt::Display for EmbeddingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRequest { detail } => write!(f, "invalid embedding request: {detail}"),
            Self::Unavailable { detail } => write!(f, "embedding backend unavailable: {detail}"),
            Self::MalformedResponse { detail } => write!(f, "malformed embedding response: {detail}"),
        }
    }
}

impl std::error::Error for EmbeddingError {}
