pub mod config;
pub mod error;

use std::time::Duration;

use thalamus_core::{
    BackendCallError, BackendExecution, BackendPort, BackendResponse, BackendUsage, CancelToken,
    EmbeddingError, EmbeddingPort, EmbeddingRequest, EmbeddingResponse, Envelope, RouteEnvelope,
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
    /// Structured `chat.completions.v1` payload for run-bound calls. When
    /// present the wire request is built from it (allowlisted fields only)
    /// instead of wrapping `prompt` in a single user message.
    chat_payload: Option<serde_json::Value>,
    /// Policy budget in tokens; clamps any client-supplied max_tokens.
    budget_max_tokens: u32,
}

/// Build the wire body for a chat-structured call: only contract fields the
/// approved client uses (messages, tools, tool_choice, max_tokens) cross the
/// wire; the model is always the resolved wire model, never client-supplied.
fn chat_request_body(plan: &BackendExecutionPlan, stream: bool) -> serde_json::Value {
    let payload = plan.chat_payload.as_ref().expect("chat plan has payload");
    let mut body = serde_json::json!({
        "model": plan.wire_model,
        "messages": payload.get("messages").cloned().unwrap_or_else(|| serde_json::json!([])),
    });
    let obj = body.as_object_mut().expect("body is an object");
    if let Some(tools) = payload.get("tools") {
        obj.insert("tools".to_owned(), tools.clone());
    }
    if let Some(tool_choice) = payload.get("tool_choice") {
        obj.insert("tool_choice".to_owned(), tool_choice.clone());
    }
    let requested = payload.get("max_tokens").and_then(|v| v.as_u64());
    let budget = u64::from(plan.budget_max_tokens);
    let effective = match requested {
        Some(req) if budget > 0 => req.min(budget),
        Some(req) => req,
        None if budget > 0 => budget,
        None => 0,
    };
    if effective > 0 {
        obj.insert("max_tokens".to_owned(), serde_json::json!(effective));
    }
    if stream {
        obj.insert("stream".to_owned(), serde_json::json!(true));
        obj.insert(
            "stream_options".to_owned(),
            serde_json::json!({ "include_usage": true }),
        );
    }
    body
}

/// Parse an OpenAI-shape usage object (`prompt_tokens` / `completion_tokens`
/// / `total_tokens`) from a JSON value.
fn parse_usage_value(usage: Option<&serde_json::Value>) -> Option<BackendUsage> {
    let u = usage?.as_object()?;
    let field = |name: &str| u.get(name).and_then(|v| v.as_u64()).map(|t| t as u32);
    Some(BackendUsage {
        prompt_tokens: field("prompt_tokens"),
        completion_tokens: field("completion_tokens"),
        total_tokens: field("total_tokens"),
    })
}

/// Incrementally assembled tool call from streamed argument deltas.
#[derive(Default)]
struct StreamedToolCall {
    id: String,
    name: String,
    arguments: String,
}

