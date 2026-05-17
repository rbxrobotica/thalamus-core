use std::collections::HashMap;
use std::time::Duration;

/// Configuration for the Agentgateway adapter.
///
/// `endpoint` is the base URL of the Agentgateway LLM routing surface
/// (e.g. `http://agentgateway.agentgateway.svc.cluster.local:8080`).
/// `model_map` translates Thalamus backend handle IDs to model names
/// recognized by Agentgateway. Unmapped IDs are passed through verbatim.
/// `auth_header` is an optional Authorization header value (e.g. "Bearer <token>").
#[derive(Debug, Clone)]
pub struct AdapterConfig {
    pub endpoint: String,
    pub model_map: HashMap<String, String>,
    pub timeout: Duration,
    pub auth_header: Option<String>,
}

impl AdapterConfig {
    pub fn default_endpoint() -> String {
        "http://agentgateway.agentgateway.svc.cluster.local:8080".to_owned()
    }

    pub fn resolve_model(&self, backend_handle_id: &str) -> String {
        self.model_map
            .get(backend_handle_id)
            .cloned()
            .unwrap_or_else(|| backend_handle_id.to_owned())
    }
}

impl Default for AdapterConfig {
    fn default() -> Self {
        Self {
            endpoint: Self::default_endpoint(),
            model_map: HashMap::new(),
            timeout: Duration::from_secs(30),
            auth_header: None,
        }
    }
}
