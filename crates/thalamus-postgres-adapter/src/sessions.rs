//! Session / run lifecycle persistence (master plan §3, slice 1).
//!
//! Runs are refused inside one transaction when the session is missing,
//! closed, or a governing budget is exhausted — the budget rows are locked
//! (`FOR UPDATE`) so concurrent run creation cannot race past a limit.
//! Idempotency keys make session/run creation retry-safe: a replayed request
//! returns the originally created row.

use postgres::Transaction;
use time::OffsetDateTime;
use uuid::Uuid;

use thalamus_core::{
    ApprovalFingerprint, BudgetLine, RecordOutcome, RunRecord, RunStatus, SessionLimits,
    SessionRecord, SessionStatus, ToolDecisionFingerprint,
};

use crate::{off_runtime, AuditStoreError, PostgresAudit};

/// Approval record input (§3). `source_system` and `idempotency_key` are the
/// governance-idempotency extension (master plan §3 slice 5): when
/// `idempotency_key` is set, `session_id` must also be set — the caller
/// (`rbx_approval`) refuses the request before it ever reaches the store.
pub struct ApprovalRecordInput<'a> {
    pub session_id: Option<&'a Uuid>,
    pub run_id: Option<&'a Uuid>,
    pub subject: &'a str,
    pub approver: &'a str,
    pub decision: &'a str,
    pub reason: Option<&'a str>,
    pub metadata: &'a serde_json::Value,
    pub source_system: &'a str,
    pub idempotency_key: Option<&'a str>,
}

/// Why recording a tool decision was refused.
#[derive(Debug, thiserror::Error)]
pub enum RecordToolDecisionError {
    #[error("unknown session")]
    UnknownSession,
    #[error("idempotency key conflict: request does not match the original")]
    IdempotencyConflict,
    #[error(transparent)]
    Store(#[from] AuditStoreError),
}

/// Why recording an approval was refused.
#[derive(Debug, thiserror::Error)]
pub enum RecordApprovalError {
    #[error("unknown session")]
    UnknownSession,
    #[error("idempotency key conflict: request does not match the original")]
    IdempotencyConflict,
    #[error(transparent)]
    Store(#[from] AuditStoreError),
}

/// Input for session creation.
pub struct NewSessionInput {
    pub tenant: String,
    pub product: String,
    pub workflow: String,
    pub principal: Option<String>,
    pub delegation_token_id: Option<String>,
    pub governance_mode: String,
    pub idempotency_key: Option<String>,
}

/// Why an execution claim on a run was refused (1:1 run <-> call invariant).
#[derive(Debug, thiserror::Error)]
pub enum ClaimRunError {
    #[error("unknown run")]
    UnknownRun,
    #[error("session is closed")]
    SessionClosed,
    #[error("run is not active")]
    RunNotActive,
    #[error("run already executed")]
    AlreadyExecuted,
    #[error("budget exhausted for {scope_type} {scope_ref}")]
    BudgetExceeded {
        scope_type: String,
        scope_ref: String,
    },
    #[error(transparent)]
    Store(#[from] AuditStoreError),
}

/// Why a run was refused.
#[derive(Debug, thiserror::Error)]
pub enum CreateRunError {
    #[error("unknown session")]
    UnknownSession,
    #[error("session is closed")]
    SessionClosed,
    #[error("budget exhausted for {scope_type} {scope_ref}")]
    BudgetExceeded {
        scope_type: String,
        scope_ref: String,
    },
    #[error(transparent)]
    Store(#[from] AuditStoreError),
}

impl PostgresAudit {
    pub fn create_session(
        &self,
        input: &NewSessionInput,
    ) -> Result<SessionRecord, AuditStoreError> {
        off_runtime(|| {
            let mut conn = self.pool.get()?;
            let mut tx = conn.transaction()?;
            if let Some(key) = &input.idempotency_key {
                if let Some(row) = tx.query_opt(
                    "SELECT session_id FROM sessions WHERE idempotency_key = $1",
                    &[key],
                )? {
                    let existing: Uuid = row.get(0);
                    let record = session_by_id(&mut tx, &existing)?
                        .expect("idempotency key points at existing session");
                    tx.commit()?;
                    return Ok(record);
                }
            }
            let session_id = Uuid::new_v4();
            tx.execute(
                "INSERT INTO sessions
                    (session_id, tenant, product, workflow, principal,
                     delegation_token_id, governance_mode, idempotency_key)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                &[
                    &session_id,
                    &input.tenant,
                    &input.product,
                    &input.workflow,
                    &input.principal,
                    &input.delegation_token_id,
                    &input.governance_mode,
                    &input.idempotency_key,
                ],
            )?;
            let record = session_by_id(&mut tx, &session_id)?.expect("session row just inserted");
            tx.commit()?;
            Ok(record)
        })
    }

