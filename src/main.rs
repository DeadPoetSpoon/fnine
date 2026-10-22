mod cache;
mod config;
mod db;
mod epub;
mod error;
mod handlers;
mod i18n;
mod middleware;
mod state;
#[cfg(test)]
mod tests;
mod validation;

use axum::extract::DefaultBodyLimit;
use axum::{Router, routing::get, routing::post};
use state::AppState;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = config::Config::from_env()?;
    let state = AppState::new(config.clone());
    let _data_lock = state.initialize().await?;
    let app = router(state);
    let address = std::net::SocketAddr::new(config.host.parse()?, config.port);
    let listener = tokio::net::TcpListener::bind(address).await?;
    tracing::info!(%address, "Starting Fnine");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

fn router(state: AppState) -> Router {
    Router::new()
        // ── Pages ──────────────────────────────────────
        .route("/", get(handlers::library::home))
        .route("/upload", get(handlers::library::upload_form))
        .route("/book/{id}", get(handlers::library::book_detail))
        .route(
            "/book/{id}/download",
            get(handlers::api_books::download_book),
        )
        // ── Settings ────────────────────────────────────
        .route(
            "/settings",
            get(handlers::api_settings::settings_page).post(handlers::api_settings::save_settings),
        )
        .route(
            "/settings/fonts",
            post(handlers::api_settings::upload_font)
                .layer::<_, std::convert::Infallible>(DefaultBodyLimit::max(
                    10 * 1024 * 1024 + 64 * 1024,
                ))
                .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024 + 64 * 1024)),
        )
        .route(
            "/settings/fonts/{id}/style.css",
            get(handlers::api_settings::font_css),
        )
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
                .layer::<_, std::convert::Infallible>(DefaultBodyLimit::max(
                    50 * 1024 * 1024 + 64 * 1024,
                ))
                .layer(RequestBodyLimitLayer::new(50 * 1024 * 1024 + 64 * 1024)),
        )
        .route("/book/{id}/delete", post(handlers::api_books::delete_book))
        // ── Progress ───────────────────────────────────
        .route("/api/progress", post(handlers::api_progress::save_progress))
        // ── Recent book ────────────────────────────────
        .route("/api/recent", get(handlers::api_recent::recent_book))
        // ── Annotations ───────────────────────────────
        .route(
            "/api/book/{id}/annotations",
            get(handlers::api_annotations::list_annotations)
                .post(handlers::api_annotations::create_annotation),
        )
        .route(
            "/api/book/{id}/annotations/{aid}",
            post(handlers::api_annotations::delete_annotation)
                .patch(handlers::api_annotations::edit_annotation),
        )
        .route(
            "/book/{id}/annotations/export",
            get(handlers::api_annotations::export_annotations),
        )
        // ── Static files ───────────────────────────────
        .nest_service("/static", ServeDir::new("static"))
        .route("/fonts/{id}", get(handlers::api_settings::font_file))
        .layer(DefaultBodyLimit::max(256 * 1024))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::coordinate,
        ))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(state)
}

async fn shutdown() {
    #[cfg(unix)]
    {
        if let Ok(mut terminate) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
