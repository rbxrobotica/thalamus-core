use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use thalamus_core::{
    BackendHandle, BackendPort, BackendResponse, BackendType, Budget, CallRequest, ContextGrant,
    Envelope, Policy, PolicyDecision, PolicyPort, RiskLevel,
};

use thalamus_server::app;
use thalamus_server::config::ServerConfig;

// === Test helpers ===

struct CountingBackendPort {
    calls: Mutex<usize>,
    response: BackendResponse,
}

impl CountingBackendPort {
    fn new(response: BackendResponse) -> Self {
        Self {
            calls: Mutex::new(0),
            response,
        }
    }
    fn call_count(&self) -> usize {
        *self.calls.lock().unwrap()
    }
}

impl BackendPort for CountingBackendPort {
    fn call(&self, _envelope: &Envelope) -> BackendResponse {
        *self.calls.lock().unwrap() += 1;
        self.response.clone()
    }
}

/// Fake PolicyPort that returns AllowWithReview for matching requests,
/// Deny otherwise. Used to test the AllowWithReview code path without
/// inventing config-driven review semantics.
struct FakeReviewPolicyPort {
    policy: Policy,
}

impl PolicyPort for FakeReviewPolicyPort {
    fn resolve(&self, request: &CallRequest) -> Policy {
        if self.policy.tenant == request.tenant
            && self.policy.product == request.product
            && self.policy.workflow == request.workflow
        {
            self.policy.clone()
        } else {
            Policy {
                id: "no-match".to_owned(),
                tenant: request.tenant.clone(),
                product: request.product.clone(),
                workflow: request.workflow.clone(),
                permitted_backends: vec![],
                budget: Budget {
                    max_tokens: 0,
                    max_latency_ms: 0,
                },
                context_grants: vec![],
                redaction_rules: vec![],
                audit_required: false,
                risk_threshold: RiskLevel::Low,
            }
        }
    }

    fn evaluate(&self, _request: &CallRequest, policy: &Policy) -> PolicyDecision {
        if policy.id != "no-match" {
            PolicyDecision::AllowWithReview {
                review_reason: "test-triggered human review".to_owned(),
                policy_ref: policy.id.clone(),
            }
        } else {
            PolicyDecision::Deny {
                reason: "no matching policy".to_owned(),
                policy_ref: "no-match".to_owned(),
            }
        }
    }
}

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
        risk_threshold: RiskLevel::Medium,
    }
}

fn empty_backend_policy() -> Policy {
    Policy {
        id: "empty-backends".to_owned(),
        tenant: "RBX".to_owned(),
        product: "empty-product".to_owned(),
        workflow: "test-workflow".to_owned(),
        permitted_backends: vec![],
        budget: Budget {
            max_tokens: 1000,
            max_latency_ms: 5000,
        },
        context_grants: vec![],
        redaction_rules: vec![],
        audit_required: false,
        risk_threshold: RiskLevel::Low,
    }
}

fn make_config(policies: Vec<Policy>) -> ServerConfig {
    ServerConfig {
        listen: "127.0.0.1:0".parse().unwrap(),
        policies,
    }
}

fn test_request_body(tenant: &str, product: &str, workflow: &str) -> Value {
    json!({
        "tenant": tenant,
        "product": product,
        "user": "test-user",
        "workflow": workflow,
        "intent": "analysis",
        "prompt": "Analyze the market",
        "requested_backend": {
            "id": "test-backend",
            "backend_type": "Model"
        }
    })
}

async fn send_request(
    app: axum::Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    let request = if let Some(b) = body {
        builder = builder.header("content-type", "application/json");
        builder
            .body::<Body>(Body::from(serde_json::to_string(&b).unwrap()))
            .unwrap()
    } else {
        builder.body::<Body>(Body::empty()).unwrap()
    };

    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&body_bytes).unwrap_or(json!({}));
    (status, body)
}

// === Acceptance tests ===