    pub fn get_session(&self, id: &Uuid) -> Result<Option<SessionRecord>, AuditStoreError> {
        off_runtime(|| {
            let mut conn = self.pool.get()?;
            let mut tx = conn.transaction()?;
            let record = session_by_id(&mut tx, id)?;
            tx.commit()?;
            Ok(record)
        })
    }

    /// Close a session (idempotent). Returns the record, or `None` when the
    /// session does not exist.
    pub fn close_session(&self, id: &Uuid) -> Result<Option<SessionRecord>, AuditStoreError> {
        off_runtime(|| {
            let mut conn = self.pool.get()?;
            let mut tx = conn.transaction()?;
            tx.execute(
                "UPDATE sessions SET status = 'closed', updated_at = now()
                 WHERE session_id = $1",
                &[id],
            )?;
            let record = session_by_id(&mut tx, id)?;
            tx.commit()?;
            Ok(record)
        })
    }

    /// Create a run under a session. Refused atomically when the session is
    /// unknown/closed or any governing budget (session / product / tenant
    /// scope) is exhausted.
    pub fn create_run(
        &self,
        session_id: &Uuid,
        model_alias: Option<&str>,
        idempotency_key: Option<&str>,
    ) -> Result<RunRecord, CreateRunError> {
        off_runtime(|| {
            let mut conn = self.pool.get().map_err(AuditStoreError::from)?;
            let mut tx = conn.transaction().map_err(AuditStoreError::from)?;

            if let Some(key) = idempotency_key {
                if let Some(row) = tx
                    .query_opt(
                        "SELECT run_id FROM runs WHERE idempotency_key = $1",
                        &[&key],
                    )
                    .map_err(AuditStoreError::from)?
                {
                    let existing: Uuid = row.get(0);
                    let record = run_by_id(&mut tx, &existing)
                        .map_err(AuditStoreError::from)?
                        .expect("idempotency key points at existing run");
                    tx.commit().map_err(AuditStoreError::from)?;
                    return Ok(record);
                }
            }

            let session = tx
                .query_opt(
                    "SELECT tenant, product, status FROM sessions
                     WHERE session_id = $1 FOR UPDATE",
                    &[session_id],
                )
                .map_err(AuditStoreError::from)?;
            let Some(session) = session else {
                return Err(CreateRunError::UnknownSession);
            };
            let tenant: String = session.get(0);
            let product: String = session.get(1);
            let status: String = session.get(2);
            if status == "closed" {
                return Err(CreateRunError::SessionClosed);
            }

            if let Some((scope_type, scope_ref)) =
                exhausted_budget(&mut tx, session_id, &tenant, &product)
                    .map_err(AuditStoreError::from)?
            {
                return Err(CreateRunError::BudgetExceeded {
                    scope_type,
                    scope_ref,
                });
            }

            let run_id = Uuid::new_v4();
            tx.execute(
                "INSERT INTO runs (run_id, session_id, model_alias, idempotency_key)
                 VALUES ($1, $2, $3, $4)",
                &[&run_id, session_id, &model_alias, &idempotency_key],
            )
            .map_err(AuditStoreError::from)?;
            let record = run_by_id(&mut tx, &run_id)
                .map_err(AuditStoreError::from)?
                .expect("run row just inserted");
            tx.commit().map_err(AuditStoreError::from)?;
            Ok(record)
        })
    }

    /// Cancel a run (idempotent for already-cancelled runs). Returns `None`
    /// when the run does not exist.
    pub fn cancel_run(&self, run_id: &Uuid) -> Result<Option<RunRecord>, AuditStoreError> {
        off_runtime(|| {
            let mut conn = self.pool.get()?;
            let mut tx = conn.transaction()?;
            tx.execute(
                "UPDATE runs SET status = 'cancelled', finished_at = now()
                 WHERE run_id = $1 AND status = 'started'",
                &[run_id],
            )?;
            let record = run_by_id(&mut tx, run_id)?;
            tx.commit()?;
            Ok(record)
        })
    }