/// Map a ureq transport/status error to the typed backend error.
fn map_wire_error(e: &ureq::Error) -> BackendCallError {
    match e {
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
    }
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

        let mut request = self
            .agent
            .post(&url)
            .header("x-trace-id", envelope.trace_id.0.to_string())
            .header("x-audit-id", envelope.audit_id.0.to_string());
        if let Some(key) = &self.config.api_key {
            request = request.header("authorization", format!("Bearer {key}"));
        }
        let response = request
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
            chat_payload: route.envelope.chat_payload.clone(),
            budget_max_tokens: route.envelope.budget.max_tokens,
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
        let request_body = if plan.chat_payload.is_some() {
            chat_request_body(plan, false)
        } else {
            serde_json::json!({
                "model": plan.wire_model,
                "messages": [{ "role": "user", "content": plan.prompt }],
            })
        };
        let start = std::time::Instant::now();
        let mut request = agent
            .post(&plan.url)
            .header("x-trace-id", &plan.trace_id)
            .header("x-audit-id", &plan.audit_id);
        if let Some(key) = &self.config.api_key {
            request = request.header("authorization", format!("Bearer {key}"));
        }
        let response = request
            .send_json(&request_body)
            .map_err(|e| map_wire_error(&e))?;

        let parsed: serde_json::Value =
            response
                .into_body()
                .read_json()
                .map_err(|e| BackendCallError::MalformedResponse {
                    detail: e.to_string(),
                })?;

        let message = parsed
            .pointer("/choices/0/message")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let text = message
            .get("content")
            .and_then(|c| c.as_str())
            .unwrap_or_default()
            .to_owned();
        let tool_calls = message.get("tool_calls").filter(|t| !t.is_null()).cloned();
        // A chat response may legitimately carry only tool calls; the audited
        // content then holds their serialization so post-call validates a
        // non-empty payload. Legacy prompt calls keep requiring text.
        let content = if !text.is_empty() {
            text
        } else if let Some(ref calls) = tool_calls {
            calls.to_string()
        } else {
            return Err(BackendCallError::MalformedResponse {
                detail: "empty content in response".to_owned(),
            });
        };

        let usage = parse_usage_value(parsed.get("usage")).unwrap_or_default();
        let finish_reason = parsed
            .pointer("/choices/0/finish_reason")
            .and_then(|v| v.as_str())
            .map(str::to_owned);

        Ok(BackendExecution {
            content,
            usage,
            latency_ms: start.elapsed().as_millis() as u64,
            backend_metadata: serde_json::json!({
                "provider_pool": PROVIDER_POOL,
                "wire_model": plan.wire_model,
                "endpoint": self.config.endpoint,
                "finish_reason": finish_reason,
                "message": if plan.chat_payload.is_some() { message } else { serde_json::Value::Null },
            }),
        })
    }
}

