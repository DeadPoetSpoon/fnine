use askama::Template;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use std::collections::HashMap;

use crate::db::books::Book;
use crate::error::AppError;
use crate::i18n::translations::Translations;
use crate::state::AppState;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Template)]
#[template(path = "search.html")]
struct SearchTemplate<'a> {
    query: &'a str,
    results: Vec<&'a Book>,
    t: &'a Translations,
    theme: &'a str,
}

/// GET /search?q=...
pub async fn search(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, AppError> {
    let q = params.get("q").map(|s| s.as_str()).unwrap_or("");
    let lang = params.get("lang").map(|s| s.as_str()).unwrap_or("zh");
    let theme = params.get("theme").map(|s| s.as_str()).unwrap_or("light");
    let t = Translations::load(lang);

    let books = state.load_books().await?;

    let results: Vec<&Book> = if q.is_empty() {
        Vec::new()
    } else {
        let q_lower = q.to_lowercase();
        books
            .iter()
            .filter(|b| {
                b.title.to_lowercase().contains(&q_lower)
                    || b.author.to_lowercase().contains(&q_lower)
            })
            .collect()
    };

    let tmpl = SearchTemplate {
        query: q,
        results,
        t: &t,
        theme,
    };
    tmpl.render()
        .map(|html| axum::response::Html(html).into_response())
        .map_err(|e| AppError::Internal(e.to_string()))
}
