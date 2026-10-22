use crate::{
    db::{annotations::AnnotationsData, books::Book, progress::ProgressEntry, store::Store},
    error::AppError,
    state::AppState,
};
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
struct Backup {
    book: Option<Book>,
    progress: Option<ProgressEntry>,
    annotations: AnnotationsData,
}

fn map_position(old: &str, new: &str, position: f64) -> f64 {
    if position <= 0.0 {
        return 0.0;
    }
    if position >= 1.0 {
        return 1.0;
    }
    let old_chars = old.chars().collect::<Vec<_>>();
    let target_units = (old.encode_utf16().count() as f64 * position) as usize;
    let mut units = 0;
    let mut offset: usize = 0;
    for character in &old_chars {
        if units >= target_units {
            break;
        }
        units += character.len_utf16();
        offset += 1;
    }
    for width in [32, 16, 8] {
        let start = offset.saturating_sub(width / 2);
        let end = (start + width).min(old_chars.len());
        let needle = old_chars[start..end].iter().collect::<String>();
        if needle.trim().is_empty() {
            continue;
        }
        let mut matches = new.match_indices(&needle);
        if let Some((found, _)) = matches.next().filter(|_| matches.next().is_none()) {
            let prefix = new[..found].encode_utf16().count();
            let within = old_chars[start..offset.min(end)]
                .iter()
                .map(|character| character.len_utf16())
                .sum::<usize>();
            return ((prefix + within) as f64 / new.encode_utf16().count().max(1) as f64)
                .clamp(0.0, 1.0);
        }
    }
    0.0
}

pub async fn upgrade(state: &AppState, book: &Book) -> Result<(), AppError> {
    if book.normalization_version >= 2 {
        return Ok(());
    }
    let backup_store = Store::<Backup>::new(
        state
            .config
            .data_dir
            .join("backups")
            .join(format!("{}-normalization-v1.toml", book.id)),
    );
    let mut backup = backup_store.load().await?;
    if backup.book.is_none() {
        backup = Backup {
            book: Some(book.clone()),
            progress: state.progress.load().await?.entries.get(&book.id).cloned(),
            annotations: state.annotations_store(&book.id).load().await?,
        };
        backup_store.save(&backup).await?;
    }
    let path = state.books_dir().join(format!("{}.epub", book.id));
    let count = book.chapter_count;
    let permit = state
        .parsers
        .clone()
        .acquire_owned()
        .await
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let migrated = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut progress = backup.progress;
        let mut annotations = backup.annotations;
        for chapter in 0..count {
            let needs_progress = progress
                .as_ref()
                .is_some_and(|entry| entry.chapter == chapter || entry.last_chapter == chapter);
            if !needs_progress
                && !annotations
                    .annotations
                    .iter()
                    .any(|annotation| annotation.chapter == chapter)
            {
                continue;
            }
            let new = super::parser::read_chapter(&path, chapter as usize)?.text();
            let old = super::parser::legacy_chapter(&path, chapter as usize)?;
            if let Some(entry) = &mut progress {
                if entry.chapter == chapter {
                    entry.position = map_position(&old, &new, entry.position);
                }
                if entry.last_chapter == chapter {
                    entry.last_position = map_position(&old, &new, entry.last_position);
                }
            }
            for annotation in annotations
                .annotations
                .iter_mut()
                .filter(|annotation| annotation.chapter == chapter)
            {
                let quote = annotation.selected_text.trim();
                let matches = if quote.is_empty() {
                    Vec::new()
                } else {
                    new.match_indices(quote)
                        .take(2)
                        .map(|(offset, _)| offset)
                        .collect::<Vec<_>>()
                };
                annotation.needs_review = matches.len() != 1;
                if let [start] = matches.as_slice() {
                    let total = new.encode_utf16().count().max(1) as f64;
                    annotation.position_start = new[..*start].encode_utf16().count() as f64 / total;
                    annotation.position_end =
                        new[..*start + quote.len()].encode_utf16().count() as f64 / total;
                    annotation.prefix = new[..*start]
                        .chars()
                        .rev()
                        .take(24)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                    annotation.suffix = new[*start + quote.len()..].chars().take(24).collect();
                }
            }
        }
        Ok::<_, AppError>((progress, annotations))
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))??;
    state.annotations_store(&book.id).save(&migrated.1).await?;
    let mut progress = state.progress.load().await?;
    if let Some(entry) = migrated.0 {
        progress.entries.insert(book.id.clone(), entry);
    }
    state.progress.save(&progress).await?;
    let mut books = state.books.load().await?;
    if let Some(entry) = books.books.iter_mut().find(|entry| entry.id == book.id) {
        entry.normalization_version = 2;
    }
    state.books.save(&books).await?;
    state.invalidate_books_cache();
    state.invalidate_progress_cache();
    Ok(())
}
