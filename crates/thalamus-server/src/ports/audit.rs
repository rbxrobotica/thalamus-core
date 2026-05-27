use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use thalamus_core::{AuditEvent, AuditId, AuditPort, Envelope, Policy};

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