impl EmbeddingPort for LiteLLMAdapter {
    fn embed(&self, request: &EmbeddingRequest) -> Result<EmbeddingResponse, EmbeddingError> {
        if request.input.is_empty() || request.input.iter().any(|value| value.trim().is_empty()) {
            return Err(EmbeddingError::InvalidRequest {
                detail: "input must contain one or more non-empty strings".to_owned(),
            });
        }
        let url = format!("{}/v1/embeddings", self.config.endpoint);
        let wire_model = self.config.resolve_model(&request.model_alias);
        let mut wire = self
            .agent
            .post(&url)
            .header("x-trace-id", request.trace_id.0.to_string())
            .header("x-audit-id", request.audit_id.0.to_string());
        if let Some(key) = &self.config.api_key {
            wire = wire.header("authorization", format!("Bearer {key}"));
        }
        let response = wire
            .send_json(serde_json::json!({ "model": wire_model, "input": request.input }))
            .map_err(|error| EmbeddingError::Unavailable {
                detail: error.to_string(),
            })?;
        let payload: serde_json::Value = response.into_body().read_json().map_err(|error| {
            EmbeddingError::MalformedResponse {
                detail: error.to_string(),
            }
        })?;
        let mut rows = payload
            .get("data")
            .and_then(|value| value.as_array())
            .cloned()
            .ok_or_else(|| EmbeddingError::MalformedResponse {
                detail: "missing data array".to_owned(),
            })?;
        rows.sort_by_key(|row| {
            row.get("index")
                .and_then(|value| value.as_u64())
                .unwrap_or(u64::MAX)
        });
        let vectors: Result<Vec<Vec<f32>>, EmbeddingError> = rows
            .into_iter()
            .map(|row| {
                row.get("embedding")
                    .and_then(|value| value.as_array())
                    .ok_or_else(|| EmbeddingError::MalformedResponse {
                        detail: "embedding row missing vector".to_owned(),
                    })
                    .and_then(|vector| {
                        vector
                            .iter()
                            .map(|value| {
                                value.as_f64().map(|number| number as f32).ok_or_else(|| {
                                    EmbeddingError::MalformedResponse {
                                        detail: "embedding vector contains non-number".to_owned(),
                                    }
                                })
                            })
                            .collect()
                    })
            })
            .collect();
        let vectors = vectors?;
        if vectors.len() != request.input.len() || vectors.iter().any(Vec::is_empty) {
            return Err(EmbeddingError::MalformedResponse {
                detail: "embedding response count or dimensions do not match request".to_owned(),
            });
        }
        let dimensions = vectors[0].len();
        if vectors.iter().any(|vector| vector.len() != dimensions) {
            return Err(EmbeddingError::MalformedResponse {
                detail: "embedding dimensions differ".to_owned(),
            });
        }
        Ok(EmbeddingResponse {
            model_alias: request.model_alias.clone(),
            vectors,
            provider_metadata: serde_json::json!({
                "provider_pool": PROVIDER_POOL,
                "wire_model": wire_model,
                "dimensions": dimensions,
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

    /// Streaming execution over the OpenAI-compatible SSE wire
    /// (`"stream": true`). Deltas are forwarded to `sink` as they arrive;
    /// the cancel token is checked between chunks and aborts the stream
    /// mid-flight with whatever usage is known ([`BackendCallError::Cancelled`]).
    fn execute_streaming(
        &self,
        route: &RouteEnvelope,
        cancel: &CancelToken,
        sink: &mut dyn FnMut(&str),
    ) -> Result<BackendExecution, BackendCallError> {
        use std::io::{BufRead, BufReader};

        if cancel.is_cancelled() {
            return Err(BackendCallError::Cancelled {
                partial_usage: BackendUsage::default(),
            });
        }
        let plan = self.plan(route)?;
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(plan.timeout))
            .build()
            .into();
        let request_body = serde_json::json!({
            "model": plan.wire_model,
            "messages": [{ "role": "user", "content": plan.prompt }],
            "stream": true,
            "stream_options": { "include_usage": true },
        });
        let start = std::time::Instant::now();
        let mut request = agent
            .post(&plan.url)
            .header("x-trace-id", &plan.trace_id)
            .header("x-audit-id", &plan.audit_id)
            // Streaming must not negotiate compression: the proxy's gzip
            // path buffers the whole SSE stream and delivers it as one
            // terminal burst (observed live 2026-07-19), destroying
            // incremental delivery. Identity keeps deltas flowing.
            .header("accept-encoding", "identity");
        if let Some(key) = &self.config.api_key {
            request = request.header("authorization", format!("Bearer {key}"));
        }
        let response = request.send_json(&request_body).map_err(|e| match &e {
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

        let reader = BufReader::new(response.into_body().into_reader());
        let mut content = String::new();
        let mut usage = BackendUsage::default();
        let mut chunks = 0u32;
        for line in reader.lines() {
            if cancel.is_cancelled() {
                return Err(BackendCallError::Cancelled {
                    partial_usage: usage,
                });
            }
            let line = line.map_err(|e| {
                if content.is_empty() {
                    BackendCallError::Unavailable {
                        detail: e.to_string(),
                    }
                } else {
                    // Stream broke mid-flight: surface as timeout-class with
                    // whatever usage is known.
                    BackendCallError::Timeout {
                        partial_usage: usage.clone(),
                    }
                }
            })?;
            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };
            if data.trim() == "[DONE]" {
                break;
            }
            let parsed: StreamChunk = match serde_json::from_str(data) {
                Ok(c) => c,
                Err(e) => {
                    return Err(BackendCallError::MalformedResponse {
                        detail: format!("bad SSE chunk: {e}"),
                    })
                }
            };
            if let Some(u) = parsed.usage {
                usage = BackendUsage {
                    prompt_tokens: u.prompt_tokens.map(|t| t as u32),
                    completion_tokens: u.completion_tokens.map(|t| t as u32),
                    total_tokens: Some(u.total_tokens as u32),
                };
            }
            if let Some(delta) = parsed
                .choices
                .first()
                .and_then(|c| c.delta.as_ref())
                .and_then(|d| d.content.as_deref())
            {
                if !delta.is_empty() {
                    chunks += 1;
                    content.push_str(delta);
                    sink(delta);
                }
            }
        }
        if content.is_empty() {
            return Err(BackendCallError::MalformedResponse {
                detail: "stream produced no content".to_owned(),
            });
        }
        Ok(BackendExecution {
            content,
            usage,
            latency_ms: start.elapsed().as_millis() as u64,
            backend_metadata: serde_json::json!({
                "provider_pool": PROVIDER_POOL,
                "wire_model": plan.wire_model,
                "endpoint": self.config.endpoint,
                "streamed": true,
                "chunks": chunks,
            }),
        })
    }

    /// Chat-structured streaming for run-bound calls: every SSE chunk is
    /// forwarded verbatim as parsed JSON (content deltas, tool_call argument
    /// deltas, finish_reason, usage), while text and tool calls are assembled
    /// for the audited final content. Cancellation is checked between chunks.
    fn execute_streaming_chat(
        &self,
        route: &RouteEnvelope,
        cancel: &CancelToken,
        on_chunk: &mut dyn FnMut(&serde_json::Value),
    ) -> Result<BackendExecution, BackendCallError> {
        use std::io::{BufRead, BufReader};

        if cancel.is_cancelled() {
            return Err(BackendCallError::Cancelled {
                partial_usage: BackendUsage::default(),
            });
        }
        let plan = self.plan(route)?;
        if plan.chat_payload.is_none() {
            // Text-only envelope: fall back to the wrapping default.
            let mut forward = |delta: &str| {
                on_chunk(&serde_json::json!({
                    "choices": [{ "index": 0, "delta": { "content": delta }, "finish_reason": null }]
                }));
            };
            return self.execute_streaming(route, cancel, &mut forward);
        }

        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(plan.timeout))
            .build()
            .into();
        let request_body = chat_request_body(&plan, true);
        let start = std::time::Instant::now();
        let mut request = agent
            .post(&plan.url)
            .header("x-trace-id", &plan.trace_id)
            .header("x-audit-id", &plan.audit_id)
            // Identity encoding on streaming: gzip through the proxy
            // buffers the entire SSE stream into one terminal burst.
            .header("accept-encoding", "identity");
        if let Some(key) = &self.config.api_key {
            request = request.header("authorization", format!("Bearer {key}"));
        }
        let response = request
            .send_json(&request_body)
            .map_err(|e| map_wire_error(&e))?;

        let reader = BufReader::new(response.into_body().into_reader());
        let mut content = String::new();
        let mut tool_calls: Vec<StreamedToolCall> = Vec::new();
        let mut usage = BackendUsage::default();
        let mut finish_reason: Option<String> = None;
        let mut chunks = 0u32;
        for line in reader.lines() {
            if cancel.is_cancelled() {
                return Err(BackendCallError::Cancelled {
                    partial_usage: usage,
                });
            }
            let line = line.map_err(|e| {
                if chunks == 0 {
                    BackendCallError::Unavailable {
                        detail: e.to_string(),
                    }
                } else {
                    BackendCallError::Timeout {
                        partial_usage: usage.clone(),
                    }
                }
            })?;
            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };
            if data.trim() == "[DONE]" {
                break;
            }
            let parsed: serde_json::Value =
                serde_json::from_str(data).map_err(|e| BackendCallError::MalformedResponse {
                    detail: format!("bad SSE chunk: {e}"),
                })?;
            chunks += 1;
            if let Some(u) = parse_usage_value(parsed.get("usage")) {
                usage = u;
            }
            if let Some(reason) = parsed
                .pointer("/choices/0/finish_reason")
                .and_then(|v| v.as_str())
            {
                finish_reason = Some(reason.to_owned());
            }
            if let Some(delta) = parsed.pointer("/choices/0/delta") {
                if let Some(text) = delta.get("content").and_then(|c| c.as_str()) {
                    content.push_str(text);
                }
                if let Some(calls) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                    for call in calls {
                        let index =
                            call.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                        while tool_calls.len() <= index {
                            tool_calls.push(StreamedToolCall::default());
                        }
                        let slot = &mut tool_calls[index];
                        if let Some(id) = call.get("id").and_then(|v| v.as_str()) {
                            slot.id = id.to_owned();
                        }
                        if let Some(name) = call.pointer("/function/name").and_then(|v| v.as_str())
                        {
                            slot.name = name.to_owned();
                        }
                        if let Some(args) =
                            call.pointer("/function/arguments").and_then(|v| v.as_str())
                        {
                            slot.arguments.push_str(args);
                        }
                    }
                }
            }
            on_chunk(&parsed);
        }

        let assembled_calls: Vec<serde_json::Value> = tool_calls
            .iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id,
                    "type": "function",
                    "function": { "name": c.name, "arguments": c.arguments },
                })
            })
            .collect();
        // Pure tool-call responses have no text; audit their serialization as
        // the call content so post-call validates a non-empty payload.
        let final_content = if !content.is_empty() {
            content
        } else if !assembled_calls.is_empty() {
            serde_json::Value::Array(assembled_calls.clone()).to_string()
        } else {
            return Err(BackendCallError::MalformedResponse {
                detail: "stream produced no content".to_owned(),
            });
        };

        Ok(BackendExecution {
            content: final_content,
            usage,
            latency_ms: start.elapsed().as_millis() as u64,
            backend_metadata: serde_json::json!({
                "provider_pool": PROVIDER_POOL,
                "wire_model": plan.wire_model,
                "endpoint": self.config.endpoint,
                "streamed": true,
                "chunks": chunks,
                "finish_reason": finish_reason,
                "tool_calls": if assembled_calls.is_empty() {
                    serde_json::Value::Null
                } else {
                    serde_json::Value::Array(assembled_calls)
                },
            }),
        })
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

