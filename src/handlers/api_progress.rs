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
    let mut data = state.progress.load().await?;

    let now = Utc::now();
    let existing = data.entries.get(&input.book_id);

    // Determine if this position is farther than the stored "last"
    let (last_ch, last_pos, last_ts) = {
        let cur = (input.chapter, input.position);
        let prev = existing.map(|e| (e.last_chapter, e.last_position));
        if prev.is_none_or(|p| cur > p) {
            (input.chapter, input.position, Some(now))
        } else {
            (
                existing.unwrap().last_chapter,
                existing.unwrap().last_position,
                existing.unwrap().last_updated_at,
            )
        }
    };

    data.entries.insert(
        input.book_id.clone(),
        ProgressEntry {
            book_id: input.book_id,
            chapter: input.chapter,
            position: input.position,
            updated_at: now,
            last_chapter: last_ch,
            last_position: last_pos,
            last_updated_at: last_ts,
        },
    );

    state.progress.save(&data).await?;
    state.invalidate_progress_cache();

    Ok(Json(ProgressOutput { ok: true }))
}
