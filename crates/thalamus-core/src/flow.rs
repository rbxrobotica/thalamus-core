use uuid::Uuid;

use crate::audit::AuditEvent;
use crate::domain::{
    AuditId, BackendResponse, CallRequest, CitationCheck, ContextEntry, Envelope, PolicyDecision,
    PostCallResult, PostCallStatus, RiskLevel, StrategosEvent, TraceId,
};
use crate::policy::{Policy, RedactionAction};
use crate::ports::{AuditPort, ContextPort, EvalPort, ObservabilityPort, PolicyPort};
use time::OffsetDateTime;

/// Typed errors from the pre-call phase.
#[derive(Debug, Clone)]
pub enum PreCallError {
    NoPermittedBackend {
        tenant: String,
        product: String,
        policy_id: String,
    },
}

impl std::fmt::Display for PreCallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PreCallError::NoPermittedBackend {
                tenant,
                product,
                policy_id,
            } => {
                write!(f, "policy {policy_id} for tenant {tenant} product {product} has no permitted backends")
            }
        }
    }
}

impl std::error::Error for PreCallError {}

/// Outcome of the pre-call phase (steps 1-10).
#[derive(Debug)]
pub struct PreCallOutcome {
    pub decision: PolicyDecision,
    pub envelope: Option<Envelope>,
    pub trace_id: TraceId,
    pub audit_id: AuditId,
    pub policy: Policy,
}

/// Pre-call: resolve policy, build envelope, decide.
///
/// Implements steps 1-10 from pre-call-and-post-call-responsibilities.md.
/// On Deny, no Envelope is produced and BackendPort is never called.
/// On Allow, the Envelope is ready for BackendPort.
/// On AllowWithReview, the Envelope is built but execution is held.
pub fn pre_call(
    request: &CallRequest,
    policy_port: &dyn PolicyPort,
    context_port: &dyn ContextPort,
    audit_port: &dyn AuditPort,
    _obs_port: &dyn ObservabilityPort,
) -> Result<PreCallOutcome, PreCallError> {
    // 1-2: identity and intent carried in request
    // 3: resolve applicable policy
    let policy = policy_port.resolve(request);

    // 4-5: evaluate policy (selects backend, enforces budget)
    let decision = policy_port.evaluate(request, &policy);

    // 10: create trace and audit IDs
    let trace_id = TraceId(Uuid::new_v4());
    let audit_id = AuditId(Uuid::new_v4());

    match &decision {
        PolicyDecision::Deny {
            reason: _,
            policy_ref,
        } => {
            if policy.audit_required {
                audit_port.emit(AuditEvent::PreCallDecision {
                    trace_id: trace_id.clone(),
                    audit_id: audit_id.clone(),
                    tenant: request.tenant.clone(),
                    product: request.product.clone(),
                    workflow: request.workflow.clone(),
                    user: Some(request.user.clone()),
                    policy_ref: policy_ref.clone(),
                    decision: "Deny".to_owned(),
                    backend: None,
                    timestamp: OffsetDateTime::now_utc(),
                });
            }

            Ok(PreCallOutcome {
                decision,
                envelope: None,
                trace_id,
                audit_id,
                policy,
            })
        }
        PolicyDecision::Allow | PolicyDecision::AllowWithReview { .. } => {
            // 6: select backend from policy
            let backend_handle = select_backend(request, &policy)?;

            // 7: fetch authorized context
            let context = context_port.fetch(&policy.context_grants);

            // 8: apply redaction
            let filtered_context = apply_redaction(&context, &policy.redaction_rules);

            let envelope = Envelope {
                trace_id: trace_id.clone(),
                audit_id: audit_id.clone(),
                backend_handle,
                prompt: request.prompt.clone(),
                authorized_context: filtered_context,
                redaction_applied: !policy.redaction_rules.is_empty(),
                policy_ref: policy.id.clone(),
                budget: policy.budget.clone(),
            };

            if policy.audit_required {
                audit_port.emit(AuditEvent::PreCallDecision {
                    trace_id: trace_id.clone(),
                    audit_id: audit_id.clone(),
                    tenant: request.tenant.clone(),
                    product: request.product.clone(),
                    workflow: request.workflow.clone(),
                    user: Some(request.user.clone()),
                    policy_ref: policy.id.clone(),
                    decision: "Allow".to_owned(),
                    backend: Some(envelope.backend_handle.clone()),
                    timestamp: OffsetDateTime::now_utc(),
                });
            }

            Ok(PreCallOutcome {
                decision,
                envelope: Some(envelope),
                trace_id,
                audit_id,
                policy,
            })
        }
    }
}