    /// Governing budgets + context policy for a session, or `None` when the
    /// session does not exist.
    pub fn session_limits(&self, id: &Uuid) -> Result<Option<SessionLimits>, AuditStoreError> {
        off_runtime(|| {
            let mut conn = self.pool.get()?;
            let mut tx = conn.transaction()?;
            let Some(session) = session_by_id(&mut tx, id)? else {
                tx.commit()?;
                return Ok(None);
            };
            let rows = tx.query(
                "SELECT scope_type, scope_ref, period, max_tokens, consumed_tokens
                 FROM budgets
                 WHERE (scope_type = 'session' AND scope_ref = $1)
                    OR (scope_type = 'product' AND scope_ref = $2)
                    OR (scope_type = 'tenant' AND scope_ref = $3)
                 ORDER BY scope_type, scope_ref, period",
                &[
                    &id.to_string(),
                    &format!("{}/{}", session.tenant, session.product),
                    &session.tenant,
                ],
            )?;
            let budgets = rows
                .iter()
                .map(|row| {
                    let max_tokens: Option<i64> = row.get(3);
                    let consumed_tokens: i64 = row.get(4);
                    BudgetLine {
                        scope_type: row.get(0),
                        scope_ref: row.get(1),
                        period: row.get(2),
                        max_tokens,
                        consumed_tokens,
                        exhausted: max_tokens.is_some_and(|max| consumed_tokens >= max),
                    }
                })
                .collect();
            tx.commit()?;
            Ok(Some(SessionLimits {
                session_id: *id,
                status: session.status,
                budgets,
                context_policy_ref: thalamus_core::DEFAULT_CONTEXT_POLICY_REF.to_owned(),
                context_utilization_limit: thalamus_core::DEFAULT_CONTEXT_UTILIZATION_LIMIT,
            }))
        })
    }

    /// Record a governed tool-invocation decision (§3). Returns `None` when
    /// the session does not exist.
    ///
    /// When `idempotency_key` is set, the insert is scoped-unique on
    /// `(tenant, source_system, idempotency_key)` (tenant from the owning
    /// session, source_system from the caller's verified credential): a
    /// replay with an identical fingerprint returns the original row
    /// (`RecordOutcome::Replayed`); a replay with a different fingerprint is
    /// refused as `IdempotencyConflict`. Without `idempotency_key`, behavior
    /// is unchanged from the original contract.
    pub fn record_tool_decision(
        &self,
        session_id: &Uuid,
        run_id: Option<&Uuid>,
        tool: &str,
        decision: &str,
        metadata: &serde_json::Value,
        source_system: &str,
        idempotency_key: Option<&str>,
    ) -> Result<RecordOutcome, RecordToolDecisionError> {
        off_runtime(|| {
            let mut conn = self.pool.get().map_err(AuditStoreError::from)?;
            let mut tx = conn.transaction().map_err(AuditStoreError::from)?;
            let Some(session) =
                session_by_id(&mut tx, session_id).map_err(AuditStoreError::from)?
            else {
                return Err(RecordToolDecisionError::UnknownSession);
            };

            if let Some(key) = idempotency_key {
                let tenant = session.tenant.as_str();
                let hash = ToolDecisionFingerprint::new(
                    tenant,
                    source_system,
                    *session_id,
                    run_id.copied(),
                    tool,
                    decision,
                )
                .hash_hex();
                let id = Uuid::new_v4();
                let inserted = tx
                    .query_opt(
                        "INSERT INTO tool_invocations
                            (invocation_id, run_id, tool, status, completed_at, metadata,
                             tenant, source_system, idempotency_key, request_fingerprint)
                         VALUES ($1, $2, $3, $4, now(), $5, $6, $7, $8, $9)
                         ON CONFLICT (tenant, source_system, idempotency_key)
                             WHERE idempotency_key IS NOT NULL
                             DO NOTHING
                         RETURNING invocation_id",
                        &[
                            &id,
                            &run_id,
                            &tool,
                            &decision,
                            metadata,
                            &tenant,
                            &source_system,
                            &key,
                            &hash,
                        ],
                    )
                    .map_err(AuditStoreError::from)?;
                if let Some(row) = inserted {
                    tx.commit().map_err(AuditStoreError::from)?;
                    return Ok(RecordOutcome::Created(row.get(0)));
                }
                let existing = tx
                    .query_one(
                        "SELECT invocation_id, request_fingerprint FROM tool_invocations
                         WHERE tenant = $1 AND source_system = $2 AND idempotency_key = $3",
                        &[&tenant, &source_system, &key],
                    )
                    .map_err(AuditStoreError::from)?;
                tx.commit().map_err(AuditStoreError::from)?;
                let existing_id: Uuid = existing.get(0);
                let existing_fingerprint: Option<String> = existing.get(1);
                return if existing_fingerprint.as_deref() == Some(hash.as_str()) {
                    Ok(RecordOutcome::Replayed(existing_id))
                } else {
                    Err(RecordToolDecisionError::IdempotencyConflict)
                };
            }

            let id = Uuid::new_v4();
            tx.execute(
                "INSERT INTO tool_invocations
                    (invocation_id, run_id, tool, status, completed_at, metadata)
                 VALUES ($1, $2, $3, $4, now(), $5)",
                &[&id, &run_id, &tool, &decision, metadata],
            )
            .map_err(AuditStoreError::from)?;
            tx.commit().map_err(AuditStoreError::from)?;
            Ok(RecordOutcome::Created(id))
        })
    }

