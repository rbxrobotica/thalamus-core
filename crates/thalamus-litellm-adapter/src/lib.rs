pub mod config;
pub mod error;

use std::time::Duration;

use thalamus_core::{
    BackendCallError, BackendExecution, BackendPort, BackendResponse, BackendUsage, CancelToken,
    Envelope, RouteEnvelope,
};

use config::AdapterConfig;
use error::AdapterError;

/// Provider pool identity of this adapter. A route envelope with a non-empty
/// `provider_pool` that does not include this pool is refused before any wire
/// call ([`BackendCallError::EnvelopeViolation`]).
pub const PROVIDER_POOL: &str = "litellm";

/// Adapter-internal execution plan (§3): everything resolved from the route
/// envelope before the wire call. Never leaves this crate.
struct BackendExecutionPlan {
    url: String,
    wire_model: String,
    timeout: Duration,
    trace_id: String,
    audit_id: String,
    prompt: String,
}

/// BackendPort adapter for the LiteLLM OpenAI-compatible data plane.
///
/// Translates Thalamus Envelopes into LiteLLM /v1/chat/completions requests
/// and parses responses into BackendResponses. This is the ONLY place where
/// HTTP client and provider/wire knowledge may exist.
pub struct LiteLLMAdapter {
    config: AdapterConfig,
    agent: ureq::Agent,
}

impl LiteLLMAdapter {
    pub fn new(config: AdapterConfig) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(config.timeout))
            .build()
            .into();
        Self { config, agent }
    }

    /// Internal call returning a typed error for testability.
    pub fn call_internal(&self, envelope: &Envelope) -> Result<BackendResponse, AdapterError> {
        let model = self.config.resolve_model(&envelope.backend_handle.id);
        let url = format!("{}/v1/chat/completions", self.config.endpoint);

        let request_body = ChatCompletionsRequest {
            model,
            messages: vec![Message {
                role: "user".to_owned(),
                content: envelope.prompt.clone(),
            }],
        };

        let start = std::time::Instant::now();

        let response = self
            .agent
            .post(&url)
            .header("x-trace-id", envelope.trace_id.0.to_string())
            .header("x-audit-id", envelope.audit_id.0.to_string())
            .send_json(&request_body)
            .map_err(|e| map_ureq_error(e, start.elapsed()))?;

        let parsed: ChatCompletionsResponse =
            response
                .into_body()
                .read_json()
                .map_err(|e| AdapterError::MalformedResponse {
                    reason: e.to_string(),
                })?;

        let content = parsed
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default();

        if content.is_empty() {
            return Err(AdapterError::MalformedResponse {
                reason: "empty content in response".to_owned(),
            });
        }

        let tokens_used = parsed.usage.map(|u| u.total_tokens as u32);
        let latency_ms = Some(start.elapsed().as_millis() as u64);

        Ok(BackendResponse {
            content,
            tokens_used,
            latency_ms,
        })
    }
}

impl LiteLLMAdapter {
    /// Validate the route envelope against this adapter's identity and build
    /// the execution plan. Refusals never reach the wire.
    fn plan(&self, route: &RouteEnvelope) -> Result<BackendExecutionPlan, BackendCallError> {
        if !route.provider_pool.is_empty()
            && !route.provider_pool.iter().any(|p| p == PROVIDER_POOL)
        {
            return Err(BackendCallError::EnvelopeViolation {
                constraint: "provider_pool".to_owned(),
                detail: format!(
                    "adapter pool '{PROVIDER_POOL}' not in permitted pools {:?}",
                    route.provider_pool
                ),
            });
        }
        if route.model_alias != route.envelope.backend_handle.id {
            return Err(BackendCallError::EnvelopeViolation {
                constraint: "model_alias".to_owned(),
                detail: format!(
                    "route model alias '{}' does not match envelope backend handle '{}'",
                    route.model_alias, route.envelope.backend_handle.id
                ),
            });
        }
        Ok(BackendExecutionPlan {
            url: format!("{}/v1/chat/completions", self.config.endpoint),
            wire_model: self.config.resolve_model(&route.model_alias),
            timeout: Duration::from_millis(route.timeout_ms),
            trace_id: route.envelope.trace_id.0.to_string(),
            audit_id: route.envelope.audit_id.0.to_string(),
            prompt: route.envelope.prompt.clone(),
        })
    }