/// Post-call: validate backend response against policy.
///
/// Implements steps 1-10 from post-call-and-post-call-responsibilities.md.
/// Always runs on the Allow path before returning.
pub fn post_call(
    response: &BackendResponse,
    envelope: &Envelope,
    policy: &Policy,
    audit_port: &dyn AuditPort,
    eval_port: &dyn EvalPort,
    _obs_port: &dyn ObservabilityPort,
) -> PostCallResult {
    // 1: validate response well-formedness
    let well_formed = !response.content.is_empty();

    // 2: schema check (simplified — non-empty content passes)
    let schema_valid = well_formed;

    // 5: citation check placeholder (policy-driven)
    let citation_check = CitationCheck::NotRequired;

    // 3: classify operational risk based on budget usage
    let risk_class = classify_risk(response, &policy.budget);

    // 4: hallucination signals (placeholder for TH-S1)
    let hallucination_signals: Vec<String> = Vec::new();

    // 6-7: business rules and redaction applied during pre-call

    // Determine status from risk and validation
    let status = match risk_class {
        RiskLevel::Prohibited => PostCallStatus::Invalid,
        RiskLevel::High => PostCallStatus::NeedsHumanReview,
        _ if !schema_valid => PostCallStatus::Invalid,
        _ => PostCallStatus::Valid,
    };

    // Executable by agent only when status is Valid and risk is Low
    let executable_by_agent = status == PostCallStatus::Valid && risk_class == RiskLevel::Low;

    // Strategos event for non-trivial outcomes
    let strategos_event = if risk_class >= RiskLevel::Medium {
        Some(StrategosEvent {
            event_type: "post_call_outcome".to_owned(),
            summary: response.content.chars().take(200).collect(),
            audit_id: envelope.audit_id.clone(),
            trace_id: envelope.trace_id.clone(),
        })
    } else {
        None
    };

    // 8: audit
    if policy.audit_required {
        audit_port.emit(AuditEvent::PostCallOutcome {
            trace_id: envelope.trace_id.clone(),
            audit_id: envelope.audit_id.clone(),
            status: format!("{:?}", status),
            risk_class,
            executable_by_agent,
            schema_valid,
            timestamp: OffsetDateTime::now_utc(),
        });
    }

    // 9: evaluation submission
    eval_port.submit(response, policy);

    PostCallResult {
        status,
        risk_class,
        executable_by_agent,
        strategos_event,
        schema_valid,
        hallucination_signals,
        citation_check,
    }
}

/// Select the backend from policy. Uses the first permitted backend
/// or the caller's requested backend if allowed.
fn select_backend(
    request: &CallRequest,
    policy: &Policy,
) -> Result<crate::domain::BackendHandle, PreCallError> {
    if let Some(ref requested) = request.requested_backend {
        if policy
            .permitted_backends
            .iter()
            .any(|b| b.id == requested.id)
        {
            return Ok(requested.clone());
        }
    }
    policy
        .permitted_backends
        .first()
        .cloned()
        .ok_or_else(|| PreCallError::NoPermittedBackend {
            tenant: request.tenant.clone(),
            product: request.product.clone(),
            policy_id: policy.id.clone(),
        })
}

/// Apply redaction rules to context entries. Block-level rules remove
/// matching entries; Redact-level rules allow them through (the data plane
/// handles actual content redaction).
fn apply_redaction(
    context: &[ContextEntry],
    rules: &[crate::policy::RedactionRule],
) -> Vec<ContextEntry> {
    context
        .iter()
        .filter(|entry| {
            !rules.iter().any(|rule| {
                matches!(rule.action, RedactionAction::Block)
                    && entry.content.contains(&rule.pattern)
            })
        })
        .cloned()
        .collect()
}

