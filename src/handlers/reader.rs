use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use std::collections::HashMap;

use crate::epub::parser::{ChapterEntry, extract_toc, read_chapter};
use crate::error::AppError;
use crate::i18n::translations::Translations;
use crate::state::AppState;

// ── Templates ──────────────────────────────────────────────

#[derive(Template)]
#[template(path = "reader.html")]
struct ReaderTemplate<'a> {
    book_id: String,
    chapter_index: usize,
    chapter_label: String,
    chapter_content: String,
    toc: Vec<ChapterEntry>,
    chapter_count: usize,
    initial_position: f64,
    font_size: u32,
    font_family: String,
    font_face: String,
    t: &'a Translations,
    theme: &'a str,
}

// ── Handlers ───────────────────────────────────────────────

/// GET /book/:id/read — redirect to last read chapter
pub async fn read_book(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Redirect, AppError> {
    let lang = query.get("lang").map(|s| s.as_str()).unwrap_or("zh");
    let theme = query.get("theme").map(|s| s.as_str()).unwrap_or("light");
    let progress = state.progress.load().await?;
    let chapter = progress
        .entries
        .get(&id)
        .map(|e| e.chapter as usize)
        .unwrap_or(0);
    Ok(Redirect::to(&format!(
        "/book/{id}/read/{chapter}?lang={lang}&theme={theme}"
    )))
}

/// GET /book/:id/read/:chapter — read a specific chapter
pub async fn read_chapter_handler(
    State(state): State<AppState>,
    Path((id, chapter)): Path<(String, usize)>,
    Query(query): Query<HashMap<String, String>>,
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

    let progress = state.progress.load().await?;
    let initial_position = progress
        .entries
        .get(&id)
        .filter(|e| e.chapter == chapter as u32)
        .map(|e| e.position)
        .unwrap_or(0.0);

    let lang = query.get("lang").map(|s| s.as_str()).unwrap_or("zh");
    let theme = query
        .get("theme")
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("light");
    let t = Translations::load(lang);

    let settings = state.settings.load().await?;
    let font_family =
        if settings.font_family.ends_with(".ttf") || settings.font_family.ends_with(".woff2") {
            settings
                .font_family
                .trim_end_matches(".ttf")
                .trim_end_matches(".woff2")
                .to_string()
        } else {
            settings.font_family.clone()
        };
    let font_face =
        if settings.font_family.ends_with(".ttf") || settings.font_family.ends_with(".woff2") {
            let fmt = if settings.font_family.ends_with(".woff2") {
                "woff2"
            } else {
                "truetype"
            };
            format!(
                "@font-face {{ font-family: '{}'; src: url('/fonts/{}') format('{}'); }}",
                font_family, settings.font_family, fmt
            )
        } else {
            String::new()
        };

    let tmpl = ReaderTemplate {
        book_id: book.id.clone(),
        chapter_index: chapter,
        chapter_label: label,
        chapter_content: content,
        toc,
        chapter_count,
        initial_position,
        font_size: settings.font_size,
        font_family,
        font_face,
        t: &t,
        theme,
    };

    tmpl.render()
        .map(|html| axum::response::Html(html).into_response())
        .map_err(|e| AppError::Internal(e.to_string()))
}
