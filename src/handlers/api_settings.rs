use askama::Template;
use axum::Form;
use axum::extract::{Multipart, Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use std::collections::HashMap;
use tokio::{fs, io::AsyncReadExt};

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
    fonts: Vec<crate::db::fonts::Font>,
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
    let settings = state.load_settings().await?;
    let (t, theme) = load_translations(&query);
    let fonts = state.fonts.load().await?.fonts;
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
    if !["zh", "en"].contains(&form.language.as_str())
        || !["light", "dark", "sepia"].contains(&form.theme.as_str())
        || !(12..=32).contains(&form.font_size)
    {
        return Err(AppError::BadRequest("Unsupported settings".into()));
    }
    if !form.font_family.is_empty() {
        known_font(&state, &form.font_family).await?;
    }
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
    state.invalidate_settings_cache();

    Ok(Redirect::to(&format!(
        "/settings?lang={lang}&theme={theme}"
    )))
}

/// POST /settings/fonts — upload a font
pub async fn upload_font(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Redirect, AppError> {
    let (staged, name, _) = super::uploads::stage(&state, &mut multipart, 10 * 1024 * 1024).await?;
    let extension = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let id = format!("{}.{}", uuid::Uuid::new_v4(), extension);
    let path = state.fonts_dir().join(&id);
    let result = async {
        let mut header = [0u8; 4];
        fs::File::open(&staged)
            .await?
            .read_exact(&mut header)
            .await
            .map_err(|_| AppError::BadRequest("Invalid font header".into()))?;
        if !matches!(
            (extension.as_str(), &header),
            ("woff2", b"wOF2") | ("ttf", b"\0\x01\0\0") | ("ttf", b"true")
        ) {
            return Err(AppError::BadRequest("Expected a TTF or WOFF2 font".into()));
        }
        fs::rename(&staged, &path).await?;
        let mut inventory = state.fonts.load().await?;
        inventory.fonts.push(crate::db::fonts::Font {
            id: id.clone(),
            name,
        });
        state.fonts.save(&inventory).await?;
        Ok(Redirect::to("/settings"))
    }
    .await;
    if result.is_err() {
        crate::state::remove_if_exists(&staged).await?;
        if !state
            .fonts
            .load()
            .await?
            .fonts
            .iter()
            .any(|font| font.id == id)
        {
            crate::state::remove_if_exists(&path).await?;
        }
    }
    result
}

/// POST /settings/fonts/delete — delete a font
pub async fn delete_font(
    State(state): State<AppState>,
    Form(form): Form<HashMap<String, String>>,
) -> Result<Redirect, AppError> {
    if let Some(name) = form.get("name") {
        known_font(&state, name).await?;
        let mut settings = state.settings.load().await?;
        if settings.font_family == *name {
            settings.font_family.clear();
            state.settings.save(&settings).await?;
            state.invalidate_settings_cache();
        }
        crate::state::remove_if_exists(&state.fonts_dir().join(name)).await?;
        let mut inventory = state.fonts.load().await?;
        inventory.fonts.retain(|font| font.id != *name);
        state.fonts.save(&inventory).await?;
    } else {
        return Err(AppError::BadRequest("Missing font identifier".into()));
    }
    Ok(Redirect::to("/settings"))
}

pub async fn known_font(state: &AppState, id: &str) -> Result<(), AppError> {
    let (stem, extension) = id
        .rsplit_once('.')
        .ok_or_else(|| AppError::BadRequest("Invalid font identifier".into()))?;
    crate::validation::id(stem)?;
    if !["ttf", "woff2"].contains(&extension)
        || !state
            .fonts
            .load()
            .await?
            .fonts
            .iter()
            .any(|font| font.id == id)
    {
        return Err(AppError::NotFound("Font not found".into()));
    }
    let metadata = fs::symlink_metadata(state.fonts_dir().join(id)).await?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(AppError::BadRequest("Invalid font file".into()));
    }
    Ok(())
}

pub async fn font_css(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    known_font(&state, &id).await?;
    let format = if id.ends_with(".woff2") {
        "woff2"
    } else {
        "truetype"
    };
    Ok((
        [("content-type", "text/css; charset=utf-8")],
        format!(
            "@font-face {{ font-family: FnineCustom; src: url('/fonts/{id}') format('{format}'); font-display: swap; }}"
        ),
    ))
}

pub async fn font_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
    request: axum::extract::Request,
) -> Result<Response, AppError> {
    known_font(&state, &id).await?;
    Ok(
        tower_http::services::ServeFile::new(state.fonts_dir().join(id))
            .try_call(request)
            .await?
            .map(axum::body::Body::new),
    )
}
