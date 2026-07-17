//! Session/run lifecycle store (master plan §3, slice 1). In-memory by
//! default; the durable Postgres store (Phase 2 schema) is authoritative when
//! the `postgres` feature + `THALAMUS_DATABASE_URL` are active.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use time::OffsetDateTime;
use uuid::Uuid;

use thalamus_core::{
    BudgetLine, RunRecord, RunStatus, SessionLimits, SessionRecord, SessionStatus,
};

/// Input for session creation (principal/token come from the verified caller,
/// never from the request body).
pub struct NewSession {
    pub tenant: String,
    pub product: String,
    pub workflow: String,
    pub principal: Option<String>,
    pub delegation_token_id: Option<String>,
    pub idempotency_key: Option<String>,
}

/// Why a run was refused.
#[derive(Debug)]
pub enum CreateRunError {
    UnknownSession,
    SessionClosed,
    BudgetExceeded {
        scope_type: String,
        scope_ref: String,
    },
    /// Constructed by the durable store glue only.
    #[cfg_attr(not(feature = "postgres"), allow(dead_code))]
    Store(String),
}

/// Governed tool-invocation decision (§3 `POST /rbx/v1/tool-decisions`).
pub struct ToolDecision {
    pub session_id: Uuid,
    pub run_id: Option<Uuid>,
    pub tool: String,
    /// `allowed` | `denied` (recorded verbatim as the invocation status).
    pub decision: String,
    pub metadata: serde_json::Value,
}

/// Approval record (§3 `POST /rbx/v1/approvals`). The approver always comes
/// from the verified credential, never from the request body.
pub struct ApprovalInput {
    pub session_id: Option<Uuid>,
    pub run_id: Option<Uuid>,
    pub subject: String,
    pub approver: String,
    pub decision: String,
    pub reason: Option<String>,
    pub metadata: serde_json::Value,
}

/// Evidence reference (§3 `POST /rbx/v1/evidence`): a pointer + content hash,
/// never the evidence payload itself.
pub struct EvidenceInput {
    pub run_id: Option<Uuid>,
    pub kind: String,
    pub uri: String,
    pub content_hash: String,
}

pub trait SessionStore: Send + Sync {
    fn create_session(&self, input: &NewSession) -> Result<SessionRecord, String>;
    fn close_session(&self, id: &Uuid) -> Result<Option<SessionRecord>, String>;
    fn create_run(
        &self,
        session_id: &Uuid,
        model_alias: Option<&str>,
        idempotency_key: Option<&str>,
    ) -> Result<RunRecord, CreateRunError>;
    fn cancel_run(&self, run_id: &Uuid) -> Result<Option<RunRecord>, String>;
    fn session_limits(&self, id: &Uuid) -> Result<Option<SessionLimits>, String>;
    /// Record a tool decision. Errors with "unknown_session" when the session
    /// does not exist. Returns the invocation id.
    fn record_tool_decision(&self, input: &ToolDecision) -> Result<Uuid, String>;
    /// Record an approval. Returns the approval id.
    fn record_approval(&self, input: &ApprovalInput) -> Result<Uuid, String>;
    /// Record an evidence reference. Returns the evidence id.
    fn record_evidence(&self, input: &EvidenceInput) -> Result<Uuid, String>;
}

pub type SharedSessionStore = Arc<dyn SessionStore + Send + Sync>;

// === In-memory implementation (tests / no-postgres builds) ===

#[derive(Default)]
struct MemState {
    sessions: HashMap<Uuid, SessionRecord>,
    runs: HashMap<Uuid, RunRecord>,
    session_idempotency: HashMap<String, Uuid>,
    run_idempotency: HashMap<String, Uuid>,
    budgets: Vec<MemBudget>,
    tool_decisions: Vec<(Uuid, String)>,
    approvals: Vec<(Uuid, String)>,
    evidence: Vec<(Uuid, String)>,
}

#[derive(Clone)]
struct MemBudget {
    scope_type: String,
    scope_ref: String,
    period: String,
    max_tokens: Option<i64>,
    consumed_tokens: i64,
}

/// Non-durable session store. Same lifecycle semantics as the Postgres store;
/// budgets are seeded via [`InMemorySessionStore::set_budget`] (tests).
#[derive(Default)]
pub struct InMemorySessionStore {
    state: Mutex<MemState>,
}

impl InMemorySessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed or replace a budget line (scope_type + scope_ref + period).
    #[allow(dead_code, reason = "used by integration tests")]
    pub fn set_budget(
        &self,
        scope_type: &str,
        scope_ref: &str,
        period: &str,
        max_tokens: Option<i64>,
        consumed_tokens: i64,
    ) {
        let mut state = self.state.lock().unwrap();
        state.budgets.retain(|b| {
            !(b.scope_type == scope_type && b.scope_ref == scope_ref && b.period == period)
        });
        state.budgets.push(MemBudget {
            scope_type: scope_type.to_owned(),
            scope_ref: scope_ref.to_owned(),
            period: period.to_owned(),
            max_tokens,
            consumed_tokens,
        });
    }
}

