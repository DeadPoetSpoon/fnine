use crate::error::AppError;
use std::path::Path;

pub struct EpubMeta {
    pub title: String,
    pub author: String,
    pub chapter_count: u32,
    pub cover_bytes: Option<Vec<u8>>,
    pub cover_ext: Option<String>,
}

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
    let (cover_bytes, cover_ext) = extract_cover(&epub);

    Ok(EpubMeta {
        title,
        author,
        chapter_count,
        cover_bytes,
        cover_ext,
    })
}

fn extract_cover(epub: &rbook::Epub) -> (Option<Vec<u8>>, Option<String>) {
    if let Some(cover) = epub.manifest().cover_image() {
        let mut buf = Vec::new();
        if cover.copy_bytes(&mut buf).is_ok() && !buf.is_empty() {
            let ext = infer_extension(&buf);
            return (Some(buf), ext);
        }
    }
    if let Some(image) = epub.manifest().images().next() {
        let mut buf = Vec::new();
        if image.copy_bytes(&mut buf).is_ok() && !buf.is_empty() {
            let ext = infer_extension(&buf);
            return (Some(buf), ext);
        }
    }
    (None, None)
}

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

// ── Chapter reading ────────────────────────────────────────

pub struct ChapterEntry {
    pub index: usize,
    pub label: String,
}

pub fn extract_toc(path: &Path) -> Result<Vec<ChapterEntry>, AppError> {
    let epub = rbook::Epub::open(path)
        .map_err(|e| AppError::Internal(format!("EPUB parse error: {e}")))?;
    let spine = epub.spine();
    let n = spine.len();
    let mut chapters = Vec::with_capacity(n);
    for i in 0..n {
        let label = spine
            .get(i)
            .and_then(|s| s.id().map(|id| id.to_owned()))
            .unwrap_or_else(|| format!("Chapter {}", i + 1));
        chapters.push(ChapterEntry { index: i, label });
    }
    Ok(chapters)
}

pub fn read_chapter(path: &Path, index: usize) -> Result<String, AppError> {
    let epub = rbook::Epub::open(path)
        .map_err(|e| AppError::Internal(format!("EPUB parse error: {e}")))?;
    let spine = epub.spine();
    let entry = spine
        .get(index)
        .ok_or_else(|| AppError::NotFound(format!("Chapter {index} not found")))?;
    let manifest_entry = entry
        .manifest_entry()
        .ok_or_else(|| AppError::Internal("Manifest entry not found".into()))?;
    let html = manifest_entry
        .read_str()
        .map_err(|e| AppError::Internal(format!("Read chapter error: {e}")))?;
    Ok(strip_html(&html))
}

// ── HTML stripping ─────────────────────────────────────────

fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut chars = html.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '<' {
            in_tag = true;
            continue;
        }
        if in_tag {
            if c == '>' {
                in_tag = false;
            }
            continue;
        }
        if c == '&' {
            let entity: String = chars.by_ref().take_while(|&ch| ch != ';').collect();
            match entity.as_str() {
                "amp" => out.push('&'),
                "lt" => out.push('<'),
                "gt" => out.push('>'),
                "quot" => out.push('"'),
                "apos" => out.push('\''),
                "nbsp" => out.push(' '),
                _ => {}
            }
            continue;
        }
        out.push(c);
    }

    let cleaned = strip_bracket_refs(&out);

    let mut result = String::with_capacity(cleaned.len());
    let mut blank = 0u32;
    for line in cleaned.lines() {
        let t = line.trim();
        if t.is_empty() {
            blank += 1;
            if blank <= 1 {
                result.push('\n');
            }
        } else {
            blank = 0;
            result.push_str(t);
            result.push('\n');
        }
    }
    result.trim().to_owned()
}

/// Remove `[N]` patterns and collapse surrounding whitespace including newlines.
fn strip_bracket_refs(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut result = String::with_capacity(text.len());
    let mut i = 0;
    while i < n {
        if chars[i] == '[' {
            let start = i;
            i += 1;
            let mut digits = 0;
            while i < n && chars[i].is_ascii_digit() {
                digits += 1;
                i += 1;
            }
            if digits > 0 && i < n && chars[i] == ']' {
                i += 1;
                // Trim trailing whitespace including newlines
                let trimmed = result.trim_end().to_string();
                result.clear();
                result.push_str(&trimmed);
                // Skip all whitespace after [N]
                while i < n && chars[i].is_ascii_whitespace() {
                    i += 1;
                }
                // Insert appropriate separator
                if !result.is_empty() && i < n && !chars[i].is_ascii_whitespace() {
                    result.push(' ');
                } else if !result.is_empty() && i < n {
                    result.push('\n');
                }
                continue;
            }
            result.push('[');
            i = start + 1;
            continue;
        }
        result.push(chars[i]);
        i += 1;
    }
    result
}
