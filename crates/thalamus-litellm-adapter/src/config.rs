use std::collections::HashMap;
use std::time::Duration;

/// Configuration for the LiteLLM adapter.
///
/// The `endpoint` is the base URL of the in-cluster LiteLLM proxy
/// (e.g. `http://llm-gateway.llm-gateway.svc.cluster.local:4000`).
/// The `model_map` translates Thalamus backend handle IDs to LiteLLM
/// model names. Unmapped IDs are passed through verbatim.
#[derive(Debug, Clone)]
pub struct AdapterConfig {
    pub endpoint: String,
    pub model_map: HashMap<String, String>,
    pub timeout: Duration,
}

impl AdapterConfig {
    /// Default in-cluster endpoint.
    pub fn default_endpoint() -> String {
        "http://llm-gateway.llm-gateway.svc.cluster.local:4000".to_owned()
    }

    /// Resolve a Thalamus backend handle ID to a LiteLLM model name.
    /// Falls back to the handle ID if no mapping exists.
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
        }
    }
}