// === Streaming wire types (OpenAI-compatible SSE) ===

#[derive(serde::Deserialize)]
struct StreamChunk {
    #[serde(default)]
    choices: Vec<StreamChoice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(serde::Deserialize)]
struct StreamChoice {
    #[serde(default)]
    delta: Option<StreamDelta>,
}

#[derive(serde::Deserialize)]
struct StreamDelta {
    #[serde(default)]
    content: Option<String>,
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
            api_key: None,
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
            chat_payload: None,
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
            api_key: None,
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
            chat_payload: None,
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

#[cfg(test)]
mod streaming_tests {
    use super::*;
    use std::collections::HashMap;
    use thalamus_core::RouteEnvelope;

    fn adapter_for(url: &str) -> LiteLLMAdapter {
        LiteLLMAdapter::new(AdapterConfig {
            endpoint: url.to_owned(),
            model_map: HashMap::new(),
            timeout: Duration::from_secs(5),
            api_key: None,
        })
    }

    fn route() -> RouteEnvelope {
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
            chat_payload: None,
        };
        RouteEnvelope::from_envelope(&envelope)
    }

    fn sse_body() -> String {
        [
            r#"data: {"choices":[{"delta":{"content":"Hel"}}]}"#,
            "",
            r#"data: {"choices":[{"delta":{"content":"lo "}}]}"#,
            "",
            r#"data: {"choices":[{"delta":{"content":"mundo"}}]}"#,
            "",
            r#"data: {"choices":[],"usage":{"prompt_tokens":5,"completion_tokens":3,"total_tokens":8}}"#,
            "",
            "data: [DONE]",
            "",
        ]
        .join("\n")
    }

