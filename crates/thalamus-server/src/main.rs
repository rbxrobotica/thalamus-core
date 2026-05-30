mod app;
mod config;
mod ports;
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
        let endpoint = std::env::var("LITELLM_ENDPOINT").unwrap_or_else(|_| {
            thalamus_litellm_adapter::config::AdapterConfig::default_endpoint()
        });
        let adapter_config = thalamus_litellm_adapter::config::AdapterConfig {
            endpoint,
            ..Default::default()
        };
        let backend: Arc<dyn BackendPort + Send + Sync> = Arc::new(
            thalamus_litellm_adapter::LiteLLMAdapter::new(adapter_config),
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
        let endpoint = std::env::var("LITELLM_ENDPOINT").unwrap_or_else(|_| {
            thalamus_litellm_adapter::config::AdapterConfig::default_endpoint()
        });
        let adapter_config = thalamus_litellm_adapter::config::AdapterConfig {
            endpoint,
            ..Default::default()
        };
        let backend: Arc<dyn BackendPort + Send + Sync> = Arc::new(
            thalamus_litellm_adapter::LiteLLMAdapter::new(adapter_config),
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

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| {
            eprintln!("failed to bind {}: {}", addr, e);
            std::process::exit(1);
        });

    tracing::info!(%addr, "thalamus-server listening");
    axum::serve(listener, app).await.unwrap();
}
