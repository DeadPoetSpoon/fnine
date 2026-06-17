mod config;
mod db;
mod error;

use axum::{Router, routing::get};
use config::Config;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = Config::from_env();

    // Ensure data directories exist
    if let Err(e) = tokio::fs::create_dir_all(&config.data_dir).await {
        tracing::warn!("Failed to create data dir: {e}");
    }
    if let Err(e) = tokio::fs::create_dir_all(config.data_dir.join("books")).await {
        tracing::warn!("Failed to create books dir: {e}");
    }
    if let Err(e) = tokio::fs::create_dir_all(config.data_dir.join("covers")).await {
        tracing::warn!("Failed to create covers dir: {e}");
    }

    let app = Router::new()
        .route("/", get(home))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(config.clone());

    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("Starting server on http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn home() -> &'static str {
    "Fnine — coming soon"
}
