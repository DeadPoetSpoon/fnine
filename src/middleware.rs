use crate::state::AppState;
use axum::{
    extract::{Request, State},
    http::{Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

pub async fn coordinate(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    if !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) {
        let origin = request
            .headers()
            .get("origin")
            .and_then(|value| value.to_str().ok());
        let host = request
            .headers()
            .get("host")
            .and_then(|value| value.to_str().ok())
            .or_else(|| {
                request
                    .uri()
                    .authority()
                    .map(|authority| authority.as_str())
            });
        let scheme = request
            .uri()
            .scheme_str()
            .or_else(|| {
                request
                    .headers()
                    .get("x-forwarded-proto")
                    .and_then(|value| value.to_str().ok())
            })
            .unwrap_or("http");
        if request
            .headers()
            .get("sec-fetch-site")
            .is_some_and(|site| site == "cross-site")
            || origin.is_some_and(|origin| {
                origin.parse::<axum::http::Uri>().map_or(true, |uri| {
                    !matches!(uri.scheme_str(), Some("http" | "https"))
                        || uri.scheme_str() != Some(scheme)
                        || uri.authority().map(|value| value.as_str()) != host
                        || uri.path() != "/"
                        || uri.query().is_some()
                })
            })
        {
            return (StatusCode::FORBIDDEN, "Cross-origin mutation rejected").into_response();
        }
    }
    let _guard = state.coordinator.lock().await;
    let settings = match state.load_settings().await {
        Ok(settings) => settings,
        Err(error) => return error.into_response(),
    };
    let query = request.uri().query().unwrap_or("");
    let params = query
        .split('&')
        .filter_map(|part| part.split_once('='))
        .collect::<std::collections::HashMap<_, _>>();
    for (name, allowed) in [
        ("lang", &["en", "zh"][..]),
        ("theme", &["light", "dark", "sepia"][..]),
    ] {
        if params
            .get(name)
            .is_some_and(|value| !allowed.contains(value))
        {
            return (StatusCode::BAD_REQUEST, "Unsupported language or theme").into_response();
        }
    }
    let mut suffix = query.to_string();
    if !params.contains_key("lang") {
        suffix.push_str(&format!(
            "&lang={}",
            if settings.language == "en" {
                "en"
            } else {
                "zh"
            }
        ));
    }
    if !params.contains_key("theme") {
        suffix.push_str(&format!(
            "&theme={}",
            if ["light", "dark", "sepia"].contains(&settings.theme.as_str()) {
                settings.theme.as_str()
            } else {
                "light"
            }
        ));
    }
    let uri = format!(
        "{}?{}",
        request.uri().path(),
        suffix.trim_start_matches('&')
    );
    match uri.parse() {
        Ok(uri) => *request.uri_mut() = uri,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    }
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        "x-content-type-options",
        axum::http::HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        "referrer-policy",
        axum::http::HeaderValue::from_static("same-origin"),
    );
    response
}
