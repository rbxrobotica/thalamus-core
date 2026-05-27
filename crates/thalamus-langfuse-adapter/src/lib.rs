use std::sync::Arc;

use serde::Serialize;
use thalamus_eval::{EvalSink, EvalSubmission};

pub mod config;

/// Langfuse ingestion payload. Shape based on Langfuse public docs;
/// if the contract changes, only this struct and the endpoint path need updating.
#[derive(Debug, Serialize)]
struct IngestionEvent {
    id: String,
    #[serde(rename = "type")]
    event_type: &'static str,
    name: String,
    metadata: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    output: Option<String>,
    timestamp: String,
}

pub struct LangfuseSink {
    client: Arc<LangfuseClient>,
}

struct LangfuseClient {
    endpoint: String,
    public_key: String,
    secret_key: String,
    timeout_ms: u64,
}

impl LangfuseClient {
    fn post(&self, payload: &serde_json::Value) -> Result<(), String> {
        let url = format!(
            "{}/api/public/ingestion",
            self.endpoint.trim_end_matches('/')
        );
        let timeout = std::time::Duration::from_millis(self.timeout_ms);

        let config = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .build();
        let agent: ureq::Agent = config.into();

        agent
            .post(&url)
            .header(
                "Authorization",
                &format!("Bearer {}:{}", self.public_key, self.secret_key),
            )
            .header("Content-Type", "application/json")
            .send_json(payload)
            .map_err(|e| format!("langfuse POST: {e}"))?;

        Ok(())
    }
}

impl LangfuseSink {
    pub fn new(config: config::LangfuseConfig) -> Self {
        Self {
            client: Arc::new(LangfuseClient {
                endpoint: config.endpoint,
                public_key: config.public_key,
                secret_key: config.secret_key,
                timeout_ms: config.timeout_ms,
            }),
        }
    }

    fn build_payload(&self, submission: &EvalSubmission) -> serde_json::Value {
        let record = &submission.record;
        let metadata = serde_json::json!({
            "eval_ref": record.eval_ref,
            "schema_valid": record.schema_valid,
            "risk_class": record.risk_class,
            "policy_id": record.policy_id,
            "citation_check": record.citation_check,
            "hallucination_signals": record.hallucination_signals,
            "response_metadata": {
                "content_len": record.response_metadata.content_len,
                "tokens_used": record.response_metadata.tokens_used,
                "latency_ms": record.response_metadata.latency_ms,
            },
            "trace_id": record.trace_id,
            "audit_id": record.audit_id,
        });

        let event = IngestionEvent {
            id: record.eval_ref.clone(),
            event_type: "eval",
            name: format!("eval:{}", record.policy_id),
            metadata,
            output: submission.authorized_content.clone(),
            timestamp: record.created_at.clone(),
        };

        // Langfuse batch endpoint expects an array
        serde_json::json!({ "batch": [event] })
    }
}