    /// Record an approval (§3). The approver comes from the verified
    /// credential upstream.
    ///
    /// When `input.idempotency_key` is set, `input.session_id` must also be
    /// set (enforced by the caller before this is reached): the session is
    /// looked up inside a transaction — unlike the no-key path below, which
    /// never validates `session_id` — and the insert is scoped-unique on
    /// `(tenant, source_system, idempotency_key)`, same replay/conflict
    /// semantics as [`PostgresAudit::record_tool_decision`]. Without
    /// `idempotency_key`, behavior is unchanged from the original contract.
    pub fn record_approval(
        &self,
        input: &ApprovalRecordInput<'_>,
    ) -> Result<RecordOutcome, RecordApprovalError> {
        off_runtime(|| {
            if let Some(key) = input.idempotency_key {
                let Some(session_id) = input.session_id else {
                    return Err(RecordApprovalError::UnknownSession);
                };
                let mut conn = self.pool.get().map_err(AuditStoreError::from)?;
                let mut tx = conn.transaction().map_err(AuditStoreError::from)?;
                let Some(session) =
                    session_by_id(&mut tx, session_id).map_err(AuditStoreError::from)?
                else {
                    return Err(RecordApprovalError::UnknownSession);
                };
                let tenant = session.tenant.as_str();
                let hash = ApprovalFingerprint::new(
                    tenant,
                    input.source_system,
                    *session_id,
                    input.run_id.copied(),
                    input.subject,
                    input.decision,
                )
                .hash_hex();
                let id = Uuid::new_v4();
                let inserted = tx
                    .query_opt(
                        "INSERT INTO approvals
                            (approval_id, session_id, run_id, subject, approver, decision,
                             reason, metadata, tenant, source_system, idempotency_key,
                             request_fingerprint)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                         ON CONFLICT (tenant, source_system, idempotency_key)
                             WHERE idempotency_key IS NOT NULL
                             DO NOTHING
                         RETURNING approval_id",
                        &[
                            &id,
                            &input.session_id,
                            &input.run_id,
                            &input.subject,
                            &input.approver,
                            &input.decision,
                            &input.reason,
                            input.metadata,
                            &tenant,
                            &input.source_system,
                            &key,
                            &hash,
                        ],
                    )
                    .map_err(AuditStoreError::from)?;
                if let Some(row) = inserted {
                    tx.commit().map_err(AuditStoreError::from)?;
                    return Ok(RecordOutcome::Created(row.get(0)));
                }
                let existing = tx
                    .query_one(
                        "SELECT approval_id, request_fingerprint FROM approvals
                         WHERE tenant = $1 AND source_system = $2 AND idempotency_key = $3",
                        &[&tenant, &input.source_system, &key],
                    )
                    .map_err(AuditStoreError::from)?;
                tx.commit().map_err(AuditStoreError::from)?;
                let existing_id: Uuid = existing.get(0);
                let existing_fingerprint: Option<String> = existing.get(1);
                return if existing_fingerprint.as_deref() == Some(hash.as_str()) {
                    Ok(RecordOutcome::Replayed(existing_id))
                } else {
                    Err(RecordApprovalError::IdempotencyConflict)
                };
            }

            let mut conn = self.pool.get().map_err(AuditStoreError::from)?;
            let id = Uuid::new_v4();
            conn.execute(
                "INSERT INTO approvals
                    (approval_id, session_id, run_id, subject, approver, decision, reason, metadata)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                &[
                    &id,
                    &input.session_id,
                    &input.run_id,
                    &input.subject,
                    &input.approver,
                    &input.decision,
                    &input.reason,
                    input.metadata,
                ],
            )
            .map_err(AuditStoreError::from)?;
            Ok(RecordOutcome::Created(id))
        })
    }