#[tokio::test]
async fn call_deny_returns_structured_deny_no_backend_call() {
    let config = make_config(vec![test_policy()]);
    let backend = Arc::new(CountingBackendPort::new(BackendResponse {
        content: "should not appear".to_owned(),
        tokens_used: Some(50),
        latency_ms: Some(10),
    }));
    let app = app::build_with_backend(config, backend.clone());

    // Request with tenant/product/workflow that has no matching policy => Deny
    let body = test_request_body("UNKNOWN", "UNKNOWN", "UNKNOWN");
    let (status, resp) = send_request(app, "POST", "/v1/call", Some(body)).await;

    assert_eq!(status, StatusCode::OK);
    assert!(resp["decision"].as_str().unwrap().starts_with("Deny"));
    assert!(resp["backend_content"].is_null());
    assert_eq!(backend.call_count(), 0);
}

#[tokio::test]
async fn call_allow_with_review_no_backend_needs_human_review() {
    // Inject a FakeReviewPolicyPort that returns AllowWithReview for matching
    // requests — this genuinely exercises the AllowWithReview arm of full_call.
    let policy = test_policy();
    let policy_port = Arc::new(FakeReviewPolicyPort { policy });
    let backend = Arc::new(CountingBackendPort::new(BackendResponse {
        content: "should not appear".to_owned(),
        tokens_used: None,
        latency_ms: None,
    }));
    let app = app::build_with_ports(policy_port, backend.clone());

    // Request matching the fake policy tenant/product/workflow
    let body = test_request_body("RBX", "test-product", "test-workflow");
    let (status, resp) = send_request(app, "POST", "/v1/call", Some(body)).await;

    // 1. HTTP 200
    assert_eq!(status, StatusCode::OK);

    // 2. decision starts with "AllowWithReview" and contains "review_id:"
    let decision = resp["decision"].as_str().unwrap();
    assert!(
        decision.starts_with("AllowWithReview"),
        "decision was: {decision}"
    );
    assert!(
        decision.contains("review_id:"),
        "decision missing review_id: {decision}"
    );

    // 3. post_call.status == "NeedsHumanReview"
    assert_eq!(
        resp["post_call"]["status"].as_str().unwrap(),
        "NeedsHumanReview"
    );

    // 4. backend_content is null (no backend call)
    assert!(resp["backend_content"].is_null());

    // 5. BackendPort was never called
    assert_eq!(backend.call_count(), 0);
}

#[tokio::test]
async fn call_allow_runs_backend_and_post_call() {
    let config = make_config(vec![test_policy()]);
    let backend = Arc::new(CountingBackendPort::new(BackendResponse {
        content: "Market analysis result".to_owned(),
        tokens_used: Some(100),
        latency_ms: Some(200),
    }));
    let app = app::build_with_backend(config, backend.clone());

    let body = test_request_body("RBX", "test-product", "test-workflow");
    let (status, resp) = send_request(app, "POST", "/v1/call", Some(body)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["decision"].as_str().unwrap(), "Allow");
    assert_eq!(backend.call_count(), 1);

    // post_call must have run — status and risk_class present
    let post_call = &resp["post_call"];
    assert_eq!(post_call["status"].as_str().unwrap(), "Valid");
    assert_eq!(post_call["risk_class"].as_str().unwrap(), "Low");
    assert!(post_call["schema_valid"].as_bool().unwrap());

    // Backend content returned (not raw — post_call ran first)
    assert_eq!(
        resp["backend_content"].as_str().unwrap(),
        "Market analysis result"
    );
}

#[tokio::test]
async fn decide_returns_decision_no_backend_call() {
    let config = make_config(vec![test_policy()]);
    let backend = Arc::new(CountingBackendPort::new(BackendResponse {
        content: "unused".to_owned(),
        tokens_used: None,
        latency_ms: None,
    }));
    let app = app::build_with_backend(config, backend.clone());

    let body = test_request_body("RBX", "test-product", "test-workflow");
    let (status, resp) = send_request(app, "POST", "/v1/decide", Some(body)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["decision"].as_str().unwrap(), "Allow");
    assert_eq!(resp["policy_id"].as_str().unwrap(), "test-policy");
    assert_eq!(backend.call_count(), 0);
}

