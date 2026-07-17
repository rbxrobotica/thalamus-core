//! Inbound credential validation for the gated `/rbx/v1/*` surface
//! (ADR-0101). Mounted only when `THALAMUS_RBX_API=on`; the legacy `/v1/*`
//! routes never pass through it.
//!
//! # First slice
//!
//! Credentials are opaque session tokens issued by `rbx-token-service`,
//! validated by introspection ([`OpaqueIntrospectionVerifier`]). The
//! [`CredentialVerifier`] trait is the swappable seam: once RBX Identity
//! brokers ZITADEL token-exchange, a local JWT verifier (against ZITADEL JWKS,
//! reusing `rbx-token-verifier`) replaces the introspection impl with **no
//! change to Thalamus's session model**.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::Request;
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};

/// A caller validated from a presented credential. Field names mirror the
/// `rbx-token-service` introspection response (loose HTTP contract: Thalamus
/// does not depend on any rbx-identity crate).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedCaller {
    pub active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jti: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audience: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_app_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delegated_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mediator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    /// Introspection inactive reason (e.g. `unknown`, `expired`, `revoked`)
    /// when `active` is false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Why a credential was rejected at the boundary.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("missing or malformed bearer credential")]
    MissingCredential,
    #[error("credential rejected by introspection: {0}")]
    Inactive(String),
    #[error("introspection endpoint unreachable: {0}")]
    IntrospectionUnavailable(String),
}

impl AuthError {
    /// Map to a P0 typed-error response (P0 standard section 11 codes).
    pub fn to_response(&self) -> Response {
        let code = match self {
            Self::Inactive(reason) if matches!(reason.as_str(), "expired" | "revoked") => {
                "session_expired"
            }
            _ => "policy_denied",
        };
        let body = serde_json::json!({
            "error": {
                "code": code,
                "message": self.to_string(),
                "retryable": false,
            }
        });
        (StatusCode::UNAUTHORIZED, Json(body)).into_response()
    }
}

/// Validates a presented bearer credential into a [`VerifiedCaller`].
#[async_trait]
pub trait CredentialVerifier: Send + Sync {
    async fn verify(&self, bearer: &str) -> Result<VerifiedCaller, AuthError>;

    /// Liveness of the verifier's upstream (used by /readyz). Default: always
    /// reachable (static/local verifiers have no upstream).
    async fn probe(&self) -> bool {
        true
    }
}

/// Validates opaque credentials by calling the `rbx-token-service`
/// `/v1/delegation/introspect` endpoint. Uses ureq (sync) off the async runtime
/// via `spawn_blocking`, matching the existing BackendPort execution pattern.
#[derive(Debug, Clone)]
pub struct OpaqueIntrospectionVerifier {
    introspection_url: String,
}

impl OpaqueIntrospectionVerifier {
    pub fn new(introspection_url: String) -> Self {
        Self { introspection_url }
    }
}

#[async_trait]
impl CredentialVerifier for OpaqueIntrospectionVerifier {
    async fn verify(&self, bearer: &str) -> Result<VerifiedCaller, AuthError> {
        let url = self.introspection_url.clone();
        let credential = bearer.to_owned();
        let caller = tokio::task::spawn_blocking(move || {
            let response = ureq::post(&url)
                .send_json(serde_json::json!({ "credential": credential }))
                .map_err(|err| AuthError::IntrospectionUnavailable(err.to_string()))?;
            let caller: VerifiedCaller = response
                .into_body()
                .read_json()
                .map_err(|err| AuthError::IntrospectionUnavailable(err.to_string()))?;
            Ok::<VerifiedCaller, AuthError>(caller)
        })
        .await
        .map_err(|err| AuthError::IntrospectionUnavailable(err.to_string()))??;

        if caller.active {
            Ok(caller)
        } else {
            Err(AuthError::Inactive(
                caller.reason.unwrap_or_else(|| "inactive".to_owned()),
            ))
        }
    }

    /// Reachability probe: any HTTP response from the introspection endpoint
    /// (including 4xx for the dummy credential) counts as reachable; only a
    /// transport failure does not.
    async fn probe(&self) -> bool {
        let url = self.introspection_url.clone();
        tokio::task::spawn_blocking(move || {
            match ureq::post(&url).send_json(serde_json::json!({ "credential": "probe" })) {
                Ok(_) => true,
                Err(ureq::Error::StatusCode(_)) => true,
                Err(_) => false,
            }
        })
        .await
        .unwrap_or(false)
    }
}

