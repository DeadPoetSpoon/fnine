use axum::body::Body;
use axum::extract::{Multipart, Path, Query, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use chrono::Utc;
use std::collections::HashMap;
use tokio::fs;
use tower_http::services::ServeFile;
use uuid::Uuid;

use crate::db::books::Book;
use crate::epub::parser::extract_metadata;
use crate::error::AppError;
use crate::state::AppState;

/// POST /upload
pub async fn upload_book(
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    let lang = query.get("lang").map(|s| s.as_str()).unwrap_or("zh");
    let theme = query.get("theme").map(|s| s.as_str()).unwrap_or("light");

    let (staged, file_name, file_size) =
        super::uploads::stage(&state, &mut multipart, 50 * 1024 * 1024).await?;
    let id = Uuid::new_v4().to_string();
    let epub_path = state.books_dir().join(format!("{id}.epub"));
    let result = async {
        let permit = state
            .parsers
            .clone()
            .acquire_owned()
            .await
            .map_err(|error| AppError::Internal(error.to_string()))?;
        let path = staged.clone();
        let meta = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            extract_metadata(&path)
        })
        .await
        .map_err(|error| AppError::Internal(error.to_string()))??;

        let import_marker = crate::db::store::Store::<crate::db::books::BooksData>::new(
            state
                .config
                .data_dir
                .join("imports")
                .join(format!("{id}.toml")),
        );
        import_marker
            .save(&crate::db::books::BooksData::default())
            .await?;
        let cover_ext = if let (Some(cover_bytes), Some(ext)) = (meta.cover_bytes, meta.cover_ext) {
            let cover_path = state.covers_dir().join(format!("{id}.{ext}"));
            fs::write(&cover_path, &cover_bytes).await?;
            fs::File::open(&cover_path).await?.sync_all().await?;
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
            normalization_version: 2,
        };

        let journal = crate::db::store::Store::<crate::db::books::BooksData>::new(
            state
                .config
                .data_dir
                .join("imports")
                .join(format!("{id}.toml")),
        );
        journal
            .save(&crate::db::books::BooksData {
                books: vec![book.clone()],
            })
            .await?;
        fs::rename(&staged, &epub_path).await?;
        fs::File::open(state.books_dir()).await?.sync_all().await?;
        fs::File::open(state.covers_dir()).await?.sync_all().await?;
        let mut data_store = state.books.load().await?;
        data_store.books.push(book);
        state.books.save(&data_store).await?;
        state.invalidate_books_cache();
        crate::state::remove_if_exists(
            &state
                .config
                .data_dir
                .join("imports")
                .join(format!("{id}.toml")),
        )
        .await?;

        tracing::info!("Book uploaded: {id}");

        Ok(Redirect::to(&format!("/book/{id}?lang={lang}&theme={theme}")).into_response())
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(&staged).await;
        let indexed = state
            .books
            .load()
            .await?
            .books
            .iter()
            .any(|book| book.id == id);
        if !indexed {
            crate::state::remove_if_exists(&epub_path).await?;
            for ext in ["jpg", "png", "gif", "webp"] {
                crate::state::remove_if_exists(&state.covers_dir().join(format!("{id}.{ext}")))
                    .await?;
            }
            crate::state::remove_if_exists(
                &state
                    .config
                    .data_dir
                    .join("imports")
                    .join(format!("{id}.toml")),
            )
            .await?;
        }
    }
    result
}

/// GET /book/:id/download — stream the original EPUB as an attachment.
pub async fn download_book(
    State(state): State<AppState>,
    Path(id): Path<String>,
    request: Request,
) -> Result<Response, AppError> {
    let book = state.book(&id).await?;

    // Only use the stored UUID for the path; the upload filename is for display.
    let epub_path = state.books_dir().join(format!("{}.epub", book.id));
    let mut response = ServeFile::new(epub_path)
        .try_call(request)
        .await?
        .map(Body::new);
    if response.status() == StatusCode::NOT_FOUND {
        return Err(AppError::NotFound("Book file not found".into()));
    }

    if response.status().is_success() {
        // RFC 8187 encoding preserves Chinese filenames and escapes header delimiters.
        let filename = book
            .file_name
            .rsplit(['/', '\\'])
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or("book.epub");
        let encoded: String = filename
            .bytes()
            .map(|byte| {
                if byte.is_ascii_alphanumeric() || b"-._".contains(&byte) {
                    (byte as char).to_string()
                } else {
                    format!("%{byte:02X}")
                }
            })
            .collect();
        let disposition = HeaderValue::from_str(&format!(
            "attachment; filename=\"book.epub\"; filename*=UTF-8''{encoded}"
        ))
        .map_err(|e| AppError::Internal(format!("Invalid download filename: {e}")))?;
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, disposition);
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/epub+zip"),
        );
    }

    Ok(response)
}

/// POST /book/:id/delete
pub async fn delete_book(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Redirect, AppError> {
    let lang = query.get("lang").map(|s| s.as_str()).unwrap_or("zh");
    let theme = query.get("theme").map(|s| s.as_str()).unwrap_or("light");

    state.book(&id).await?;
    let journal = crate::db::store::Store::<crate::db::books::BooksData>::new(
        state
            .config
            .data_dir
            .join("deletions")
            .join(format!("{id}.toml")),
    );
    journal
        .save(&crate::db::books::BooksData::default())
        .await?;
    state.finish_delete(&id).await?;

    tracing::info!("Book deleted: {id}");

    Ok(Redirect::to(&format!("/?lang={lang}&theme={theme}")))
}
