use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Book metadata stored in `data/books.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Book {
    pub id: String, // UUID v4
    pub title: String,
    pub author: String,
    pub cover_ext: Option<String>, // Cover image file extension ("jpg", "png", …)
    pub chapter_count: u32,
    pub file_name: String, // Original filename
    pub file_size: u64,
    pub uploaded_at: DateTime<Utc>,
}

/// Top-level structure for `data/books.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BooksData {
    pub books: Vec<Book>,
}