/// In-memory verifier for tests and local development: returns a configured
/// [`VerifiedCaller`] for known tokens, or a fixed inactive reason. Public so
/// integration tests (a separate crate) can construct it.
#[allow(dead_code)] // exercised by integration tests; unused in the binary
#[derive(Debug, Default, Clone)]
pub struct StaticCredentialVerifier {
    valid: HashMap<String, VerifiedCaller>,
    inactive_reason: Option<String>,
}

#[allow(dead_code)] // exercised by integration tests; unused in the binary
impl StaticCredentialVerifier {
    /// Accept `token` and return `caller` for it.
    pub fn with_valid(token: &str, caller: VerifiedCaller) -> Self {
        let mut valid = HashMap::new();
        valid.insert(token.to_owned(), caller);
        Self {
            valid,
            inactive_reason: None,
        }
    }

    /// Reject every credential with this inactive reason (e.g. `expired`,
    /// `revoked`, `missing_entitlement`).
    pub fn always_inactive(reason: &str) -> Self {
        Self {
            valid: HashMap::new(),
            inactive_reason: Some(reason.to_owned()),
        }
    }
}

#[async_trait]
impl CredentialVerifier for StaticCredentialVerifier {
    async fn verify(&self, bearer: &str) -> Result<VerifiedCaller, AuthError> {
        if let Some(reason) = &self.inactive_reason {
            return Err(AuthError::Inactive(reason.clone()));
        }
        self.valid
            .get(bearer)
            .cloned()
            .ok_or_else(|| AuthError::Inactive("unknown".to_owned()))
    }
}

/// axum middleware: require a valid bearer credential on `/rbx/v1/*`. On
/// success the [`VerifiedCaller`] is inserted as a request extension for the
/// handler; on failure a P0 typed-error 401 is returned without invoking the
/// handler (the session/run is never created).
pub async fn require_credential(
    verifier: Arc<dyn CredentialVerifier + Send + Sync>,
    mut req: Request,
    next: Next,
) -> Response {
    let bearer = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer ").map(str::to_owned));
    let Some(bearer) = bearer else {
        return AuthError::MissingCredential.to_response();
    };
    match verifier.verify(&bearer).await {
        Ok(caller) => {
            req.extensions_mut().insert(caller);
            next.run(req).await
        }
        Err(err) => err.to_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_caller() -> VerifiedCaller {
        VerifiedCaller {
            active: true,
            subject: Some("ldamasio@gmail.com".to_owned()),
            session_id: Some("00000000-0000-0000-0000-000000000001".to_owned()),
            jti: Some("jti-1".to_owned()),
            audience: vec!["thalamus".to_owned()],
            scopes: vec!["kulinaryos:access".to_owned()],
            client_app_id: Some("robson-code".to_owned()),
            actor: None,
            delegated_by: None,
            mediator: Some("rbx-token-service".to_owned()),
            expires_at: None,
            reason: None,
        }
    }

    #[tokio::test]
    async fn static_verifier_accepts_known_token() {
        let v = StaticCredentialVerifier::with_valid("rbxsess_ok", active_caller());
        let caller = v.verify("rbxsess_ok").await.unwrap();
        assert_eq!(caller.subject.as_deref(), Some("ldamasio@gmail.com"));
        assert!(matches!(
            v.verify("rbxsess_other").await,
            Err(AuthError::Inactive(reason)) if reason == "unknown"
        ));
    }

    #[tokio::test]
    async fn static_verifier_always_inactive() {
        let v = StaticCredentialVerifier::always_inactive("revoked");
        assert!(matches!(
            v.verify("anything").await,
            Err(AuthError::Inactive(reason)) if reason == "revoked"
        ));
    }

    #[tokio::test]
    async fn auth_error_maps_expired_and_revoked_to_session_expired() {
        assert_eq!(
            response_code(&AuthError::Inactive("expired".into())).await,
            "session_expired"
        );
        assert_eq!(
            response_code(&AuthError::Inactive("revoked".into())).await,
            "session_expired"
        );
        assert_eq!(
            response_code(&AuthError::Inactive("missing_entitlement".into())).await,
            "policy_denied"
        );
        assert_eq!(
            response_code(&AuthError::MissingCredential).await,
            "policy_denied"
        );
    }

    async fn response_code(err: &AuthError) -> String {
        let resp = err.to_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        v["error"]["code"].as_str().unwrap().to_owned()
    }
}
