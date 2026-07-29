use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt::{Debug, Formatter};
use std::time::Duration;

const REQUIRED_ENV: [&str; 4] = [
    "THALAMUS_RBX_MEMORY_URL",
    "THALAMUS_RBX_MEMORY_TOKEN",
    "THALAMUS_RAG_PACKAGE_ID",
    "THALAMUS_RAG_VISIBILITY",
];
const DEFAULT_TIMEOUT_MS: u64 = 2_000;
const MAX_TIMEOUT_MS: u64 = 10_000;
const MAX_RESPONSE_BYTES: u64 = 1_048_576;
const MAX_METADATA_BYTES: usize = 2_048;
const MAX_CONTENT_BYTES: usize = 65_536;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetrievalScope {
    pub package_id: String,
    pub visibility: String,
}

impl RetrievalScope {
    pub fn backend_id(&self) -> String {
        format!("rbx-memory:{}:{}", self.package_id, self.visibility)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetrievalPortRequest {
    pub query: String,
    pub limit: u16,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RetrievalHit {
    pub chunk_id: String,
    pub document_id: String,
    pub content: String,
    pub locale: String,
    pub source_uri: Option<String>,
    pub score: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RetrievalPortResponse {
    pub package_id: String,
    pub visibility: String,
    pub model_alias: String,
    pub trace_id: String,
    pub audit_id: String,
    pub hits: Vec<RetrievalHit>,
}

#[derive(Debug, thiserror::Error)]
pub enum RetrievalPortError {
    #[error("invalid retrieval configuration: {0}")]
    InvalidConfiguration(String),
    #[error("rbx-memory retrieval transport failed")]
    Transport,
    #[error("rbx-memory retrieval refused with HTTP {0}")]
    Refused(u16),
    #[error("invalid rbx-memory retrieval response: {0}")]
    InvalidResponse(String),
}

#[async_trait]
pub trait RetrievalPort: Send + Sync {
    fn scope(&self) -> &RetrievalScope;

    async fn retrieve(
        &self,
        request: RetrievalPortRequest,
    ) -> Result<RetrievalPortResponse, RetrievalPortError>;
}

#[derive(Clone)]
pub struct HttpRetrievalPort {
    endpoint: String,
    bearer_token: String,
    scope: RetrievalScope,
    timeout: Duration,
}

impl Debug for HttpRetrievalPort {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HttpRetrievalPort")
            .field("endpoint", &self.endpoint)
            .field("bearer_token", &"<redacted>")
            .field("scope", &self.scope)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl HttpRetrievalPort {
    pub fn new(
        base_url: &str,
        bearer_token: String,
        scope: RetrievalScope,
        timeout: Duration,
    ) -> Result<Self, RetrievalPortError> {
        let base_url = base_url.trim().trim_end_matches('/');
        let authority = base_url
            .strip_prefix("https://")
            .or_else(|| base_url.strip_prefix("http://"))
            .and_then(|remainder| remainder.split('/').next());
        if authority.is_none_or(|value| {
            value.is_empty()
                || value.contains('@')
                || value.chars().any(char::is_whitespace)
                || base_url.contains('?')
                || base_url.contains('#')
        }) {
            return Err(RetrievalPortError::InvalidConfiguration(
                "THALAMUS_RBX_MEMORY_URL must be an HTTP(S) service URL without credentials, query, or fragment"
                    .to_owned(),
            ));
        }
        if bearer_token.trim().is_empty() {
            return Err(RetrievalPortError::InvalidConfiguration(
                "THALAMUS_RBX_MEMORY_TOKEN must not be empty".to_owned(),
            ));
        }
        validate_scope(&scope)?;
        if timeout.is_zero() || timeout > Duration::from_millis(MAX_TIMEOUT_MS) {
            return Err(RetrievalPortError::InvalidConfiguration(format!(
                "THALAMUS_RAG_TIMEOUT_MS must be between 1 and {MAX_TIMEOUT_MS}"
            )));
        }

        Ok(Self {
            endpoint: format!("{base_url}/v1/retrieval"),
            bearer_token,
            scope,
            timeout,
        })
    }

    pub fn from_env() -> Result<Option<Self>, RetrievalPortError> {
        if REQUIRED_ENV.iter().all(|name| env::var(name).is_err()) {
            return Ok(None);
        }
        let missing = REQUIRED_ENV
            .iter()
            .copied()
            .filter(|name| env::var(name).is_err())
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(RetrievalPortError::InvalidConfiguration(format!(
                "missing environment variables: {}",
                missing.join(", ")
            )));
        }

        let timeout_ms = env::var("THALAMUS_RAG_TIMEOUT_MS")
            .ok()
            .map(|value| {
                value.parse::<u64>().map_err(|_| {
                    RetrievalPortError::InvalidConfiguration(
                        "THALAMUS_RAG_TIMEOUT_MS must be an integer".to_owned(),
                    )
                })
            })
            .transpose()?
            .unwrap_or(DEFAULT_TIMEOUT_MS);
        Self::new(
            &required_env("THALAMUS_RBX_MEMORY_URL")?,
            required_env("THALAMUS_RBX_MEMORY_TOKEN")?,
            RetrievalScope {
                package_id: required_env("THALAMUS_RAG_PACKAGE_ID")?,
                visibility: required_env("THALAMUS_RAG_VISIBILITY")?,
            },
            Duration::from_millis(timeout_ms),
        )
        .map(Some)
    }

    fn validate_response(
        &self,
        request: &RetrievalPortRequest,
        response: &RetrievalPortResponse,
    ) -> Result<(), RetrievalPortError> {
        if response.package_id != self.scope.package_id
            || response.visibility != self.scope.visibility
        {
            return Err(RetrievalPortError::InvalidResponse(
                "package_id or visibility escaped the configured scope".to_owned(),
            ));
        }
        if response.model_alias.trim().is_empty()
            || response.model_alias.len() > 128
            || response.trace_id.trim().is_empty()
            || response.trace_id.len() > 128
            || response.audit_id.trim().is_empty()
            || response.audit_id.len() > 128
        {
            return Err(RetrievalPortError::InvalidResponse(
                "model_alias, trace_id, and audit_id are required and bounded".to_owned(),
            ));
        }
        if response.hits.len() > usize::from(request.limit)
            || response.hits.iter().any(|hit| {
                hit.chunk_id.trim().is_empty()
                    || hit.chunk_id.len() > MAX_METADATA_BYTES
                    || hit.document_id.trim().is_empty()
                    || hit.document_id.len() > MAX_METADATA_BYTES
                    || hit.content.trim().is_empty()
                    || hit.content.len() > MAX_CONTENT_BYTES
                    || hit.locale.trim().is_empty()
                    || hit.locale.len() > 64
                    || hit
                        .source_uri
                        .as_ref()
                        .is_some_and(|value| value.len() > MAX_METADATA_BYTES)
                    || !hit.score.is_finite()
            })
        {
            return Err(RetrievalPortError::InvalidResponse(
                "retrieval hits were malformed or exceeded the requested limit".to_owned(),
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl RetrievalPort for HttpRetrievalPort {
    fn scope(&self) -> &RetrievalScope {
        &self.scope
    }

    async fn retrieve(
        &self,
        request: RetrievalPortRequest,
    ) -> Result<RetrievalPortResponse, RetrievalPortError> {
        let endpoint = self.endpoint.clone();
        let bearer_token = self.bearer_token.clone();
        let timeout = self.timeout;
        let wire_request = request.clone();
        let response = tokio::task::spawn_blocking(move || {
            let config = ureq::Agent::config_builder()
                .timeout_global(Some(timeout))
                .build();
            let agent: ureq::Agent = config.into();
            let mut response = agent
                .post(&endpoint)
                .header("Authorization", &format!("Bearer {bearer_token}"))
                .header("Content-Type", "application/json")
                .send_json(serde_json::json!({
                    "query": wire_request.query,
                    "limit": wire_request.limit,
                }))
                .map_err(|error| match error {
                    ureq::Error::StatusCode(status) => RetrievalPortError::Refused(status),
                    _ => RetrievalPortError::Transport,
                })?;
            response
                .body_mut()
                .with_config()
                .limit(MAX_RESPONSE_BYTES)
                .read_json::<RetrievalPortResponse>()
                .map_err(|_| {
                    RetrievalPortError::InvalidResponse(
                        "response body exceeded its limit or was not valid retrieval JSON"
                            .to_owned(),
                    )
                })
        })
        .await
        .map_err(|_| RetrievalPortError::Transport)??;
        self.validate_response(&request, &response)?;
        Ok(response)
    }
}

fn required_env(name: &'static str) -> Result<String, RetrievalPortError> {
    let value = env::var(name).map_err(|_| {
        RetrievalPortError::InvalidConfiguration(format!("missing environment variable: {name}"))
    })?;
    if value.trim().is_empty() {
        return Err(RetrievalPortError::InvalidConfiguration(format!(
            "environment variable {name} must not be empty"
        )));
    }
    Ok(value)
}

fn validate_scope(scope: &RetrievalScope) -> Result<(), RetrievalPortError> {
    let valid_id = |value: &str| {
        !value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    };
    if !valid_id(&scope.package_id) {
        return Err(RetrievalPortError::InvalidConfiguration(
            "THALAMUS_RAG_PACKAGE_ID is invalid".to_owned(),
        ));
    }
    if !matches!(
        scope.visibility.as_str(),
        "public" | "internal" | "restricted"
    ) {
        return Err(RetrievalPortError::InvalidConfiguration(
            "THALAMUS_RAG_VISIBILITY must be public, internal, or restricted".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_is_fail_closed_and_redacts_debug_token() {
        let port = HttpRetrievalPort::new(
            "https://memory.example",
            "super-secret".to_owned(),
            RetrievalScope {
                package_id: "rbx-rag-public-assistant".to_owned(),
                visibility: "public".to_owned(),
            },
            Duration::from_millis(500),
        )
        .unwrap();
        let debug = format!("{port:?}");
        assert!(!debug.contains("super-secret"));
        assert!(debug.contains("<redacted>"));
        for invalid_url in [
            "file:///tmp/memory",
            "https://user:password@memory.example",
            "https://memory.example?token=secret",
            "https://memory.example#fragment",
        ] {
            assert!(HttpRetrievalPort::new(
                invalid_url,
                "token".to_owned(),
                port.scope.clone(),
                Duration::from_millis(500),
            )
            .is_err());
        }
        assert!(HttpRetrievalPort::new(
            "https://memory.example",
            "token".to_owned(),
            RetrievalScope {
                package_id: "../private".to_owned(),
                visibility: "public".to_owned(),
            },
            Duration::from_millis(500),
        )
        .is_err());
    }

    #[tokio::test]
    async fn calls_only_the_scoped_rbx_memory_contract() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/retrieval")
            .match_header("authorization", "Bearer memory-token")
            .match_body(mockito::Matcher::Json(serde_json::json!({
                "query": "What is RBX?",
                "limit": 3
            })))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "package_id": "rbx-rag-public-assistant",
                    "visibility": "public",
                    "model_alias": "embedding-public",
                    "trace_id": "embedding-trace-1",
                    "audit_id": "embedding-audit-1",
                    "hits": [{
                        "chunk_id": "chunk-1",
                        "document_id": "doc-1",
                        "content": "RBX fact",
                        "locale": "en",
                        "source_uri": null,
                        "score": 0.9
                    }]
                })
                .to_string(),
            )
            .create_async()
            .await;
        let port = HttpRetrievalPort::new(
            &server.url(),
            "memory-token".to_owned(),
            RetrievalScope {
                package_id: "rbx-rag-public-assistant".to_owned(),
                visibility: "public".to_owned(),
            },
            Duration::from_secs(1),
        )
        .unwrap();

        let response = port
            .retrieve(RetrievalPortRequest {
                query: "What is RBX?".to_owned(),
                limit: 3,
            })
            .await
            .unwrap();

        assert_eq!(response.package_id, "rbx-rag-public-assistant");
        assert_eq!(response.visibility, "public");
        assert_eq!(response.hits.len(), 1);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn refuses_scope_escape_in_memory_response() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/retrieval")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "package_id": "rbx-rag-sales-enablement",
                    "visibility": "internal",
                    "model_alias": "embedding-public",
                    "trace_id": "embedding-trace-1",
                    "audit_id": "embedding-audit-1",
                    "hits": []
                })
                .to_string(),
            )
            .create_async()
            .await;
        let port = HttpRetrievalPort::new(
            &server.url(),
            "memory-token".to_owned(),
            RetrievalScope {
                package_id: "rbx-rag-public-assistant".to_owned(),
                visibility: "public".to_owned(),
            },
            Duration::from_secs(1),
        )
        .unwrap();

        let error = port
            .retrieve(RetrievalPortRequest {
                query: "query".to_owned(),
                limit: 3,
            })
            .await
            .unwrap_err();

        assert!(matches!(error, RetrievalPortError::InvalidResponse(_)));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn refuses_oversized_memory_response_before_deserialization() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/retrieval")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("x".repeat(MAX_RESPONSE_BYTES as usize + 1))
            .create_async()
            .await;
        let port = HttpRetrievalPort::new(
            &server.url(),
            "memory-token".to_owned(),
            RetrievalScope {
                package_id: "rbx-rag-public-assistant".to_owned(),
                visibility: "public".to_owned(),
            },
            Duration::from_secs(1),
        )
        .unwrap();

        let error = port
            .retrieve(RetrievalPortRequest {
                query: "query".to_owned(),
                limit: 1,
            })
            .await
            .unwrap_err();

        assert!(matches!(error, RetrievalPortError::InvalidResponse(_)));
        mock.assert_async().await;
    }

    #[test]
    fn refuses_oversized_hit_fields_after_deserialization() {
        let port = HttpRetrievalPort::new(
            "https://memory.example",
            "memory-token".to_owned(),
            RetrievalScope {
                package_id: "rbx-rag-public-assistant".to_owned(),
                visibility: "public".to_owned(),
            },
            Duration::from_secs(1),
        )
        .unwrap();
        let response = RetrievalPortResponse {
            package_id: "rbx-rag-public-assistant".to_owned(),
            visibility: "public".to_owned(),
            model_alias: "embedding-public".to_owned(),
            trace_id: "embedding-trace-1".to_owned(),
            audit_id: "embedding-audit-1".to_owned(),
            hits: vec![RetrievalHit {
                chunk_id: "chunk-1".to_owned(),
                document_id: "doc-1".to_owned(),
                content: "x".repeat(MAX_CONTENT_BYTES + 1),
                locale: "en".to_owned(),
                source_uri: None,
                score: 0.9,
            }],
        };

        assert!(matches!(
            port.validate_response(
                &RetrievalPortRequest {
                    query: "query".to_owned(),
                    limit: 1,
                },
                &response,
            ),
            Err(RetrievalPortError::InvalidResponse(_))
        ));
    }
}
