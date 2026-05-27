pub mod config;
pub mod error;

use std::time::Duration;

use thalamus_core::{BackendPort, BackendResponse, Envelope};

use config::AdapterConfig;
use error::AdapterError;

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

        let parsed: ChatCompletionsResponse = response
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

impl BackendPort for LiteLLMAdapter {
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