    /// Record an evidence reference (§3): pointer + content hash only.
    pub fn record_evidence(
        &self,
        run_id: Option<&Uuid>,
        kind: &str,
        uri: &str,
        content_hash: &str,
    ) -> Result<Uuid, AuditStoreError> {
        off_runtime(|| {
            let mut conn = self.pool.get()?;
            let id = Uuid::new_v4();
            conn.execute(
                "INSERT INTO evidence_refs (evidence_id, run_id, kind, uri, content_hash)
                 VALUES ($1, $2, $3, $4, $5)",
                &[&id, &run_id, &kind, &uri, &content_hash],
            )?;
            Ok(id)
        })
    }

    /// Add consumed tokens to every budget governing this session. Budgets are
    /// provisioned by policy, never auto-created here.
    pub fn record_usage(&self, session_id: &Uuid, tokens: i64) -> Result<(), AuditStoreError> {
        off_runtime(|| {
            let mut conn = self.pool.get()?;
            let mut tx = conn.transaction()?;
            let Some(session) = session_by_id(&mut tx, session_id)? else {
                tx.commit()?;
                return Ok(());
            };
            tx.execute(
                "UPDATE budgets SET consumed_tokens = consumed_tokens + $4, updated_at = now()
                 WHERE (scope_type = 'session' AND scope_ref = $1)
                    OR (scope_type = 'product' AND scope_ref = $2)
                    OR (scope_type = 'tenant' AND scope_ref = $3)",
                &[
                    &session_id.to_string(),
                    &format!("{}/{}", session.tenant, session.product),
                    &session.tenant,
                    &tokens,
                ],
            )?;
            tx.commit()?;
            Ok(())
        })
    }

    /// Load a run together with its owning session (ownership checks I2/I4).
    pub fn run_with_session(
        &self,
        run_id: &Uuid,
    ) -> Result<Option<(RunRecord, SessionRecord)>, AuditStoreError> {
        off_runtime(|| {
            let mut conn = self.pool.get()?;
            let mut tx = conn.transaction()?;
            let Some(run) = run_by_id(&mut tx, run_id)? else {
                tx.commit()?;
                return Ok(None);
            };
            let session = session_by_id(&mut tx, &run.session_id)?
                .expect("run row references an existing session");
            tx.commit()?;
            Ok(Some((run, session)))
        })
    }

    /// Atomically claim a run for execution (1:1 run <-> call): only a
    /// `pending` claim on an active run under an open session succeeds, and
    /// governing budgets are re-checked under lock at claim time. Returns the
    /// claimed run + owning session.
    pub fn claim_run_execution(
        &self,
        run_id: &Uuid,
    ) -> Result<(RunRecord, SessionRecord), ClaimRunError> {
        off_runtime(|| {
            let mut conn = self.pool.get().map_err(AuditStoreError::from)?;
            let mut tx = conn.transaction().map_err(AuditStoreError::from)?;

            let row = tx
                .query_opt(
                    "SELECT session_id, status, execution_state FROM runs
                     WHERE run_id = $1 FOR UPDATE",
                    &[run_id],
                )
                .map_err(AuditStoreError::from)?;
            let Some(row) = row else {
                return Err(ClaimRunError::UnknownRun);
            };
            let session_id: Uuid = row.get(0);
            let status: String = row.get(1);
            let execution_state: String = row.get(2);

            let session = session_by_id(&mut tx, &session_id)
                .map_err(AuditStoreError::from)?
                .expect("run row references an existing session");
            if session.status == SessionStatus::Closed {
                return Err(ClaimRunError::SessionClosed);
            }
            // An executed run reports run_already_executed even after its
            // status turned terminal: the 1:1 violation is the more precise
            // refusal.
            if execution_state != "pending" {
                return Err(ClaimRunError::AlreadyExecuted);
            }
            if status != "started" {
                return Err(ClaimRunError::RunNotActive);
            }

            if let Some((scope_type, scope_ref)) =
                exhausted_budget(&mut tx, &session_id, &session.tenant, &session.product)
                    .map_err(AuditStoreError::from)?
            {
                return Err(ClaimRunError::BudgetExceeded {
                    scope_type,
                    scope_ref,
                });
            }

            tx.execute(
                "UPDATE runs SET execution_state = 'executing' WHERE run_id = $1",
                &[run_id],
            )
            .map_err(AuditStoreError::from)?;
            let run = run_by_id(&mut tx, run_id)
                .map_err(AuditStoreError::from)?
                .expect("run row locked above");
            tx.commit().map_err(AuditStoreError::from)?;
            Ok((run, session))
        })
    }