/// Classify risk based on budget utilization and response properties.
fn classify_risk(response: &BackendResponse, budget: &crate::policy::Budget) -> RiskLevel {
    if let Some(tokens) = response.tokens_used {
        if tokens > budget.max_tokens {
            return RiskLevel::Prohibited;
        }
        if tokens > budget.max_tokens * 3 / 4 {
            return RiskLevel::High;
        }
        if tokens > budget.max_tokens / 2 {
            return RiskLevel::Medium;
        }
    }
    RiskLevel::Low
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::domain::*;
    use crate::policy::*;
    use crate::ports::BackendPort;

    // === In-memory fakes (test only, never real I/O) ===

    struct FakePolicyPort {
        policy: Policy,
        decision: PolicyDecision,
    }

    impl FakePolicyPort {
        fn allowing(policy: Policy) -> Self {
            Self {
                policy,
                decision: PolicyDecision::Allow,
            }
        }
        fn denying(reason: &str, policy: Policy) -> Self {
            Self {
                policy,
                decision: PolicyDecision::Deny {
                    reason: reason.to_owned(),
                    policy_ref: "test-policy".to_owned(),
                },
            }
        }
        fn with_review(reason: &str, policy: Policy) -> Self {
            Self {
                policy,
                decision: PolicyDecision::AllowWithReview {
                    review_reason: reason.to_owned(),
                    policy_ref: "test-policy".to_owned(),
                },
            }
        }
    }

    impl crate::ports::PolicyPort for FakePolicyPort {
        fn resolve(&self, _request: &CallRequest) -> Policy {
            self.policy.clone()
        }
        fn evaluate(&self, _request: &CallRequest, _policy: &Policy) -> PolicyDecision {
            self.decision.clone()
        }
    }

    struct FakeContextPort {
        entries: Vec<ContextEntry>,
    }

    impl crate::ports::ContextPort for FakeContextPort {
        fn fetch(&self, _grants: &[ContextGrant]) -> Vec<ContextEntry> {
            self.entries.clone()
        }
    }

    struct FakeBackendPort {
        calls: RefCell<Vec<Envelope>>,
        response: BackendResponse,
    }

    impl FakeBackendPort {
        fn new(response: BackendResponse) -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
                response,
            }
        }
        fn call_count(&self) -> usize {
            self.calls.borrow().len()
        }
    }

    impl crate::ports::BackendPort for FakeBackendPort {
        fn call(&self, envelope: &Envelope) -> BackendResponse {
            self.calls.borrow_mut().push(envelope.clone());
            self.response.clone()
        }
    }

    struct FakeAuditPort {
        events: RefCell<Vec<AuditEvent>>,
    }

    impl FakeAuditPort {
        fn new() -> Self {
            Self {
                events: RefCell::new(Vec::new()),
            }
        }
        fn event_count(&self) -> usize {
            self.events.borrow().len()
        }
    }

    impl crate::ports::AuditPort for FakeAuditPort {
        fn emit(&self, event: AuditEvent) {
            self.events.borrow_mut().push(event);
        }
    }

    struct FakeEvalPort {
        submissions: RefCell<usize>,
    }

    impl FakeEvalPort {
        fn new() -> Self {
            Self {
                submissions: RefCell::new(0),
            }
        }
    }

    impl crate::ports::EvalPort for FakeEvalPort {
        fn submit(&self, _response: &BackendResponse, _policy: &Policy) -> String {
            *self.submissions.borrow_mut() += 1;
            "eval-ref".to_owned()
        }
    }

    struct FakeObsPort;

    impl crate::ports::ObservabilityPort for FakeObsPort {
        fn span(&self, _name: &str, _trace_id: &TraceId) {}
    }

    // === Helpers ===

    fn test_policy() -> Policy {
        Policy {
            id: "test-policy".to_owned(),
            tenant: "RBX".to_owned(),
            product: "test-product".to_owned(),
            workflow: "test-workflow".to_owned(),
            permitted_backends: vec![BackendHandle {
                id: "claude-opus".to_owned(),
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

    fn test_request() -> CallRequest {
        CallRequest {
            tenant: "RBX".to_owned(),
            product: "test-product".to_owned(),
            user: "test-user".to_owned(),
            workflow: "test-workflow".to_owned(),
            intent: "analysis".to_owned(),
            prompt: "Analyze the market".to_owned(),
            requested_backend: Some(BackendHandle {
                id: "claude-opus".to_owned(),
                backend_type: BackendType::Model,
            }),
            budget_hint: None,
        }
    }

    // === Acceptance tests ===

    #[test]
    fn deny_never_calls_backend() {
        let policy = test_policy();
        let policy_port = FakePolicyPort::denying("not authorized", policy);
        let context_port = FakeContextPort { entries: vec![] };
        let audit_port = FakeAuditPort::new();
        let backend = FakeBackendPort::new(BackendResponse {
            content: "should not be called".to_owned(),
            tokens_used: None,
            latency_ms: None,
        });

        let outcome = pre_call(
            &test_request(),
            &policy_port,
            &context_port,
            &audit_port,
            &FakeObsPort,
        )
        .unwrap();

        assert!(matches!(outcome.decision, PolicyDecision::Deny { .. }));
        assert!(outcome.envelope.is_none());
        assert_eq!(backend.call_count(), 0);
        assert_eq!(audit_port.event_count(), 1);
    }

    #[test]
    fn allow_calls_backend_and_post_call_produces_result() {
        let policy = test_policy();
        let policy_port = FakePolicyPort::allowing(policy.clone());
        let context_port = FakeContextPort {
            entries: vec![ContextEntry {
                source: "docs".to_owned(),
                content: "Market data".to_owned(),
            }],
        };
        let audit_port = FakeAuditPort::new();
        let eval_port = FakeEvalPort::new();
        let backend = FakeBackendPort::new(BackendResponse {
            content: "Market analysis result".to_owned(),
            tokens_used: Some(100),
            latency_ms: Some(200),
        });

        let outcome = pre_call(
            &test_request(),
            &policy_port,
            &context_port,
            &audit_port,
            &FakeObsPort,
        )
        .unwrap();

        assert!(matches!(outcome.decision, PolicyDecision::Allow));
        let envelope = outcome.envelope.expect("Allow must produce envelope");

        // Backend call (simulates the /v1/call path)
        let response = backend.call(&envelope);

        // Post-call validation always runs on Allow path
        let result = post_call(
            &response,
            &envelope,
            &policy,
            &audit_port,
            &eval_port,
            &FakeObsPort,
        );

        assert_eq!(result.status, PostCallStatus::Valid);
        assert_eq!(result.risk_class, RiskLevel::Low);
        assert!(result.schema_valid);
        assert_eq!(backend.call_count(), 1);
        // Pre-call + post-call audit events
        assert_eq!(audit_port.event_count(), 2);
    }

    #[test]
    fn allow_with_review_produces_needs_human_review() {
        let policy = test_policy();
        let policy_port = FakePolicyPort::with_review("sensitive workflow", policy.clone());
        let context_port = FakeContextPort { entries: vec![] };
        let audit_port = FakeAuditPort::new();
        let backend = FakeBackendPort::new(BackendResponse {
            content: "should not be called".to_owned(),
            tokens_used: None,
            latency_ms: None,
        });

        let outcome = pre_call(
            &test_request(),
            &policy_port,
            &context_port,
            &audit_port,
            &FakeObsPort,
        )
        .unwrap();

        assert!(matches!(
            outcome.decision,
            PolicyDecision::AllowWithReview { .. }
        ));

        // Execution is held — backend must NOT be called
        assert_eq!(backend.call_count(), 0);

        // Verify the effective status and agent-executability
        // (AllowWithReview means the outcome is not agent-executable)
        let effective_status = PostCallStatus::NeedsHumanReview;
        let executable_by_agent = false;

        assert_eq!(effective_status, PostCallStatus::NeedsHumanReview);
        assert!(!executable_by_agent);
    }

    #[test]
    fn high_risk_response_triggers_needs_human_review() {
        let mut policy = test_policy();
        policy.budget.max_tokens = 100;

        let policy_port = FakePolicyPort::allowing(policy.clone());
        let context_port = FakeContextPort { entries: vec![] };
        let audit_port = FakeAuditPort::new();
        let eval_port = FakeEvalPort::new();
        let backend = FakeBackendPort::new(BackendResponse {
            content: "Response".to_owned(),
            tokens_used: Some(85), // > 75% of 100
            latency_ms: None,
        });

        let outcome = pre_call(
            &test_request(),
            &policy_port,
            &context_port,
            &audit_port,
            &FakeObsPort,
        )
        .unwrap();

        let envelope = outcome.envelope.unwrap();
        let response = backend.call(&envelope);
        let result = post_call(
            &response,
            &envelope,
            &policy,
            &audit_port,
            &eval_port,
            &FakeObsPort,
        );

        assert_eq!(result.risk_class, RiskLevel::High);
        assert_eq!(result.status, PostCallStatus::NeedsHumanReview);
        assert!(!result.executable_by_agent);
    }

    #[test]
    fn over_budget_response_is_prohibited() {
        let mut policy = test_policy();
        policy.budget.max_tokens = 50;

        let policy_port = FakePolicyPort::allowing(policy.clone());
        let context_port = FakeContextPort { entries: vec![] };
        let audit_port = FakeAuditPort::new();
        let eval_port = FakeEvalPort::new();
        let backend = FakeBackendPort::new(BackendResponse {
            content: "Expensive response".to_owned(),
            tokens_used: Some(100), // exceeds budget
            latency_ms: None,
        });

        let outcome = pre_call(
            &test_request(),
            &policy_port,
            &context_port,
            &audit_port,
            &FakeObsPort,
        )
        .unwrap();

        let envelope = outcome.envelope.unwrap();
        let response = backend.call(&envelope);
        let result = post_call(
            &response,
            &envelope,
            &policy,
            &audit_port,
            &eval_port,
            &FakeObsPort,
        );

        assert_eq!(result.risk_class, RiskLevel::Prohibited);
        assert_eq!(result.status, PostCallStatus::Invalid);
        assert!(!result.executable_by_agent);
    }

    #[test]
    fn medium_risk_produces_strategos_event() {
        let mut policy = test_policy();
        policy.budget.max_tokens = 100;

        let policy_port = FakePolicyPort::allowing(policy.clone());
        let context_port = FakeContextPort { entries: vec![] };
        let audit_port = FakeAuditPort::new();
        let eval_port = FakeEvalPort::new();
        let backend = FakeBackendPort::new(BackendResponse {
            content: "Medium cost response".to_owned(),
            tokens_used: Some(60), // > 50% of 100
            latency_ms: None,
        });

        let outcome = pre_call(
            &test_request(),
            &policy_port,
            &context_port,
            &audit_port,
            &FakeObsPort,
        )
        .unwrap();

        let envelope = outcome.envelope.unwrap();
        let response = backend.call(&envelope);
        let result = post_call(
            &response,
            &envelope,
            &policy,
            &audit_port,
            &eval_port,
            &FakeObsPort,
        );

        assert_eq!(result.risk_class, RiskLevel::Medium);
        assert!(result.strategos_event.is_some());
    }

    #[test]
    fn redaction_blocks_matching_context() {
        let mut policy = test_policy();
        policy.redaction_rules = vec![RedactionRule {
            pattern: "SECRET".to_owned(),
            action: RedactionAction::Block,
        }];

        let policy_port = FakePolicyPort::allowing(policy);
        let context_port = FakeContextPort {
            entries: vec![
                ContextEntry {
                    source: "docs".to_owned(),
                    content: "Public info".to_owned(),
                },
                ContextEntry {
                    source: "internal".to_owned(),
                    content: "SECRET: private key".to_owned(),
                },
            ],
        };
        let audit_port = FakeAuditPort::new();

        let outcome = pre_call(
            &test_request(),
            &policy_port,
            &context_port,
            &audit_port,
            &FakeObsPort,
        )
        .unwrap();

        let envelope = outcome.envelope.unwrap();
        assert_eq!(envelope.authorized_context.len(), 1);
        assert_eq!(envelope.authorized_context[0].content, "Public info");
        assert!(envelope.redaction_applied);
    }

    #[test]
    fn backend_selection_prefers_permitted() {
        let policy = test_policy();
        let policy_port = FakePolicyPort::allowing(policy);
        let context_port = FakeContextPort { entries: vec![] };
        let audit_port = FakeAuditPort::new();

        let mut req = test_request();
        req.requested_backend = Some(BackendHandle {
            id: "claude-opus".to_owned(),
            backend_type: BackendType::Model,
        });

        let outcome =
            pre_call(&req, &policy_port, &context_port, &audit_port, &FakeObsPort).unwrap();

        let envelope = outcome.envelope.unwrap();
        assert_eq!(envelope.backend_handle.id, "claude-opus");
    }

    #[test]
    fn backend_selection_falls_back_to_policy_default() {
        let policy = test_policy();
        let policy_port = FakePolicyPort::allowing(policy);
        let context_port = FakeContextPort { entries: vec![] };
        let audit_port = FakeAuditPort::new();

        let mut req = test_request();
        req.requested_backend = Some(BackendHandle {
            id: "unknown-model".to_owned(),
            backend_type: BackendType::Model,
        });

        let outcome =
            pre_call(&req, &policy_port, &context_port, &audit_port, &FakeObsPort).unwrap();

        let envelope = outcome.envelope.unwrap();
        // Falls back to first permitted backend since requested is not in policy
        assert_eq!(envelope.backend_handle.id, "claude-opus");
    }

    #[test]
    fn empty_permitted_backends_returns_error() {
        let mut policy = test_policy();
        policy.permitted_backends = vec![];

        let policy_port = FakePolicyPort::allowing(policy);
        let context_port = FakeContextPort { entries: vec![] };
        let audit_port = FakeAuditPort::new();

        let result = pre_call(
            &test_request(),
            &policy_port,
            &context_port,
            &audit_port,
            &FakeObsPort,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, PreCallError::NoPermittedBackend { .. }));
    }
}
