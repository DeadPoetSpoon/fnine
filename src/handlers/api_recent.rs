use axum::Json;
use axum::extract::State;
use serde::Serialize;

use crate::db::progress::ProgressEntry;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct RecentBookOutput {
    pub title: String,
    pub author: String,
    pub cover_url: String,
    pub progress: Option<RecentProgress>,
    pub reader_url: String,
}

#[derive(Debug, Serialize)]
pub struct RecentProgress {
    pub chapter: u32,
    pub position: f64, // 0.0 ~ 1.0
    pub updated_at: String,
}

/// GET /api/recent — return the last-read book as JSON
pub async fn recent_book(
    State(state): State<AppState>,
) -> Result<Json<RecentBookOutput>, AppError> {
    let progress_data = state.load_progress().await?;

    let books = state.load_books().await?;

    // Find the most recently updated progress entry
    let latest: Option<&ProgressEntry> =
        progress_data.entries.values().max_by_key(|e| e.updated_at);

    match latest {
        Some(entry) => {
            let book = books
                .iter()
                .find(|b| b.id == entry.book_id)
                .ok_or_else(|| AppError::NotFound("Book not found".into()))?;

            Ok(Json(RecentBookOutput {
                title: book.title.clone(),
                author: book.author.clone(),
                cover_url: format!("/covers/{}", book.id),
                progress: Some(RecentProgress {
                    chapter: entry.chapter,
                    position: entry.position,
                    updated_at: entry.updated_at.to_rfc3339(),
                }),
                reader_url: format!("/book/{}/read", book.id),
            }))
        }
        None => {
            // No reading progress yet – return the most recently uploaded book instead
            let latest_book = books.iter().max_by_key(|b| b.uploaded_at);

            match latest_book {
                Some(book) => Ok(Json(RecentBookOutput {
                    title: book.title.clone(),
                    author: book.author.clone(),
                    cover_url: format!("/covers/{}", book.id),
                    progress: None,
                    reader_url: format!("/book/{}/read", book.id),
                })),
                None => Err(AppError::NotFound("No books in library".into())),
            }
        }
    }
}
