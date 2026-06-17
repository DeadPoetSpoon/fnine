use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::fmt;

/// Unified application error type.
#[derive(Debug)]
#[allow(dead_code)] // Will be used in Phase 2+
pub enum AppError {
    Io(std::io::Error),
    TomlSerialize(toml::ser::Error),
    TomlDeserialize(toml::de::Error),
    NotFound(String),
    Internal(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::TomlSerialize(e) => write!(f, "TOML serialize error: {e}"),
            Self::TomlDeserialize(e) => write!(f, "TOML deserialize error: {e}"),
            Self::NotFound(msg) => write!(f, "Not found: {msg}"),
            Self::Internal(msg) => write!(f, "Internal error: {msg}"),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::TomlSerialize(e) => Some(e),
            Self::TomlDeserialize(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<toml::ser::Error> for AppError {
    fn from(e: toml::ser::Error) -> Self {
        Self::TomlSerialize(e)
    }
}

impl From<toml::de::Error> for AppError {
    fn from(e: toml::de::Error) -> Self {
        Self::TomlDeserialize(e)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        tracing::error!("{self}");

        (status, self.to_string()).into_response()
    }
}