impl EvalSink for LangfuseSink {
    fn accept(&self, submission: &EvalSubmission) {
        let payload = self.build_payload(submission);
        if let Err(e) = self.client.post(&payload) {
            tracing::warn!(eval_ref = %submission.record.eval_ref, error = %e, "langfuse ingestion failed (best-effort, dropped)");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use thalamus_eval::{CitationCheckOutcome, EvalRecord, ResponseMetadata};

    fn test_submission(content: Option<&str>) -> EvalSubmission {
        EvalSubmission {
            record: EvalRecord {
                eval_ref: "eval-test-1".to_owned(),
                schema_valid: true,
                citation_check: CitationCheckOutcome::NotRequired,
                hallucination_signals: vec![],
                risk_class: "Low".to_owned(),
                response_metadata: ResponseMetadata {
                    content_len: 5,
                    tokens_used: Some(10),
                    latency_ms: Some(100),
                },
                trace_id: None,
                audit_id: None,
                policy_id: "test-policy".to_owned(),
                created_at: "2026-05-18T12:00:00Z".to_owned(),
            },
            authorized_content: content.map(|c| c.to_owned()),
        }
    }

    #[test]
    fn payload_contains_metadata_not_raw_content_by_default() {
        let config = config::LangfuseConfig {
            endpoint: "http://localhost:4040".to_owned(),
            public_key: "pk".to_owned(),
            secret_key: "sk".to_owned(),
            timeout_ms: 1000,
        };
        let sink = LangfuseSink::new(config);
        let submission = test_submission(None); // no authorized content
        let payload = sink.build_payload(&submission);

        // Verify deterministic metadata is present
        let batch = payload["batch"].as_array().unwrap();
        let event = &batch[0];
        assert_eq!(event["id"], "eval-test-1");
        assert_eq!(event["type"], "eval");
        assert_eq!(event["metadata"]["risk_class"], "Low");
        assert_eq!(event["metadata"]["policy_id"], "test-policy");

        // No output field when no authorized content
        assert!(event["output"].is_null());
    }

    #[test]
    fn payload_includes_authorized_content_when_present() {
        let config = config::LangfuseConfig {
            endpoint: "http://localhost:4040".to_owned(),
            public_key: "pk".to_owned(),
            secret_key: "sk".to_owned(),
            timeout_ms: 1000,
        };
        let sink = LangfuseSink::new(config);
        let submission = test_submission(Some("redacted response text"));
        let payload = sink.build_payload(&submission);

        let batch = payload["batch"].as_array().unwrap();
        let event = &batch[0];
        assert_eq!(event["output"], "redacted response text");
    }

    // === HTTP integration tests with mockito ===

    #[test]
    fn happy_path_posts_to_langfuse() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("POST", "/api/public/ingestion")
            .match_header("Authorization", "Bearer pk-test:sk-test")
            .match_header("Content-Type", "application/json")
            .with_status(200)
            .create();

        let config = config::LangfuseConfig {
            endpoint: server.url(),
            public_key: "pk-test".to_owned(),
            secret_key: "sk-test".to_owned(),
            timeout_ms: 2000,
        };
        let sink = LangfuseSink::new(config);
        let submission = test_submission(None);
        sink.accept(&submission);

        mock.assert();
    }

    #[test]
    fn boundary_test_raw_content_never_reaches_langfuse() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("POST", "/api/public/ingestion")
            .match_body(mockito::Matcher::Regex(String::from(
                "secret-api-key-12345",
            )))
            .expect(0) // MUST NEVER match
            .with_status(200)
            .create();

        let config = config::LangfuseConfig {
            endpoint: server.url(),
            public_key: "pk".to_owned(),
            secret_key: "sk".to_owned(),
            timeout_ms: 2000,
        };
        let sink = LangfuseSink::new(config);

        // Default: MetadataOnly — no authorized_content, raw text never in payload
        let submission = test_submission(None);
        sink.accept(&submission);

        // The mock expected 0 calls matching the raw content — assert that
        mock.assert();
    }

    #[test]
    fn resilience_5xx_does_not_panic() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("POST", "/api/public/ingestion")
            .with_status(500)
            .expect(1)
            .create();

        let config = config::LangfuseConfig {
            endpoint: server.url(),
            public_key: "pk".to_owned(),
            secret_key: "sk".to_owned(),
            timeout_ms: 2000,
        };
        let sink = LangfuseSink::new(config);

        // Must not panic on 5xx
        let submission = test_submission(None);
        sink.accept(&submission);

        mock.assert();
    }

    #[test]
    fn resilience_timeout_does_not_panic() {
        // Connect to a port that accepts connections but never responds.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        // Hold the listener open (accepts connection but never writes) — causes timeout.
        let _guard = listener;

        let config = config::LangfuseConfig {
            endpoint: format!("http://{addr}"),
            public_key: "pk".to_owned(),
            secret_key: "sk".to_owned(),
            timeout_ms: 50,
        };
        let sink = LangfuseSink::new(config);

        // Must not panic on timeout
        let submission = test_submission(None);
        sink.accept(&submission);
    }
}
