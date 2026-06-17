use askama::Template;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::db::books::Book;
use crate::error::AppError;
use crate::state::AppState;

// ── Templates ──────────────────────────────────────────────

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    books: &'a [Book],
}

#[derive(Template)]
#[template(path = "upload.html")]
struct UploadTemplate;

#[derive(Template)]
#[template(path = "book_detail.html")]
struct BookDetailTemplate<'a> {
    book: &'a Book,
}

// ── Handlers ───────────────────────────────────────────────

/// GET / — library home page
pub async fn home(State(state): State<AppState>) -> Result<Response, AppError> {
    let data = state.books.load().await?;
    let tmpl = IndexTemplate { books: &data.books };
    tmpl.render()
        .map(|html| axum::response::Html(html).into_response())
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// GET /upload — upload form
pub async fn upload_form() -> impl IntoResponse {
    let tmpl = UploadTemplate;
    tmpl.render()
        .map(axum::response::Html)
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// GET /book/:id — book detail page
pub async fn book_detail(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let data = state.books.load().await?;
    let book = data
        .books
        .iter()
        .find(|b| b.id == id)
        .ok_or_else(|| AppError::NotFound("Book not found".into()))?;
    let tmpl = BookDetailTemplate { book };
    tmpl.render()
        .map(|html| axum::response::Html(html).into_response())
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// GET /covers/:id — serve cover image
pub async fn cover_image(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let data = state.books.load().await?;
    let book = data
        .books
        .iter()
        .find(|b| b.id == id)
        .ok_or_else(|| AppError::NotFound("Book not found".into()))?;

    let ext = book.cover_ext.as_deref().unwrap_or("jpg");
    let cover_path = state.covers_dir().join(format!("{id}.{ext}"));

    let bytes = tokio::fs::read(&cover_path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            AppError::NotFound("Cover not found".into())
        } else {
            AppError::Io(e)
        }
    })?;

    let mime = match ext {
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "image/jpeg",
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", mime)
        .header("Cache-Control", "public, max-age=86400")
        .body(axum::body::Body::from(bytes))
        .unwrap())
}
