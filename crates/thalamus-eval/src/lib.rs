use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crossbeam_channel::{bounded, Receiver, Sender};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use thalamus_core::{BackendResponse, EvalPort, Policy};

// === EvalRecord: structured, deterministic facts per evaluation submission ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalRecord {
    pub eval_ref: String,
    pub schema_valid: bool,
    pub citation_check: CitationCheckOutcome,
    pub hallucination_signals: Vec<String>,
    pub risk_class: String,
    pub response_metadata: ResponseMetadata,
    pub trace_id: Option<String>,
    pub audit_id: Option<String>,
    pub policy_id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CitationCheckOutcome {
    NotRequired,
    Passed,
    Failed { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMetadata {
    pub content_len: usize,
    pub tokens_used: Option<u32>,
    pub latency_ms: Option<u64>,
}

// === EvalStore: in-memory store keyed by eval_ref ===

#[derive(Debug, Clone)]
pub struct EvalStore {
    records: Arc<Mutex<HashMap<String, EvalRecord>>>,
}

impl EvalStore {
    pub fn new() -> Self {
        Self {
            records: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn insert(&self, record: EvalRecord) {
        let key = record.eval_ref.clone();
        self.records.lock().unwrap().insert(key, record);
    }

    pub fn get(&self, eval_ref: &str) -> Option<EvalRecord> {
        self.records.lock().unwrap().get(eval_ref).cloned()
    }

    pub fn len(&self) -> usize {
        self.records.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.lock().unwrap().is_empty()
    }

    pub fn all_refs(&self) -> Vec<String> {
        self.records.lock().unwrap().keys().cloned().collect()
    }

    /// Block until at least `n` records are stored or timeout elapses.
    /// Returns the count actually stored. Useful for tests.
    pub fn wait_for_count(&self, n: usize, timeout_ms: u64) -> usize {
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
        loop {
            let current = self.len();
            if current >= n {
                return current;
            }
            if std::time::Instant::now() > deadline {
                return current;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}

// === Internal message for the worker channel ===

enum EvalMessage {
    Record(EvalRecord),
    Shutdown,
}

// === ChannelEvalPort: non-blocking EvalPort implementation ===

/// An EvalPort that sends evaluation records through a bounded channel to a
/// dedicated worker thread. Submission is O(1) and never blocks the async
/// runtime. If the channel is full, the record is dropped (eval is best-effort
/// per observability-and-evaluation.md).
pub struct ChannelEvalPort {
    tx: Sender<EvalMessage>,
    store: EvalStore,
    worker_handle: Option<std::thread::JoinHandle<()>>,
}

impl ChannelEvalPort {
    /// Create a new ChannelEvalPort with a bounded channel of capacity `cap`.
    pub fn new(cap: usize) -> Self {
        let (tx, rx) = bounded::<EvalMessage>(cap);
        let store = EvalStore::new();
        let worker_store = store.clone();

        let handle = std::thread::Builder::new()
            .name("thalamus-eval-worker".to_owned())
            .spawn(move || {
                worker_loop(rx, &worker_store);
            })
            .expect("failed to spawn eval worker thread");

        Self {
            tx,
            store,
            worker_handle: Some(handle),
        }
    }

    /// Access the store for inspection (tests, future console/TH-S6b).
    pub fn store(&self) -> &EvalStore {
        &self.store
    }

    /// Gracefully shut down the worker thread.
    pub fn shutdown(&mut self) {
        let _ = self.tx.send(EvalMessage::Shutdown);
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ChannelEvalPort {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl EvalPort for ChannelEvalPort {
    fn submit(&self, response: &BackendResponse, policy: &Policy) -> String {
        let eval_ref = format!("eval-{}", Uuid::new_v4());

        let record = build_record(
            eval_ref.clone(),
            response,
            policy,
            None,
            None,
        );

        // Non-blocking send: if channel full, drop (best-effort eval)
        match self.tx.try_send(EvalMessage::Record(record)) {
            Ok(()) => {}
            Err(_) => {
                tracing::warn!(
                    eval_ref = %eval_ref,
                    "eval channel full or closed — dropping evaluation (best-effort)"
                );
            }
        }

        eval_ref
    }
}

fn worker_loop(rx: Receiver<EvalMessage>, store: &EvalStore) {
    loop {
        match rx.recv() {
            Ok(EvalMessage::Record(record)) => {
                store.insert(record);
            }
            Ok(EvalMessage::Shutdown) | Err(_) => {
                break;
            }
        }
    }
}

/// Build an EvalRecord from the deterministic facts available at post_call.
///
/// No ML, no fabricated numeric scores, no simulated hallucination detection.
/// Every field is derived from policy, response metadata, or structural checks.
fn build_record(
    eval_ref: String,
    response: &BackendResponse,
    policy: &Policy,
    trace_id: Option<String>,
    audit_id: Option<String>,
) -> EvalRecord {
    let content_len = response.content.len();
    let tokens_used = response.tokens_used;
    let latency_ms = response.latency_ms;

    // Schema validity: non-empty content passes (same heuristic as flow.rs)
    let schema_valid = !response.content.is_empty();

    // Citation check outcome: placeholder (NotRequired) — same as flow.rs
    let citation_check = CitationCheckOutcome::NotRequired;

    // Hallucination signals: empty — no ML detection exists
    // This is a placeholder field; any future heuristic must be explicitly named.
    let hallucination_signals: Vec<String> = Vec::new();

    // Risk classification: derived from budget usage (same logic as flow.rs)
    let risk_class = classify_risk_from_budget(tokens_used, policy.budget.max_tokens);

    EvalRecord {
        eval_ref,
        schema_valid,
        citation_check,
        hallucination_signals,
        risk_class,
        response_metadata: ResponseMetadata {
            content_len,
            tokens_used,
            latency_ms,
        },
        trace_id,
        audit_id,
        policy_id: policy.id.clone(),
        created_at: OffsetDateTime::now_utc().to_string(),
    }
}

fn classify_risk_from_budget(tokens_used: Option<u32>, max_tokens: u32) -> String {
    match tokens_used {
        Some(tokens) if tokens > max_tokens => "Prohibited".to_owned(),
        Some(tokens) if tokens > max_tokens * 3 / 4 => "High".to_owned(),
        Some(tokens) if tokens > max_tokens / 2 => "Medium".to_owned(),
        _ => "Low".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use thalamus_core::{BackendHandle, BackendType, Budget, ContextGrant};

    fn test_policy() -> Policy {
        Policy {
            id: "test-policy".to_owned(),
            tenant: "RBX".to_owned(),
            product: "test-product".to_owned(),
            workflow: "test-workflow".to_owned(),
            permitted_backends: vec![BackendHandle {
                id: "test-backend".to_owned(),
                backend_type: BackendType::Model,
            }],
            budget: Budget {
                max_tokens: 1000,
                max_latency_ms: 5000,
            },
            context_grants: vec![ContextGrant {
                source: "docs".to_owned(),
                authorized: true,
            }],
            redaction_rules: vec![],
            audit_required: true,
            risk_threshold: thalamus_core::RiskLevel::Medium,
        }
    }

    fn test_response(content: &str, tokens: Option<u32>) -> BackendResponse {
        BackendResponse {
            content: content.to_owned(),
            tokens_used: tokens,
            latency_ms: Some(200),
        }
    }

    // === EvalStore unit tests ===

    #[test]
    fn store_insert_and_retrieve() {
        let store = EvalStore::new();
        let record = EvalRecord {
            eval_ref: "eval-test-1".to_owned(),
            schema_valid: true,
            citation_check: CitationCheckOutcome::NotRequired,
            hallucination_signals: vec![],
            risk_class: "Low".to_owned(),
            response_metadata: ResponseMetadata {
                content_len: 10,
                tokens_used: Some(50),
                latency_ms: Some(200),
            },
            trace_id: None,
            audit_id: None,
            policy_id: "test-policy".to_owned(),
            created_at: "2026-05-18T00:00:00Z".to_owned(),
        };

        store.insert(record.clone());
        assert_eq!(store.len(), 1);

        let retrieved = store.get("eval-test-1").unwrap();
        assert_eq!(retrieved.eval_ref, "eval-test-1");
        assert_eq!(retrieved.schema_valid, true);
        assert_eq!(retrieved.risk_class, "Low");
    }

    #[test]
    fn store_missing_ref_returns_none() {
        let store = EvalStore::new();
        assert!(store.get("nonexistent").is_none());
        assert!(store.is_empty());
    }

    #[test]
    fn store_all_refs() {
        let store = EvalStore::new();
        for i in 0..3 {
            let record = EvalRecord {
                eval_ref: format!("eval-{}", i),
                schema_valid: true,
                citation_check: CitationCheckOutcome::NotRequired,
                hallucination_signals: vec![],
                risk_class: "Low".to_owned(),
                response_metadata: ResponseMetadata {
                    content_len: 0,
                    tokens_used: None,
                    latency_ms: None,
                },
                trace_id: None,
                audit_id: None,
                policy_id: "p".to_owned(),
                created_at: "t".to_owned(),
            };
            store.insert(record);
        }
        let mut refs = store.all_refs();
        refs.sort();
        assert_eq!(refs, vec!["eval-0", "eval-1", "eval-2"]);
    }

    // === build_record unit tests ===

    #[test]
    fn build_record_derives_schema_valid_from_content() {
        let policy = test_policy();
        let resp = test_response("Hello", Some(100));
        let record = build_record("eval-1".to_owned(), &resp, &policy, None, None);
        assert!(record.schema_valid);

        let empty_resp = test_response("", Some(100));
        let record = build_record("eval-2".to_owned(), &empty_resp, &policy, None, None);
        assert!(!record.schema_valid);
    }

    #[test]
    fn build_record_classifies_risk_from_budget() {
        let policy = test_policy(); // max_tokens = 1000

        let low_resp = test_response("ok", Some(100));
        assert_eq!(
            build_record("r1".to_owned(), &low_resp, &policy, None, None).risk_class,
            "Low"
        );

        let med_resp = test_response("ok", Some(600)); // > 50%
        assert_eq!(
            build_record("r2".to_owned(), &med_resp, &policy, None, None).risk_class,
            "Medium"
        );

        let high_resp = test_response("ok", Some(800)); // > 75%
        assert_eq!(
            build_record("r3".to_owned(), &high_resp, &policy, None, None).risk_class,
            "High"
        );

        let prohib_resp = test_response("ok", Some(1200)); // > max
        assert_eq!(
            build_record("r4".to_owned(), &prohib_resp, &policy, None, None).risk_class,
            "Prohibited"
        );
    }

    #[test]
    fn build_record_no_hallucination_signals() {
        let policy = test_policy();
        let resp = test_response("content", Some(50));
        let record = build_record("eval-h".to_owned(), &resp, &policy, None, None);
        assert!(record.hallucination_signals.is_empty());
    }

    // === ChannelEvalPort integration ===

    #[test]
    fn channel_eval_port_stores_record() {
        let port = ChannelEvalPort::new(64);
        let policy = test_policy();
        let resp = test_response("Test output", Some(100));

        let eval_ref = port.submit(&resp, &policy);

        let count = port.store().wait_for_count(1, 2000);
        assert_eq!(count, 1);

        let record = port.store().get(&eval_ref).expect("record must exist");
        assert_eq!(record.policy_id, "test-policy");
        assert!(record.schema_valid);
        assert_eq!(record.response_metadata.tokens_used, Some(100));
        assert_eq!(record.risk_class, "Low");
    }

    #[test]
    fn channel_eval_port_returns_unique_refs() {
        let port = ChannelEvalPort::new(64);
        let policy = test_policy();
        let resp = test_response("content", Some(50));

        let ref1 = port.submit(&resp, &policy);
        let ref2 = port.submit(&resp, &policy);
        assert_ne!(ref1, ref2);

        let count = port.store().wait_for_count(2, 2000);
        assert_eq!(count, 2);
    }

    #[test]
    fn channel_eval_port_drops_on_full_channel() {
        // Capacity 1: second send should be dropped without panicking
        let port = ChannelEvalPort::new(1);
        let policy = test_policy();
        let resp = test_response("content", Some(50));

        let ref1 = port.submit(&resp, &policy);
        // Fill channel: second record may or may not fit depending on worker speed,
        // but submit must never panic or block.
        let ref2 = port.submit(&resp, &policy);
        // Both refs valid strings regardless of drop
        assert!(!ref1.is_empty());
        assert!(!ref2.is_empty());
    }

    #[test]
    fn eval_record_serializes_to_json() {
        let record = EvalRecord {
            eval_ref: "eval-serde".to_owned(),
            schema_valid: true,
            citation_check: CitationCheckOutcome::NotRequired,
            hallucination_signals: vec![],
            risk_class: "Low".to_owned(),
            response_metadata: ResponseMetadata {
                content_len: 5,
                tokens_used: Some(10),
                latency_ms: Some(100),
            },
            trace_id: Some("trace-123".to_owned()),
            audit_id: Some("audit-456".to_owned()),
            policy_id: "policy-1".to_owned(),
            created_at: "2026-05-18T12:00:00Z".to_owned(),
        };

        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("\"eval_ref\":\"eval-serde\""));
        assert!(json.contains("\"risk_class\":\"Low\""));

        let deserialized: EvalRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.eval_ref, record.eval_ref);
    }

    // === Non-blocking concurrency proof ===

    #[test]
    fn submit_does_not_block_caller_thread() {
        // Create a port with capacity 1 and a worker that processes slowly.
        // Submit N times rapidly; total wall time must be << N * worker_delay.
        let n = 50;
        let worker_delay_ms = 50;

        let (tx, rx) = bounded::<EvalMessage>(1);
        let store = EvalStore::new();
        let worker_store = store.clone();

        let handle = std::thread::Builder::new()
            .name("slow-eval-worker".to_owned())
            .spawn(move || {
                loop {
                    match rx.recv() {
                        Ok(EvalMessage::Record(record)) => {
                            std::thread::sleep(std::time::Duration::from_millis(worker_delay_ms));
                            worker_store.insert(record);
                        }
                        Ok(EvalMessage::Shutdown) | Err(_) => break,
                    }
                }
            })
            .expect("spawn worker");

        let policy = test_policy();
        let resp = test_response("concurrent", Some(100));

        let start = std::time::Instant::now();
        for _ in 0..n {
            let eval_ref = format!("eval-{}", Uuid::new_v4());
            let record = build_record(eval_ref, &resp, &policy, None, None);
            let _ = tx.try_send(EvalMessage::Record(record));
        }
        let elapsed = start.elapsed();

        let _ = tx.send(EvalMessage::Shutdown);
        let _ = handle.join();

        // If submission were blocking (serial), elapsed >= n * worker_delay_ms.
        // With non-blocking, elapsed should be < n * worker_delay_ms / 2.
        let serial_min_ms = n * worker_delay_ms;
        let elapsed_ms = elapsed.as_millis() as u64;
        assert!(
            elapsed_ms < serial_min_ms / 2,
            "submit took {}ms but serial would be >= {}ms — submissions are not decoupled",
            elapsed_ms,
            serial_min_ms,
        );
    }
}
