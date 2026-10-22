use std::path::PathBuf;

/// Application configuration loaded from environment variables.
#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub data_dir: PathBuf,
}

impl Config {
    pub fn from_env() -> Result<Self, crate::error::AppError> {
        let port = std::env::var("FNINE_PORT")
            .unwrap_or_else(|_| "3000".into())
            .parse()
            .map_err(|_| crate::error::AppError::BadRequest("Invalid FNINE_PORT".into()))?;
        let host = std::env::var("FNINE_HOST").unwrap_or_else(|_| "127.0.0.1".into());
        host.parse::<std::net::IpAddr>().map_err(|_| {
            crate::error::AppError::BadRequest("FNINE_HOST must be an IP address".into())
        })?;
        Ok(Self {
            host,
            port,
            data_dir: std::env::var("FNINE_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("./data")),
        })
    }
}
