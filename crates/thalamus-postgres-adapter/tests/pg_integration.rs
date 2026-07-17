//! Integration tests against a real Postgres. Skipped unless
//! `THALAMUS_TEST_DATABASE_URL` is set (e.g. a throwaway container):
//!
//! ```text
//! podman run --rm -d -e POSTGRES_PASSWORD=test -p 55432:5432 postgres:16
//! THALAMUS_TEST_DATABASE_URL=postgres://postgres:test@127.0.0.1:55432/postgres \
//!     cargo test -p thalamus-postgres-adapter -- --nocapture
//! ```

use time::OffsetDateTime;
use uuid::Uuid;

use thalamus_core::{AuditEvent, AuditId, AuditPort, RiskLevel, TraceId};
use thalamus_postgres_adapter::PostgresAudit;

fn test_url() -> Option<String> {
    std::env::var("THALAMUS_TEST_DATABASE_URL").ok()
}

/// Tests share one database; concurrent migration runs would race on DDL.
fn migrate_once(url: &str) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        PostgresAudit::run_migrations(url).expect("migrate");
    });
}

fn pre_call_event(audit_id: AuditId) -> AuditEvent {
    AuditEvent::PreCallDecision {
        trace_id: TraceId(Uuid::new_v4()),
        audit_id,
        tenant: "rbx".to_owned(),
        product: "kulinaryos".to_owned(),
        workflow: "test".to_owned(),
        policy_ref: "policy-test".to_owned(),
        decision: "Allow".to_owned(),
        backend: None,
        timestamp: OffsetDateTime::now_utc(),
    }
}

fn post_call_event(audit_id: AuditId) -> AuditEvent {
    AuditEvent::PostCallOutcome {
        trace_id: TraceId(Uuid::new_v4()),
        audit_id,
        status: "Valid".to_owned(),
        risk_class: RiskLevel::Low,
        executable_by_agent: true,
        schema_valid: true,
        timestamp: OffsetDateTime::now_utc(),
    }
}

#[test]
fn migrations_are_idempotent() {
    let Some(url) = test_url() else {
        eprintln!("skipped: THALAMUS_TEST_DATABASE_URL not set");
        return;
    };
    migrate_once(&url);
    let rerun = PostgresAudit::run_migrations(&url).expect("re-run");
    assert!(rerun.is_empty(), "re-run must apply nothing, got {rerun:?}");
}

#[test]
fn emit_chains_and_deduplicates() {
    let Some(url) = test_url() else {
        eprintln!("skipped: THALAMUS_TEST_DATABASE_URL not set");
        return;
    };
    migrate_once(&url);
    let store = PostgresAudit::connect(&url).expect("connect");

    let audit_id = AuditId(Uuid::new_v4());
    let pre = pre_call_event(audit_id.clone());
    let post = post_call_event(audit_id.clone());

    store.emit(pre.clone());
    store.emit(pre.clone()); // duplicate: must not create a second row
    store.emit(post.clone());
    assert!(store.healthy());

    let events = store.events_by_audit_id(&audit_id).expect("read back");
    assert_eq!(events.len(), 2, "duplicate emit must be deduplicated");
    assert!(matches!(events[0], AuditEvent::PreCallDecision { .. }));
    assert!(matches!(events[1], AuditEvent::PostCallOutcome { .. }));

    // Chain integrity: seq 1..2, previous_hash links, recomputable.
    let mut client = postgres::Client::connect(&url, postgres::NoTls).expect("psql");
    let rows = client
        .query(
            "SELECT seq, previous_hash, event_hash, payload::text
             FROM audit_events WHERE stream_id = $1 ORDER BY seq",
            &[&audit_id.0.to_string()],
        )
        .expect("chain rows");
    assert_eq!(rows.len(), 2);
    let seq1: i64 = rows[0].get(0);
    let prev1: String = rows[0].get(1);
    let hash1: String = rows[0].get(2);
    let seq2: i64 = rows[1].get(0);
    let prev2: String = rows[1].get(1);
    assert_eq!((seq1, seq2), (1, 2));
    assert_eq!(prev1, "");
    assert_eq!(prev2, hash1, "second event must chain to the first");
}

#[test]
fn audit_events_reject_mutation() {
    let Some(url) = test_url() else {
        eprintln!("skipped: THALAMUS_TEST_DATABASE_URL not set");
        return;
    };
    migrate_once(&url);
    let store = PostgresAudit::connect(&url).expect("connect");
    let audit_id = AuditId(Uuid::new_v4());
    store.emit(pre_call_event(audit_id.clone()));

    let mut client = postgres::Client::connect(&url, postgres::NoTls).expect("psql");
    let update = client.execute(
        "UPDATE audit_events SET event_type = 'tampered' WHERE stream_id = $1",
        &[&audit_id.0.to_string()],
    );
    assert!(update.is_err(), "append-only trigger must reject UPDATE");
    let delete = client.execute(
        "DELETE FROM audit_events WHERE stream_id = $1",
        &[&audit_id.0.to_string()],
    );
    assert!(delete.is_err(), "append-only trigger must reject DELETE");
}

#[test]
fn route_envelope_roundtrip_survives_reconnect() {
    let Some(url) = test_url() else {
        eprintln!("skipped: THALAMUS_TEST_DATABASE_URL not set");
        return;
    };
    migrate_once(&url);

    let audit_id = AuditId(Uuid::new_v4());
    let envelope = thalamus_core::Envelope {
        trace_id: TraceId(Uuid::new_v4()),
        audit_id: audit_id.clone(),
        backend_handle: thalamus_core::BackendHandle {
            id: "glm-5.2".to_owned(),
            backend_type: thalamus_core::BackendType::Model,
        },
        prompt: "prompt".to_owned(),
        authorized_context: vec![],
        redaction_applied: false,
        policy_ref: "policy-test".to_owned(),
        budget: thalamus_core::Budget {
            max_tokens: 1000,
            max_latency_ms: 30_000,
        },
    };
    let policy = thalamus_core::Policy {
        id: "policy-test".to_owned(),
        tenant: "rbx".to_owned(),
        product: "kulinaryos".to_owned(),
        workflow: "test".to_owned(),
        permitted_backends: vec![envelope.backend_handle.clone()],
        budget: envelope.budget.clone(),
        context_grants: vec![],
        redaction_rules: vec![],
        audit_required: true,
        risk_threshold: RiskLevel::High,
    };

    {
        let store = PostgresAudit::connect(&url).expect("connect");
        store
            .store_route_envelope(&audit_id, &envelope, &policy)
            .expect("store");
        // Idempotent second store.
        store
            .store_route_envelope(&audit_id, &envelope, &policy)
            .expect("store again");
    } // drop = simulated restart

    let store = PostgresAudit::connect(&url).expect("reconnect");
    let record = store
        .route_envelope(&audit_id)
        .expect("read")
        .expect("record must survive reconnect");
    assert_eq!(record.envelope.prompt, "prompt");
    assert_eq!(record.policy.id, "policy-test");
}
