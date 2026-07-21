//! Durable Postgres audit store for Thalamus (execution master plan §2).
//!
//! Authoritative, append-only audit storage on the shared Jaguar Postgres
//! server. Events are hash-chained per stream (`previous_hash`/`event_hash`),
//! sequenced per stream, and deduplicated by content-derived idempotency key,
//! so retries and duplicate emits are safe. Pre-call correlation records are
//! persisted in `route_envelopes` so post-call validation survives restarts.

mod sessions;
pub use sessions::{
    ApprovalRecordInput, ClaimRunError, CreateRunError, NewSessionInput, RecordApprovalError,
    RecordToolDecisionError,
};

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use postgres::NoTls;
use r2d2_postgres::PostgresConnectionManager;
use sha2::{Digest, Sha256};

use thalamus_core::{AuditEvent, AuditId, AuditPort, Envelope, Policy};

const EMIT_RETRIES: u32 = 3;
const EMIT_RETRY_BACKOFF: Duration = Duration::from_millis(100);

/// Embedded, ordered migrations. Applied by `run_migrations` (each inside its
/// own transaction) and recorded in `schema_migrations`. Migrations are owned
/// exclusively by the `thalamus_migrator` role.
const MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001_audit_schema",
        include_str!("../migrations/0001_audit_schema.sql"),
    ),
    (
        "0002_lifecycle_idempotency",
        include_str!("../migrations/0002_lifecycle_idempotency.sql"),
    ),
    (
        "0003_governed_calls",
        include_str!("../migrations/0003_governed_calls.sql"),
    ),
    (
        "0004_maintenance_grants",
        include_str!("../migrations/0004_maintenance_grants.sql"),
    ),
    (
        "0005_governance_idempotency",
        include_str!("../migrations/0005_governance_idempotency.sql"),
    ),
];

/// The sync `postgres` client drives its own internal tokio runtime; calling
/// it from a thread inside the server's async runtime panics ("cannot start a
/// runtime from within a runtime"). Every database operation therefore runs on
/// a short-lived scoped thread, off the caller's runtime. Pilot audit volume
/// makes the per-operation thread cost irrelevant.
fn off_runtime<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|scope| match scope.spawn(f).join() {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    })
}