    #[test]
    fn streaming_delivers_deltas_and_final_usage() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body())
            .create();
        let adapter = adapter_for(&server.url());
        let mut deltas: Vec<String> = Vec::new();
        let exec = adapter
            .execute_streaming(&route(), &CancelToken::new(), &mut |d| {
                deltas.push(d.to_owned())
            })
            .expect("stream succeeds");
        assert_eq!(deltas, vec!["Hel", "lo ", "mundo"]);
        assert_eq!(exec.content, "Hello mundo");
        assert_eq!(exec.usage.total_tokens, Some(8));
        assert_eq!(exec.backend_metadata["streamed"], true);
        assert_eq!(exec.backend_metadata["chunks"], 3);
    }

    /// Streaming requests must pin identity encoding: a gzip-negotiated SSE
    /// response is buffered whole by the proxy and arrives as one terminal
    /// burst (observed live 2026-07-19), which breaks incremental delivery.
    /// The mock only matches when the header is present.
    #[test]
    fn streaming_requests_pin_identity_encoding() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .match_header("accept-encoding", "identity")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .with_body(sse_body())
            .expect(1)
            .create();
        let adapter = adapter_for(&server.url());
        adapter
            .execute_streaming(&route(), &CancelToken::new(), &mut |_| {})
            .expect("stream succeeds only when identity encoding is pinned");
        mock.assert();
    }

    #[test]
    fn streaming_cancels_mid_flight_between_chunks() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        // Slow SSE server: two chunks with a pause, so cancellation lands
        // between them. Proves the token aborts a stream already in flight.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = socket.read(&mut buf); // consume request
            let chunk1 = "data: {\"choices\":[{\"delta\":{\"content\":\"first\"}}]}\n\n";
            let chunk2 =
                "data: {\"choices\":[{\"delta\":{\"content\":\"second\"}}]}\n\ndata: [DONE]\n\n";
            let _ = write!(
                socket,
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\n\r\n",
                chunk1.len() + chunk2.len()
            );
            let _ = socket.write_all(chunk1.as_bytes());
            let _ = socket.flush();
            std::thread::sleep(Duration::from_millis(500));
            let _ = socket.write_all(chunk2.as_bytes());
        });

        let adapter = adapter_for(&format!("http://{addr}"));
        let cancel = CancelToken::new();
        let cancel_in_sink = cancel.clone();
        let mut deltas: Vec<String> = Vec::new();
        let err = adapter
            .execute_streaming(&route(), &cancel, &mut |d| {
                deltas.push(d.to_owned());
                cancel_in_sink.cancel(); // simulate client disconnect after first delta
            })
            .expect_err("must cancel mid-flight");
        assert_eq!(deltas, vec!["first"], "only the first chunk was delivered");
        assert!(
            matches!(err, BackendCallError::Cancelled { .. }),
            "got {err:?}"
        );
    }
}