fn governing<'a>(
    budgets: &'a [MemBudget],
    session_id: &Uuid,
    tenant: &str,
    product: &str,
) -> Vec<&'a MemBudget> {
    let session_ref = session_id.to_string();
    let product_ref = format!("{tenant}/{product}");
    budgets
        .iter()
        .filter(|b| {
            (b.scope_type == "session" && b.scope_ref == session_ref)
                || (b.scope_type == "product" && b.scope_ref == product_ref)
                || (b.scope_type == "tenant" && b.scope_ref == tenant)
        })
        .collect()
}

impl SessionStore for InMemorySessionStore {
    fn create_session(&self, input: &NewSession) -> Result<SessionRecord, String> {
        let mut state = self.state.lock().unwrap();
        if let Some(key) = &input.idempotency_key {
            if let Some(existing) = state.session_idempotency.get(key) {
                return Ok(state.sessions[existing].clone());
            }
        }
        let now = OffsetDateTime::now_utc();
        let record = SessionRecord {
            session_id: Uuid::new_v4(),
            tenant: input.tenant.clone(),
            product: input.product.clone(),
            workflow: input.workflow.clone(),
            principal: input.principal.clone(),
            delegation_token_id: input.delegation_token_id.clone(),
            status: SessionStatus::Open,
            retention_class: "standard".to_owned(),
            created_at: now,
            updated_at: now,
        };
        if let Some(key) = &input.idempotency_key {
            state
                .session_idempotency
                .insert(key.clone(), record.session_id);
        }
        state.sessions.insert(record.session_id, record.clone());
        Ok(record)
    }

    fn close_session(&self, id: &Uuid) -> Result<Option<SessionRecord>, String> {
        let mut state = self.state.lock().unwrap();
        Ok(state.sessions.get_mut(id).map(|s| {
            s.status = SessionStatus::Closed;
            s.updated_at = OffsetDateTime::now_utc();
            s.clone()
        }))
    }

    fn create_run(
        &self,
        session_id: &Uuid,
        model_alias: Option<&str>,
        idempotency_key: Option<&str>,
    ) -> Result<RunRecord, CreateRunError> {
        let mut state = self.state.lock().unwrap();
        if let Some(key) = idempotency_key {
            if let Some(existing) = state.run_idempotency.get(key) {
                return Ok(state.runs[existing].clone());
            }
        }
        let session = state
            .sessions
            .get(session_id)
            .ok_or(CreateRunError::UnknownSession)?;
        if session.status == SessionStatus::Closed {
            return Err(CreateRunError::SessionClosed);
        }
        let (tenant, product) = (session.tenant.clone(), session.product.clone());
        if let Some(budget) = governing(&state.budgets, session_id, &tenant, &product)
            .into_iter()
            .find(|b| b.max_tokens.is_some_and(|max| b.consumed_tokens >= max))
        {
            return Err(CreateRunError::BudgetExceeded {
                scope_type: budget.scope_type.clone(),
                scope_ref: budget.scope_ref.clone(),
            });
        }
        let record = RunRecord {
            run_id: Uuid::new_v4(),
            session_id: *session_id,
            status: RunStatus::Started,
            model_alias: model_alias.map(str::to_owned),
            backend_id: None,
            started_at: OffsetDateTime::now_utc(),
            finished_at: None,
            metadata: serde_json::json!({}),
        };
        if let Some(key) = idempotency_key {
            state.run_idempotency.insert(key.to_owned(), record.run_id);
        }
        state.runs.insert(record.run_id, record.clone());
        Ok(record)
    }

    fn cancel_run(&self, run_id: &Uuid) -> Result<Option<RunRecord>, String> {
        let mut state = self.state.lock().unwrap();
        Ok(state.runs.get_mut(run_id).map(|r| {
            if r.status == RunStatus::Started {
                r.status = RunStatus::Cancelled;
                r.finished_at = Some(OffsetDateTime::now_utc());
            }
            r.clone()
        }))
    }

    fn record_tool_decision(&self, input: &ToolDecision) -> Result<Uuid, String> {
        let mut state = self.state.lock().unwrap();
        if !state.sessions.contains_key(&input.session_id) {
            return Err("unknown_session".to_owned());
        }
        let id = Uuid::new_v4();
        state.tool_decisions.push((id, input.tool.clone()));
        Ok(id)
    }

    fn record_approval(&self, input: &ApprovalInput) -> Result<Uuid, String> {
        let mut state = self.state.lock().unwrap();
        let id = Uuid::new_v4();
        state.approvals.push((id, input.subject.clone()));
        Ok(id)
    }

