use std::collections::HashMap;

/// Flattened translations loaded from TOML, keyed by dot-path (e.g., "nav.home").
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Translations {
    map: HashMap<String, String>,
    lang: String,
}

#[allow(dead_code)]
impl Translations {
    pub fn load(lang: &str) -> Self {
        let toml_str = match lang {
            "zh" => include_str!("zh.toml"),
            _ => include_str!("en.toml"),
        };

        let value: toml::Value =
            toml::from_str(toml_str).unwrap_or(toml::Value::Table(Default::default()));
        let mut map = HashMap::new();
        flatten_toml(&value, "", &mut map);

        Self {
            map,
            lang: lang.to_string(),
        }
    }

    pub fn get(&self, key: &str) -> String {
        self.map
            .get(key)
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }

    pub fn lang(&self) -> &str {
        &self.lang
    }
}

#[allow(dead_code)]
fn flatten_toml(value: &toml::Value, prefix: &str, map: &mut HashMap<String, String>) {
    match value {
        toml::Value::Table(table) => {
            for (k, v) in table {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten_toml(v, &key, map);
            }
        }
        toml::Value::String(s) => {
            map.insert(prefix.to_string(), s.clone());
        }
        _ => {
            map.insert(prefix.to_string(), value.to_string());
        }
    }
}