    fn execute_plan(
        &self,
        plan: &BackendExecutionPlan,
    ) -> Result<BackendExecution, BackendCallError> {
        // One-shot agent so the route envelope's timeout governs this request
        // (the shared agent keeps the config-level default for legacy call()).
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(plan.timeout))
            .build()
            .into();
        let request_body = ChatCompletionsRequest {
            model: plan.wire_model.clone(),
            messages: vec![Message {
                role: "user".to_owned(),
                content: plan.prompt.clone(),
            }],
        };
        let start = std::time::Instant::now();
        let response = agent
            .post(&plan.url)
            .header("x-trace-id", &plan.trace_id)
            .header("x-audit-id", &plan.audit_id)
            .send_json(&request_body)
            .map_err(|e| match &e {
                ureq::Error::Timeout(_) => BackendCallError::Timeout {
                    partial_usage: BackendUsage::default(),
                },
                ureq::Error::StatusCode(429) => BackendCallError::RateLimited {
                    retry_after_ms: None,
                },
                ureq::Error::StatusCode(code) => BackendCallError::Unavailable {
                    detail: format!("backend returned status {code}"),
                },
                _ => BackendCallError::Unavailable {
                    detail: e.to_string(),
                },
            })?;

        let parsed: ChatCompletionsResponse =
            response
                .into_body()
                .read_json()
                .map_err(|e| BackendCallError::MalformedResponse {
                    detail: e.to_string(),
                })?;

        let content = parsed
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default();
        if content.is_empty() {
            return Err(BackendCallError::MalformedResponse {
                detail: "empty content in response".to_owned(),
            });
        }

        let usage = parsed
            .usage
            .map(|u| BackendUsage {
                prompt_tokens: u.prompt_tokens.map(|t| t as u32),
                completion_tokens: u.completion_tokens.map(|t| t as u32),
                total_tokens: Some(u.total_tokens as u32),
            })
            .unwrap_or_default();

        Ok(BackendExecution {
            content,
            usage,
            latency_ms: start.elapsed().as_millis() as u64,
            backend_metadata: serde_json::json!({
                "provider_pool": PROVIDER_POOL,
                "wire_model": plan.wire_model,
                "endpoint": self.config.endpoint,
            }),
        })
    }
}

impl BackendPort for LiteLLMAdapter {
    /// §3 execution path: constraint validation, per-request timeout from the
    /// route envelope, typed errors, usage and backend metadata. Cooperative
    /// cancellation is checked at execution boundaries (mid-flight streaming
    /// cancel lands with the streaming slice).
    fn execute(
        &self,
        route: &RouteEnvelope,
        cancel: &CancelToken,
    ) -> Result<BackendExecution, BackendCallError> {
        if cancel.is_cancelled() {
            return Err(BackendCallError::Cancelled {
                partial_usage: BackendUsage::default(),
            });
        }
        let plan = self.plan(route)?;
        let result = self.execute_plan(&plan);
        if cancel.is_cancelled() {
            return Err(BackendCallError::Cancelled {
                partial_usage: result.as_ref().map(|r| r.usage.clone()).unwrap_or_default(),
            });
        }
        result
    }

    fn call(&self, envelope: &Envelope) -> BackendResponse {
        match self.call_internal(envelope) {
            Ok(resp) => resp,
            Err(e) => {
                tracing::error!(
                    audit_id = %envelope.audit_id.0,
                    trace_id = %envelope.trace_id.0,
                    error = %e,
                    "litellm_adapter_error"
                );
                BackendResponse {
                    content: String::new(),
                    tokens_used: None,
                    latency_ms: None,
                }
            }
        }
    }
}

