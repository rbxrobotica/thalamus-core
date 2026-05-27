use std::net::SocketAddr;

use thalamus_core::{BackendHandle, BackendType, Budget, ContextGrant, Policy, RiskLevel};

/// Server configuration loaded from a JSON file.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub listen: SocketAddr,
    pub policies: Vec<Policy>,
}

impl ServerConfig {
    pub fn listen_addr(&self) -> SocketAddr {
        self.listen
    }
}

/// Intermediate JSON shape for the config file.
#[derive(serde::Deserialize)]
struct ConfigFile {
    listen: String,
    policies: Vec<PolicyFile>,
}

#[derive(serde::Deserialize)]
struct PolicyFile {
    id: String,
    tenant: String,
    product: String,
    workflow: String,
    permitted_backends: Vec<BackendHandleFile>,
    max_tokens: u32,
    max_latency_ms: u64,
    context_grants: Vec<ContextGrant>,
    redaction_rules: Vec<thalamus_core::RedactionRule>,
    audit_required: bool,
    risk_threshold: RiskLevel,
}

#[derive(serde::Deserialize)]
struct BackendHandleFile {
    id: String,
    backend_type: String,
}

impl From<BackendHandleFile> for BackendHandle {
    fn from(f: BackendHandleFile) -> Self {
        let backend_type = match f.backend_type.as_str() {
            "Model" => BackendType::Model,
            "Tool" => BackendType::Tool,
            "McpServer" => BackendType::McpServer,
            "A2AAgent" => BackendType::A2AAgent,
            other => BackendType::Custom(other.to_owned()),
        };
        BackendHandle {
            id: f.id,
            backend_type,
        }
    }
}

impl From<PolicyFile> for Policy {
    fn from(f: PolicyFile) -> Self {
        Policy {
            id: f.id,
            tenant: f.tenant,
            product: f.product,
            workflow: f.workflow,
            permitted_backends: f.permitted_backends.into_iter().map(Into::into).collect(),
            budget: Budget {
                max_tokens: f.max_tokens,
                max_latency_ms: f.max_latency_ms,
            },
            context_grants: f.context_grants,
            redaction_rules: f.redaction_rules,
            audit_required: f.audit_required,
            risk_threshold: f.risk_threshold,
        }
    }
}

pub fn load_config(path: &str) -> Result<ServerConfig, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let file: ConfigFile = serde_json::from_str(&content)?;
    let listen: SocketAddr = file.listen.parse()?;
    let policies = file.policies.into_iter().map(Into::into).collect();
    Ok(ServerConfig { listen, policies })
}
