mod app;
mod config;
mod ports;
mod routes;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .json()
        .with_target(false)
        .init();

    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config/policy.json".to_owned());

    let config = config::load_config(&config_path).unwrap_or_else(|e| {
        eprintln!("failed to load config from {}: {}", config_path, e);
        std::process::exit(1);
    });

    let addr = config.listen_addr();
    let app = app::build(config);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap_or_else(|e| {
        eprintln!("failed to bind {}: {}", addr, e);
        std::process::exit(1);
    });

    tracing::info!(%addr, "thalamus-server listening");
    axum::serve(listener, app).await.unwrap();
}
