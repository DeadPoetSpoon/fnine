use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub id: String,
    pub book_id: String,
    pub chapter: u32,
    pub selected_text: String,
    pub note: Option<String>,
    pub position_start: f64,
    pub position_end: f64,
    pub color: String,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub needs_review: bool,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub suffix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnnotationsData {
    pub annotations: Vec<Annotation>,
}
