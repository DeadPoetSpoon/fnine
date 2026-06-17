use askama::Template;
use axum::Form;
use axum::extract::{Multipart, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use std::collections::HashMap;
use tokio::fs;

use crate::db::settings::Settings;
use crate::error::AppError;
use crate::i18n::translations::Translations;
use crate::state::AppState;

#[derive(Template)]
#[template(path = "settings.html")]
struct SettingsTemplate<'a> {
    settings: &'a Settings,
    t: &'a Translations,
    theme: &'a str,
    fonts: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SettingsForm {
    pub language: String,
    pub theme: String,
    pub font_size: u32,
    #[serde(default)]
    pub font_family: String,
}

fn load_translations(query: &HashMap<String, String>) -> (Translations, String) {
    let lang = query
        .get("lang")
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("zh");
    let theme = query
        .get("theme")
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("light")
        .to_string();
    (Translations::load(lang), theme)
}

/// GET /settings
pub async fn settings_page(
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, AppError> {
    let settings = if let Some(cached) = state.settings_cache.get(&()) {
        cached
    } else {
        let s = state.settings.load().await?;
        state.settings_cache.insert((), s.clone());
        s
    };
    let (t, theme) = load_translations(&query);
    let mut fonts = Vec::new();
    if let Ok(mut entries) = fs::read_dir(state.fonts_dir()).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if let Some(name) = entry.file_name().to_str() {
                fonts.push(name.to_string());
            }
        }
    }
    let tmpl = SettingsTemplate {
        settings: &settings,
        t: &t,
        theme: &theme,
        fonts,
    };
    tmpl.render()
        .map(|html| axum::response::Html(html).into_response())
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// POST /settings
pub async fn save_settings(
    State(state): State<AppState>,
    Form(form): Form<SettingsForm>,
) -> Result<Redirect, AppError> {
    let lang = form.language.clone();
    let theme = form.theme.clone();
    state
        .settings
        .save(&Settings {
            language: form.language,
            theme: form.theme,
            font_size: form.font_size,
            font_family: form.font_family,
        })
        .await?;
    state.settings_cache.invalidate(&());

    Ok(Redirect::to(&format!(
        "/settings?lang={lang}&theme={theme}"
    )))
}

/// POST /settings/fonts — upload a font
pub async fn upload_font(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Redirect, AppError> {
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.file_name().unwrap_or("font.ttf").to_string();
        let lower = name.to_lowercase();
        if !lower.ends_with(".ttf") && !lower.ends_with(".woff2") {
            continue;
        }
        let data = field
            .bytes()
            .await
            .map_err(|e| AppError::Internal(format!("Read error: {e}")))?;
        let path = state.fonts_dir().join(&name);
        fs::write(&path, &data).await?;
        tracing::info!("Font uploaded: {name}");
        return Ok(Redirect::to("/settings"));
    }
    Err(AppError::Internal("No font file found".into()))
}

/// POST /settings/fonts/delete — delete a font
pub async fn delete_font(
    State(state): State<AppState>,
    Form(form): Form<HashMap<String, String>>,
) -> Result<Redirect, AppError> {
    if let Some(name) = form.get("name") {
        let path = state.fonts_dir().join(name);
        let _ = fs::remove_file(&path).await;
    }
    Ok(Redirect::to("/settings"))
}