fn map_ureq_error(e: ureq::Error, elapsed: Duration) -> AdapterError {
    match &e {
        ureq::Error::StatusCode(code) => AdapterError::ServerError {
            status: *code,
            body: e.to_string(),
        },
        ureq::Error::Timeout(_) => AdapterError::Timeout { duration: elapsed },
        _ => AdapterError::Connection {
            detail: e.to_string(),
        },
    }
}

// === Wire types (OpenAI-compatible) ===

#[derive(serde::Serialize)]
struct ChatCompletionsRequest {
    model: String,
    messages: Vec<Message>,
}

#[derive(serde::Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(serde::Deserialize)]
struct ChatCompletionsResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(serde::Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(serde::Deserialize)]
struct ChoiceMessage {
    content: Option<String>,
}

#[derive(serde::Deserialize)]
struct Usage {
    total_tokens: u64,
    #[serde(default)]
    prompt_tokens: Option<u64>,
    #[serde(default)]
    completion_tokens: Option<u64>,
}

// === Unit tests ===

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_adapter(url: &str) -> LiteLLMAdapter {
        LiteLLMAdapter::new(AdapterConfig {
            endpoint: url.to_owned(),
            model_map: HashMap::new(),
            timeout: Duration::from_secs(5),
        })
    }

    fn test_envelope() -> Envelope {
        use thalamus_core::*;
        Envelope {
            trace_id: TraceId(uuid::Uuid::new_v4()),
            audit_id: AuditId(uuid::Uuid::new_v4()),
            backend_handle: BackendHandle {
                id: "test-model".to_owned(),
                backend_type: BackendType::Model,
            },
            prompt: "Hello".to_owned(),
            authorized_context: vec![],
            redaction_applied: false,
            policy_ref: "test".to_owned(),
            budget: Budget {
                max_tokens: 1000,
                max_latency_ms: 5000,
            },
        }
    }

    #[test]
    fn adapter_returns_error_on_connection_failure() {
        let adapter = test_adapter("http://127.0.0.1:1");
        let envelope = test_envelope();
        let result = adapter.call_internal(&envelope);
        assert!(result.is_err());
        match result.unwrap_err() {
            AdapterError::Connection { .. } => {}
            other => panic!("expected Connection error, got: {other}"),
        }
    }

    #[test]
    fn adapter_returns_empty_on_connection_failure_via_trait() {
        let adapter = test_adapter("http://127.0.0.1:1");
        let envelope = test_envelope();
        let response = adapter.call(&envelope);
        assert!(response.content.is_empty());
        assert!(response.tokens_used.is_none());
    }
}

#[cfg(test)]
mod execute_tests {
    use super::*;
    use std::collections::HashMap;
    use thalamus_core::RouteEnvelope;

    fn adapter_for(url: &str) -> LiteLLMAdapter {
        let mut model_map = HashMap::new();
        model_map.insert("glm-5.2".to_owned(), "anthropic/glm-5.2".to_owned());
        LiteLLMAdapter::new(AdapterConfig {
            endpoint: url.to_owned(),
            model_map,
            timeout: Duration::from_secs(5),
        })
    }

    fn route_for(url: &str) -> RouteEnvelope {
        use thalamus_core::*;
        let envelope = Envelope {
            trace_id: TraceId(uuid::Uuid::new_v4()),
            audit_id: AuditId(uuid::Uuid::new_v4()),
            backend_handle: BackendHandle {
                id: "glm-5.2".to_owned(),
                backend_type: BackendType::Model,
            },
            prompt: "Hello".to_owned(),
            authorized_context: vec![],
            redaction_applied: false,
            policy_ref: "test".to_owned(),
            budget: Budget {
                max_tokens: 1000,
                max_latency_ms: 5000,
            },
        };
        let _ = url;
        RouteEnvelope::from_envelope(&envelope)
    }

