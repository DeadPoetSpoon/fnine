use axum::Json;
use axum::extract::{Path, State};
use axum::response::Redirect;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
        }
    }
}

/// GET /api/book/:id/annotations
pub async fn list_annotations(
    State(state): State<AppState>,
    Path(book_id): Path<String>,
) -> Result<Json<Vec<AnnotationOutput>>, AppError> {
    let store = state.annotations_store(&book_id);
    let data = store.load().await?;
    let list: Vec<AnnotationOutput> = data.annotations.into_iter().map(Into::into).collect();
    Ok(Json(list))
}

/// POST /api/book/:id/annotations
pub async fn create_annotation(
    State(state): State<AppState>,
    Path(book_id): Path<String>,
    Json(input): Json<CreateAnnotationInput>,
) -> Result<Json<AnnotationOutput>, AppError> {
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
    };

    let output = AnnotationOutput::from(annotation.clone());

    let store = state.annotations_store(&book_id);
    let mut data = store.load().await?;
    data.annotations.push(annotation);
    store.save(&data).await?;

    Ok(Json(output))
}

/// POST /api/book/:id/annotations/:aid — delete (form submit)
pub async fn delete_annotation(
    State(state): State<AppState>,
    Path((book_id, aid)): Path<(String, String)>,
) -> Result<Redirect, AppError> {
    let store = state.annotations_store(&book_id);
    let mut data = store.load().await?;
    data.annotations.retain(|a| a.id != aid);
    store.save(&data).await?;

    Ok(Redirect::to(&format!("/book/{book_id}")))
}
