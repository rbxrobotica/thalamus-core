mod app;
mod auth;
mod config;
mod ports;
mod rate_limit;
mod redact;
mod routes;

#[cfg(any(feature = "litellm", feature = "agentgateway", feature = "langfuse"))]
use std::sync::Arc;
#[cfg(any(feature = "litellm", feature = "agentgateway"))]
use thalamus_core::BackendPort;

#[cfg(feature = "langfuse")]
fn default_trace_exporter_sink() -> Arc<dyn thalamus_eval::EvalSink + Send + Sync> {
    ports::trace_exporter::trace_exporter_sink_from_env()
        .unwrap_or_else(|| Arc::new(thalamus_eval::NoOpSink))
}

/// LiteLLM adapter config from the environment: `LITELLM_ENDPOINT`,
/// `LITELLM_API_KEY` and `THALAMUS_MODEL_MAP` (JSON object mapping
/// institutional aliases to gateway model names, e.g.
/// `{"coding.standard": "anthropic/glm-5.2"}`). A malformed map is a startup
/// error: running with silently-dropped alias resolution would surface as
/// unknown-model failures at call time instead of a clear boot refusal.
#[cfg(feature = "litellm")]
fn litellm_adapter_config_from_env() -> thalamus_litellm_adapter::config::AdapterConfig {
    let endpoint = std::env::var("LITELLM_ENDPOINT")
        .unwrap_or_else(|_| thalamus_litellm_adapter::config::AdapterConfig::default_endpoint());
    let model_map = match std::env::var("THALAMUS_MODEL_MAP") {
        Ok(raw) => thalamus_litellm_adapter::config::AdapterConfig::parse_model_map(&raw)
            .unwrap_or_else(|e| {
                eprintln!("invalid THALAMUS_MODEL_MAP: {e}");
                std::process::exit(1);
            }),
        Err(_) => std::collections::HashMap::new(),
    };
    thalamus_litellm_adapter::config::AdapterConfig {
        endpoint,
        model_map,
        api_key: std::env::var("LITELLM_API_KEY").ok(),
        ..Default::default()
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().json().with_target(false).init();

    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config/policy.json".to_owned());

    let config = config::load_config(&config_path).unwrap_or_else(|e| {
        eprintln!("failed to load config from {}: {}", config_path, e);
        std::process::exit(1);
    });

    let addr = config.listen_addr();

    // === Trace exporter + LiteLLM ===
    #[cfg(all(feature = "langfuse", feature = "litellm"))]
    let app = {
        let backend: Arc<dyn BackendPort + Send + Sync> = Arc::new(
            thalamus_litellm_adapter::LiteLLMAdapter::new(litellm_adapter_config_from_env()),
        );
        let sink = default_trace_exporter_sink();
        app::build_with_eval_sink(
            config,
            Some(backend),
            sink,
            thalamus_eval::ContentPolicy::MetadataOnly,
        )
    };

    // === Trace exporter + AgentGateway ===
    #[cfg(all(
        feature = "langfuse",
        feature = "agentgateway",
        not(feature = "litellm")
    ))]
    let app = {
        let endpoint = std::env::var("AGENTGATEWAY_ENDPOINT").unwrap_or_else(|_| {
            thalamus_agentgateway_adapter::config::AdapterConfig::default_endpoint()
        });
        let auth_header = std::env::var("AGENTGATEWAY_AUTH_HEADER").ok();
        let adapter_config = thalamus_agentgateway_adapter::config::AdapterConfig {
            endpoint,
            auth_header,
            ..Default::default()
        };
        let backend: Arc<dyn BackendPort + Send + Sync> = Arc::new(
            thalamus_agentgateway_adapter::AgentgatewayAdapter::new(adapter_config),
        );
        let sink = default_trace_exporter_sink();
        app::build_with_eval_sink(
            config,
            Some(backend),
            sink,
            thalamus_eval::ContentPolicy::MetadataOnly,
        )
    };

    // === Trace exporter, no backend ===
    #[cfg(all(
        feature = "langfuse",
        not(any(feature = "litellm", feature = "agentgateway"))
    ))]
    let app = {
        let sink = default_trace_exporter_sink();
        app::build_with_eval_sink(
            config,
            None,
            sink,
            thalamus_eval::ContentPolicy::MetadataOnly,
        )
    };

    // === LiteLLM, no Langfuse ===
    #[cfg(all(not(feature = "langfuse"), feature = "litellm"))]
    let app = {
        let backend: Arc<dyn BackendPort + Send + Sync> = Arc::new(
            thalamus_litellm_adapter::LiteLLMAdapter::new(litellm_adapter_config_from_env()),
        );
        if let Some(sink) = ports::trace_exporter::trace_exporter_sink_from_env() {
            app::build_with_eval_sink(
                config,
                Some(backend),
                sink,
                thalamus_eval::ContentPolicy::MetadataOnly,
            )
        } else {
            app::build_with_backend(config, backend)
        }
    };

    // === AgentGateway, no Langfuse ===
    #[cfg(all(
        not(feature = "langfuse"),
        feature = "agentgateway",
        not(feature = "litellm")
    ))]
    let app = {
        let endpoint = std::env::var("AGENTGATEWAY_ENDPOINT").unwrap_or_else(|_| {
            thalamus_agentgateway_adapter::config::AdapterConfig::default_endpoint()
        });
        let auth_header = std::env::var("AGENTGATEWAY_AUTH_HEADER").ok();
        let adapter_config = thalamus_agentgateway_adapter::config::AdapterConfig {
            endpoint,
            auth_header,
            ..Default::default()
        };
        let backend: Arc<dyn BackendPort + Send + Sync> = Arc::new(
            thalamus_agentgateway_adapter::AgentgatewayAdapter::new(adapter_config),
        );
        if let Some(sink) = ports::trace_exporter::trace_exporter_sink_from_env() {
            app::build_with_eval_sink(
                config,
                Some(backend),
                sink,
                thalamus_eval::ContentPolicy::MetadataOnly,
            )
        } else {
            app::build_with_backend(config, backend)
        }
    };

    // === No backend, no Langfuse ===
    #[cfg(all(
        not(feature = "langfuse"),
        not(any(feature = "litellm", feature = "agentgateway"))
    ))]
    let app = if let Some(sink) = ports::trace_exporter::trace_exporter_sink_from_env() {
        app::build_with_eval_sink(
            config,
            None,
            sink,
            thalamus_eval::ContentPolicy::MetadataOnly,
        )
    } else {
        app::build(config)
    };

    // THALAMUS_RBX_API (default off): serve the gated /rbx/v1/* surface with
    // the credential middleware. Phase 1 Gate A exercises identity/middleware
    // only; LLM adapters are not wired in this mode (no provider key, no
    // LiteLLM exposure). When off, `app` from the adapter branches is used
    // unchanged and /rbx/v1/* is not mounted.
    let app = if matches!(
        std::env::var("THALAMUS_RBX_API")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "on" | "true" | "yes"
    ) {
        let introspection_url =
            std::env::var("THALAMUS_TOKEN_INTROSPECTION_URL").unwrap_or_else(|_| {
                eprintln!(
                    "THALAMUS_RBX_API=on requires THALAMUS_TOKEN_INTROSPECTION_URL \
                     (rbx-token-service /v1/delegation/introspect)"
                );
                std::process::exit(1);
            });
        let cfg = config::load_config(&config_path).unwrap_or_else(|e| {
            eprintln!("failed to reload config from {}: {}", config_path, e);
            std::process::exit(1);
        });
        let verifier: std::sync::Arc<dyn auth::CredentialVerifier + Send + Sync> =
            std::sync::Arc::new(auth::OpaqueIntrospectionVerifier::new(introspection_url));
        // Keep the data plane wired in governed mode. The same concrete
        // LiteLLM adapter implements two independent domain seams; server code
        // sees only BackendPort and EmbeddingPort trait objects.
        #[cfg(feature = "litellm")]
        let adapter = std::sync::Arc::new(thalamus_litellm_adapter::LiteLLMAdapter::new(
            litellm_adapter_config_from_env(),
        ));
        #[cfg(feature = "litellm")]
        let rbx_backend: Option<
            std::sync::Arc<dyn thalamus_core::BackendPort + Send + Sync>,
        > = Some(adapter.clone());
        #[cfg(feature = "litellm")]
        let rbx_embedding: Option<
            std::sync::Arc<dyn thalamus_core::EmbeddingPort + Send + Sync>,
        > = Some(adapter);
        #[cfg(not(feature = "litellm"))]
        let rbx_backend: Option<
            std::sync::Arc<dyn thalamus_core::BackendPort + Send + Sync>,
        > = None;
        #[cfg(not(feature = "litellm"))]
        let rbx_embedding: Option<
            std::sync::Arc<dyn thalamus_core::EmbeddingPort + Send + Sync>,
        > = None;
        app::build_with_rbx_api_and_embedding(cfg, verifier, rbx_backend, rbx_embedding)
    } else {
        app
    };

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| {
            eprintln!("failed to bind {}: {}", addr, e);
            std::process::exit(1);
        });

    tracing::info!(%addr, "thalamus-server listening");
    axum::serve(listener, app).await.unwrap();
}
