use axum::Json;
use axum::extract::State;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::db::progress::ProgressEntry;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ProgressInput {
    pub book_id: String,
    pub chapter: u32,
    pub position: f64,
}

#[derive(Debug, Serialize)]
pub struct ProgressOutput {
    pub ok: bool,
}

/// POST /api/progress — save reading progress
pub async fn save_progress(
    State(state): State<AppState>,
    Json(input): Json<ProgressInput>,
) -> Result<Json<ProgressOutput>, AppError> {
    let book = state.book(&input.book_id).await?;
    crate::epub::migration::upgrade(&state, &book).await?;
    if input.chapter >= book.chapter_count {
        return Err(AppError::BadRequest("Chapter out of range".into()));
    }
    crate::validation::position(input.position)?;
    let mut data = state.progress.load().await?;

    let now = Utc::now();

    data.entries.insert(
        input.book_id.clone(),
        ProgressEntry {
            book_id: input.book_id,
            chapter: input.chapter,
            position: input.position,
            updated_at: now,
            last_chapter: 0,
            last_position: 0.0,
            last_updated_at: None,
        },
    );

    state.progress.save(&data).await?;
    state.invalidate_progress_cache();

    Ok(Json(ProgressOutput { ok: true }))
}
