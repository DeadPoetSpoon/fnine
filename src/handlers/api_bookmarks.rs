use crate::{
    db::bookmarks::Bookmark,
    error::AppError,
    i18n::translations::Translations,
    pagination::{self, Page},
    state::AppState,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use chrono::Utc;
use serde::Deserialize;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct CreateInput {
    pub chapter: u32,
    pub position: f64,
    #[serde(default)]
    pub name: String,
}

#[derive(Deserialize)]
pub struct RenameInput {
    pub name: String,
}

pub fn sort(bookmarks: &mut [Bookmark]) {
    bookmarks.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| right.id.cmp(&left.id))
    });
}

fn name(value: &str, fallback: String) -> Result<String, AppError> {
    let value = value.trim();
    if value.chars().count() > 200 {
        return Err(AppError::BadRequest(
            "Bookmark name exceeds 200 characters".into(),
        ));
    }
    Ok(if value.is_empty() {
        fallback
    } else {
        value.to_string()
    })
}

async fn default_name(
    state: &AppState,
    book_id: &str,
    chapter: u32,
    position: f64,
    lang: &str,
) -> Result<String, AppError> {
    let mut entry = state
        .toc(book_id)
        .await?
        .into_iter()
        .find(|entry| entry.index == chapter as usize)
        .ok_or_else(|| AppError::BadRequest("Chapter is not readable".into()))?;
    let t = Translations::load(lang);
    if entry.is_cover {
        entry.label = t.get("reader.cover");
    }
    let suffix = format!(" · {:.0}%", position * 100.0);
    let label = entry
        .display_label(&t.get("reader.continued"))
        .chars()
        .take(200 - suffix.chars().count())
        .collect::<String>();
    Ok(format!("{label}{suffix}"))
}

pub async fn list(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Page<Bookmark>>, AppError> {
    state.book(&id).await?;
    let mut data = state.bookmarks_store(&id).load().await?;
    sort(&mut data.bookmarks);
    Ok(Json(Page::new(
        data.bookmarks,
        pagination::number(query.get("page"))?,
    )))
}

pub async fn create(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Json(input): Json<CreateInput>,
) -> Result<(StatusCode, Json<Bookmark>), AppError> {
    let book = state.book(&id).await?;
    crate::epub::migration::upgrade(&state, &book).await?;
    crate::validation::position(input.position)?;
    let fallback = default_name(
        &state,
        &id,
        input.chapter,
        input.position,
        query.get("lang").map(String::as_str).unwrap_or("zh"),
    )
    .await?;
    let bookmark = Bookmark {
        id: Uuid::new_v4().to_string(),
        book_id: id.clone(),
        chapter: input.chapter,
        position: input.position,
        name: name(&input.name, fallback)?,
        created_at: Utc::now(),
    };
    let store = state.bookmarks_store(&id);
    let mut data = store.load().await?;
    data.bookmarks.push(bookmark.clone());
    store.save(&data).await?;
    Ok((StatusCode::CREATED, Json(bookmark)))
}

pub async fn rename(
    State(state): State<AppState>,
    Path((id, bid)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
    Json(input): Json<RenameInput>,
) -> Result<Json<Bookmark>, AppError> {
    state.book(&id).await?;
    crate::validation::id(&bid)?;
    let store = state.bookmarks_store(&id);
    let mut data = store.load().await?;
    let bookmark = data
        .bookmarks
        .iter_mut()
        .find(|bookmark| bookmark.id == bid)
        .ok_or_else(|| AppError::NotFound("Bookmark not found".into()))?;
    let fallback = default_name(
        &state,
        &id,
        bookmark.chapter,
        bookmark.position,
        query.get("lang").map(String::as_str).unwrap_or("zh"),
    )
    .await?;
    bookmark.name = name(&input.name, fallback)?;
    let output = bookmark.clone();
    store.save(&data).await?;
    Ok(Json(output))
}

pub async fn delete(
    State(state): State<AppState>,
    Path((id, bid)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    state.book(&id).await?;
    crate::validation::id(&bid)?;
    let store = state.bookmarks_store(&id);
    let mut data = store.load().await?;
    let index = data
        .bookmarks
        .iter()
        .position(|bookmark| bookmark.id == bid)
        .ok_or_else(|| AppError::NotFound("Bookmark not found".into()))?;
    data.bookmarks.remove(index);
    store.save(&data).await?;
    Ok(StatusCode::NO_CONTENT)
}
