/// Configuration for the Langfuse ingestion adapter.
pub struct LangfuseConfig {
    pub endpoint: String,
    pub public_key: String,
    pub secret_key: String,
    pub timeout_ms: u64,
}

impl LangfuseConfig {
    pub fn from_env() -> Self {
        Self {
            endpoint: std::env::var("LANGFUSE_PUBLIC_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:3000".to_owned()),
            public_key: std::env::var("LANGFUSE_PUBLIC_KEY")
                .unwrap_or_else(|_| "pk-placeholder".to_owned()),
            secret_key: std::env::var("LANGFUSE_SECRET_KEY")
                .unwrap_or_else(|_| "sk-placeholder".to_owned()),
            timeout_ms: std::env::var("LANGFUSE_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5000),
        }
    }
}