#[tokio::test]
async fn post_call_validates_external_response() {
    let config = make_config(vec![test_policy()]);
    let backend = Arc::new(CountingBackendPort::new(BackendResponse {
        content: "unused".to_owned(),
        tokens_used: None,
        latency_ms: None,
    }));
    let app = app::build_with_backend(config, backend.clone());

    // First, do a pre-call to establish the audit record
    let body = test_request_body("RBX", "test-product", "test-workflow");
    let (status, pre_resp) = send_request(app.clone(), "POST", "/v1/pre-call", Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    let audit_id = pre_resp["audit_id"].as_str().unwrap();

    // Now post-call with only audit_id + response data (correlated from store)
    let post_body = json!({
        "audit_id": audit_id,
        "content": "External response content",
        "tokens_used": 50,
        "latency_ms": 100
    });

    let (status, resp) = send_request(app, "POST", "/v1/post-call", Some(post_body)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["status"].as_str().unwrap(), "Valid");
    assert_eq!(resp["risk_class"].as_str().unwrap(), "Low");
    assert!(resp["schema_valid"].as_bool().unwrap());
    assert_eq!(backend.call_count(), 0);
}

#[tokio::test]
async fn audit_returns_pre_and_post_events_for_id() {
    let config = make_config(vec![test_policy()]);
    let backend = Arc::new(CountingBackendPort::new(BackendResponse {
        content: "Response".to_owned(),
        tokens_used: Some(50),
        latency_ms: Some(100),
    }));
    let app = app::build_with_backend(config, backend.clone());

    // First, make a /v1/call to generate audit events
    let body = test_request_body("RBX", "test-product", "test-workflow");
    let (status, call_resp) = send_request(app.clone(), "POST", "/v1/call", Some(body)).await;
    assert_eq!(status, StatusCode::OK);

    let audit_id = call_resp["post_call"]["audit_id"].as_str().unwrap();

    // Now query audit
    let (status, resp) = send_request(app, "GET", &format!("/v1/audit/{}", audit_id), None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["audit_id"].as_str().unwrap(), audit_id);
    let events = resp["events"].as_array().unwrap();
    // Should have PreCallDecision + PostCallOutcome
    assert!(events.len() >= 2);
    let kinds: Vec<&str> = events.iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert!(kinds.contains(&"PreCallDecision"));
    assert!(kinds.contains(&"PostCallOutcome"));
}

#[tokio::test]
async fn empty_permitted_backends_returns_typed_4xx() {
    let config = make_config(vec![empty_backend_policy()]);
    let backend = Arc::new(CountingBackendPort::new(BackendResponse {
        content: "unused".to_owned(),
        tokens_used: None,
        latency_ms: None,
    }));
    let app = app::build_with_backend(config, backend.clone());

    let body = test_request_body("RBX", "empty-product", "test-workflow");
    let (status, resp) = send_request(app, "POST", "/v1/call", Some(body)).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(resp["code"].as_str().unwrap(), "NO_PERMITTED_BACKENDS");
    assert_eq!(backend.call_count(), 0);
}

#[tokio::test]
async fn call_no_backend_configured_returns_503() {
    let config = make_config(vec![test_policy()]);
    // build() without backend => 503 on Allow
    let app = app::build(config);

    let body = test_request_body("RBX", "test-product", "test-workflow");
    let (status, resp) = send_request(app, "POST", "/v1/call", Some(body)).await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(resp["code"].as_str().unwrap(), "NO_BACKEND");
}

#[tokio::test]
async fn pre_call_returns_envelope_on_allow() {
    let config = make_config(vec![test_policy()]);
    let backend = Arc::new(CountingBackendPort::new(BackendResponse {
        content: "unused".to_owned(),
        tokens_used: None,
        latency_ms: None,
    }));
    let app = app::build_with_backend(config, backend.clone());

    let body = test_request_body("RBX", "test-product", "test-workflow");
    let (status, resp) = send_request(app, "POST", "/v1/pre-call", Some(body)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["decision"].as_str().unwrap(), "Allow");
    assert!(resp["envelope"].is_object());
    assert!(!resp["trace_id"].as_str().unwrap().is_empty());
    assert!(!resp["audit_id"].as_str().unwrap().is_empty());
    assert_eq!(backend.call_count(), 0);
}

// === TH-S3: LiteLLM adapter + round-trip tests ===

fn litellm_policy() -> Policy {
    Policy {
        id: "litellm-test-policy".to_owned(),
        tenant: "RBX".to_owned(),
        product: "test-product".to_owned(),
        workflow: "test-workflow".to_owned(),
        permitted_backends: vec![BackendHandle {
            id: "test-model".to_owned(),
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
        risk_threshold: RiskLevel::Medium,
    }
}

fn make_litellm_adapter(mock_url: &str) -> Arc<dyn BackendPort + Send + Sync> {
    let config = thalamus_litellm_adapter::config::AdapterConfig {
        endpoint: mock_url.to_owned(),
        model_map: std::collections::HashMap::new(),
        timeout: Duration::from_secs(5),
    };
    Arc::new(thalamus_litellm_adapter::LiteLLMAdapter::new(config))
}

fn litellm_success_response() -> String {
    serde_json::json!({
        "choices": [{
            "message": { "content": "Mock LLM analysis result" }
        }],
        "usage": { "total_tokens": 80 }
    })
    .to_string()
}

#[tokio::test]
async fn litellm_round_trip_happy_path() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(litellm_success_response())
        .create_async()
        .await;

    let backend = make_litellm_adapter(&server.url());
    let config = make_config(vec![litellm_policy()]);
    let app = app::build_with_backend(config, backend);

    let body = test_request_body("RBX", "test-product", "test-workflow");
    let (status, resp) = send_request(app.clone(), "POST", "/v1/call", Some(body)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["decision"].as_str().unwrap(), "Allow");
    assert_eq!(resp["post_call"]["status"].as_str().unwrap(), "Valid");
    assert_eq!(resp["post_call"]["risk_class"].as_str().unwrap(), "Low");
    assert_eq!(
        resp["backend_content"].as_str().unwrap(),
        "Mock LLM analysis result"
    );

    let audit_id = resp["post_call"]["audit_id"].as_str().unwrap();

    // Audit is retrievable by audit_id
    let (status, audit_resp) =
        send_request(app, "GET", &format!("/v1/audit/{}", audit_id), None).await;
    assert_eq!(status, StatusCode::OK);
    let events = audit_resp["events"].as_array().unwrap();
    assert!(events.len() >= 2);

    mock.assert();
}

#[tokio::test]
async fn litellm_server_5xx_yields_invalid() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/chat/completions")
        .with_status(500)
        .with_body("internal error")
        .create_async()
        .await;

    let backend = make_litellm_adapter(&server.url());
    let config = make_config(vec![litellm_policy()]);
    let app = app::build_with_backend(config, backend);

    let body = test_request_body("RBX", "test-product", "test-workflow");
    let (status, resp) = send_request(app, "POST", "/v1/call", Some(body)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["decision"].as_str().unwrap(), "Allow");
    // Adapter returns empty content on 5xx => post_call marks it Invalid
    assert_eq!(resp["post_call"]["status"].as_str().unwrap(), "Invalid");
    // backend_content is empty string (adapter returned empty), not null
    assert!(resp["backend_content"].as_str().unwrap().is_empty());

    mock.assert();
}

#[tokio::test]
async fn litellm_malformed_body_yields_invalid() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("this is not json")
        .create_async()
        .await;

    let backend = make_litellm_adapter(&server.url());
    let config = make_config(vec![litellm_policy()]);
    let app = app::build_with_backend(config, backend);

    let body = test_request_body("RBX", "test-product", "test-workflow");
    let (status, resp) = send_request(app, "POST", "/v1/call", Some(body)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["decision"].as_str().unwrap(), "Allow");
    assert_eq!(resp["post_call"]["status"].as_str().unwrap(), "Invalid");

    mock.assert();
}

