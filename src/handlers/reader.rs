use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use std::collections::HashMap;

use crate::epub::parser::{ChapterContent, ChapterEntry};
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
    chapter_number: usize,
    previous_chapter: Option<usize>,
    next_chapter: Option<usize>,
    finished: bool,
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

    let mut toc = state.toc(&id).await?;
    let chapter_count = toc.len();
    let lang = query.get("lang").map(|s| s.as_str()).unwrap_or("zh");
    let theme = query.get("theme").map(|s| s.as_str()).unwrap_or("light");
    let t = Translations::load(lang);
    if toc.is_empty() {
        return Ok(axum::response::Html(format!(
            "<html lang=\"{}\"><meta charset=\"UTF-8\"><p>{}</p></html>",
            t.lang(),
            t.get("reader.no_content")
        ))
        .into_response());
    }
    if chapter == 0 && toc[0].index != 0 {
        return Ok(Redirect::to(&format!(
            "/book/{id}/read/{}?pos=0&lang={lang}&theme={theme}",
            toc[0].index
        ))
        .into_response());
    }
    let Some(ordinal) = toc.iter().position(|entry| entry.index == chapter) else {
        return Err(AppError::NotFound("Chapter not found".into()));
    };
    let previous_chapter = ordinal.checked_sub(1).map(|index| toc[index].index);
    let next_chapter = toc.get(ordinal + 1).map(|entry| entry.index);

    let content = state.chapter(&id, chapter).await?;

    let progress = state.load_progress().await?;

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

    let continued = t.get("reader.continued");
    for entry in &mut toc {
        if entry.is_cover {
            entry.label = t.get("reader.cover");
        }
        entry.label = entry.display_label(&continued);
    }
    let label = toc[ordinal].label.clone();

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
        chapter_number: ordinal + 1,
        previous_chapter,
        next_chapter,
        finished: book.finished,
        font_size: settings.font_size.clamp(12, 32),
        custom_font,
        t: &t,
        theme,
    };

    tmpl.render()
        .map(|html| axum::response::Html(html).into_response())
        .map_err(|e| AppError::Internal(e.to_string()))
}