    fn record_evidence(&self, input: &EvidenceInput) -> Result<Uuid, String> {
        let mut state = self.state.lock().unwrap();
        let id = Uuid::new_v4();
        state.evidence.push((id, input.uri.clone()));
        Ok(id)
    }

    fn session_limits(&self, id: &Uuid) -> Result<Option<SessionLimits>, String> {
        let state = self.state.lock().unwrap();
        let Some(session) = state.sessions.get(id) else {
            return Ok(None);
        };
        let budgets = governing(&state.budgets, id, &session.tenant, &session.product)
            .into_iter()
            .map(|b| BudgetLine {
                scope_type: b.scope_type.clone(),
                scope_ref: b.scope_ref.clone(),
                period: b.period.clone(),
                max_tokens: b.max_tokens,
                consumed_tokens: b.consumed_tokens,
                exhausted: b.max_tokens.is_some_and(|max| b.consumed_tokens >= max),
            })
            .collect();
        Ok(Some(SessionLimits {
            session_id: *id,
            status: session.status,
            budgets,
            context_policy_ref: thalamus_core::DEFAULT_CONTEXT_POLICY_REF.to_owned(),
            context_utilization_limit: thalamus_core::DEFAULT_CONTEXT_UTILIZATION_LIMIT,
        }))
    }
}

// === Durable (Postgres) implementation glue ===

#[cfg(feature = "postgres")]
impl SessionStore for thalamus_postgres_adapter::PostgresAudit {
    fn create_session(&self, input: &NewSession) -> Result<SessionRecord, String> {
        thalamus_postgres_adapter::PostgresAudit::create_session(
            self,
            &thalamus_postgres_adapter::NewSessionInput {
                tenant: input.tenant.clone(),
                product: input.product.clone(),
                workflow: input.workflow.clone(),
                principal: input.principal.clone(),
                delegation_token_id: input.delegation_token_id.clone(),
                idempotency_key: input.idempotency_key.clone(),
            },
        )
        .map_err(|e| e.to_string())
    }

    fn close_session(&self, id: &Uuid) -> Result<Option<SessionRecord>, String> {
        thalamus_postgres_adapter::PostgresAudit::close_session(self, id).map_err(|e| e.to_string())
    }

    fn create_run(
        &self,
        session_id: &Uuid,
        model_alias: Option<&str>,
        idempotency_key: Option<&str>,
    ) -> Result<RunRecord, CreateRunError> {
        thalamus_postgres_adapter::PostgresAudit::create_run(
            self,
            session_id,
            model_alias,
            idempotency_key,
        )
        .map_err(|e| match e {
            thalamus_postgres_adapter::CreateRunError::UnknownSession => {
                CreateRunError::UnknownSession
            }
            thalamus_postgres_adapter::CreateRunError::SessionClosed => {
                CreateRunError::SessionClosed
            }
            thalamus_postgres_adapter::CreateRunError::BudgetExceeded {
                scope_type,
                scope_ref,
            } => CreateRunError::BudgetExceeded {
                scope_type,
                scope_ref,
            },
            thalamus_postgres_adapter::CreateRunError::Store(err) => {
                CreateRunError::Store(err.to_string())
            }
        })
    }

    fn cancel_run(&self, run_id: &Uuid) -> Result<Option<RunRecord>, String> {
        thalamus_postgres_adapter::PostgresAudit::cancel_run(self, run_id)
            .map_err(|e| e.to_string())
    }

    fn session_limits(&self, id: &Uuid) -> Result<Option<SessionLimits>, String> {
        thalamus_postgres_adapter::PostgresAudit::session_limits(self, id)
            .map_err(|e| e.to_string())
    }

    fn record_tool_decision(&self, input: &ToolDecision) -> Result<Uuid, String> {
        match thalamus_postgres_adapter::PostgresAudit::record_tool_decision(
            self,
            &input.session_id,
            input.run_id.as_ref(),
            &input.tool,
            &input.decision,
            &input.metadata,
        ) {
            Ok(Some(id)) => Ok(id),
            Ok(None) => Err("unknown_session".to_owned()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn record_approval(&self, input: &ApprovalInput) -> Result<Uuid, String> {
        thalamus_postgres_adapter::PostgresAudit::record_approval(
            self,
            &thalamus_postgres_adapter::ApprovalRecordInput {
                session_id: input.session_id.as_ref(),
                run_id: input.run_id.as_ref(),
                subject: &input.subject,
                approver: &input.approver,
                decision: &input.decision,
                reason: input.reason.as_deref(),
                metadata: &input.metadata,
            },
        )
        .map_err(|e| e.to_string())
    }

    fn record_evidence(&self, input: &EvidenceInput) -> Result<Uuid, String> {
        thalamus_postgres_adapter::PostgresAudit::record_evidence(
            self,
            input.run_id.as_ref(),
            &input.kind,
            &input.uri,
            &input.content_hash,
        )
        .map_err(|e| e.to_string())
    }
}
