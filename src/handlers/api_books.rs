use askama::Template;
use axum::extract::{Multipart, Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use chrono::Utc;
use std::collections::HashMap;
use tokio::fs;
use uuid::Uuid;

use crate::db::books::Book;
use crate::epub::parser::extract_metadata;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Template)]
#[template(path = "components/book_card.html")]
struct BookCardTemplate<'a> {
    book: &'a Book,
}

/// POST /upload
pub async fn upload_book(
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    let lang = query.get("lang").map(|s| s.as_str()).unwrap_or("zh");
    let theme = query.get("theme").map(|s| s.as_str()).unwrap_or("light");

    if let Ok(Some(field)) = multipart.next_field().await {
        let file_name = field.file_name().unwrap_or("unknown.epub").to_string();
        let data = field
            .bytes()
            .await
            .map_err(|e| AppError::Internal(format!("Read error: {e}")))?;
        if data.is_empty() {
            return Err(AppError::Internal("Empty file".into()));
        }
        let file_size = data.len() as u64;
        let id = Uuid::new_v4().to_string();

        let epub_path = state.books_dir().join(format!("{id}.epub"));
        fs::write(&epub_path, &data).await?;

        let meta = extract_metadata(&epub_path)?;

        let cover_ext = if let (Some(cover_bytes), Some(ext)) = (meta.cover_bytes, meta.cover_ext) {
            let cover_path = state.covers_dir().join(format!("{id}.{ext}"));
            fs::write(&cover_path, &cover_bytes).await?;
            Some(ext)
        } else {
            None
        };

        let book = Book {
            id: id.clone(),
            title: meta.title,
            author: meta.author,
            cover_ext,
            chapter_count: meta.chapter_count,
            file_name,
            file_size,
            uploaded_at: Utc::now(),
        };

        let mut data_store = state.books.load().await?;
        data_store.books.push(book);
        state.books.save(&data_store).await?;

        tracing::info!("Book uploaded: {id}");

        return Ok(Redirect::to(&format!("/book/{id}?lang={lang}&theme={theme}")).into_response());
    }

    Err(AppError::Internal("No file found in upload".into()))
}

/// POST /book/:id/delete
pub async fn delete_book(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Redirect, AppError> {
    let lang = query.get("lang").map(|s| s.as_str()).unwrap_or("zh");
    let theme = query.get("theme").map(|s| s.as_str()).unwrap_or("light");

    let mut data = state.books.load().await?;
    let book = data
        .books
        .iter()
        .find(|b| b.id == id)
        .ok_or_else(|| AppError::NotFound("Book not found".into()))?;
    let book = book.clone();

    data.books.retain(|b| b.id != id);
    state.books.save(&data).await?;

    let epub_path = state.books_dir().join(format!("{id}.epub"));
    let _ = fs::remove_file(&epub_path).await;

    if let Some(ext) = &book.cover_ext {
        let cover_path = state.covers_dir().join(format!("{id}.{ext}"));
        let _ = fs::remove_file(&cover_path).await;
    }

    tracing::info!("Book deleted: {id}");

    Ok(Redirect::to(&format!("/?lang={lang}&theme={theme}")))
}
