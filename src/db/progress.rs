use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressEntry {
    pub book_id: String,
    /// Manually saved reading position.
    pub chapter: u32,
    pub position: f64, // 0.0 ~ 1.0
    pub updated_at: DateTime<Utc>,
    /// Legacy fields retained for old progress and normalization backups.
    #[serde(default)]
    pub last_chapter: u32,
    #[serde(default)]
    pub last_position: f64, // 0.0 ~ 1.0
    #[serde(default)]
    pub last_updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProgressData {
    pub entries: HashMap<String, ProgressEntry>,
}
