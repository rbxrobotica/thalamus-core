use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use thalamus_core::{AuditEvent, AuditId, AuditPort, Envelope, Policy};

/// Authoritative durable audit surface (Phase 2, master plan §2). When wired,
/// the in-memory store is no longer authoritative: audit events and pre-call
/// correlation records live in Postgres (Jaguar) and survive restarts.
pub trait DurableAuditStore: AuditPort + Send + Sync {
    /// Last-write health as observed by the emit path (fail-closed signal).
    fn healthy(&self) -> bool;
    /// Live readiness probe.
    fn probe(&self) -> bool;
    fn events_by_audit_id(&self, audit_id: &AuditId) -> Result<Vec<AuditEvent>, String>;
    fn store_pre_call_record(
        &self,
        audit_id: &AuditId,
        envelope: &Envelope,
        policy: &Policy,
    ) -> Result<(), String>;
    /// Correlated variant for run-bound governed calls (SLICE-T1): the stored
    /// record carries the owning session and run ids.
    fn store_pre_call_record_correlated(
        &self,
        audit_id: &AuditId,
        envelope: &Envelope,
        policy: &Policy,
        session_id: &uuid::Uuid,
        run_id: &uuid::Uuid,
    ) -> Result<(), String>;
    fn get_pre_call_record(&self, audit_id: &AuditId) -> Result<Option<PreCallRecord>, String>;
}

pub type SharedAuditPort = Arc<dyn AuditPort + Send + Sync>;
pub type SharedDurableAudit = Arc<dyn DurableAuditStore + Send + Sync>;

#[cfg(feature = "postgres")]
impl DurableAuditStore for thalamus_postgres_adapter::PostgresAudit {
    fn healthy(&self) -> bool {
        thalamus_postgres_adapter::PostgresAudit::healthy(self)
    }

    fn probe(&self) -> bool {
        thalamus_postgres_adapter::PostgresAudit::probe(self)
    }

    fn events_by_audit_id(&self, audit_id: &AuditId) -> Result<Vec<AuditEvent>, String> {
        thalamus_postgres_adapter::PostgresAudit::events_by_audit_id(self, audit_id)
            .map_err(|e| e.to_string())
    }

    fn store_pre_call_record(
        &self,
        audit_id: &AuditId,
        envelope: &Envelope,
        policy: &Policy,
    ) -> Result<(), String> {
        self.store_route_envelope(audit_id, envelope, policy)
            .map_err(|e| e.to_string())
    }

    fn store_pre_call_record_correlated(
        &self,
        audit_id: &AuditId,
        envelope: &Envelope,
        policy: &Policy,
        session_id: &uuid::Uuid,
        run_id: &uuid::Uuid,
    ) -> Result<(), String> {
        self.store_route_envelope_correlated(audit_id, envelope, policy, session_id, run_id)
            .map_err(|e| e.to_string())
    }

    fn get_pre_call_record(&self, audit_id: &AuditId) -> Result<Option<PreCallRecord>, String> {
        self.route_envelope(audit_id)
            .map(|opt| {
                opt.map(|r| PreCallRecord {
                    envelope: r.envelope,
                    policy: r.policy,
                })
            })
            .map_err(|e| e.to_string())
    }
}

/// Build the audit wiring for the app: in-memory by default; authoritative
/// Postgres when the `postgres` feature is compiled and `THALAMUS_DATABASE_URL`
/// is set (disable explicitly with `THALAMUS_DURABLE_AUDIT=off`). Startup
/// fails fast when the durable store is configured but unreachable: the pilot
/// must not run without authoritative audit writes.
pub fn audit_wiring() -> (
    SharedAuditPort,
    AuditStore,
    Option<SharedDurableAudit>,
    crate::ports::sessions::SharedSessionStore,
) {
    let mem = Arc::new(InMemoryAuditPort::new());
    let store = mem.store();

    #[cfg(feature = "postgres")]
    if durable_audit_enabled() {
        let url = std::env::var("THALAMUS_DATABASE_URL")
            .expect("durable_audit_enabled implies THALAMUS_DATABASE_URL");
        match thalamus_postgres_adapter::PostgresAudit::connect(&url) {
            Ok(pg) => {
                let pg: Arc<thalamus_postgres_adapter::PostgresAudit> = Arc::new(pg);
                tracing::info!("durable audit store enabled (Postgres authoritative)");
                return (pg.clone(), store, Some(pg.clone()), pg);
            }
            Err(err) => {
                eprintln!("fatal: durable audit store configured but unreachable: {err}");
                std::process::exit(1);
            }
        }
    }

    let sessions = Arc::new(crate::ports::sessions::InMemorySessionStore::new());
    (mem, store, None, sessions)
}

#[cfg(feature = "postgres")]
fn durable_audit_enabled() -> bool {
    if std::env::var("THALAMUS_DATABASE_URL").is_err() {
        return false;
    }
    !matches!(
        std::env::var("THALAMUS_DURABLE_AUDIT")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "0" | "off" | "false" | "no"
    )
}

/// Append-only audit port: emits structured log lines and stores events
/// in memory for /v1/audit retrieval.
pub struct InMemoryAuditPort {
    events: Arc<Mutex<Vec<AuditEvent>>>,
    records: Arc<Mutex<HashMap<AuditId, PreCallRecord>>>,
}

impl Default for InMemoryAuditPort {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryAuditPort {
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            records: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn store(&self) -> AuditStore {
        AuditStore {
            events: Arc::clone(&self.events),
            records: Arc::clone(&self.records),
        }
    }
}

impl AuditPort for InMemoryAuditPort {
    fn emit(&self, event: AuditEvent) {
        tracing::info!(event = ?event, "audit_event");
        self.events.lock().unwrap().push(event);
    }
}

/// Pre-call record stored for post-call correlation.
#[derive(Clone)]
pub struct PreCallRecord {
    pub envelope: Envelope,
    pub policy: Policy,
}

/// Handle for querying stored audit events and pre-call records.
#[derive(Clone)]
pub struct AuditStore {
    events: Arc<Mutex<Vec<AuditEvent>>>,
    records: Arc<Mutex<HashMap<AuditId, PreCallRecord>>>,
}

impl AuditStore {
    pub fn get_by_audit_id(&self, audit_id: &AuditId) -> Vec<AuditEvent> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| match e {
                AuditEvent::PreCallDecision { audit_id: aid, .. } => aid == audit_id,
                AuditEvent::PostCallOutcome { audit_id: aid, .. } => aid == audit_id,
                AuditEvent::Lifecycle { audit_id: aid, .. } => aid == audit_id,
                AuditEvent::RouteEnvelope { audit_id: aid, .. } => aid == audit_id,
            })
            .cloned()
            .collect()
    }

    pub fn store_pre_call_record(&self, audit_id: AuditId, envelope: Envelope, policy: Policy) {
        self.records
            .lock()
            .unwrap()
            .insert(audit_id, PreCallRecord { envelope, policy });
    }

    pub fn get_pre_call_record(&self, audit_id: &AuditId) -> Option<PreCallRecord> {
        self.records.lock().unwrap().get(audit_id).cloned()
    }
}
