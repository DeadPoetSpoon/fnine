use crate::error::AppError;
use serde::{Serialize, de::DeserializeOwned};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Generic TOML-backed persistent store.
///
/// Data is lazily loaded on first access (via foyer cache in later phases).
/// Writes use atomic rename to prevent corruption.
#[allow(dead_code)] // Will be used in Phase 2+
pub struct Store<T> {
    path: PathBuf,
    _marker: PhantomData<T>,
}

#[allow(dead_code)] // Will be used in Phase 2+
impl<T> Store<T>
where
    T: Serialize + DeserializeOwned + Default,
{
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            _marker: PhantomData,
        }
    }

    /// Load data from the TOML file.
    /// Returns `T::default()` if the file does not exist.
    pub async fn load(&self) -> Result<T, AppError> {
        if fs::symlink_metadata(&self.path)
            .await
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(AppError::Internal("Store path cannot be a symlink".into()));
        }
        match fs::read_to_string(&self.path).await {
            Ok(content) => {
                let data: T = toml::from_str(&content)?;
                Ok(data)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
            Err(e) => Err(AppError::Io(e)),
        }
    }

    /// Atomically save data to the TOML file.
    ///
    /// Writes to a temporary file first, then renames it over the target path
    /// to avoid corruption on crash.
    pub async fn save(&self, data: &T) -> Result<(), AppError> {
        let serialized = toml::to_string_pretty(data)?;

        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Write to temp file, then rename atomically
        let tmp_path = self
            .path
            .with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        let result = async {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tmp_path)
                .await?;
            file.write_all(serialized.as_bytes()).await?;
            file.sync_all().await?;
            drop(file);
            fs::rename(&tmp_path, &self.path).await?;
            if let Some(parent) = self.path.parent() {
                fs::File::open(parent).await?.sync_all().await?;
            }
            Ok::<_, std::io::Error>(())
        }
        .await;
        if result.is_err() {
            let _ = fs::remove_file(&tmp_path).await;
        }
        result?;

        Ok(())
    }
}
