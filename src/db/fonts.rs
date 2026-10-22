use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Font {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct FontsData {
    pub fonts: Vec<Font>,
}
