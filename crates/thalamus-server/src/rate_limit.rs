//! Fixed-window rate limiting for the governed `/rbx/v1/*` surface (§3
//! security: limits by user and client app; session/project scoping arrives
//! when calls carry those ids). Configured by `THALAMUS_RBX_RATE_LIMIT`
//! (requests per minute per key; `0`/`off` disables; default 120).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_secs(60);
const DEFAULT_LIMIT_PER_MINUTE: u32 = 120;

pub struct RateLimiter {
    limit: u32,
    state: Mutex<HashMap<String, (Instant, u32)>>,
}

impl RateLimiter {
    pub fn new(limit: u32) -> Self {
        Self {
            limit,
            state: Mutex::new(HashMap::new()),
        }
    }

    /// Build from `THALAMUS_RBX_RATE_LIMIT`. `None` = disabled.
    pub fn from_env() -> Option<Self> {
        match std::env::var("THALAMUS_RBX_RATE_LIMIT") {
            Err(_) => Some(Self::new(DEFAULT_LIMIT_PER_MINUTE)),
            Ok(raw) => match raw.trim().to_ascii_lowercase().as_str() {
                "0" | "off" | "false" | "no" => None,
                other => Some(Self::new(other.parse().unwrap_or(DEFAULT_LIMIT_PER_MINUTE))),
            },
        }
    }

    /// Count one request against every key; if any key is over its window
    /// limit, refuse with seconds-until-reset.
    pub fn check(&self, keys: &[String]) -> Result<(), u64> {
        let now = Instant::now();
        let mut state = self.state.lock().unwrap();
        // Evict stale windows opportunistically so the map stays bounded.
        if state.len() > 4096 {
            state.retain(|_, (start, _)| now.duration_since(*start) < WINDOW);
        }
        let mut retry_after = None;
        for key in keys {
            let entry = state.entry(key.clone()).or_insert((now, 0));
            if now.duration_since(entry.0) >= WINDOW {
                *entry = (now, 0);
            }
            entry.1 += 1;
            if entry.1 > self.limit {
                let elapsed = now.duration_since(entry.0);
                let remaining = WINDOW.saturating_sub(elapsed).as_secs().max(1);
                retry_after = Some(retry_after.map_or(remaining, |r: u64| r.max(remaining)));
            }
        }
        match retry_after {
            None => Ok(()),
            Some(secs) => Err(secs),
        }
    }
}

/// axum middleware: enforce limits per verified caller (subject + client
/// app). Runs inside the credential middleware so the caller is known.
pub async fn enforce(
    limiter: Option<std::sync::Arc<RateLimiter>>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    let Some(limiter) = limiter else {
        return next.run(req).await;
    };
    let mut keys = Vec::new();
    if let Some(caller) = req.extensions().get::<crate::auth::VerifiedCaller>() {
        if let Some(subject) = &caller.subject {
            keys.push(format!("sub:{subject}"));
        }
        if let Some(app) = &caller.client_app_id {
            keys.push(format!("app:{app}"));
        }
    }
    if keys.is_empty() {
        keys.push("anon".to_owned());
    }
    match limiter.check(&keys) {
        Ok(()) => next.run(req).await,
        Err(retry_after_secs) => {
            let body = serde_json::json!({
                "error": {
                    "code": "rate_limited",
                    "message": "rate limit exceeded for caller",
                    "retryable": true,
                    "retry_after_seconds": retry_after_secs,
                }
            });
            (
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                [(
                    axum::http::header::RETRY_AFTER,
                    retry_after_secs.to_string(),
                )],
                axum::Json(body),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_after_limit_within_window() {
        let limiter = RateLimiter::new(3);
        let keys = vec!["sub:leandro".to_owned()];
        for _ in 0..3 {
            assert!(limiter.check(&keys).is_ok());
        }
        let retry = limiter.check(&keys).expect_err("4th request refused");
        assert!((1..=60).contains(&retry));
    }

    #[test]
    fn keys_are_independent() {
        let limiter = RateLimiter::new(1);
        assert!(limiter.check(&["sub:a".to_owned()]).is_ok());
        assert!(limiter.check(&["sub:b".to_owned()]).is_ok());
        assert!(limiter.check(&["sub:a".to_owned()]).is_err());
    }
}
