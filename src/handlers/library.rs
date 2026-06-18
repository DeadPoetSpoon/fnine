use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::collections::HashMap;

use crate::db::annotations::Annotation;
use crate::db::books::Book;
use crate::error::AppError;
use crate::i18n::translations::Translations;
use crate::state::AppState;

// ── Templates ──────────────────────────────────────────────

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    books: &'a [Book],
    t: &'a Translations,
    theme: &'a str,
}

#[derive(Template)]
#[template(path = "upload.html")]
struct UploadTemplate<'a> {
    t: &'a Translations,
    theme: &'a str,
}

#[derive(Template)]
#[template(path = "book_detail.html")]
struct BookDetailTemplate<'a> {
    book: &'a Book,
    annotations: Vec<Annotation>,
    file_size_display: String,
    t: &'a Translations,
    theme: &'a str,
}

// ── Handlers ───────────────────────────────────────────────

fn load_translations(query: &HashMap<String, String>) -> (Translations, String) {
    let lang = query
        .get("lang")
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("zh");
    let theme = query
        .get("theme")
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("light")
        .to_string();
    (Translations::load(lang), theme)
}

/// GET / — library home page
pub async fn home(
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, AppError> {
    let books = state.load_books().await?;
    let (t, theme) = load_translations(&query);
    let tmpl = IndexTemplate {
        books: &books,
        t: &t,
        theme: &theme,
    };
    tmpl.render()
        .map(|html| axum::response::Html(html).into_response())
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// GET /upload — upload form
pub async fn upload_form(Query(query): Query<HashMap<String, String>>) -> impl IntoResponse {
    let (t, theme) = load_translations(&query);
    let tmpl = UploadTemplate {
        t: &t,
        theme: &theme,
    };
    tmpl.render()
        .map(axum::response::Html)
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// GET /book/:id — book detail page
pub async fn book_detail(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, AppError> {
    let books = state.load_books().await?;
    let book = books
        .iter()
        .find(|b| b.id == id)
        .ok_or_else(|| AppError::NotFound("Book not found".into()))?;

    let annot_store = state.annotations_store(&id);
    let annot_data = annot_store.load().await?;
    let (t, theme) = load_translations(&query);

    let file_size_display = if book.file_size < 1024 {
        format!("{} B", book.file_size)
    } else if book.file_size < 1048576 {
        format!("{:.1} KB", book.file_size as f64 / 1024.0)
    } else {
        format!("{:.1} MB", book.file_size as f64 / 1048576.0)
    };

    let tmpl = BookDetailTemplate {
        book,
        annotations: annot_data.annotations,
        file_size_display,
        t: &t,
        theme: &theme,
    };
    tmpl.render()
        .map(|html| axum::response::Html(html).into_response())
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// GET /covers/:id — serve cover image
pub async fn cover_image(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let books = state.load_books().await?;
    let book = books
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