    /// Finish a claimed execution: final run status (`completed` / `failed` /
    /// `cancelled`), `execution_state = executed`, outcome metadata merged.
    pub fn finish_run_execution(
        &self,
        run_id: &Uuid,
        final_status: &str,
        outcome: &serde_json::Value,
    ) -> Result<(), AuditStoreError> {
        off_runtime(|| {
            let mut conn = self.pool.get()?;
            conn.execute(
                "UPDATE runs SET status = $2, execution_state = 'executed',
                        finished_at = now(), metadata = metadata || $3
                 WHERE run_id = $1",
                &[run_id, &final_status, outcome],
            )?;
            Ok(())
        })
    }
}

fn session_by_id(
    tx: &mut Transaction<'_>,
    id: &Uuid,
) -> Result<Option<SessionRecord>, postgres::Error> {
    let row = tx.query_opt(
        "SELECT session_id, tenant, product, workflow, principal,
                delegation_token_id, status, governance_mode, retention_class,
                created_at, updated_at
         FROM sessions WHERE session_id = $1",
        &[id],
    )?;
    Ok(row.map(|row| SessionRecord {
        session_id: row.get(0),
        tenant: row.get(1),
        product: row.get(2),
        workflow: row.get(3),
        principal: row.get(4),
        delegation_token_id: row.get(5),
        status: parse_session_status(row.get::<_, String>(6).as_str()),
        governance_mode: row.get(7),
        retention_class: row.get(8),
        created_at: row.get::<_, OffsetDateTime>(9),
        updated_at: row.get::<_, OffsetDateTime>(10),
    }))
}

fn run_by_id(tx: &mut Transaction<'_>, id: &Uuid) -> Result<Option<RunRecord>, postgres::Error> {
    let row = tx.query_opt(
        "SELECT run_id, session_id, status, execution_state, model_alias, backend_id,
                started_at, finished_at, metadata
         FROM runs WHERE run_id = $1",
        &[id],
    )?;
    Ok(row.map(|row| RunRecord {
        run_id: row.get(0),
        session_id: row.get(1),
        status: parse_run_status(row.get::<_, String>(2).as_str()),
        execution_state: row.get(3),
        model_alias: row.get(4),
        backend_id: row.get(5),
        started_at: row.get::<_, OffsetDateTime>(6),
        finished_at: row.get::<_, Option<OffsetDateTime>>(7),
        metadata: row.get(8),
    }))
}

/// First exhausted governing budget for this session, locking the rows so a
/// concurrent run creation cannot race past the limit.
fn exhausted_budget(
    tx: &mut Transaction<'_>,
    session_id: &Uuid,
    tenant: &str,
    product: &str,
) -> Result<Option<(String, String)>, postgres::Error> {
    let rows = tx.query(
        "SELECT scope_type, scope_ref, max_tokens, consumed_tokens FROM budgets
         WHERE (scope_type = 'session' AND scope_ref = $1)
            OR (scope_type = 'product' AND scope_ref = $2)
            OR (scope_type = 'tenant' AND scope_ref = $3)
         FOR UPDATE",
        &[
            &session_id.to_string(),
            &format!("{tenant}/{product}"),
            &tenant,
        ],
    )?;
    for row in rows {
        let max_tokens: Option<i64> = row.get(2);
        let consumed: i64 = row.get(3);
        if max_tokens.is_some_and(|max| consumed >= max) {
            return Ok(Some((row.get(0), row.get(1))));
        }
    }
    Ok(None)
}

fn parse_session_status(raw: &str) -> SessionStatus {
    match raw {
        "closed" => SessionStatus::Closed,
        _ => SessionStatus::Open,
    }
}

fn parse_run_status(raw: &str) -> RunStatus {
    match raw {
        "completed" => RunStatus::Completed,
        "failed" => RunStatus::Failed,
        "cancelled" => RunStatus::Cancelled,
        _ => RunStatus::Started,
    }
}