#[cfg(test)]
mod auth_tests {
    use super::*;
    use std::collections::HashMap;
    use thalamus_core::RouteEnvelope;

    #[test]
    fn execute_sends_bearer_key_when_configured() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("POST", "/v1/chat/completions")
            .match_header("authorization", "Bearer sk-test-master")
            .with_status(200)
            .with_body(
                serde_json::json!({
                    "choices": [{ "message": { "content": "ok" } }],
                    "usage": { "total_tokens": 2 }
                })
                .to_string(),
            )
            .create();
        let adapter = LiteLLMAdapter::new(AdapterConfig {
            endpoint: server.url(),
            model_map: HashMap::new(),
            timeout: Duration::from_secs(5),
            api_key: Some("sk-test-master".to_owned()),
        });
        let envelope = {
            use thalamus_core::*;
            Envelope {
                trace_id: TraceId(uuid::Uuid::new_v4()),
                audit_id: AuditId(uuid::Uuid::new_v4()),
                backend_handle: BackendHandle {
                    id: "glm-5.2".to_owned(),
                    backend_type: BackendType::Model,
                },
                prompt: "hi".to_owned(),
                authorized_context: vec![],
                redaction_applied: false,
                policy_ref: "t".to_owned(),
                budget: Budget {
                    max_tokens: 10,
                    max_latency_ms: 5000,
                },
                chat_payload: None,
            }
        };
        let route = RouteEnvelope::from_envelope(&envelope);
        // The mock only matches with the header; success proves it was sent.
        let exec = adapter
            .execute(&route, &CancelToken::new())
            .expect("authorized call succeeds");
        assert_eq!(exec.content, "ok");
    }
}
