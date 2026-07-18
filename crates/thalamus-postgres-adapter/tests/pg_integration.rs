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
        user: Some("ldamasio@gmail.com".to_owned()),
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

// === Phase 3 slice 1: session/run lifecycle on the durable store ===

use thalamus_core::{RunStatus, SessionStatus};
use thalamus_postgres_adapter::{CreateRunError, NewSessionInput};

fn new_session_input(idempotency_key: Option<&str>) -> NewSessionInput {
    NewSessionInput {
        tenant: format!("t-{}", Uuid::new_v4()),
        product: "kulinaryos".to_owned(),
        workflow: "coding".to_owned(),
        principal: Some("ldamasio@gmail.com".to_owned()),
        delegation_token_id: Some("jti-1".to_owned()),
        idempotency_key: idempotency_key.map(str::to_owned),
    }
}

#[test]
fn session_run_lifecycle_and_closed_session_refusal() {
    let Some(url) = test_url() else {
        eprintln!("skipped: THALAMUS_TEST_DATABASE_URL not set");
        return;
    };
    migrate_once(&url);
    let store = PostgresAudit::connect(&url).expect("connect");

    let session = store
        .create_session(&new_session_input(None))
        .expect("create session");
    assert_eq!(session.status, SessionStatus::Open);

    let run = store
        .create_run(&session.session_id, Some("glm-5.2"), None)
        .expect("create run");
    assert_eq!(run.status, RunStatus::Started);
    assert_eq!(run.session_id, session.session_id);

    let cancelled = store
        .cancel_run(&run.run_id)
        .expect("cancel")
        .expect("run exists");
    assert_eq!(cancelled.status, RunStatus::Cancelled);
    assert!(cancelled.finished_at.is_some());

    let closed = store
        .close_session(&session.session_id)
        .expect("close")
        .expect("session exists");
    assert_eq!(closed.status, SessionStatus::Closed);

    let refused = store.create_run(&session.session_id, None, None);
    assert!(matches!(refused, Err(CreateRunError::SessionClosed)));
}

#[test]
fn budget_exhaustion_blocks_runs_and_shows_in_limits() {
    let Some(url) = test_url() else {
        eprintln!("skipped: THALAMUS_TEST_DATABASE_URL not set");
        return;
    };
    migrate_once(&url);
    let store = PostgresAudit::connect(&url).expect("connect");

    let input = new_session_input(None);
    let product_scope = format!("{}/{}", input.tenant, input.product);
    let session = store.create_session(&input).expect("create session");

    // Provision a product budget with headroom, consume it, then verify.
    let mut client = postgres::Client::connect(&url, postgres::NoTls).expect("psql");
    client
        .execute(
            "INSERT INTO budgets (scope_type, scope_ref, period, max_tokens)
             VALUES ('product', $1, 'total', 500)",
            &[&product_scope],
        )
        .expect("seed budget");

    let run = store.create_run(&session.session_id, None, None);
    assert!(run.is_ok(), "budget with headroom must allow runs");

    store
        .record_usage(&session.session_id, 500)
        .expect("record usage");

    let refused = store.create_run(&session.session_id, None, None);
    assert!(
        matches!(
            refused,
            Err(CreateRunError::BudgetExceeded { ref scope_type, .. }) if scope_type == "product"
        ),
        "exhausted budget must refuse runs, got {refused:?}"
    );

    let limits = store
        .session_limits(&session.session_id)
        .expect("limits")
        .expect("session exists");
    let line = limits
        .budgets
        .iter()
        .find(|b| b.scope_ref == product_scope)
        .expect("budget line present");
    assert!(line.exhausted);
    assert_eq!(line.consumed_tokens, 500);
    assert_eq!(limits.context_utilization_limit, 0.7);
}

#[test]
fn idempotent_session_and_run_creation_survive_replay() {
    let Some(url) = test_url() else {
        eprintln!("skipped: THALAMUS_TEST_DATABASE_URL not set");
        return;
    };
    migrate_once(&url);
    let store = PostgresAudit::connect(&url).expect("connect");

    let key = format!("idem-{}", Uuid::new_v4());
    let input = new_session_input(Some(&key));
    let first = store.create_session(&input).expect("first");
    let second = store.create_session(&input).expect("replay");
    assert_eq!(first.session_id, second.session_id);

    let run_key = format!("idem-run-{}", Uuid::new_v4());
    let run_a = store
        .create_run(&first.session_id, None, Some(&run_key))
        .expect("first run");
    let run_b = store
        .create_run(&first.session_id, None, Some(&run_key))
        .expect("replayed run");
    assert_eq!(run_a.run_id, run_b.run_id);
}

// === Phase 3 slice 4: governance records on the durable store ===

#[test]
fn governance_records_persist_on_postgres() {
    let Some(url) = test_url() else {
        eprintln!("skipped: THALAMUS_TEST_DATABASE_URL not set");
        return;
    };
    migrate_once(&url);
    let store = PostgresAudit::connect(&url).expect("connect");

    let session = store
        .create_session(&new_session_input(None))
        .expect("session");
    let run = store
        .create_run(&session.session_id, None, None)
        .expect("run");

    let invocation = store
        .record_tool_decision(
            &session.session_id,
            Some(&run.run_id),
            "shell",
            "denied",
            &serde_json::json!({ "cmd": "redacted" }),
        )
        .expect("tool decision")
        .expect("session exists");

    let missing = store
        .record_tool_decision(
            &Uuid::new_v4(),
            None,
            "shell",
            "allowed",
            &serde_json::json!({}),
        )
        .expect("no store error");
    assert!(missing.is_none(), "unknown session must be refused");

    let approval = store
        .record_approval(&thalamus_postgres_adapter::ApprovalRecordInput {
            session_id: Some(&session.session_id),
            run_id: Some(&run.run_id),
            subject: "patch:abc",
            approver: "ldamasio@gmail.com",
            decision: "approved",
            reason: Some("looks good"),
            metadata: &serde_json::json!({}),
        })
        .expect("approval");

    let evidence = store
        .record_evidence(
            Some(&run.run_id),
            "test-run",
            "s3://rbx-evidence/x",
            "deadbeef",
        )
        .expect("evidence");

    let mut client = postgres::Client::connect(&url, postgres::NoTls).expect("psql");
    let inv: i64 = client
        .query_one(
            "SELECT count(*) FROM tool_invocations WHERE invocation_id = $1 AND status = 'denied'",
            &[&invocation],
        )
        .unwrap()
        .get(0);
    let appr: i64 = client
        .query_one(
            "SELECT count(*) FROM approvals WHERE approval_id = $1 AND approver = 'ldamasio@gmail.com'",
            &[&approval],
        )
        .unwrap()
        .get(0);
    let evid: i64 = client
        .query_one(
            "SELECT count(*) FROM evidence_refs WHERE evidence_id = $1 AND content_hash = 'deadbeef'",
            &[&evidence],
        )
        .unwrap()
        .get(0);
    assert_eq!((inv, appr, evid), (1, 1, 1));
}
