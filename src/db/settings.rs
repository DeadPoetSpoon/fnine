use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_lang")]
    pub language: String,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_font_size")]
    pub font_size: u32,
    #[serde(default = "default_font_family")]
    pub font_family: String,
}

fn default_lang() -> String {
    "zh".into()
}
fn default_theme() -> String {
    "light".into()
}
fn default_font_size() -> u32 {
    18
}
fn default_font_family() -> String {
    String::new()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: default_lang(),
            theme: default_theme(),
            font_size: default_font_size(),
            font_family: default_font_family(),
        }
    }
}
