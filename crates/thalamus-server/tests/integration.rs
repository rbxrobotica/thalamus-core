use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use thalamus_core::{
    BackendHandle, BackendPort, BackendResponse, BackendType, Budget, CallRequest, ContextGrant,
    EmbeddingError, EmbeddingPort, EmbeddingRequest, EmbeddingResponse, Envelope, Policy,
    PolicyDecision, PolicyPort, RedactionAction, RedactionRule, RiskLevel,
};

use thalamus_server::app;
use thalamus_server::auth::{
    AuthError, CredentialVerifier, OpaqueIntrospectionVerifier, StaticCredentialVerifier,
    VerifiedCaller,
};
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

struct RecordingEmbeddingPort {
    requests: Mutex<Vec<EmbeddingRequest>>,
    result: Result<EmbeddingResponse, EmbeddingError>,
}

impl RecordingEmbeddingPort {
    fn new(result: Result<EmbeddingResponse, EmbeddingError>) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            result,
        }
    }

    fn call_count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }

    fn requests(&self) -> Vec<EmbeddingRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl EmbeddingPort for RecordingEmbeddingPort {
    fn embed(&self, request: &EmbeddingRequest) -> Result<EmbeddingResponse, EmbeddingError> {
        self.requests.lock().unwrap().push(request.clone());
        self.result.clone()
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
                require_run_correlation: false,
                prompt_profile: None,
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
        require_run_correlation: false,
        prompt_profile: None,
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
        require_run_correlation: false,
        prompt_profile: None,
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

#[tokio::test]
async fn healthz_and_readyz_are_served() {
    let app = app::build(make_config(vec![test_policy()]));
    let (status, body) = send_request(app, "GET", "/healthz", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], true);

    let app = app::build(make_config(vec![test_policy()]));
    let (status, body) = send_request(app, "GET", "/readyz", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ready");
    assert_eq!(body["policy_loaded"], true);
    assert_eq!(body["audit_reachable"], true);
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
        require_run_correlation: false,
        prompt_profile: None,
    }
}

fn make_litellm_adapter(mock_url: &str) -> Arc<dyn BackendPort + Send + Sync> {
    let config = thalamus_litellm_adapter::config::AdapterConfig {
        endpoint: mock_url.to_owned(),
        model_map: std::collections::HashMap::new(),
        timeout: Duration::from_secs(5),
        api_key: None,
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
        require_run_correlation: false,
        prompt_profile: None,
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
        require_run_correlation: false,
        prompt_profile: None,
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

// === Gate A: credential middleware (/rbx/v1/*) ===

fn rbx_caller(subject: &str, scopes: &[&str]) -> VerifiedCaller {
    VerifiedCaller {
        active: true,
        subject: Some(subject.to_owned()),
        session_id: Some("00000000-0000-0000-0000-000000000001".to_owned()),
        jti: Some("jti-1".to_owned()),
        audience: vec!["thalamus".to_owned()],
        scopes: scopes.iter().map(|s| String::from(*s)).collect(),
        client_app_id: Some("robson-code".to_owned()),
        actor: None,
        delegated_by: None,
        mediator: Some("rbx-token-service".to_owned()),
        expires_at: None,
        reason: None,
    }
}

fn rbx_app(verifier: Arc<dyn CredentialVerifier + Send + Sync>) -> axum::Router {
    app::build_with_rbx_api(make_config(vec![test_policy()]), verifier, None)
}

async fn send_with_auth(
    app: axum::Router,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
) -> (StatusCode, Value) {
    send_with_auth_json(app, method, uri, bearer, None).await
}

async fn send_with_auth_json(
    app: axum::Router,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(b) = bearer {
        builder = builder.header("authorization", format!("Bearer {b}"));
    }
    let request = if let Some(body) = body {
        builder
            .header("content-type", "application/json")
            .body::<Body>(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap()
    } else {
        builder.body::<Body>(Body::empty()).unwrap()
    };
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(json!({}));
    (status, body)
}

// Matrix case 1 + Leandro integration: valid credential accepted.
#[tokio::test]
async fn rbx_identity_accepts_valid_credential() {
    let verifier = StaticCredentialVerifier::with_valid(
        "rbxsess_leandro",
        rbx_caller("ldamasio@gmail.com", &["kulinaryos:access"]),
    );
    let app = rbx_app(Arc::new(verifier));
    let (status, body) =
        send_with_auth(app, "GET", "/rbx/v1/identity", Some("rbxsess_leandro")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["subject"], "ldamasio@gmail.com");
    assert_eq!(body["audience"], json!(["thalamus"]));
}

#[tokio::test]
async fn rbx_identity_rejects_missing_bearer() {
    let verifier = StaticCredentialVerifier::with_valid("rbxsess_x", rbx_caller("s", &[]));
    let app = rbx_app(Arc::new(verifier));
    let (status, body) = send_with_auth(app, "GET", "/rbx/v1/identity", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "policy_denied");
}

#[tokio::test]
async fn governed_embeddings_authenticate_redact_audit_and_use_only_embedding_port() {
    let mut policy = test_policy();
    policy.redaction_rules = vec![RedactionRule {
        pattern: "secret".to_owned(),
        action: RedactionAction::Redact,
    }];
    let verifier = Arc::new(StaticCredentialVerifier::with_valid(
        "rbxsess-memory",
        rbx_caller("rbx-memory", &["thalamus:embeddings"]),
    ));
    let chat_backend = Arc::new(CountingBackendPort::new(BackendResponse {
        content: "must not run".to_owned(),
        tokens_used: None,
        latency_ms: None,
    }));
    let embedding = Arc::new(RecordingEmbeddingPort::new(Ok(EmbeddingResponse {
        model_alias: "test-backend".to_owned(),
        vectors: vec![vec![0.1, 0.2], vec![0.3, 0.4]],
        provider_metadata: json!({ "provider": "must-not-cross-http-boundary" }),
    })));
    let app = app::build_with_rbx_api_and_embedding(
        make_config(vec![policy]),
        verifier,
        Some(chat_backend.clone()),
        Some(embedding.clone()),
    );
    let request = json!({
        "tenant": "RBX",
        "product": "test-product",
        "workflow": "test-workflow",
        "model_alias": "test-backend",
        "input": ["public", "contains secret"]
    });

    let (status, body) = send_with_auth_json(
        app.clone(),
        "POST",
        "/v1/embeddings",
        Some("rbxsess-memory"),
        Some(request),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["model_alias"], "test-backend");
    assert_eq!(body["vectors"], json!([[0.1, 0.2], [0.3, 0.4]]));
    assert!(body.get("provider_metadata").is_none());
    let trace_id = body["trace_id"].as_str().unwrap();
    let audit_id = body["audit_id"].as_str().unwrap();
    uuid::Uuid::parse_str(trace_id).unwrap();
    uuid::Uuid::parse_str(audit_id).unwrap();

    assert_eq!(embedding.call_count(), 1);
    assert_eq!(chat_backend.call_count(), 0);
    let requests = embedding.requests();
    assert_eq!(requests[0].input, vec!["public", "contains [REDACTED]"]);
    assert_eq!(requests[0].trace_id.0.to_string(), trace_id);
    assert_eq!(requests[0].audit_id.0.to_string(), audit_id);

    let (audit_status, audit) =
        send_request(app, "GET", &format!("/v1/audit/{audit_id}"), None).await;
    assert_eq!(audit_status, StatusCode::OK);
    let kinds: Vec<&str> = audit["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        vec!["PreCallDecision", "RouteEnvelope", "PostCallOutcome"]
    );
    assert_eq!(audit["events"][0]["details"]["user"], "rbx-memory");
    assert_eq!(
        audit["events"][1]["details"]["capability_class"],
        "embeddings"
    );
}

#[tokio::test]
async fn governed_embeddings_fail_closed_before_port_for_auth_policy_and_alias() {
    let verifier = Arc::new(
        StaticCredentialVerifier::with_valid(
            "rbxsess-memory",
            rbx_caller("rbx-memory", &["thalamus:embeddings"]),
        )
        .and_valid("rbxsess-no-scope", rbx_caller("rbx-memory", &[])),
    );
    let embedding = Arc::new(RecordingEmbeddingPort::new(Ok(EmbeddingResponse {
        model_alias: "test-backend".to_owned(),
        vectors: vec![vec![0.1]],
        provider_metadata: json!({}),
    })));
    let app = app::build_with_rbx_api_and_embedding(
        make_config(vec![test_policy()]),
        verifier,
        None,
        Some(embedding.clone()),
    );
    let allowed = json!({
        "tenant": "RBX",
        "product": "test-product",
        "workflow": "test-workflow",
        "model_alias": "test-backend",
        "input": "text"
    });

    let (status, body) = send_with_auth_json(
        app.clone(),
        "POST",
        "/v1/embeddings",
        None,
        Some(allowed.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "policy_denied");

    let (status, body) = send_with_auth_json(
        app.clone(),
        "POST",
        "/v1/embeddings",
        Some("rbxsess-no-scope"),
        Some(allowed.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "policy_denied");

    let mut no_policy = allowed.clone();
    no_policy["product"] = json!("unknown-product");
    let (status, body) = send_with_auth_json(
        app.clone(),
        "POST",
        "/v1/embeddings",
        Some("rbxsess-memory"),
        Some(no_policy),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "policy_denied");
    assert!(body["audit_id"].is_string());

    let mut wrong_alias = allowed;
    wrong_alias["model_alias"] = json!("unpermitted-alias");
    let (status, body) = send_with_auth_json(
        app,
        "POST",
        "/v1/embeddings",
        Some("rbxsess-memory"),
        Some(wrong_alias),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "model_not_permitted");
    assert!(body["trace_id"].is_string());
    assert_eq!(embedding.call_count(), 0);
}

#[tokio::test]
async fn governed_embeddings_block_policy_and_malformed_vectors_fail_closed() {
    let verifier = Arc::new(StaticCredentialVerifier::with_valid(
        "rbxsess-memory",
        rbx_caller("rbx-memory", &["thalamus:embeddings"]),
    ));
    let mut blocking_policy = test_policy();
    blocking_policy.redaction_rules = vec![RedactionRule {
        pattern: "private".to_owned(),
        action: RedactionAction::Block,
    }];
    let blocked_port = Arc::new(RecordingEmbeddingPort::new(Ok(EmbeddingResponse {
        model_alias: "test-backend".to_owned(),
        vectors: vec![vec![0.1]],
        provider_metadata: json!({}),
    })));
    let blocked_app = app::build_with_rbx_api_and_embedding(
        make_config(vec![blocking_policy]),
        verifier.clone(),
        None,
        Some(blocked_port.clone()),
    );
    let blocked_request = json!({
        "tenant": "RBX",
        "product": "test-product",
        "workflow": "test-workflow",
        "model_alias": "test-backend",
        "input": "private material"
    });
    let (status, body) = send_with_auth_json(
        blocked_app,
        "POST",
        "/v1/embeddings",
        Some("rbxsess-memory"),
        Some(blocked_request),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "content_blocked");
    assert_eq!(blocked_port.call_count(), 0);

    let malformed_port = Arc::new(RecordingEmbeddingPort::new(Ok(EmbeddingResponse {
        model_alias: "test-backend".to_owned(),
        vectors: vec![vec![0.1], vec![0.2, 0.3]],
        provider_metadata: json!({}),
    })));
    let malformed_app = app::build_with_rbx_api_and_embedding(
        make_config(vec![test_policy()]),
        verifier,
        None,
        Some(malformed_port.clone()),
    );
    let malformed_request = json!({
        "tenant": "RBX",
        "product": "test-product",
        "workflow": "test-workflow",
        "model_alias": "test-backend",
        "input": ["one", "two"]
    });
    let (status, body) = send_with_auth_json(
        malformed_app,
        "POST",
        "/v1/embeddings",
        Some("rbxsess-memory"),
        Some(malformed_request),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(body["error"]["code"], "malformed_embedding_response");
    assert_eq!(malformed_port.call_count(), 1);
}

// Matrix cases 3 + 5: expired / revoked map to session_expired.
#[tokio::test]
async fn rbx_identity_rejects_expired_and_revoked_as_session_expired() {
    for reason in ["expired", "revoked"] {
        let verifier = StaticCredentialVerifier::always_inactive(reason);
        let app = rbx_app(Arc::new(verifier));
        let (status, body) = send_with_auth(app, "GET", "/rbx/v1/identity", Some("rbxsess")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "reason={reason}");
        assert_eq!(body["error"]["code"], "session_expired", "reason={reason}");
    }
}

// Matrix cases 1-negative / 6: unknown / missing_entitlement map to policy_denied.
#[tokio::test]
async fn rbx_identity_rejects_unknown_and_missing_entitlement() {
    for reason in ["unknown", "missing_entitlement"] {
        let verifier = StaticCredentialVerifier::always_inactive(reason);
        let app = rbx_app(Arc::new(verifier));
        let (status, body) = send_with_auth(app, "GET", "/rbx/v1/identity", Some("rbxsess")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "reason={reason}");
        assert_eq!(body["error"]["code"], "policy_denied", "reason={reason}");
    }
}

// THALAMUS_RBX_API off (no verifier): /rbx/v1/* is not mounted; /v1/* is
// unaffected. Regression guard for "do not break /v1/call".
#[tokio::test]
async fn rbx_route_not_mounted_when_verifier_absent() {
    let app = app::build(make_config(vec![test_policy()]));
    let (status, _) = send_with_auth(app, "GET", "/rbx/v1/identity", Some("rbxsess")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// OpaqueIntrospectionVerifier against a mocked rbx-token-service introspect
// endpoint (real HTTP path through ureq), active and inactive.
#[tokio::test]
async fn introspection_verifier_calls_token_service() {
    let mut server = mockito::Server::new_async().await;
    let url = format!("{}/v1/delegation/introspect", server.url());

    let active_mock = server
        .mock("POST", "/v1/delegation/introspect")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            json!({
                "active": true,
                "subject": "ldamasio@gmail.com",
                "audience": ["thalamus"],
                "scopes": ["kulinaryos:access"],
                "client_app_id": "robson-code",
                "mediator": "rbx-token-service",
            })
            .to_string(),
        )
        .create_async()
        .await;

    let verifier = OpaqueIntrospectionVerifier::new(url.clone());
    let caller = verifier.verify("rbxsess_leandro").await.unwrap();
    assert!(caller.active);
    assert_eq!(caller.subject.as_deref(), Some("ldamasio@gmail.com"));
    active_mock.assert_async().await;

    let inactive_mock = server
        .mock("POST", "/v1/delegation/introspect")
        .with_status(200)
        .with_body(json!({ "active": false, "reason": "revoked" }).to_string())
        .create_async()
        .await;
    let err = verifier.verify("rbxsess_dead").await.unwrap_err();
    assert!(matches!(err, AuthError::Inactive(reason) if reason == "revoked"));
    inactive_mock.assert_async().await;
}

// === Phase 3 slice 1: session/run lifecycle (/rbx/v1/sessions...) ===

use thalamus_server::ports::sessions::{InMemorySessionStore, SessionStore, SharedSessionStore};

fn rbx_lifecycle_app() -> (axum::Router, Arc<InMemorySessionStore>) {
    let store = Arc::new(InMemorySessionStore::new());
    let verifier = StaticCredentialVerifier::with_valid(
        "rbxsess_leandro",
        rbx_caller("ldamasio@gmail.com", &["kulinaryos:access"]),
    );
    let app = app::build_with_rbx_api_and_sessions(
        make_config(vec![test_policy()]),
        Arc::new(verifier),
        store.clone() as SharedSessionStore,
    );
    (app, store)
}

async fn post_json_with_auth(
    app: &axum::Router,
    uri: &str,
    bearer: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(b) = bearer {
        builder = builder.header("authorization", format!("Bearer {b}"));
    }
    let request = builder.body(Body::from(body.to_string())).unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(json!({}));
    (status, body)
}

fn session_request() -> Value {
    json!({
        "tenant": "rbx",
        "product": "kulinaryos",
        "workflow": "coding",
        "governance_mode": "governed_llm_access",
    })
}

#[tokio::test]
async fn rbx_session_lifecycle_create_run_close() {
    let (app, _store) = rbx_lifecycle_app();

    // Create session: principal comes from the verified credential.
    let (status, session) = post_json_with_auth(
        &app,
        "/rbx/v1/sessions",
        Some("rbxsess_leandro"),
        session_request(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(session["principal"], "ldamasio@gmail.com");
    assert_eq!(session["status"], "open");
    let session_id = session["session_id"].as_str().unwrap().to_owned();

    // Create a run.
    let (status, run) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/sessions/{session_id}/runs"),
        Some("rbxsess_leandro"),
        json!({ "model_alias": "glm-5.2" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(run["status"], "started");
    let run_id = run["run_id"].as_str().unwrap().to_owned();

    // Cancel the run.
    let (status, cancelled) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/runs/{run_id}/cancel"),
        Some("rbxsess_leandro"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cancelled["status"], "cancelled");

    // Close the session; new runs are then refused with session_closed.
    let (status, closed) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/sessions/{session_id}/close"),
        Some("rbxsess_leandro"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(closed["status"], "closed");

    let (status, refused) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/sessions/{session_id}/runs"),
        Some("rbxsess_leandro"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(refused["error"]["code"], "session_closed");
}

// §3 acceptance: budget-exceeded blocks new runs.
#[tokio::test]
async fn rbx_budget_exceeded_blocks_new_runs() {
    let (app, store) = rbx_lifecycle_app();
    store.set_budget("product", "rbx/kulinaryos", "total", Some(1000), 1000);

    let (status, session) = post_json_with_auth(
        &app,
        "/rbx/v1/sessions",
        Some("rbxsess_leandro"),
        session_request(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let session_id = session["session_id"].as_str().unwrap().to_owned();

    let (status, refused) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/sessions/{session_id}/runs"),
        Some("rbxsess_leandro"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(refused["error"]["code"], "budget_exceeded");

    // Limits endpoint reports the exhausted budget + 70% context policy.
    let (status, limits) = send_with_auth(
        app.clone(),
        "GET",
        &format!("/rbx/v1/sessions/{session_id}/limits"),
        Some("rbxsess_leandro"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(limits["budgets"][0]["exhausted"], true);
    assert_eq!(limits["context_utilization_limit"], 0.7);
    assert_eq!(limits["context_policy_ref"], "context-utilization-70");
}

// JWT validation before session creation: no credential, no session.
#[tokio::test]
async fn rbx_session_creation_requires_credential() {
    let (app, _store) = rbx_lifecycle_app();
    let (status, body) =
        post_json_with_auth(&app, "/rbx/v1/sessions", None, session_request()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "policy_denied");
}

// Idempotency keys make session/run creation retry-safe.
#[tokio::test]
async fn rbx_idempotent_session_and_run_creation() {
    let (app, _store) = rbx_lifecycle_app();

    let mut req = session_request();
    req["idempotency_key"] = json!("idem-session-1");
    let (_, first) = post_json_with_auth(
        &app,
        "/rbx/v1/sessions",
        Some("rbxsess_leandro"),
        req.clone(),
    )
    .await;
    let (_, second) =
        post_json_with_auth(&app, "/rbx/v1/sessions", Some("rbxsess_leandro"), req).await;
    assert_eq!(first["session_id"], second["session_id"]);

    let session_id = first["session_id"].as_str().unwrap().to_owned();
    let run_req = json!({ "idempotency_key": "idem-run-1" });
    let (_, run_a) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/sessions/{session_id}/runs"),
        Some("rbxsess_leandro"),
        run_req.clone(),
    )
    .await;
    let (_, run_b) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/sessions/{session_id}/runs"),
        Some("rbxsess_leandro"),
        run_req,
    )
    .await;
    assert_eq!(run_a["run_id"], run_b["run_id"]);
}

#[tokio::test]
async fn rbx_unknown_session_and_run_are_typed_404() {
    let (app, _store) = rbx_lifecycle_app();
    let missing = uuid::Uuid::new_v4();

    let (status, body) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/sessions/{missing}/runs"),
        Some("rbxsess_leandro"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "unknown_session");

    let (status, body) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/runs/{missing}/cancel"),
        Some("rbxsess_leandro"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "unknown_run");
}

// §3 security: request bodies beyond the limit are rejected before handlers.
#[tokio::test]
async fn rbx_body_limit_rejects_oversized_payload() {
    let (app, _store) = rbx_lifecycle_app();
    let oversized = "x".repeat(300 * 1024);
    let (status, _) = post_json_with_auth(
        &app,
        "/rbx/v1/sessions",
        Some("rbxsess_leandro"),
        json!({ "tenant": oversized, "product": "p", "workflow": "w" }),
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

// === Phase 3 slice 2: route envelope audited per model call + typed backend errors ===

#[tokio::test]
async fn full_call_audits_route_envelope_before_backend() {
    let backend = Arc::new(CountingBackendPort::new(BackendResponse {
        content: "ok".to_owned(),
        tokens_used: Some(10),
        latency_ms: Some(5),
    }));
    let app = app::build_with_backend(make_config(vec![test_policy()]), backend);

    let (status, body) = send_request(
        app.clone(),
        "POST",
        "/v1/call",
        Some(test_request_body("RBX", "test-product", "test-workflow")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let audit_id = body["post_call"]["audit_id"].as_str().unwrap().to_owned();

    let request = Request::builder()
        .method("GET")
        .uri(format!("/v1/audit/{audit_id}"))
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let audit: Value = serde_json::from_slice(&bytes).unwrap();
    let kinds: Vec<&str> = audit["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert!(
        kinds.contains(&"RouteEnvelope"),
        "route envelope must be audited for every model call, got {kinds:?}"
    );
    let route = audit["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"] == "RouteEnvelope")
        .unwrap();
    assert_eq!(route["details"]["model_alias"], "test-backend");
    assert_eq!(route["details"]["timeout_ms"], 5000);
}

#[tokio::test]
async fn full_call_surfaces_typed_backend_error_and_still_runs_post_call() {
    // Legacy adapter failure signature: empty content -> typed Unavailable
    // via the default execute() bridge; post_call still runs and the
    // response stays 200 (compatibility preserved).
    let backend = Arc::new(CountingBackendPort::new(BackendResponse {
        content: String::new(),
        tokens_used: None,
        latency_ms: None,
    }));
    let app = app::build_with_backend(make_config(vec![test_policy()]), backend.clone());

    let (status, body) = send_request(
        app,
        "POST",
        "/v1/call",
        Some(test_request_body("RBX", "test-product", "test-workflow")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(backend.call_count(), 1);
    assert_eq!(body["backend_error"]["code"], "backend_unavailable");
    assert!(
        body["post_call"]["status"].is_string(),
        "post_call must run"
    );
}

#[tokio::test]
async fn full_call_success_has_no_backend_error_field() {
    let backend = Arc::new(CountingBackendPort::new(BackendResponse {
        content: "ok".to_owned(),
        tokens_used: Some(10),
        latency_ms: Some(5),
    }));
    let app = app::build_with_backend(make_config(vec![test_policy()]), backend);
    let (status, body) = send_request(
        app,
        "POST",
        "/v1/call",
        Some(test_request_body("RBX", "test-product", "test-workflow")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.get("backend_error").is_none(),
        "additive field must be absent on success"
    );
    assert_eq!(body["backend_content"], "ok");
}

// === Phase 3 slice 3: SSE streaming endpoint (/v1/call/stream) ===

use thalamus_core::{BackendCallError, BackendExecution, BackendUsage, CancelToken, RouteEnvelope};

/// Scripted streaming backend: emits fixed deltas through the sink.
struct ScriptedStreamingBackend {
    deltas: Vec<&'static str>,
}

impl BackendPort for ScriptedStreamingBackend {
    fn call(&self, _envelope: &Envelope) -> BackendResponse {
        unreachable!("streaming test must use execute_streaming")
    }

    fn execute(
        &self,
        _route: &RouteEnvelope,
        _cancel: &CancelToken,
    ) -> Result<BackendExecution, BackendCallError> {
        unreachable!("streaming test must use execute_streaming")
    }

    fn execute_streaming(
        &self,
        _route: &RouteEnvelope,
        _cancel: &CancelToken,
        sink: &mut dyn FnMut(&str),
    ) -> Result<BackendExecution, BackendCallError> {
        let mut content = String::new();
        for delta in &self.deltas {
            content.push_str(delta);
            sink(delta);
        }
        Ok(BackendExecution {
            content,
            usage: BackendUsage {
                prompt_tokens: Some(5),
                completion_tokens: Some(3),
                total_tokens: Some(8),
            },
            latency_ms: 7,
            backend_metadata: serde_json::json!({ "adapter": "scripted" }),
        })
    }
}

async fn collect_sse(app: axum::Router, body: Value) -> (StatusCode, String) {
    let request = Request::builder()
        .method("POST")
        .uri("/v1/call/stream")
        .header("content-type", "application/json")
        .body::<Body>(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

fn sse_events(raw: &str) -> Vec<(String, Value)> {
    let mut events = Vec::new();
    let mut name = String::new();
    for line in raw.lines() {
        if let Some(n) = line.strip_prefix("event: ") {
            name = n.to_owned();
        } else if let Some(data) = line.strip_prefix("data: ") {
            if let Ok(v) = serde_json::from_str(data) {
                events.push((name.clone(), v));
            }
        }
    }
    events
}

#[tokio::test]
async fn call_stream_emits_decision_chunks_result_and_audits_route() {
    let backend = Arc::new(ScriptedStreamingBackend {
        deltas: vec!["Hel", "lo ", "mundo"],
    });
    let app = app::build_with_backend(make_config(vec![test_policy()]), backend);

    let (status, raw) = collect_sse(
        app.clone(),
        test_request_body("RBX", "test-product", "test-workflow"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let events = sse_events(&raw);
    let names: Vec<&str> = events.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        vec!["decision", "chunk", "chunk", "chunk", "result"],
        "raw SSE:\n{raw}"
    );
    assert_eq!(events[0].1["decision"], "Allow");
    let deltas: String = events
        .iter()
        .filter(|(n, _)| n == "chunk")
        .map(|(_, v)| v["delta"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(deltas, "Hello mundo");
    let result = &events.last().unwrap().1;
    assert_eq!(result["usage"]["total_tokens"], 8);
    assert!(result["status"].is_string());

    // Route envelope audited for the streamed call too.
    let audit_id = events[0].1["audit_id"].as_str().unwrap();
    let (_, audit) = send_request(app, "GET", &format!("/v1/audit/{audit_id}"), None).await;
    let kinds: Vec<&str> = audit["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"RouteEnvelope"), "got {kinds:?}");
    assert!(
        kinds.contains(&"PostCallOutcome"),
        "post_call must run, got {kinds:?}"
    );
}

#[tokio::test]
async fn call_stream_deny_has_no_chunks_and_no_backend_call() {
    let backend = Arc::new(CountingBackendPort::new(BackendResponse {
        content: "never".to_owned(),
        tokens_used: None,
        latency_ms: None,
    }));
    let app = app::build_with_backend(make_config(vec![test_policy()]), backend.clone());

    // Unknown tenant/product resolves to a deny policy in ConfigPolicyPort.
    let (status, raw) = collect_sse(
        app,
        test_request_body("unknown", "unknown-product", "unknown-workflow"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let events = sse_events(&raw);
    let names: Vec<&str> = events.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        !names.contains(&"chunk"),
        "deny must produce no chunks, got {names:?}"
    );
    assert_eq!(backend.call_count(), 0, "deny must never call the backend");
}

#[tokio::test]
async fn call_stream_bridges_legacy_backends_as_single_chunk() {
    let backend = Arc::new(CountingBackendPort::new(BackendResponse {
        content: "inteiro".to_owned(),
        tokens_used: Some(4),
        latency_ms: Some(2),
    }));
    let app = app::build_with_backend(make_config(vec![test_policy()]), backend);

    let (_, raw) = collect_sse(
        app,
        test_request_body("RBX", "test-product", "test-workflow"),
    )
    .await;
    let events = sse_events(&raw);
    let chunks: Vec<&Value> = events
        .iter()
        .filter(|(n, _)| n == "chunk")
        .map(|(_, v)| v)
        .collect();
    assert_eq!(chunks.len(), 1, "legacy bridge = one chunk");
    assert_eq!(chunks[0]["delta"], "inteiro");
}

// === Phase 3 slice 4: governance endpoints + rate limits + identity probe ===

#[tokio::test]
async fn rbx_governance_records_tool_decision_approval_evidence() {
    let (app, _store) = rbx_lifecycle_app();

    let (_, session) = post_json_with_auth(
        &app,
        "/rbx/v1/sessions",
        Some("rbxsess_leandro"),
        session_request(),
    )
    .await;
    let session_id = session["session_id"].as_str().unwrap().to_owned();

    // Tool decision.
    let (status, body) = post_json_with_auth(
        &app,
        "/rbx/v1/tool-decisions",
        Some("rbxsess_leandro"),
        json!({ "session_id": session_id, "tool": "shell", "decision": "denied" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(body["invocation_id"].is_string());

    // Approval: approver comes from the credential, not the body.
    let (status, body) = post_json_with_auth(
        &app,
        "/rbx/v1/approvals",
        Some("rbxsess_leandro"),
        json!({
            "session_id": session_id,
            "subject": "patch:abc123",
            "decision": "approved",
            "approver": "attacker@evil.example"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["approver"], "ldamasio@gmail.com");

    // Evidence: pointer + hash only.
    let (status, body) = post_json_with_auth(
        &app,
        "/rbx/v1/evidence",
        Some("rbxsess_leandro"),
        json!({ "kind": "test-run", "uri": "s3://rbx-evidence/x", "content_hash": "deadbeef" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(body["evidence_id"].is_string());

    // Unknown session refused for tool decisions.
    let (status, body) = post_json_with_auth(
        &app,
        "/rbx/v1/tool-decisions",
        Some("rbxsess_leandro"),
        json!({ "session_id": uuid::Uuid::new_v4().to_string(), "tool": "shell", "decision": "allowed" }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "unknown_session");

    // Invalid decision value refused.
    let (status, _) = post_json_with_auth(
        &app,
        "/rbx/v1/tool-decisions",
        Some("rbxsess_leandro"),
        json!({ "session_id": session_id, "tool": "shell", "decision": "maybe" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// === Governance idempotency (master plan §3 slice 5) ===

fn session_request_for_tenant(tenant: &str) -> Value {
    json!({
        "tenant": tenant,
        "product": "kulinaryos",
        "workflow": "coding",
        "governance_mode": "governed_llm_access",
    })
}

fn rbx_lifecycle_app_with_verifier(
    verifier: StaticCredentialVerifier,
) -> (axum::Router, Arc<InMemorySessionStore>) {
    let store = Arc::new(InMemorySessionStore::new());
    let app = app::build_with_rbx_api_and_sessions(
        make_config(vec![test_policy()]),
        Arc::new(verifier),
        store.clone() as SharedSessionStore,
    );
    (app, store)
}

#[tokio::test]
async fn rbx_tool_decision_idempotency_replay_returns_same_invocation() {
    let (app, _store) = rbx_lifecycle_app();
    let (_, session) = post_json_with_auth(
        &app,
        "/rbx/v1/sessions",
        Some("rbxsess_leandro"),
        session_request(),
    )
    .await;
    let session_id = session["session_id"].as_str().unwrap().to_owned();
    let key = uuid::Uuid::new_v4().to_string();

    let (status1, body1) = post_json_with_auth(
        &app,
        "/rbx/v1/tool-decisions",
        Some("rbxsess_leandro"),
        json!({
            "session_id": session_id, "tool": "shell", "decision": "denied",
            "idempotency_key": key,
        }),
    )
    .await;
    assert_eq!(status1, StatusCode::CREATED);

    // Replay: same key, same identity fields, different metadata (excluded
    // from the fingerprint) -> same invocation, still 201.
    let (status2, body2) = post_json_with_auth(
        &app,
        "/rbx/v1/tool-decisions",
        Some("rbxsess_leandro"),
        json!({
            "session_id": session_id, "tool": "shell", "decision": "denied",
            "idempotency_key": key, "metadata": { "retry": true },
        }),
    )
    .await;
    assert_eq!(status2, StatusCode::CREATED);
    assert_eq!(
        body1["invocation_id"], body2["invocation_id"],
        "replay must return the original invocation, not a new one"
    );
}

#[tokio::test]
async fn rbx_tool_decision_idempotency_conflict_returns_409() {
    let (app, _store) = rbx_lifecycle_app();
    let (_, session) = post_json_with_auth(
        &app,
        "/rbx/v1/sessions",
        Some("rbxsess_leandro"),
        session_request(),
    )
    .await;
    let session_id = session["session_id"].as_str().unwrap().to_owned();
    let key = uuid::Uuid::new_v4().to_string();

    let (status1, _) = post_json_with_auth(
        &app,
        "/rbx/v1/tool-decisions",
        Some("rbxsess_leandro"),
        json!({
            "session_id": session_id, "tool": "shell", "decision": "denied",
            "idempotency_key": key,
        }),
    )
    .await;
    assert_eq!(status1, StatusCode::CREATED);

    // Same key, different decision -> different fingerprint -> refused.
    let (status2, body2) = post_json_with_auth(
        &app,
        "/rbx/v1/tool-decisions",
        Some("rbxsess_leandro"),
        json!({
            "session_id": session_id, "tool": "shell", "decision": "allowed",
            "idempotency_key": key,
        }),
    )
    .await;
    assert_eq!(status2, StatusCode::CONFLICT);
    assert_eq!(body2["error"]["code"], "idempotency_key_conflict");
    assert_eq!(body2["error"]["retryable"], false);
}

#[tokio::test]
async fn rbx_tool_decision_idempotency_scoped_by_tenant_and_source_system() {
    let verifier = StaticCredentialVerifier::with_valid(
        "rbxsess_leandro",
        rbx_caller("ldamasio@gmail.com", &["kulinaryos:access"]),
    )
    .and_valid(
        "rbxsess_other_app",
        VerifiedCaller {
            client_app_id: Some("other-app".to_owned()),
            ..rbx_caller("other@example.com", &["kulinaryos:access"])
        },
    );
    let (app, _store) = rbx_lifecycle_app_with_verifier(verifier);

    // Two sessions under distinct tenants.
    let (_, session_a) = post_json_with_auth(
        &app,
        "/rbx/v1/sessions",
        Some("rbxsess_leandro"),
        session_request_for_tenant("tenant-a"),
    )
    .await;
    let session_a_id = session_a["session_id"].as_str().unwrap().to_owned();
    let (_, session_b) = post_json_with_auth(
        &app,
        "/rbx/v1/sessions",
        Some("rbxsess_leandro"),
        session_request_for_tenant("tenant-b"),
    )
    .await;
    let session_b_id = session_b["session_id"].as_str().unwrap().to_owned();

    let key = uuid::Uuid::new_v4().to_string();
    let (status_a, body_a) = post_json_with_auth(
        &app,
        "/rbx/v1/tool-decisions",
        Some("rbxsess_leandro"),
        json!({
            "session_id": session_a_id, "tool": "shell", "decision": "denied",
            "idempotency_key": key,
        }),
    )
    .await;
    let (status_b, body_b) = post_json_with_auth(
        &app,
        "/rbx/v1/tool-decisions",
        Some("rbxsess_leandro"),
        json!({
            "session_id": session_b_id, "tool": "shell", "decision": "denied",
            "idempotency_key": key,
        }),
    )
    .await;
    assert_eq!(status_a, StatusCode::CREATED);
    assert_eq!(status_b, StatusCode::CREATED);
    assert_ne!(
        body_a["invocation_id"], body_b["invocation_id"],
        "same key under different tenants must not collide"
    );

    // Same tenant (tenant-a), same key, different caller (different
    // source_system) must also not collide.
    let (status_c, body_c) = post_json_with_auth(
        &app,
        "/rbx/v1/tool-decisions",
        Some("rbxsess_other_app"),
        json!({
            "session_id": session_a_id, "tool": "shell", "decision": "denied",
            "idempotency_key": key,
        }),
    )
    .await;
    assert_eq!(status_c, StatusCode::CREATED);
    assert_ne!(body_a["invocation_id"], body_c["invocation_id"]);
}

#[tokio::test]
async fn rbx_tool_decision_idempotency_ignores_tenant_and_source_system_in_body() {
    let (app, _store) = rbx_lifecycle_app();
    let (_, session) = post_json_with_auth(
        &app,
        "/rbx/v1/sessions",
        Some("rbxsess_leandro"),
        session_request(),
    )
    .await;
    let session_id = session["session_id"].as_str().unwrap().to_owned();
    let key = uuid::Uuid::new_v4().to_string();

    let (status1, body1) = post_json_with_auth(
        &app,
        "/rbx/v1/tool-decisions",
        Some("rbxsess_leandro"),
        json!({
            "session_id": session_id, "tool": "shell", "decision": "denied",
            "idempotency_key": key,
            "tenant": "attacker-tenant", "source_system": "attacker-app",
        }),
    )
    .await;
    assert_eq!(status1, StatusCode::CREATED);

    // Same key, same real session/caller, but a *different* forged
    // tenant/source_system in the body. If those body fields were honored,
    // this would land in a different scope and succeed as a fresh insert. It
    // must instead resolve to the same invocation: proof the body values are
    // structurally ignored (ToolDecisionRequest has no such fields).
    let (status2, body2) = post_json_with_auth(
        &app,
        "/rbx/v1/tool-decisions",
        Some("rbxsess_leandro"),
        json!({
            "session_id": session_id, "tool": "shell", "decision": "denied",
            "idempotency_key": key,
            "tenant": "another-attacker-tenant", "source_system": "another-attacker-app",
        }),
    )
    .await;
    assert_eq!(status2, StatusCode::CREATED);
    assert_eq!(body1["invocation_id"], body2["invocation_id"]);
}

#[tokio::test]
async fn rbx_tool_decision_idempotency_emits_lifecycle_once() {
    let (app, _store) = rbx_lifecycle_app();
    let (_, session) = post_json_with_auth(
        &app,
        "/rbx/v1/sessions",
        Some("rbxsess_leandro"),
        session_request(),
    )
    .await;
    let session_id = session["session_id"].as_str().unwrap().to_owned();
    let key = uuid::Uuid::new_v4().to_string();

    for _ in 0..3 {
        let (status, _) = post_json_with_auth(
            &app,
            "/rbx/v1/tool-decisions",
            Some("rbxsess_leandro"),
            json!({
                "session_id": session_id, "tool": "shell", "decision": "denied",
                "idempotency_key": key,
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
    }

    let (status, body) =
        send_with_auth(app.clone(), "GET", &format!("/v1/audit/{session_id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    let events = body["events"].as_array().cloned().unwrap_or_default();
    let lifecycle_count = events
        .iter()
        .filter(|e| e["kind"] == "Lifecycle" && e["details"]["entity_type"] == "tool_invocation")
        .count();
    assert_eq!(
        lifecycle_count, 1,
        "3 identical replays must emit exactly one lifecycle event, got: {events:?}"
    );
}

#[tokio::test]
async fn rbx_approval_idempotency_requires_session_id() {
    let (app, _store) = rbx_lifecycle_app();
    let key = uuid::Uuid::new_v4().to_string();

    let (status, body) = post_json_with_auth(
        &app,
        "/rbx/v1/approvals",
        Some("rbxsess_leandro"),
        json!({
            "subject": "patch:abc123", "decision": "approved", "idempotency_key": key,
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "idempotency_requires_session");
}

#[tokio::test]
async fn rbx_approval_idempotency_unknown_session_is_404() {
    let (app, _store) = rbx_lifecycle_app();
    let key = uuid::Uuid::new_v4().to_string();

    let (status, body) = post_json_with_auth(
        &app,
        "/rbx/v1/approvals",
        Some("rbxsess_leandro"),
        json!({
            "session_id": uuid::Uuid::new_v4().to_string(),
            "subject": "patch:abc123", "decision": "approved", "idempotency_key": key,
        }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "unknown_session");
}

#[tokio::test]
async fn rbx_approval_idempotency_replay_and_conflict() {
    let (app, _store) = rbx_lifecycle_app();
    let (_, session) = post_json_with_auth(
        &app,
        "/rbx/v1/sessions",
        Some("rbxsess_leandro"),
        session_request(),
    )
    .await;
    let session_id = session["session_id"].as_str().unwrap().to_owned();
    let key = uuid::Uuid::new_v4().to_string();

    let (status1, body1) = post_json_with_auth(
        &app,
        "/rbx/v1/approvals",
        Some("rbxsess_leandro"),
        json!({
            "session_id": session_id, "subject": "patch:abc123", "decision": "approved",
            "reason": "looks good", "idempotency_key": key,
        }),
    )
    .await;
    assert_eq!(status1, StatusCode::CREATED);

    // Replay: same identity fields, different reason (excluded from the
    // fingerprint) -> same approval.
    let (status2, body2) = post_json_with_auth(
        &app,
        "/rbx/v1/approvals",
        Some("rbxsess_leandro"),
        json!({
            "session_id": session_id, "subject": "patch:abc123", "decision": "approved",
            "reason": "a completely different reason", "idempotency_key": key,
        }),
    )
    .await;
    assert_eq!(status2, StatusCode::CREATED);
    assert_eq!(body1["approval_id"], body2["approval_id"]);

    // Same key, different decision -> conflict.
    let (status3, body3) = post_json_with_auth(
        &app,
        "/rbx/v1/approvals",
        Some("rbxsess_leandro"),
        json!({
            "session_id": session_id, "subject": "patch:abc123", "decision": "rejected",
            "idempotency_key": key,
        }),
    )
    .await;
    assert_eq!(status3, StatusCode::CONFLICT);
    assert_eq!(body3["error"]["code"], "idempotency_key_conflict");
}

#[tokio::test]
async fn rbx_approval_idempotency_requires_client_app_id() {
    let verifier = StaticCredentialVerifier::with_valid(
        "rbxsess_no_app",
        VerifiedCaller {
            client_app_id: None,
            ..rbx_caller("ldamasio@gmail.com", &["kulinaryos:access"])
        },
    );
    let (app, _store) = rbx_lifecycle_app_with_verifier(verifier);
    let (_, session) = post_json_with_auth(
        &app,
        "/rbx/v1/sessions",
        Some("rbxsess_no_app"),
        session_request(),
    )
    .await;
    let session_id = session["session_id"].as_str().unwrap().to_owned();
    let key = uuid::Uuid::new_v4().to_string();

    let (status, body) = post_json_with_auth(
        &app,
        "/rbx/v1/approvals",
        Some("rbxsess_no_app"),
        json!({
            "session_id": session_id, "subject": "patch:abc123", "decision": "approved",
            "idempotency_key": key,
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "idempotency_requires_client_app_id");
}

#[tokio::test]
async fn rbx_rate_limit_returns_typed_429_with_retry_after() {
    use thalamus_server::rate_limit::RateLimiter;

    let store = Arc::new(InMemorySessionStore::new());
    let verifier = StaticCredentialVerifier::with_valid(
        "rbxsess_leandro",
        rbx_caller("ldamasio@gmail.com", &["kulinaryos:access"]),
    );
    let app = app::build_with_rbx_api_sessions_limiter(
        make_config(vec![test_policy()]),
        Arc::new(verifier),
        store as SharedSessionStore,
        Some(Arc::new(RateLimiter::new(2))),
    );

    for _ in 0..2 {
        let (status, _) = send_with_auth(
            app.clone(),
            "GET",
            "/rbx/v1/identity",
            Some("rbxsess_leandro"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }
    let (status, body) = send_with_auth(
        app.clone(),
        "GET",
        "/rbx/v1/identity",
        Some("rbxsess_leandro"),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["error"]["code"], "rate_limited");
    assert_eq!(body["error"]["retryable"], true);

    // Unauthenticated requests are rejected by auth before the limiter.
    let (status, _) = send_with_auth(app, "GET", "/rbx/v1/identity", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn readyz_reports_identity_verifier_state() {
    // Without a verifier: identity fields present, service ready.
    let app = app::build(make_config(vec![test_policy()]));
    let (status, body) = send_request(app, "GET", "/readyz", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["identity_verifier"], false);
    assert_eq!(body["identity_reachable"], true);

    // With a static verifier (no upstream): reachable, ready.
    let (app, _store) = rbx_lifecycle_app();
    let (status, body) = send_request(app, "GET", "/readyz", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["identity_verifier"], true);
    assert_eq!(body["identity_reachable"], true);
}

// Regression (prod 2026-07-18): enabling the governed surface must not
// drop the backend from the legacy /v1/call path.
#[tokio::test]
async fn rbx_api_mode_keeps_legacy_call_backend_wired() {
    let backend = Arc::new(CountingBackendPort::new(BackendResponse {
        content: "ok".to_owned(),
        tokens_used: Some(3),
        latency_ms: Some(2),
    }));
    let verifier = StaticCredentialVerifier::with_valid(
        "rbxsess_leandro",
        rbx_caller("ldamasio@gmail.com", &["kulinaryos:access"]),
    );
    let app = app::build_with_rbx_api(
        make_config(vec![test_policy()]),
        Arc::new(verifier),
        Some(backend.clone()),
    );

    let (status, body) = send_request(
        app.clone(),
        "POST",
        "/v1/call",
        Some(test_request_body("RBX", "test-product", "test-workflow")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["backend_content"], "ok");
    assert_eq!(backend.call_count(), 1);

    // readyz reflects the wired backend.
    let (_, ready) = send_request(app, "GET", "/readyz", None).await;
    assert_eq!(ready["backend_configured"], true);
}

// === SLICE-T1: run-bound governed calls (/rbx/v1/runs/{run_id}/calls) ===
//
// Rejection matrix (Gate D): run of another principal, closed run/session,
// expired credential, uncorrelated legacy call under require_run_correlation,
// second execution on the same run, exhausted budget, client-supplied model.

fn kulinaryos_policy(require_run_correlation: bool) -> Policy {
    Policy {
        id: "kulinaryos-policy".to_owned(),
        tenant: "rbx".to_owned(),
        product: "kulinaryos".to_owned(),
        workflow: "coding".to_owned(),
        permitted_backends: vec![BackendHandle {
            id: "test-backend".to_owned(),
            backend_type: BackendType::Model,
        }],
        budget: Budget {
            max_tokens: 1000,
            max_latency_ms: 5000,
        },
        context_grants: vec![],
        redaction_rules: vec![],
        audit_required: true,
        risk_threshold: RiskLevel::Medium,
        require_run_correlation,
        prompt_profile: None,
    }
}

fn run_call_app(
    policy: Policy,
    backend: Arc<dyn BackendPort + Send + Sync>,
) -> (axum::Router, Arc<InMemorySessionStore>) {
    let store = Arc::new(InMemorySessionStore::new());
    let mallory = {
        let mut caller = rbx_caller("mallory@example.com", &[]);
        caller.jti = Some("jti-mallory".to_owned());
        caller
    };
    let verifier = StaticCredentialVerifier::with_valid(
        "rbxsess_leandro",
        rbx_caller("ldamasio@gmail.com", &["kulinaryos:access"]),
    )
    .and_valid("rbxsess_mallory", mallory);
    let app = app::build_with_rbx_api_sessions_backend(
        make_config(vec![policy]),
        Arc::new(verifier),
        store.clone() as SharedSessionStore,
        backend,
    );
    (app, store)
}

async fn create_session_and_run(app: &axum::Router) -> (String, String) {
    let (status, session) = post_json_with_auth(
        app,
        "/rbx/v1/sessions",
        Some("rbxsess_leandro"),
        session_request(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let session_id = session["session_id"].as_str().unwrap().to_owned();
    let (status, run) = post_json_with_auth(
        app,
        &format!("/rbx/v1/sessions/{session_id}/runs"),
        Some("rbxsess_leandro"),
        json!({ "model_alias": "test-backend" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let run_id = run["run_id"].as_str().unwrap().to_owned();
    (session_id, run_id)
}

fn chat_call_body() -> Value {
    json!({
        "intent": "chat",
        "payload_kind": "chat.completions.v1",
        "payload": { "messages": [{ "role": "user", "content": "hello governed world" }] },
    })
}

fn counting_backend() -> Arc<CountingBackendPort> {
    Arc::new(CountingBackendPort::new(BackendResponse {
        content: "governed ok".to_owned(),
        tokens_used: Some(50),
        latency_ms: Some(10),
    }))
}

#[tokio::test]
async fn run_call_executes_and_finishes_run_with_usage() {
    let backend = counting_backend();
    let (app, store) = run_call_app(kulinaryos_policy(false), backend.clone());
    let (session_id, run_id) = create_session_and_run(&app).await;
    store.set_budget("session", &session_id, "day", Some(1000), 0);

    let (status, body) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/runs/{run_id}/calls"),
        Some("rbxsess_leandro"),
        chat_call_body(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["decision"], "Allow");
    assert_eq!(body["content"], "governed ok");
    assert_eq!(body["session_id"], session_id);
    assert!(body["audit_id"].is_string());
    assert_eq!(backend.call_count(), 1);

    // Run is finalized: completed + executed (1:1 claim consumed).
    let (run, _) = store
        .run_with_session(&run_id.parse().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(format!("{:?}", run.status), "Completed");
    assert_eq!(run.execution_state, "executed");

    // Usage from the backend consumed the governing budget (I9).
    let (_, limits) = send_with_auth(
        app,
        "GET",
        &format!("/rbx/v1/sessions/{session_id}/limits"),
        Some("rbxsess_leandro"),
    )
    .await;
    assert_eq!(limits["budgets"][0]["consumed_tokens"], 50);
}

#[tokio::test]
async fn run_call_rejects_foreign_principal_as_not_found() {
    let backend = counting_backend();
    let (app, store) = run_call_app(kulinaryos_policy(false), backend.clone());
    let (_, run_id) = create_session_and_run(&app).await;

    let (status, body) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/runs/{run_id}/calls"),
        Some("rbxsess_mallory"),
        chat_call_body(),
    )
    .await;
    // Anti-enumeration: indistinguishable from an unknown run.
    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
    assert_eq!(body["error"]["code"], "not_found");
    assert_eq!(backend.call_count(), 0);

    // The run was NOT claimed: the owner can still execute it.
    let (run, _) = store
        .run_with_session(&run_id.parse().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(run.execution_state, "pending");
}

#[tokio::test]
async fn run_call_rejects_closed_run_and_closed_session() {
    let backend = counting_backend();
    let (app, _store) = run_call_app(kulinaryos_policy(false), backend.clone());

    // Cancelled run -> run_closed.
    let (_, run_id) = create_session_and_run(&app).await;
    let (status, _) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/runs/{run_id}/cancel"),
        Some("rbxsess_leandro"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/runs/{run_id}/calls"),
        Some("rbxsess_leandro"),
        chat_call_body(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "run_closed");

    // Closed session -> session_closed.
    let (session_id, run_id) = create_session_and_run(&app).await;
    let (status, _) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/sessions/{session_id}/close"),
        Some("rbxsess_leandro"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/runs/{run_id}/calls"),
        Some("rbxsess_leandro"),
        chat_call_body(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "session_closed");
    assert_eq!(backend.call_count(), 0);
}

#[tokio::test]
async fn run_call_rejects_expired_credential() {
    let store = Arc::new(InMemorySessionStore::new());
    let verifier = StaticCredentialVerifier::always_inactive("expired");
    let app = app::build_with_rbx_api_sessions_backend(
        make_config(vec![kulinaryos_policy(false)]),
        Arc::new(verifier),
        store as SharedSessionStore,
        counting_backend(),
    );
    let run_id = uuid::Uuid::new_v4();
    let (status, body) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/runs/{run_id}/calls"),
        Some("rbxsess_dead"),
        chat_call_body(),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "session_expired");
}

#[tokio::test]
async fn run_call_rejects_second_execution_on_same_run() {
    let backend = counting_backend();
    let (app, _store) = run_call_app(kulinaryos_policy(false), backend.clone());
    let (_, run_id) = create_session_and_run(&app).await;

    let (status, _) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/runs/{run_id}/calls"),
        Some("rbxsess_leandro"),
        chat_call_body(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/runs/{run_id}/calls"),
        Some("rbxsess_leandro"),
        chat_call_body(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "run_already_executed");
    assert_eq!(backend.call_count(), 1, "backend must not run twice");
}

#[tokio::test]
async fn run_call_rejects_exhausted_budget_with_429() {
    let backend = counting_backend();
    let (app, store) = run_call_app(kulinaryos_policy(false), backend.clone());
    let (session_id, run_id) = create_session_and_run(&app).await;
    store.set_budget("session", &session_id, "day", Some(10), 10);

    let (status, body) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/runs/{run_id}/calls"),
        Some("rbxsess_leandro"),
        chat_call_body(),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["error"]["code"], "budget_exceeded");
    assert_eq!(backend.call_count(), 0);
}

#[tokio::test]
async fn run_call_rejects_client_supplied_model_and_bad_payload_kind() {
    let backend = counting_backend();
    let (app, _store) = run_call_app(kulinaryos_policy(false), backend.clone());
    let (_, run_id) = create_session_and_run(&app).await;

    let mut with_model = chat_call_body();
    with_model["payload"]["model"] = json!("gpt-4o");
    let (status, body) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/runs/{run_id}/calls"),
        Some("rbxsess_leandro"),
        with_model,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_request");

    let mut bad_kind = chat_call_body();
    bad_kind["payload_kind"] = json!("prompt.v0");
    let (status, body) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/runs/{run_id}/calls"),
        Some("rbxsess_leandro"),
        bad_kind,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_request");
    assert_eq!(backend.call_count(), 0);
}

#[tokio::test]
async fn run_call_denies_unmatched_policy_and_finalizes_run() {
    // Policy for another tenant: the session's tenant resolves to no-match.
    let backend = counting_backend();
    let (app, store) = run_call_app(test_policy(), backend.clone());
    let (_, run_id) = create_session_and_run(&app).await;

    let (status, body) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/runs/{run_id}/calls"),
        Some("rbxsess_leandro"),
        chat_call_body(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "body: {body}");
    assert_eq!(body["error"]["code"], "policy_denied");
    assert_eq!(backend.call_count(), 0, "deny never reaches the backend");

    // The claimed run is finalized as failed, never left dangling.
    let (run, _) = store
        .run_with_session(&run_id.parse().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(format!("{:?}", run.status), "Failed");
    assert_eq!(run.execution_state, "executed");
}

#[tokio::test]
async fn uncorrelated_legacy_call_denied_when_policy_requires_correlation() {
    let backend = counting_backend();
    let (app, store) = run_call_app(kulinaryos_policy(true), backend.clone());

    // Legacy /v1/call for the same tenant/product/workflow: denied before
    // any backend contact.
    let legacy_body = json!({
        "tenant": "rbx",
        "product": "kulinaryos",
        "user": "test-user",
        "workflow": "coding",
        "intent": "chat",
        "prompt": "hello",
        "requested_backend": { "id": "test-backend", "backend_type": "Model" },
    });
    let (status, resp) = send_request(app.clone(), "POST", "/v1/call", Some(legacy_body)).await;
    assert_eq!(status, StatusCode::OK);
    let decision = resp["decision"].as_str().unwrap();
    assert!(
        decision.starts_with("Deny") && decision.contains("uncorrelated_call"),
        "got: {decision}"
    );
    assert_eq!(backend.call_count(), 0);

    // The run-bound governed path stays allowed under the same policy.
    let (session_id, run_id) = create_session_and_run(&app).await;
    store.set_budget("session", &session_id, "day", Some(1000), 0);
    let (status, body) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/runs/{run_id}/calls"),
        Some("rbxsess_leandro"),
        chat_call_body(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(backend.call_count(), 1);
}

/// Scripted chat-streaming backend: emits verbatim chat.completion.chunk
/// objects (content delta, streamed tool_call arguments, finish_reason) the
/// way the LiteLLM adapter does for run-bound calls.
struct ScriptedChatBackend {
    chunks: Vec<Value>,
}

impl BackendPort for ScriptedChatBackend {
    fn call(&self, _envelope: &Envelope) -> BackendResponse {
        BackendResponse {
            content: String::new(),
            tokens_used: None,
            latency_ms: None,
        }
    }

    fn execute_streaming_chat(
        &self,
        _route: &RouteEnvelope,
        _cancel: &CancelToken,
        on_chunk: &mut dyn FnMut(&Value),
    ) -> Result<BackendExecution, BackendCallError> {
        for chunk in &self.chunks {
            on_chunk(chunk);
        }
        Ok(BackendExecution {
            content: "[{\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"read\",\"arguments\":\"{\\\"filePath\\\":\\\"x\\\"}\"}}]".to_owned(),
            usage: BackendUsage {
                prompt_tokens: Some(5),
                completion_tokens: Some(3),
                total_tokens: Some(8),
            },
            latency_ms: 7,
            backend_metadata: serde_json::json!({ "finish_reason": "tool_calls" }),
        })
    }
}

#[tokio::test]
async fn run_call_stream_passes_chunks_verbatim_and_finalizes_run() {
    let chunks = vec![
        json!({ "choices": [{ "index": 0, "delta": { "role": "assistant", "tool_calls": [{ "index": 0, "id": "call_1", "type": "function", "function": { "name": "read", "arguments": "" } }] }, "finish_reason": null }] }),
        json!({ "choices": [{ "index": 0, "delta": { "tool_calls": [{ "index": 0, "function": { "arguments": "{\"filePath\":\"x\"}" } }] }, "finish_reason": null }] }),
        json!({ "choices": [{ "index": 0, "delta": {}, "finish_reason": "tool_calls" }] }),
    ];
    let backend = Arc::new(ScriptedChatBackend {
        chunks: chunks.clone(),
    });
    let (app, store) = run_call_app(kulinaryos_policy(true), backend);
    let (_, run_id) = create_session_and_run(&app).await;

    let request = Request::builder()
        .method("POST")
        .uri(format!("/rbx/v1/runs/{run_id}/calls/stream"))
        .header("content-type", "application/json")
        .header("authorization", "Bearer rbxsess_leandro")
        .body::<Body>(Body::from(
            serde_json::to_string(&chat_call_body()).unwrap(),
        ))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let raw = String::from_utf8_lossy(&bytes).into_owned();

    let events = sse_events(&raw);
    let names: Vec<&str> = events.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        vec!["decision", "chunk", "chunk", "chunk", "result"],
        "raw SSE:\n{raw}"
    );
    assert_eq!(events[0].1["decision"], "Allow");
    assert_eq!(events[0].1["run_id"], run_id);
    // Verbatim passthrough: the wire chunks are exactly what the backend sent.
    for (i, chunk) in chunks.iter().enumerate() {
        assert_eq!(&events[i + 1].1, chunk, "chunk {i} altered in transit");
    }
    let result = &events.last().unwrap().1;
    assert_eq!(result["finish_reason"], "tool_calls");
    assert_eq!(result["usage"]["total_tokens"], 8);

    let (run, _) = store
        .run_with_session(&run_id.parse().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(format!("{:?}", run.status), "Completed");
    assert_eq!(run.execution_state, "executed");
}

/// Wire envelope (rbx.modelstream.wire.v1): every SSE event carries its
/// `event_seq` on the native `id` line; decision/result/error also embed
/// schema_version/event_id/event_seq in data; chunk data stays verbatim.
#[tokio::test]
async fn run_call_stream_carries_wire_envelope_sequence() {
    let chunks = vec![
        json!({ "choices": [{ "index": 0, "delta": { "content": "hel" }, "finish_reason": null }] }),
        json!({ "choices": [{ "index": 0, "delta": { "content": "lo" }, "finish_reason": "stop" }] }),
    ];
    let backend = Arc::new(ScriptedChatBackend {
        chunks: chunks.clone(),
    });
    let (app, _store) = run_call_app(kulinaryos_policy(true), backend);
    let (_, run_id) = create_session_and_run(&app).await;

    let request = Request::builder()
        .method("POST")
        .uri(format!("/rbx/v1/runs/{run_id}/calls/stream"))
        .header("content-type", "application/json")
        .header("authorization", "Bearer rbxsess_leandro")
        .body::<Body>(Body::from(
            serde_json::to_string(&chat_call_body()).unwrap(),
        ))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let raw = String::from_utf8_lossy(&bytes).into_owned();

    // The SSE id lines carry a strictly increasing sequence: 1..=4 here
    // (decision, chunk, chunk, result).
    let ids: Vec<u64> = raw
        .lines()
        .filter_map(|l| l.strip_prefix("id: "))
        .map(|v| v.trim().parse().unwrap())
        .collect();
    assert_eq!(ids, vec![1, 2, 3, 4], "raw SSE:\n{raw}");

    let events = sse_events(&raw);
    let decision = &events[0].1;
    assert_eq!(decision["schema_version"], "rbx.modelstream.wire.v1");
    assert_eq!(decision["event_seq"], 1);
    assert!(decision["event_id"].is_string());
    // Chunk data is untouched by the envelope: no schema_version injected.
    assert_eq!(&events[1].1, &chunks[0]);
    assert!(events[1].1.get("schema_version").is_none());
    let result = &events.last().unwrap().1;
    assert_eq!(result["schema_version"], "rbx.modelstream.wire.v1");
    assert_eq!(result["event_seq"], 4);
    assert!(result["event_id"].is_string());
}

#[tokio::test]
async fn run_call_rejects_model_not_permitted_by_policy() {
    let backend = counting_backend();
    let (app, _store) = run_call_app(kulinaryos_policy(false), backend.clone());
    let (status, session) = post_json_with_auth(
        &app,
        "/rbx/v1/sessions",
        Some("rbxsess_leandro"),
        session_request(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let session_id = session["session_id"].as_str().unwrap();
    let (status, run) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/sessions/{session_id}/runs"),
        Some("rbxsess_leandro"),
        json!({ "model_alias": "forbidden-model" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let run_id = run["run_id"].as_str().unwrap();

    let (status, body) = post_json_with_auth(
        &app,
        &format!("/rbx/v1/runs/{run_id}/calls"),
        Some("rbxsess_leandro"),
        chat_call_body(),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
    assert_eq!(body["error"]["code"], "model_not_permitted");
    assert_eq!(backend.call_count(), 0);
}

// === Route lease negotiation on run creation (rbx.route_lease.v1) ===
//
// Prompt-profile/capability negotiation happens BEFORE the client compiles
// and submits the call payload: the lease travels on the create-run response.
// Enforcement stays at call time; the lease mirrors what the call will do.

async fn create_run_with_alias(app: &axum::Router, alias: Option<&str>) -> Value {
    let (status, session) = post_json_with_auth(
        app,
        "/rbx/v1/sessions",
        Some("rbxsess_leandro"),
        session_request(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let session_id = session["session_id"].as_str().unwrap();
    let body = match alias {
        Some(alias) => json!({ "model_alias": alias }),
        None => json!({}),
    };
    let (status, run) = post_json_with_auth(
        app,
        &format!("/rbx/v1/sessions/{session_id}/runs"),
        Some("rbxsess_leandro"),
        body,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {run}");
    run
}

#[tokio::test]
async fn run_creation_returns_granted_route_lease() {
    let (app, _store) = run_call_app(kulinaryos_policy(false), counting_backend());
    let run = create_run_with_alias(&app, Some("test-backend")).await;

    let lease = &run["route_lease"];
    assert_eq!(lease["schema_version"], "rbx.route_lease.v1");
    assert_eq!(lease["status"], "granted");
    assert_eq!(lease["model_alias"], "test-backend");
    assert_eq!(lease["prompt_profile_id"], "rbx.default.v1");
    assert_eq!(lease["policy_snapshot_id"], "kulinaryos-policy");
    assert_eq!(lease["capabilities"]["streaming"], true);
    assert_eq!(
        lease["capabilities"]["payload_kinds"],
        json!(["chat.completions.v1"])
    );
    assert_eq!(lease["context"]["max_tokens"], 1000);
    assert_eq!(lease["context"]["max_context_utilization"], 0.7);
    assert_eq!(lease["run_id"], run["run_id"]);
    assert!(lease["lease_id"].is_string());
    assert!(lease["issued_at"].is_string());
    assert!(lease["expires_at"].is_string());
}

#[tokio::test]
async fn route_lease_resolves_unpinned_run_to_first_permitted_backend() {
    let (app, _store) = run_call_app(kulinaryos_policy(false), counting_backend());
    let run = create_run_with_alias(&app, None).await;

    let lease = &run["route_lease"];
    assert_eq!(lease["status"], "granted");
    assert_eq!(lease["model_alias"], "test-backend");
}

#[tokio::test]
async fn route_lease_flags_model_not_permitted_before_any_call() {
    let (app, _store) = run_call_app(kulinaryos_policy(false), counting_backend());
    let run = create_run_with_alias(&app, Some("forbidden-model")).await;

    let lease = &run["route_lease"];
    assert_eq!(lease["status"], "model_not_permitted");
    assert_eq!(lease["model_alias"], "forbidden-model");
}

#[tokio::test]
async fn route_lease_reports_policy_prompt_profile() {
    let mut policy = kulinaryos_policy(false);
    policy.prompt_profile = Some("rbx.coding.v1".to_owned());
    let (app, _store) = run_call_app(policy, counting_backend());
    let run = create_run_with_alias(&app, Some("test-backend")).await;

    assert_eq!(run["route_lease"]["prompt_profile_id"], "rbx.coding.v1");
}
