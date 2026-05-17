use thalamus_core::{BackendResponse, EvalPort, Policy};

/// Logging eval stub: logs evaluation submissions, returns a placeholder ref.
pub struct LoggingEvalPort;

impl EvalPort for LoggingEvalPort {
    fn submit(&self, response: &BackendResponse, policy: &Policy) -> String {
        tracing::info!(policy_id = %policy.id, tokens = ?response.tokens_used, "eval_submit");
        format!("eval-stub-{}", policy.id)
    }
}
