use std::sync::{Arc, Mutex};

use thalamus_core::{AuditEvent, AuditId, AuditPort};

/// Append-only audit port: emits structured log lines and stores events
/// in memory for /v1/audit retrieval.
pub struct InMemoryAuditPort {
    events: Arc<Mutex<Vec<AuditEvent>>>,
}

impl InMemoryAuditPort {
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn store(&self) -> AuditStore {
        AuditStore {
            events: Arc::clone(&self.events),
        }
    }
}

impl AuditPort for InMemoryAuditPort {
    fn emit(&self, event: AuditEvent) {
        tracing::info!(event = ?event, "audit_event");
        self.events.lock().unwrap().push(event);
    }
}

/// Handle for querying stored audit events.
#[derive(Clone)]
pub struct AuditStore {
    events: Arc<Mutex<Vec<AuditEvent>>>,
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
}
