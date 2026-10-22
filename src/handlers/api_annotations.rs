use axum::Json;
use axum::extract::{Path, Query, State};
use axum::response::Redirect;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use std::collections::HashMap;

use crate::db::annotations::Annotation;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateAnnotationInput {
    pub chapter: u32,
    pub selected_text: String,
    #[serde(default)]
    pub note: Option<String>,
    pub position_start: f64,
    pub position_end: f64,
    #[serde(default = "default_color")]
    pub color: String,
}

fn default_color() -> String {
    "#ffee77".into()
}

#[derive(Debug, Serialize)]
pub struct AnnotationOutput {
    pub id: String,
    pub book_id: String,
    pub chapter: u32,
    pub selected_text: String,
    pub note: Option<String>,
    pub position_start: f64,
    pub position_end: f64,
    pub color: String,
    pub created_at: String,
    pub needs_review: bool,
}

impl From<Annotation> for AnnotationOutput {
    fn from(a: Annotation) -> Self {
        Self {
            id: a.id,
            book_id: a.book_id,
            chapter: a.chapter,
            selected_text: a.selected_text,
            note: a.note,
            position_start: a.position_start,
            position_end: a.position_end,
            color: a.color,
            created_at: a.created_at.to_rfc3339(),
            needs_review: a.needs_review,
        }
    }
}

pub async fn list_annotations(
    State(state): State<AppState>,
    Path(book_id): Path<String>,
) -> Result<Json<Vec<AnnotationOutput>>, AppError> {
    let book = state.book(&book_id).await?;
    crate::epub::migration::upgrade(&state, &book).await?;
    let store = state.annotations_store(&book_id);
    let data = store.load().await?;
    let list: Vec<AnnotationOutput> = data.annotations.into_iter().map(Into::into).collect();
    Ok(Json(list))
}

pub async fn create_annotation(
    State(state): State<AppState>,
    Path(book_id): Path<String>,
    Json(input): Json<CreateAnnotationInput>,
) -> Result<Json<AnnotationOutput>, AppError> {
    let book = state.book(&book_id).await?;
    crate::epub::migration::upgrade(&state, &book).await?;
    if input.chapter >= book.chapter_count {
        return Err(AppError::BadRequest("Chapter out of range".into()));
    }
    crate::validation::position(input.position_start)?;
    crate::validation::position(input.position_end)?;
    crate::validation::color(&input.color)?;
    crate::validation::text(&input.selected_text)?;
    if input.position_start >= input.position_end || input.selected_text.trim().is_empty() {
        return Err(AppError::BadRequest("Invalid annotation selection".into()));
    }
    if let Some(note) = &input.note {
        crate::validation::text(note)?;
    }
    let canonical = state
        .chapter(&book_id, input.chapter as usize)
        .await?
        .text();
    let total = canonical.encode_utf16().count();
    let start_units = (input.position_start * total as f64).round() as usize;
    let end_units = (input.position_end * total as f64).round() as usize;
    let mut units = 0;
    let mut start = canonical.len();
    let mut end = canonical.len();
    for (offset, character) in canonical.char_indices() {
        if units >= start_units && start == canonical.len() {
            start = offset;
        }
        if units >= end_units {
            end = offset;
            break;
        }
        units += character.len_utf16();
    }
    let needs_review = canonical.get(start..end) != Some(input.selected_text.as_str());
    let prefix = canonical[..start]
        .chars()
        .rev()
        .take(24)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let suffix = canonical[end..].chars().take(24).collect();
    let annotation = Annotation {
        id: Uuid::new_v4().to_string(),
        book_id: book_id.clone(),
        chapter: input.chapter,
        selected_text: input.selected_text,
        note: input.note,
        position_start: input.position_start,
        position_end: input.position_end,
        color: input.color,
        created_at: Utc::now(),
        needs_review,
        prefix,
        suffix,
    };

    let output = AnnotationOutput::from(annotation.clone());

    let store = state.annotations_store(&book_id);
    let mut data = store.load().await?;
    data.annotations.push(annotation);
    store.save(&data).await?;

    Ok(Json(output))
}

pub async fn delete_annotation(
    State(state): State<AppState>,
    Path((book_id, aid)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Redirect, AppError> {
    state.book(&book_id).await?;
    crate::validation::id(&aid)?;
    let lang = query.get("lang").map(|s| s.as_str()).unwrap_or("zh");
    let theme = query.get("theme").map(|s| s.as_str()).unwrap_or("light");
    let store = state.annotations_store(&book_id);
    let mut data = store.load().await?;
    if !data
        .annotations
        .iter()
        .any(|annotation| annotation.id == aid)
    {
        return Err(AppError::NotFound("Annotation not found".into()));
    }
    data.annotations.retain(|a| a.id != aid);
    store.save(&data).await?;

    Ok(Redirect::to(&format!(
        "/book/{book_id}?lang={lang}&theme={theme}"
    )))
}

#[derive(Deserialize)]
pub struct EditAnnotationInput {
    pub note: Option<String>,
    pub color: String,
}

pub async fn edit_annotation(
    State(state): State<AppState>,
    Path((book_id, aid)): Path<(String, String)>,
    Json(input): Json<EditAnnotationInput>,
) -> Result<Json<AnnotationOutput>, AppError> {
    let book = state.book(&book_id).await?;
    crate::epub::migration::upgrade(&state, &book).await?;
    crate::validation::id(&aid)?;
    crate::validation::color(&input.color)?;
    if let Some(note) = &input.note {
        crate::validation::text(note)?;
    }
    let store = state.annotations_store(&book_id);
    let mut data = store.load().await?;
    let annotation = data
        .annotations
        .iter_mut()
        .find(|annotation| annotation.id == aid)
        .ok_or_else(|| AppError::NotFound("Annotation not found".into()))?;
    annotation.note = input.note;
    annotation.color = input.color;
    let output = annotation.clone().into();
    store.save(&data).await?;
    Ok(Json(output))
}

pub async fn export_annotations(
    State(state): State<AppState>,
    Path(book_id): Path<String>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    let book = state.book(&book_id).await?;
    crate::epub::migration::upgrade(&state, &book).await?;
    let data = state.annotations_store(&book_id).load().await?;
    let mut output = format!("# {}\n\n{}\n", book.title, book.author);
    for annotation in data.annotations {
        output.push_str(&format!(
            "\n## Chapter {}\n\n{}\n\n",
            annotation.chapter + 1,
            annotation.created_at.to_rfc3339()
        ));
        for line in annotation.selected_text.lines() {
            output.push_str(&format!("> {line}\n"));
        }
        output.push_str(&format!(
            "\n{}\n\nColor: {}\n",
            annotation.note.as_deref().unwrap_or(""),
            annotation.color
        ));
        if annotation.needs_review {
            output.push_str("\nLocation needs review.\n");
        }
    }
    Ok((
        [
            ("content-type", "text/markdown; charset=utf-8"),
            ("content-disposition", "attachment; filename=annotations.md"),
        ],
        output,
    ))
}
