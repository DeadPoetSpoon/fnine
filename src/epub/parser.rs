use crate::error::AppError;
use std::path::Path;

/// Metadata extracted from an EPUB file.
pub struct EpubMeta {
    pub title: String,
    pub author: String,
    pub chapter_count: u32,
    pub cover_bytes: Option<Vec<u8>>,
    pub cover_ext: Option<String>,
}

/// Extract metadata from an EPUB file at `path`.
pub fn extract_metadata(path: &Path) -> Result<EpubMeta, AppError> {
    let epub = rbook::Epub::open(path)
        .map_err(|e| AppError::Internal(format!("EPUB parse error: {e}")))?;

    let metadata = epub.metadata();

    let title = metadata
        .title()
        .map(|t| t.value().to_owned())
        .unwrap_or_else(|| "Unknown Title".into());

    let author = metadata
        .creators()
        .next()
        .map(|c| c.value().to_owned())
        .unwrap_or_else(|| "Unknown Author".into());

    let chapter_count = epub.spine().len() as u32;

    // Try to extract the cover image via manifest
    let (cover_bytes, cover_ext) = extract_cover(&epub);

    Ok(EpubMeta {
        title,
        author,
        chapter_count,
        cover_bytes,
        cover_ext,
    })
}

/// Attempt to locate and extract the cover image via the manifest.
fn extract_cover(epub: &rbook::Epub) -> (Option<Vec<u8>>, Option<String>) {
    // Try the manifest's cover_image() method
    if let Some(cover) = epub.manifest().cover_image() {
        let mut buf = Vec::new();
        if cover.copy_bytes(&mut buf).is_ok() && !buf.is_empty() {
            let ext = infer_extension(&buf);
            return (Some(buf), ext);
        }
    }

    // Fallback: pick the first image from the manifest
    if let Some(image) = epub.manifest().images().next() {
        let mut buf = Vec::new();
        if image.copy_bytes(&mut buf).is_ok() && !buf.is_empty() {
            let ext = infer_extension(&buf);
            return (Some(buf), ext);
        }
    }

    (None, None)
}

/// Guess image extension from magic bytes.
fn infer_extension(data: &[u8]) -> Option<String> {
    if data.len() < 4 {
        return None;
    }
    if data.starts_with(b"\xFF\xD8\xFF") {
        Some("jpg".into())
    } else if data.starts_with(b"\x89PNG\r\n\x1A\n") {
        Some("png".into())
    } else if data.starts_with(b"GIF8") {
        Some("gif".into())
    } else if data.starts_with(b"RIFF") && data.len() > 8 && &data[8..12] == b"WEBP" {
        Some("webp".into())
    } else {
        None
    }
}
