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
    recent: Option<Book>,
    sort: String,
    status: String,
}

#[derive(Template)]
#[template(path = "upload.html")]
struct UploadTemplate<'a> {
    t: &'a Translations,
    theme: &'a str,
}

/// Annotation with display-ready fields (timestamp pre-formatted to seconds).
struct AnnotationDisplay {
    id: String,
    chapter: u32,
    selected_text: String,
    note: Option<String>,
    position_start: f64,
    created_at: String,
    needs_review: bool,
    color: String,
}

impl From<Annotation> for AnnotationDisplay {
    fn from(a: Annotation) -> Self {
        Self {
            id: a.id,
            chapter: a.chapter,
            selected_text: a.selected_text,
            note: a.note,
            position_start: a.position_start,
            created_at: a
                .created_at
                .with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
            needs_review: a.needs_review,
            color: a.color,
        }
    }
}

#[derive(Template)]
#[template(path = "book_detail.html")]
struct BookDetailTemplate<'a> {
    book: &'a Book,
    annotations: Vec<AnnotationDisplay>,
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
    let mut books = state.load_books().await?;
    let progress = state.load_progress().await?;
    let recent = progress
        .entries
        .values()
        .filter(|entry| books.iter().any(|book| book.id == entry.book_id))
        .max_by_key(|entry| entry.updated_at)
        .and_then(|entry| books.iter().find(|book| book.id == entry.book_id))
        .cloned();
    let sort = query
        .get("sort")
        .cloned()
        .unwrap_or_else(|| "uploaded".into());
    let status = query.get("status").cloned().unwrap_or_else(|| "all".into());
    if !["uploaded", "last_read", "title", "author"].contains(&sort.as_str())
        || !["all", "unread", "reading", "finished"].contains(&status.as_str())
    {
        return Err(AppError::BadRequest("Unsupported library filter".into()));
    }
    books.retain(|book| {
        let entry = progress.entries.get(&book.id);
        let finished = entry.is_some_and(|entry| {
            entry.last_chapter.saturating_add(1) >= book.chapter_count
                && entry.last_position >= 0.99
        });
        match status.as_str() {
            "unread" => entry.is_none(),
            "reading" => entry.is_some() && !finished,
            "finished" => finished,
            _ => true,
        }
    });
    books.sort_by(|left, right| {
        let ordering = match sort.as_str() {
            "title" => left.title.to_lowercase().cmp(&right.title.to_lowercase()),
            "author" => left.author.to_lowercase().cmp(&right.author.to_lowercase()),
            "last_read" => progress
                .entries
                .get(&right.id)
                .map(|entry| entry.updated_at)
                .cmp(&progress.entries.get(&left.id).map(|entry| entry.updated_at)),
            _ => right.uploaded_at.cmp(&left.uploaded_at),
        };
        ordering.then_with(|| left.id.cmp(&right.id))
    });
    let (t, theme) = load_translations(&query);
    let tmpl = IndexTemplate {
        books: &books,
        t: &t,
        theme: &theme,
        recent,
        sort,
        status,
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
    crate::validation::id(&id)?;
    let existing = state.book(&id).await?;
    crate::epub::migration::upgrade(&state, &existing).await?;
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
        annotations: annot_data
            .annotations
            .into_iter()
            .map(AnnotationDisplay::from)
            .collect(),
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
    crate::validation::id(&id)?;
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
