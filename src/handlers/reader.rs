use askama::Template;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};

use crate::epub::parser::{ChapterEntry, extract_toc, read_chapter};
use crate::error::AppError;
use crate::state::AppState;

// ── Templates ──────────────────────────────────────────────

#[derive(Template)]
#[template(path = "reader.html")]
struct ReaderTemplate {
    book_id: String,
    book_title: String,
    chapter_index: usize,
    chapter_label: String,
    chapter_content: String,
    toc: Vec<ChapterEntry>,
    chapter_count: usize,
}

// ── Handlers ───────────────────────────────────────────────

/// GET /book/:id/read — redirect to chapter 0
pub async fn read_book(Path(id): Path<String>) -> Result<Redirect, AppError> {
    Ok(Redirect::to(&format!("/book/{id}/read/0")))
}

/// GET /book/:id/read/:chapter — read a specific chapter
pub async fn read_chapter_handler(
    State(state): State<AppState>,
    Path((id, chapter)): Path<(String, usize)>,
) -> Result<Response, AppError> {
    let data = state.books.load().await?;
    let book = data
        .books
        .iter()
        .find(|b| b.id == id)
        .ok_or_else(|| AppError::NotFound("Book not found".into()))?;

    let epub_path = state.books_dir().join(format!("{}.epub", book.id));

    let toc = extract_toc(&epub_path)?;
    let chapter_count = toc.len();

    if chapter >= chapter_count {
        return Err(AppError::NotFound("Chapter not found".into()));
    }

    let content = read_chapter(&epub_path, chapter)?;
    let label = toc[chapter].label.clone();

    let tmpl = ReaderTemplate {
        book_id: book.id.clone(),
        book_title: book.title.clone(),
        chapter_index: chapter,
        chapter_label: label,
        chapter_content: content,
        toc,
        chapter_count,
    };

    tmpl.render()
        .map(|html| axum::response::Html(html).into_response())
        .map_err(|e| AppError::Internal(e.to_string()))
}
