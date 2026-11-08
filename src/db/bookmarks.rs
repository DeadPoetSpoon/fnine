use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: String,
    pub book_id: String,
    pub chapter: u32,
    pub position: f64,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct BookmarksData {
    pub bookmarks: Vec<Bookmark>,
}
