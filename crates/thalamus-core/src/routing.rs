//! Route envelope and backend execution contract (master plan §3, slice 2).
//!
//! The [`RouteEnvelope`] is the ONLY authority on what the data plane may
//! touch: provider pool, region, data class, capability class, cost class and
//! timeout all travel inside it, and adapters must refuse execution that
//! would cross any constraint ([`BackendCallError::EnvelopeViolation`]).
//! Adapter-specific execution plans (resolved model, wire URL, headers) are
//! internal to each adapter and never leak into the domain.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::domain::Envelope;

/// Routing constraints for one backend execution. Built by the control plane
/// from policy + envelope; carried opaquely through the data plane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteEnvelope {
    pub envelope: Envelope,
    /// Policy-level model alias (never a provider wire model id).
    pub model_alias: String,
    /// Permitted provider pools. Empty = the adapter's single default pool.
    #[serde(default)]
    pub provider_pool: Vec<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub data_class: Option<String>,
    #[serde(default)]
    pub capability_class: Option<String>,
    #[serde(default)]
    pub cost_class: Option<String>,
    /// Wall-clock budget for the backend call.
    pub timeout_ms: u64,
}

impl RouteEnvelope {
    /// Standard construction from an envelope: alias = backend handle id,
    /// timeout = policy budget latency. Constraint fields default to
    /// unconstrained until policy carries them (§3 pending decisions).
    pub fn from_envelope(envelope: &Envelope) -> Self {
        Self {
            model_alias: envelope.backend_handle.id.clone(),
            timeout_ms: envelope.budget.max_latency_ms,
            provider_pool: Vec::new(),
            region: None,
            data_class: None,
            capability_class: None,
            cost_class: None,
            envelope: envelope.clone(),
        }
    }
}

/// Token usage reported by a backend. All fields optional: streaming and
/// failures may only know partial usage.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendUsage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

/// Successful backend execution result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendExecution {
    pub content: String,
    pub usage: BackendUsage,
    pub latency_ms: u64,
    /// Adapter-specific metadata (provider, wire model, request id, ...).
    /// Opaque to the domain; audited, never interpreted.
    pub backend_metadata: serde_json::Value,
}

/// Typed backend execution failure. Timeout and cancellation carry partial
/// usage so budgets can account for interrupted streams.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackendCallError {
    Timeout {
        partial_usage: BackendUsage,
    },
    Cancelled {
        partial_usage: BackendUsage,
    },
    RateLimited {
        retry_after_ms: Option<u64>,
    },
    Unavailable {
        detail: String,
    },
    MalformedResponse {
        detail: String,
    },
    /// The execution would cross a route-envelope constraint (provider,
    /// region, data class, capability class, cost class, model alias). Never
    /// retried; always audited.
    EnvelopeViolation {
        constraint: String,
        detail: String,
    },
}

impl std::fmt::Display for BackendCallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout { .. } => write!(f, "backend call timed out"),
            Self::Cancelled { .. } => write!(f, "backend call cancelled"),
            Self::RateLimited { .. } => write!(f, "backend rate limited"),
            Self::Unavailable { detail } => write!(f, "backend unavailable: {detail}"),
            Self::MalformedResponse { detail } => write!(f, "malformed backend response: {detail}"),
            Self::EnvelopeViolation { constraint, detail } => {
                write!(f, "route envelope violation ({constraint}): {detail}")
            }
        }
    }
}

impl BackendCallError {
    /// Stable machine code for typed-error responses and audit records.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Timeout { .. } => "backend_timeout",
            Self::Cancelled { .. } => "backend_cancelled",
            Self::RateLimited { .. } => "backend_rate_limited",
            Self::Unavailable { .. } => "backend_unavailable",
            Self::MalformedResponse { .. } => "backend_malformed_response",
            Self::EnvelopeViolation { .. } => "envelope_violation",
        }
    }
}

/// Cooperative cancellation token. Sync adapters check it at execution
/// boundaries; streaming adapters (later slice) check it between chunks.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    cancelled: Arc<AtomicBool>,
}

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_token_flips_once_cancelled() {
        let token = CancelToken::new();
        assert!(!token.is_cancelled());
        let clone = token.clone();
        clone.cancel();
        assert!(token.is_cancelled(), "clones share cancellation state");
    }

    #[test]
    fn backend_call_error_codes_are_stable() {
        assert_eq!(
            BackendCallError::Timeout {
                partial_usage: BackendUsage::default()
            }
            .code(),
            "backend_timeout"
        );
        assert_eq!(
            BackendCallError::EnvelopeViolation {
                constraint: "provider_pool".into(),
                detail: "x".into()
            }
            .code(),
            "envelope_violation"
        );
    }
}
