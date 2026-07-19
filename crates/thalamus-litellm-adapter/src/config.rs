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
    /// Bearer key for the LiteLLM proxy (`LITELLM_API_KEY`). DB-less LiteLLM
    /// authenticates every call with the master key; the key never leaves
    /// this adapter (callers authenticate to Thalamus, not to LiteLLM).
    pub api_key: Option<String>,
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

    /// Parse a model map from its JSON object form, e.g.
    /// `{"coding.standard": "anthropic/glm-5.2"}`. This is the wire format of
    /// the `THALAMUS_MODEL_MAP` environment variable: institutional aliases
    /// stay a Thalamus concern (policy `permitted_backends` + this map);
    /// clients never see gateway model names.
    pub fn parse_model_map(json: &str) -> Result<HashMap<String, String>, String> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("model map is not valid JSON: {e}"))?;
        let Some(object) = value.as_object() else {
            return Err("model map must be a JSON object of alias -> model".to_owned());
        };
        let mut map = HashMap::new();
        for (alias, model) in object {
            let Some(model) = model.as_str() else {
                return Err(format!("model map entry '{alias}' must map to a string"));
            };
            if alias.trim().is_empty() || model.trim().is_empty() {
                return Err(format!(
                    "model map entry '{alias}' has an empty alias or model"
                ));
            }
            map.insert(alias.clone(), model.to_owned());
        }
        Ok(map)
    }
}

impl Default for AdapterConfig {
    fn default() -> Self {
        Self {
            endpoint: Self::default_endpoint(),
            model_map: HashMap::new(),
            timeout: Duration::from_secs(30),
            api_key: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_model_map_maps_aliases() {
        let map =
            AdapterConfig::parse_model_map(r#"{"coding.standard": "anthropic/glm-5.2"}"#).unwrap();
        let config = AdapterConfig {
            model_map: map,
            ..Default::default()
        };
        assert_eq!(config.resolve_model("coding.standard"), "anthropic/glm-5.2");
        // Unmapped IDs still pass through verbatim.
        assert_eq!(config.resolve_model("glm-test"), "glm-test");
    }

    #[test]
    fn parse_model_map_rejects_non_object() {
        assert!(AdapterConfig::parse_model_map("[]").is_err());
        assert!(AdapterConfig::parse_model_map("not json").is_err());
    }

    #[test]
    fn parse_model_map_rejects_non_string_or_empty_entries() {
        assert!(AdapterConfig::parse_model_map(r#"{"a": 1}"#).is_err());
        assert!(AdapterConfig::parse_model_map(r#"{"a": ""}"#).is_err());
        assert!(AdapterConfig::parse_model_map(r#"{" ": "m"}"#).is_err());
    }
}
