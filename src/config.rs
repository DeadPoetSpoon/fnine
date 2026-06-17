use std::path::PathBuf;

/// Application configuration loaded from environment variables.
#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub data_dir: PathBuf,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("FNINE_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: std::env::var("FNINE_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            data_dir: std::env::var("FNINE_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("./data")),
        }
    }
}