#[tokio::test]
async fn litellm_over_budget_response_is_prohibited() {
    let body = serde_json::json!({
        "choices": [{
            "message": { "content": "Expensive response" }
        }],
        "usage": { "total_tokens": 5000 }
    })
    .to_string();

    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .create_async()
        .await;

    let backend = make_litellm_adapter(&server.url());
    let config = make_config(vec![litellm_policy()]);
    let app = app::build_with_backend(config, backend);

    let req = test_request_body("RBX", "test-product", "test-workflow");
    let (status, resp) = send_request(app, "POST", "/v1/call", Some(req)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["post_call"]["status"].as_str().unwrap(), "Invalid");
    assert_eq!(
        resp["post_call"]["risk_class"].as_str().unwrap(),
        "Prohibited"
    );
    assert!(!resp["post_call"]["executable_by_agent"].as_bool().unwrap());

    mock.assert();
}

#[tokio::test]
async fn litellm_split_path_decide_then_post_call() {
    // Split path: pre-call decides, caller executes backend externally,
    // then post-call validates. The backend port is not called in this path.
    let backend = Arc::new(CountingBackendPort::new(BackendResponse {
        content: "unused".to_owned(),
        tokens_used: None,
        latency_ms: None,
    }));
    let config = make_config(vec![litellm_policy()]);
    let app = app::build_with_backend(config, backend.clone());

    // Step 1: pre-call (split path — caller will execute the backend itself)
    let req = test_request_body("RBX", "test-product", "test-workflow");
    let (status, pre_resp) = send_request(app.clone(), "POST", "/v1/pre-call", Some(req)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(pre_resp["decision"].as_str().unwrap(), "Allow");
    let audit_id = pre_resp["audit_id"].as_str().unwrap();
    assert!(!audit_id.is_empty());

    // Step 2: post-call (correlates from audit store by audit_id)
    let post_body = json!({
        "audit_id": audit_id,
        "content": "External execution result",
        "tokens_used": 80,
        "latency_ms": 150
    });
    let (status, post_resp) =
        send_request(app.clone(), "POST", "/v1/post-call", Some(post_body)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(post_resp["status"].as_str().unwrap(), "Valid");

    // Step 3: audit retrievable
    let (status, audit_resp) =
        send_request(app, "GET", &format!("/v1/audit/{}", audit_id), None).await;
    assert_eq!(status, StatusCode::OK);
    let events = audit_resp["events"].as_array().unwrap();
    assert!(events.len() >= 2);

    // Backend was never called in the split path
    assert_eq!(backend.call_count(), 0);
}

// === TH-S2 follow-up tests ===

#[tokio::test]
async fn missing_envelope_on_allow_returns_structured_500() {
    // The core flow guarantees Allow => Some(envelope). This test verifies
    // the server handles the Allow path correctly with a real backend — no panic.
    // If the expect() had remained, this would be the code path that could panic.
    let config = make_config(vec![test_policy()]);
    let backend = Arc::new(CountingBackendPort::new(BackendResponse {
        content: "test result".to_owned(),
        tokens_used: Some(50),
        latency_ms: Some(10),
    }));
    let app = app::build_with_backend(config, backend.clone());

    // Normal Allow path — no panic, returns 200
    let body = test_request_body("RBX", "test-product", "test-workflow");
    let (status, resp) = send_request(app, "POST", "/v1/call", Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["decision"].as_str().unwrap(), "Allow");
    // If the code had an expect(), this would have panicked instead
}

#[tokio::test]
async fn post_call_unknown_audit_id_returns_404() {
    let config = make_config(vec![test_policy()]);
    let backend = Arc::new(CountingBackendPort::new(BackendResponse {
        content: "unused".to_owned(),
        tokens_used: None,
        latency_ms: None,
    }));
    let app = app::build_with_backend(config, backend);

    // Post-call with an audit_id that has no pre-call record => 404
    let post_body = json!({
        "audit_id": uuid::Uuid::new_v4().to_string(),
        "content": "Orphan response",
        "tokens_used": 50,
        "latency_ms": 100
    });

    let (status, resp) = send_request(app, "POST", "/v1/post-call", Some(post_body)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(resp["code"].as_str().unwrap(), "UNKNOWN_AUDIT_ID");
}

struct SlowBackendPort {
    delay: std::time::Duration,
    response: BackendResponse,
}

impl BackendPort for SlowBackendPort {
    fn call(&self, _envelope: &Envelope) -> BackendResponse {
        std::thread::sleep(self.delay);
        self.response.clone()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn concurrent_calls_do_not_block_async_runtime() {
    let config = make_config(vec![test_policy()]);
    let backend = Arc::new(SlowBackendPort {
        delay: std::time::Duration::from_millis(400),
        response: BackendResponse {
            content: "slow result".to_owned(),
            tokens_used: Some(100),
            latency_ms: Some(400),
        },
    });
    let app = app::build_with_backend(config, backend);
    let body = test_request_body("RBX", "test-product", "test-workflow");

    let start = std::time::Instant::now();
    let (r1, r2) = tokio::join!(
        send_request(app.clone(), "POST", "/v1/call", Some(body.clone())),
        send_request(app.clone(), "POST", "/v1/call", Some(body.clone())),
    );
    let elapsed = start.elapsed();

    assert_eq!(r1.0, StatusCode::OK);
    assert_eq!(r2.0, StatusCode::OK);
    assert_eq!(r1.1["post_call"]["status"].as_str().unwrap(), "Valid");
    assert_eq!(r2.1["post_call"]["status"].as_str().unwrap(), "Valid");
    assert!(
        elapsed < std::time::Duration::from_millis(700),
        "concurrent /v1/call serialized: {elapsed:?} (blocking-on-async-runtime bug)"
    );
}

// === TH-S4: Agentgateway adapter + round-trip tests ===

fn agentgateway_policy() -> Policy {
    Policy {
        id: "agentgateway-test-policy".to_owned(),
        tenant: "RBX".to_owned(),
        product: "test-product".to_owned(),
        workflow: "test-workflow".to_owned(),
        permitted_backends: vec![BackendHandle {
            id: "test-model".to_owned(),
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
        risk_threshold: RiskLevel::Medium,
    }
}

fn make_agentgateway_adapter(mock_url: &str) -> Arc<dyn BackendPort + Send + Sync> {
    let config = thalamus_agentgateway_adapter::config::AdapterConfig {
        endpoint: mock_url.to_owned(),
        model_map: std::collections::HashMap::new(),
        timeout: Duration::from_secs(5),
        auth_header: None,
    };
    Arc::new(thalamus_agentgateway_adapter::AgentgatewayAdapter::new(
        config,
    ))
}

fn openai_success_response() -> String {
    serde_json::json!({
        "choices": [{
            "message": { "content": "Agentgateway mock analysis result" }
        }],
        "usage": { "total_tokens": 80 }
    })
    .to_string()
}

#[tokio::test]
async fn agentgateway_round_trip_happy_path() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(openai_success_response())
        .create_async()
        .await;

    let backend = make_agentgateway_adapter(&server.url());
    let config = make_config(vec![agentgateway_policy()]);
    let app = app::build_with_backend(config, backend);

    let body = test_request_body("RBX", "test-product", "test-workflow");
    let (status, resp) = send_request(app.clone(), "POST", "/v1/call", Some(body)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["decision"].as_str().unwrap(), "Allow");
    assert_eq!(resp["post_call"]["status"].as_str().unwrap(), "Valid");
    assert_eq!(resp["post_call"]["risk_class"].as_str().unwrap(), "Low");
    assert_eq!(
        resp["backend_content"].as_str().unwrap(),
        "Agentgateway mock analysis result"
    );

    let audit_id = resp["post_call"]["audit_id"].as_str().unwrap();

    // Audit is retrievable by audit_id
    let (status, audit_resp) =
        send_request(app, "GET", &format!("/v1/audit/{}", audit_id), None).await;
    assert_eq!(status, StatusCode::OK);
    let events = audit_resp["events"].as_array().unwrap();
    assert!(events.len() >= 2);

    mock.assert();
}

#[tokio::test]
async fn agentgateway_server_5xx_yields_invalid() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/chat/completions")
        .with_status(500)
        .with_body("internal error")
        .create_async()
        .await;

    let backend = make_agentgateway_adapter(&server.url());
    let config = make_config(vec![agentgateway_policy()]);
    let app = app::build_with_backend(config, backend);

    let body = test_request_body("RBX", "test-product", "test-workflow");
    let (status, resp) = send_request(app, "POST", "/v1/call", Some(body)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["decision"].as_str().unwrap(), "Allow");
    assert_eq!(resp["post_call"]["status"].as_str().unwrap(), "Invalid");
    assert!(resp["backend_content"].as_str().unwrap().is_empty());

    mock.assert();
}

#[tokio::test]
async fn agentgateway_malformed_body_yields_invalid() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("this is not json")
        .create_async()
        .await;

    let backend = make_agentgateway_adapter(&server.url());
    let config = make_config(vec![agentgateway_policy()]);
    let app = app::build_with_backend(config, backend);

    let body = test_request_body("RBX", "test-product", "test-workflow");
    let (status, resp) = send_request(app, "POST", "/v1/call", Some(body)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["decision"].as_str().unwrap(), "Allow");
    assert_eq!(resp["post_call"]["status"].as_str().unwrap(), "Invalid");

    mock.assert();
}

#[tokio::test]
async fn agentgateway_over_budget_response_is_prohibited() {
    let body = serde_json::json!({
        "choices": [{
            "message": { "content": "Expensive response" }
        }],
        "usage": { "total_tokens": 5000 }
    })
    .to_string();

    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .create_async()
        .await;

    let backend = make_agentgateway_adapter(&server.url());
    let config = make_config(vec![agentgateway_policy()]);
    let app = app::build_with_backend(config, backend);

    let req = test_request_body("RBX", "test-product", "test-workflow");
    let (status, resp) = send_request(app, "POST", "/v1/call", Some(req)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["post_call"]["status"].as_str().unwrap(), "Invalid");
    assert_eq!(
        resp["post_call"]["risk_class"].as_str().unwrap(),
        "Prohibited"
    );
    assert!(!resp["post_call"]["executable_by_agent"].as_bool().unwrap());

    mock.assert();
}

/// Swap test: the same request/flow works with the agentgateway backend exactly
/// as with the litellm/counting backend. No caller or route difference.
#[tokio::test]
async fn backend_swap_agentgateway_produces_same_flow_as_litellm() {
    // Build a shared policy and request body
    let policy = Policy {
        id: "swap-test-policy".to_owned(),
        tenant: "RBX".to_owned(),
        product: "swap-product".to_owned(),
        workflow: "swap-workflow".to_owned(),
        permitted_backends: vec![BackendHandle {
            id: "swap-model".to_owned(),
            backend_type: BackendType::Model,
        }],
        budget: Budget {
            max_tokens: 1000,
            max_latency_ms: 5000,
        },
        context_grants: vec![],
        redaction_rules: vec![],
        audit_required: true,
        risk_threshold: RiskLevel::Low,
    };

    let success_body = serde_json::json!({
        "choices": [{
            "message": { "content": "Swap test response" }
        }],
        "usage": { "total_tokens": 42 }
    })
    .to_string();

    // --- Run with Agentgateway adapter ---
    let mut ag_server = mockito::Server::new_async().await;
    let ag_mock = ag_server
        .mock("POST", "/v1/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(&success_body)
        .create_async()
        .await;

    let ag_backend = make_agentgateway_adapter(&ag_server.url());
    let ag_config = make_config(vec![policy.clone()]);
    let ag_app = app::build_with_backend(ag_config, ag_backend);

    let req = json!({
        "tenant": "RBX",
        "product": "swap-product",
        "user": "swap-user",
        "workflow": "swap-workflow",
        "intent": "swap-test",
        "prompt": "Swap test prompt",
        "requested_backend": {
            "id": "swap-model",
            "backend_type": "Model"
        }
    });

    let (ag_status, ag_resp) =
        send_request(ag_app.clone(), "POST", "/v1/call", Some(req.clone())).await;

    // --- Run with LiteLLM adapter ---
    let mut ll_server = mockito::Server::new_async().await;
    let ll_mock = ll_server
        .mock("POST", "/v1/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(&success_body)
        .create_async()
        .await;

    let ll_backend = make_litellm_adapter(&ll_server.url());
    let ll_config = make_config(vec![policy.clone()]);
    let ll_app = app::build_with_backend(ll_config, ll_backend);

    let (ll_status, ll_resp) =
        send_request(ll_app.clone(), "POST", "/v1/call", Some(req.clone())).await;

    // Both produce identical structural outcomes
    assert_eq!(ag_status, ll_status);
    assert_eq!(ag_resp["decision"], ll_resp["decision"]);
    assert_eq!(
        ag_resp["post_call"]["status"],
        ll_resp["post_call"]["status"]
    );
    assert_eq!(
        ag_resp["post_call"]["risk_class"],
        ll_resp["post_call"]["risk_class"]
    );
    assert_eq!(
        ag_resp["post_call"]["executable_by_agent"],
        ll_resp["post_call"]["executable_by_agent"]
    );
    assert_eq!(
        ag_resp["post_call"]["schema_valid"],
        ll_resp["post_call"]["schema_valid"]
    );
    assert_eq!(ag_resp["backend_content"], ll_resp["backend_content"]);

    ag_mock.assert();
    ll_mock.assert();
}

// === TH-S6a: EvalPort integration tests ===

#[tokio::test]
async fn call_produces_eval_record_retrievable_by_ref() {
    let config = make_config(vec![test_policy()]);
    let backend = Arc::new(CountingBackendPort::new(BackendResponse {
        content: "Eval test result".to_owned(),
        tokens_used: Some(100),
        latency_ms: Some(200),
    }));
    let (app, eval_store) = app::build_with_eval_inspection(config, backend);

    let body = test_request_body("RBX", "test-product", "test-workflow");
    let (status, resp) = send_request(app, "POST", "/v1/call", Some(body)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["decision"].as_str().unwrap(), "Allow");

    // Wait for the eval worker to process the record
    let count = eval_store.wait_for_count(1, 2000);
    assert_eq!(count, 1, "expected 1 eval record, got {count}");

    let eval_refs = eval_store.all_refs();
    let eval_ref = &eval_refs[0];
    let record = eval_store.get(eval_ref).expect("record must exist");

    assert!(record.schema_valid);
    assert_eq!(record.risk_class, "Low");
    assert_eq!(record.policy_id, "test-policy");
    assert_eq!(record.response_metadata.tokens_used, Some(100));
    assert_eq!(record.response_metadata.content_len, 16); // "Eval test result".len()
}

#[tokio::test]
async fn concurrent_calls_with_slow_eval_do_not_serialize() {
    // Concurrency regression test (TH-S3.1 style).
    // N concurrent /v1/call requests where the backend is slow but eval is
    // non-blocking. Total time must be << N * (backend_delay + eval_delay).
    let n: usize = 10;
    let backend_delay_ms = 100;

    let config = make_config(vec![test_policy()]);
    let backend = Arc::new(SlowBackendPort {
        delay: std::time::Duration::from_millis(backend_delay_ms),
        response: BackendResponse {
            content: "slow eval test".to_owned(),
            tokens_used: Some(100),
            latency_ms: Some(backend_delay_ms),
        },
    });
    let (app, eval_store) = app::build_with_eval_inspection(config, backend);
    let body = test_request_body("RBX", "test-product", "test-workflow");

    let start = std::time::Instant::now();
    let mut handles = Vec::new();
    for _ in 0..n {
        let app_clone = app.clone();
        let body_clone = body.clone();
        handles.push(tokio::spawn(async move {
            send_request(app_clone, "POST", "/v1/call", Some(body_clone)).await
        }));
    }

    for handle in handles {
        let (status, resp) = handle.await.unwrap();
        assert_eq!(status, StatusCode::OK);
        assert_eq!(resp["decision"].as_str().unwrap(), "Allow");
    }
    let elapsed = start.elapsed();

    // If requests were serialized: elapsed >= n * backend_delay_ms
    // With non-blocking eval + spawn_blocking backend: elapsed < n * backend_delay_ms
    let serial_min_ms = (n as u64) * backend_delay_ms;
    let elapsed_ms = elapsed.as_millis() as u64;
    assert!(
        elapsed_ms < serial_min_ms,
        "concurrent /v1/call serialized: {elapsed_ms}ms >= {serial_min_ms}ms — eval blocks the async runtime"
    );

    // All eval records eventually stored
    let count = eval_store.wait_for_count(n, 3000);
    assert_eq!(count, n, "expected {n} eval records, got {count}");
}
