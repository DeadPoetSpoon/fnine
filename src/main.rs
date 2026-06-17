mod config;
mod db;
mod epub;
mod error;
mod handlers;
mod i18n;
mod state;

use axum::extract::DefaultBodyLimit;
use axum::{Router, routing::get, routing::post};
use state::AppState;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = config::Config::from_env();
    let state = AppState::new(config.clone());

    // Ensure data directories exist
    let _ = tokio::fs::create_dir_all(&config.data_dir).await;
    let _ = tokio::fs::create_dir_all(state.books_dir()).await;
    let _ = tokio::fs::create_dir_all(state.covers_dir()).await;
    let _ = tokio::fs::create_dir_all(state.fonts_dir()).await;
    let _ = tokio::fs::create_dir_all(config.data_dir.join("annotations")).await;

    let app = Router::new()
        // ── Pages ──────────────────────────────────────
        .route("/", get(handlers::library::home))
        .route("/upload", get(handlers::library::upload_form))
        .route("/book/{id}", get(handlers::library::book_detail))
        // ── Settings ────────────────────────────────────
        .route(
            "/settings",
            get(handlers::api_settings::settings_page).post(handlers::api_settings::save_settings),
        )
        .route("/settings/fonts", post(handlers::api_settings::upload_font))
        .route(
            "/settings/fonts/delete",
            post(handlers::api_settings::delete_font),
        )
        // ── Reader ──────────────────────────────────────
        .route("/book/{id}/read", get(handlers::reader::read_book))
        .route(
            "/book/{id}/read/{chapter}",
            get(handlers::reader::read_chapter_handler),
        )
        // ── Cover images ───────────────────────────────
        .route("/covers/{id}", get(handlers::library::cover_image))
        // ── API ────────────────────────────────────────
        .route(
            "/upload",
            post(handlers::api_books::upload_book)
                .layer(RequestBodyLimitLayer::new(50 * 1024 * 1024)), // 50 MB
        )
        .route("/book/{id}/delete", post(handlers::api_books::delete_book))
        // ── Progress ───────────────────────────────────
        .route("/api/progress", post(handlers::api_progress::save_progress))
        // ── Annotations ───────────────────────────────
        .route(
            "/api/book/{id}/annotations",
            get(handlers::api_annotations::list_annotations)
                .post(handlers::api_annotations::create_annotation),
        )
        .route(
            "/api/book/{id}/annotations/{aid}",
            post(handlers::api_annotations::delete_annotation),
        )
        // ── Static files ───────────────────────────────
        .nest_service("/static", ServeDir::new("static"))
        .nest_service("/fonts", ServeDir::new(config.data_dir.join("fonts")))
        .layer(DefaultBodyLimit::disable())
        .with_state(state);

    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("Starting server on http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