#[derive(Debug, thiserror::Error)]
pub enum AuditStoreError {
    #[error("postgres error: {0}")]
    Postgres(#[from] postgres::Error),
    #[error("connection pool error: {0}")]
    Pool(#[from] r2d2::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Pre-call record recovered from the durable store.
pub struct DurablePreCallRecord {
    pub envelope: Envelope,
    pub policy: Policy,
}

/// Durable audit store backed by Jaguar Postgres.
pub struct PostgresAudit {
    pool: r2d2::Pool<PostgresConnectionManager<NoTls>>,
    healthy: AtomicBool,
}

impl PostgresAudit {
    /// Connect and fail fast: the pilot must not run without authoritative
    /// audit storage.
    pub fn connect(database_url: &str) -> Result<Self, AuditStoreError> {
        let config: postgres::Config = database_url.parse().map_err(AuditStoreError::Postgres)?;
        off_runtime(move || {
            let manager = PostgresConnectionManager::new(config, NoTls);
            let pool = r2d2::Pool::builder()
                .max_size(5)
                .connection_timeout(Duration::from_secs(5))
                .build(manager)?;
            pool.get()?.batch_execute("SELECT 1")?;
            Ok(Self {
                pool,
                healthy: AtomicBool::new(true),
            })
        })
    }

    /// Apply pending embedded migrations. Must run with `thalamus_migrator`
    /// credentials; the app role cannot create tables.
    pub fn run_migrations(database_url: &str) -> Result<Vec<String>, AuditStoreError> {
        off_runtime(move || Self::run_migrations_inner(database_url))
    }

    fn run_migrations_inner(database_url: &str) -> Result<Vec<String>, AuditStoreError> {
        let mut client = postgres::Client::connect(database_url, NoTls)?;
        client.batch_execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version text PRIMARY KEY,
                applied_at timestamptz NOT NULL DEFAULT now()
            )",
        )?;
        let mut applied = Vec::new();
        for (version, sql) in MIGRATIONS {
            let done = client
                .query_opt(
                    "SELECT 1 FROM schema_migrations WHERE version = $1",
                    &[version],
                )?
                .is_some();
            if done {
                continue;
            }
            let mut tx = client.transaction()?;
            tx.batch_execute(sql)?;
            tx.execute(
                "INSERT INTO schema_migrations (version) VALUES ($1)",
                &[version],
            )?;
            tx.commit()?;
            applied.push((*version).to_owned());
        }
        Ok(applied)
    }

    /// Last-write health as observed by the emit path.
    pub fn healthy(&self) -> bool {
        self.healthy.load(Ordering::Relaxed)
    }

    /// Live readiness probe (used by /readyz).
    pub fn probe(&self) -> bool {
        off_runtime(|| match self.pool.get() {
            Ok(mut conn) => conn.batch_execute("SELECT 1").is_ok(),
            Err(_) => false,
        })
    }

    /// Durably append one audit event. Idempotent: re-emitting an identical
    /// event is a no-op (content-derived idempotency key).
    pub fn emit_durable(&self, event: &AuditEvent) -> Result<(), AuditStoreError> {
        off_runtime(|| self.emit_durable_inner(event))
    }

    fn emit_durable_inner(&self, event: &AuditEvent) -> Result<(), AuditStoreError> {
        let payload = serde_json::to_value(event)?;
        let payload_bytes = serde_json::to_vec(event)?;
        let idempotency_key = hex_sha256(&payload_bytes);
        let meta = EventMeta::of(event);

        let mut conn = self.pool.get()?;
        let mut tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO audit_streams (stream_id) VALUES ($1)
             ON CONFLICT (stream_id) DO NOTHING",
            &[&meta.stream_id],
        )?;
        let head = tx.query_one(
            "SELECT last_seq, last_hash FROM audit_streams
             WHERE stream_id = $1 FOR UPDATE",
            &[&meta.stream_id],
        )?;
        let last_seq: i64 = head.get(0);
        let last_hash: String = head.get(1);

        let duplicate = tx
            .query_opt(
                "SELECT 1 FROM audit_events WHERE idempotency_key = $1",
                &[&idempotency_key],
            )?
            .is_some();
        if duplicate {
            tx.commit()?;
            return Ok(());
        }

        let seq = last_seq + 1;
        let event_hash = chain_hash(&last_hash, &payload_bytes);
        tx.execute(
            "INSERT INTO audit_events
                (stream_id, seq, event_type, audit_id, trace_id, payload,
                 previous_hash, event_hash, idempotency_key, occurred_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            &[
                &meta.stream_id,
                &seq,
                &meta.event_type,
                &meta.audit_id,
                &meta.trace_id,
                &payload,
                &last_hash,
                &event_hash,
                &idempotency_key,
                &meta.occurred_at,
            ],
        )?;
        tx.execute(
            "UPDATE audit_streams
             SET last_seq = $2, last_hash = $3, updated_at = now()
             WHERE stream_id = $1",
            &[&meta.stream_id, &seq, &event_hash],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// All events for an audit id, in chain order.
    pub fn events_by_audit_id(
        &self,
        audit_id: &AuditId,
    ) -> Result<Vec<AuditEvent>, AuditStoreError> {
        off_runtime(|| self.events_by_audit_id_inner(audit_id))
    }

    fn events_by_audit_id_inner(
        &self,
        audit_id: &AuditId,
    ) -> Result<Vec<AuditEvent>, AuditStoreError> {
        let mut conn = self.pool.get()?;
        let rows = conn.query(
            "SELECT payload FROM audit_events
             WHERE audit_id = $1 ORDER BY stream_id, seq",
            &[&audit_id.0.to_string()],
        )?;
        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let payload: serde_json::Value = row.get(0);
            events.push(serde_json::from_value(payload)?);
        }
        Ok(events)
    }

    /// Persist the pre-call correlation record (idempotent per audit_id).
    pub fn store_route_envelope(
        &self,
        audit_id: &AuditId,
        envelope: &Envelope,
        policy: &Policy,
    ) -> Result<(), AuditStoreError> {
        off_runtime(|| self.store_route_envelope_inner(audit_id, envelope, policy, None, None))
    }

    /// Correlated variant for run-bound governed calls: the stored record
    /// carries the owning session and run, so every model call is joinable to
    /// its identity/session/run chain (SLICE-T1 invariant I8).
    pub fn store_route_envelope_correlated(
        &self,
        audit_id: &AuditId,
        envelope: &Envelope,
        policy: &Policy,
        session_id: &uuid::Uuid,
        run_id: &uuid::Uuid,
    ) -> Result<(), AuditStoreError> {
        off_runtime(|| {
            self.store_route_envelope_inner(
                audit_id,
                envelope,
                policy,
                Some(session_id),
                Some(run_id),
            )
        })
    }

    fn store_route_envelope_inner(
        &self,
        audit_id: &AuditId,
        envelope: &Envelope,
        policy: &Policy,
        session_id: Option<&uuid::Uuid>,
        run_id: Option<&uuid::Uuid>,
    ) -> Result<(), AuditStoreError> {
        let mut conn = self.pool.get()?;
        conn.execute(
            "INSERT INTO route_envelopes
                (audit_id, envelope, policy, policy_ref, model_alias, session_id, run_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (audit_id) DO NOTHING",
            &[
                &audit_id.0.to_string(),
                &serde_json::to_value(envelope)?,
                &serde_json::to_value(policy)?,
                &envelope.policy_ref,
                &envelope.backend_handle.id,
                &session_id,
                &run_id,
            ],
        )?;
        Ok(())
    }

    /// Recover the pre-call correlation record for post-call validation.
    pub fn route_envelope(
        &self,
        audit_id: &AuditId,
    ) -> Result<Option<DurablePreCallRecord>, AuditStoreError> {
        off_runtime(|| self.route_envelope_inner(audit_id))
    }

    fn route_envelope_inner(
        &self,
        audit_id: &AuditId,
    ) -> Result<Option<DurablePreCallRecord>, AuditStoreError> {
        let mut conn = self.pool.get()?;
        let row = conn.query_opt(
            "SELECT envelope, policy FROM route_envelopes WHERE audit_id = $1",
            &[&audit_id.0.to_string()],
        )?;
        match row {
            None => Ok(None),
            Some(row) => {
                let envelope: serde_json::Value = row.get(0);
                let policy: serde_json::Value = row.get(1);
                Ok(Some(DurablePreCallRecord {
                    envelope: serde_json::from_value(envelope)?,
                    policy: serde_json::from_value(policy)?,
                }))
            }
        }
    }
}

impl AuditPort for PostgresAudit {
    fn emit(&self, event: AuditEvent) {
        tracing::info!(event = ?event, "audit_event");
        for attempt in 1..=EMIT_RETRIES {
            match self.emit_durable(&event) {
                Ok(()) => {
                    self.healthy.store(true, Ordering::Relaxed);
                    return;
                }
                Err(err) if attempt < EMIT_RETRIES => {
                    tracing::warn!(%err, attempt, "durable audit write failed, retrying");
                    std::thread::sleep(EMIT_RETRY_BACKOFF * attempt);
                }
                Err(err) => {
                    // Fail-closed signal: routes and /readyz consult healthy().
                    self.healthy.store(false, Ordering::Relaxed);
                    tracing::error!(%err, "durable audit write failed after retries; store marked unhealthy");
                }
            }
        }
    }
}

struct EventMeta {
    stream_id: String,
    event_type: String,
    audit_id: String,
    trace_id: String,
    occurred_at: time::OffsetDateTime,
}

impl EventMeta {
    fn of(event: &AuditEvent) -> Self {
        match event {
            AuditEvent::PreCallDecision {
                trace_id,
                audit_id,
                timestamp,
                ..
            } => Self {
                stream_id: audit_id.0.to_string(),
                event_type: "PreCallDecision".to_owned(),
                audit_id: audit_id.0.to_string(),
                trace_id: trace_id.0.to_string(),
                occurred_at: *timestamp,
            },
            AuditEvent::PostCallOutcome {
                trace_id,
                audit_id,
                timestamp,
                ..
            } => Self {
                stream_id: audit_id.0.to_string(),
                event_type: "PostCallOutcome".to_owned(),
                audit_id: audit_id.0.to_string(),
                trace_id: trace_id.0.to_string(),
                occurred_at: *timestamp,
            },
            AuditEvent::RouteEnvelope {
                trace_id,
                audit_id,
                timestamp,
                ..
            } => Self {
                stream_id: audit_id.0.to_string(),
                event_type: "RouteEnvelope".to_owned(),
                audit_id: audit_id.0.to_string(),
                trace_id: trace_id.0.to_string(),
                occurred_at: *timestamp,
            },
            AuditEvent::Lifecycle {
                trace_id,
                audit_id,
                timestamp,
                ..
            } => Self {
                // Lifecycle streams chain on the audit id (= session id), so
                // one session's whole lifecycle forms a single hash chain.
                stream_id: audit_id.0.to_string(),
                event_type: "Lifecycle".to_owned(),
                audit_id: audit_id.0.to_string(),
                trace_id: trace_id.0.to_string(),
                occurred_at: *timestamp,
            },
        }
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// event_hash = sha256(previous_hash_hex || payload_bytes), hex-encoded.
fn chain_hash(previous_hash: &str, payload_bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(previous_hash.as_bytes());
    hasher.update(payload_bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_hash_links_previous_hash() {
        let h1 = chain_hash("", b"event-1");
        let h2 = chain_hash(&h1, b"event-2");
        let h2_other_parent = chain_hash("", b"event-2");
        assert_ne!(h1, h2);
        assert_ne!(h2, h2_other_parent);
        // Deterministic for identical inputs (idempotent re-emit).
        assert_eq!(h2, chain_hash(&h1, b"event-2"));
    }

    #[test]
    fn identical_events_share_idempotency_key() {
        let a = hex_sha256(b"same-payload");
        let b = hex_sha256(b"same-payload");
        assert_eq!(a, b);
    }
}
