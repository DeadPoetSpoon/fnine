use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use std::collections::HashMap;

use crate::epub::parser::{ChapterContent, ChapterEntry, extract_toc};
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
    chapter_content: std::sync::Arc<ChapterContent>,
    toc: Vec<ChapterEntry>,
    chapter_count: usize,
    initial_position: f64,
    last_chapter: usize,
    last_position: f64,
    font_size: u32,
    custom_font: String,
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
    let book = state.book(&id).await?;
    crate::epub::migration::upgrade(&state, &book).await?;
    let lang = query.get("lang").map(|s| s.as_str()).unwrap_or("zh");
    let theme = query.get("theme").map(|s| s.as_str()).unwrap_or("light");
    let progress = state.load_progress().await?;
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
    let book = state.book(&id).await?;
    crate::epub::migration::upgrade(&state, &book).await?;

    let epub_path = state.books_dir().join(format!("{}.epub", book.id));

    let mut toc = if let Some(toc) = state.toc_cache.get(&id) {
        toc
    } else {
        let path = epub_path.clone();
        let permit = state
            .parsers
            .clone()
            .acquire_owned()
            .await
            .map_err(|error| AppError::Internal(error.to_string()))?;
        let toc = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            extract_toc(&path)
        })
        .await
        .map_err(|error| AppError::Internal(error.to_string()))??;
        let weight = toc
            .iter()
            .map(|entry| entry.label.len() + std::mem::size_of::<ChapterEntry>())
            .sum();
        state
            .toc_cache
            .insert_weighted(id.clone(), toc.clone(), weight);
        toc
    };
    let chapter_count = toc.len();

    if chapter >= chapter_count {
        return Err(AppError::NotFound("Chapter not found".into()));
    }

    let content = state.chapter(&id, chapter).await?;

    let progress = state.load_progress().await?;

    // Use explicit pos parameter if provided (jump-to-last), else fall back to progress
    let initial_position = if let Some(pos_str) = query.get("pos") {
        let position = pos_str
            .parse::<f64>()
            .map_err(|_| AppError::BadRequest("Invalid position".into()))?;
        crate::validation::position(position)?;
        position
    } else {
        progress
            .entries
            .get(&id)
            .filter(|e| e.chapter == chapter as u32)
            .map(|e| e.position)
            .unwrap_or(0.0)
    };

    let (last_chapter, last_position) = progress
        .entries
        .get(&id)
        .map(|e| (e.last_chapter as usize, e.last_position))
        .unwrap_or((0, 0.0));

    let lang = query.get("lang").map(|s| s.as_str()).unwrap_or("zh");
    let theme = query
        .get("theme")
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("light");
    let t = Translations::load(lang);
    let continued = t.get("reader.continued");
    for entry in &mut toc {
        entry.label = entry.display_label(&continued);
    }
    let label = toc[chapter].label.clone();

    let settings = state.load_settings().await?;
    let custom_font = if settings.font_family.is_empty()
        || super::api_settings::known_font(&state, &settings.font_family)
            .await
            .is_err()
    {
        String::new()
    } else {
        settings.font_family
    };

    let tmpl = ReaderTemplate {
        book_id: book.id.clone(),
        chapter_index: chapter,
        chapter_label: label,
        chapter_content: content,
        toc,
        chapter_count,
        initial_position,
        last_chapter,
        last_position,
        font_size: settings.font_size.clamp(12, 32),
        custom_font,
        t: &t,
        theme,
    };

    tmpl.render()
        .map(|html| axum::response::Html(html).into_response())
        .map_err(|e| AppError::Internal(e.to_string()))
}
