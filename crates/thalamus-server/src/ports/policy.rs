use thalamus_core::{CallRequest, Policy, PolicyDecision, PolicyPort};
use crate::config::ServerConfig;

/// Concrete PolicyPort: matches request tenant+product+workflow to loaded
/// policies. Evaluation is Allow when a matching policy exists with at least
/// one permitted backend, Deny otherwise.
pub struct ConfigPolicyPort {
    policies: Vec<Policy>,
}

impl ConfigPolicyPort {
    pub fn from_config(config: &ServerConfig) -> Self {
        Self {
            policies: config.policies.clone(),
        }
    }

    pub fn from_policies(policies: Vec<Policy>) -> Self {
        Self { policies }
    }
}

impl PolicyPort for ConfigPolicyPort {
    fn resolve(&self, request: &CallRequest) -> Policy {
        self.policies
            .iter()
            .find(|p| {
                p.tenant == request.tenant
                    && p.product == request.product
                    && p.workflow == request.workflow
            })
            .cloned()
            .unwrap_or_else(|| Policy {
                id: "no-match".to_owned(),
                tenant: request.tenant.clone(),
                product: request.product.clone(),
                workflow: request.workflow.clone(),
                permitted_backends: vec![],
                budget: thalamus_core::Budget {
                    max_tokens: 0,
                    max_latency_ms: 0,
                },
                context_grants: vec![],
                redaction_rules: vec![],
                audit_required: false,
                risk_threshold: thalamus_core::RiskLevel::Low,
            })
    }

    fn evaluate(&self, _request: &CallRequest, policy: &Policy) -> PolicyDecision {
        if policy.id == "no-match" {
            return PolicyDecision::Deny {
                reason: "no matching policy".to_owned(),
                policy_ref: "no-match".to_owned(),
            };
        }
        // Empty permitted_backends is handled by select_backend returning
        // PreCallError::NoPermittedBackend, which produces a typed 4xx.
        PolicyDecision::Allow
    }
}
