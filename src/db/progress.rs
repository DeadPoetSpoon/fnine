use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressEntry {
    pub book_id: String,
    pub chapter: u32,
    pub position: f64, // 0.0 ~ 1.0
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProgressData {
    pub entries: HashMap<String, ProgressEntry>,
}
