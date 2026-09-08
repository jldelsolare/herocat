use anyhow::{Context, Result};
use serde_json::{Map, Value, json};
use std::path::PathBuf;

pub struct HeroicConfig {
    pub path: PathBuf,
    pub data: Value,
}

impl HeroicConfig {
    pub fn load(root: &std::path::Path) -> Result<Self> {
        let path = root.join("store/config.json");
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("No se pudo leer {}", path.display()))?;
        let data = serde_json::from_str(&content)
            .with_context(|| format!("Error parseando {}", path.display()))?;
        Ok(Self { path, data })
    }

    pub fn categories(&self) -> Map<String, Value> {
        self.data
            .pointer("/games/customCategories")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default()
    }

    pub fn set_categories(&mut self, categories: Map<String, Value>) {
        self.ensure_games_object();
        self.data["games"]["customCategories"] = Value::Object(categories);
    }

    pub fn add_game_to_category(&mut self, category: &str, app_key: &str) {
        self.ensure_games_object();

        let games = self.data["games"].as_object_mut().unwrap();
        let cats = games
            .entry("customCategories".to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        let cats = cats.as_object_mut().unwrap();

        let arr = cats.entry(category.to_string()).or_insert_with(|| json!([]));
        let arr = arr.as_array_mut().unwrap();

        if !arr.iter().any(|v| v.as_str() == Some(app_key)) {
            arr.push(Value::String(app_key.to_string()));
        }
    }

    fn ensure_games_object(&mut self) {
        if !self.data.get("games").is_some_and(Value::is_object) {
            self.data["games"] = Value::Object(Map::new());
        }
    }

    pub fn save(&self) -> Result<()> {
        let content =
            serde_json::to_string_pretty(&self.data).context("Error serializando config.json")?;
        std::fs::write(&self.path, content)
            .with_context(|| format!("No se pudo escribir {}", self.path.display()))?;
        Ok(())
    }
}
