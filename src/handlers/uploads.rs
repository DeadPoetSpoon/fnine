use crate::{error::AppError, state::AppState};
use axum::extract::Multipart;
use tokio::io::AsyncWriteExt;

pub async fn stage(
    state: &AppState,
    multipart: &mut Multipart,
    limit: usize,
) -> Result<(std::path::PathBuf, String, u64), AppError> {
    let mut field = multipart
        .next_field()
        .await
        .map_err(|error| AppError::BadRequest(error.to_string()))?
        .ok_or_else(|| AppError::BadRequest("No file found".into()))?;
    let name = field
        .file_name()
        .unwrap_or("upload")
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("upload")
        .to_string();
    if name.len() > 255 {
        return Err(AppError::BadRequest("Filename too long".into()));
    }
    let path = state
        .config
        .data_dir
        .join("staging")
        .join(uuid::Uuid::new_v4().to_string());
    let result = async {
        let mut file = tokio::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .await?;
        let mut size = 0;
        while let Some(chunk) = field.chunk().await.map_err(|_| AppError::TooLarge)? {
            size += chunk.len();
            if size > limit {
                return Err(AppError::TooLarge);
            }
            file.write_all(&chunk).await?;
        }
        if size == 0 {
            return Err(AppError::BadRequest("Empty file".into()));
        }
        file.sync_all().await?;
        Ok((path.clone(), name, size as u64))
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&path).await;
    }
    result
}