    #[test]
    fn execute_returns_usage_and_metadata() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(200)
            .with_body(
                serde_json::json!({
                    "choices": [{ "message": { "content": "resposta" } }],
                    "usage": { "prompt_tokens": 12, "completion_tokens": 30, "total_tokens": 42 }
                })
                .to_string(),
            )
            .create();
        let adapter = adapter_for(&server.url());
        let route = route_for(&server.url());
        let exec = adapter
            .execute(&route, &CancelToken::new())
            .expect("execute succeeds");
        assert_eq!(exec.content, "resposta");
        assert_eq!(exec.usage.total_tokens, Some(42));
        assert_eq!(exec.usage.prompt_tokens, Some(12));
        assert_eq!(exec.usage.completion_tokens, Some(30));
        assert_eq!(exec.backend_metadata["provider_pool"], "litellm");
        assert_eq!(exec.backend_metadata["wire_model"], "anthropic/glm-5.2");
    }

    #[test]
    fn execute_refuses_provider_pool_crossing_before_wire() {
        // Unroutable endpoint proves the refusal happens before any wire call.
        let adapter = adapter_for("http://127.0.0.1:1");
        let mut route = route_for("http://127.0.0.1:1");
        route.provider_pool = vec!["bedrock".to_owned()];
        let err = adapter
            .execute(&route, &CancelToken::new())
            .expect_err("must refuse");
        assert!(
            matches!(err, BackendCallError::EnvelopeViolation { ref constraint, .. } if constraint == "provider_pool"),
            "got {err:?}"
        );
    }

    #[test]
    fn execute_refuses_model_alias_mismatch() {
        let adapter = adapter_for("http://127.0.0.1:1");
        let mut route = route_for("http://127.0.0.1:1");
        route.model_alias = "other-model".to_owned();
        let err = adapter
            .execute(&route, &CancelToken::new())
            .expect_err("must refuse");
        assert!(
            matches!(err, BackendCallError::EnvelopeViolation { ref constraint, .. } if constraint == "model_alias"),
            "got {err:?}"
        );
    }

    #[test]
    fn execute_maps_429_to_rate_limited_and_5xx_to_unavailable() {
        let mut server = mockito::Server::new();
        let mock_429 = server
            .mock("POST", "/v1/chat/completions")
            .with_status(429)
            .create();
        let adapter = adapter_for(&server.url());
        let route = route_for(&server.url());
        let err = adapter
            .execute(&route, &CancelToken::new())
            .expect_err("429");
        assert!(
            matches!(err, BackendCallError::RateLimited { .. }),
            "got {err:?}"
        );
        mock_429.remove();

        let _mock_500 = server
            .mock("POST", "/v1/chat/completions")
            .with_status(500)
            .create();
        let err = adapter
            .execute(&route, &CancelToken::new())
            .expect_err("500");
        assert!(
            matches!(err, BackendCallError::Unavailable { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn execute_respects_cancellation_before_dispatch() {
        let adapter = adapter_for("http://127.0.0.1:1");
        let route = route_for("http://127.0.0.1:1");
        let cancel = CancelToken::new();
        cancel.cancel();
        let err = adapter.execute(&route, &cancel).expect_err("cancelled");
        assert!(
            matches!(err, BackendCallError::Cancelled { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn execute_times_out_within_route_budget() {
        let adapter = adapter_for("http://10.255.255.1:9"); // non-routable: connect stalls
        let mut route = route_for("");
        route.timeout_ms = 200;
        let start = std::time::Instant::now();
        let err = adapter
            .execute(&route, &CancelToken::new())
            .expect_err("must fail");
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "route timeout must bound the call, took {:?}",
            start.elapsed()
        );
        assert!(
            matches!(
                err,
                BackendCallError::Timeout { .. } | BackendCallError::Unavailable { .. }
            ),
            "got {err:?}"
        );
    }
}
