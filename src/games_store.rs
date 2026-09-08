use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::catalog::user_games_path;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GamesData {
    pub version: u32,
    #[serde(default)]
    pub games: BTreeMap<String, GameEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GameEntry {
    pub title: String,
    #[serde(default)]
    pub sources: JsonMap<String, Value>,
    #[serde(default)]
    pub categories: Vec<String>,
}

pub struct GamesStore {
    pub path: PathBuf,
    pub data: GamesData,
}

impl GamesStore {
    pub fn load() -> Result<Self> {
        Self::load_from(user_games_path())
    }

    pub fn load_from(path: PathBuf) -> Result<Self> {
        if !path.exists() {
            return Ok(Self {
                path,
                data: GamesData {
                    version: 1,
                    games: BTreeMap::new(),
                },
            });
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("No se pudo leer {}", path.display()))?;
        let data: GamesData = serde_json::from_str(&content)
            .with_context(|| format!("Error parseando {}", path.display()))?;
        Ok(Self { path, data })
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content =
            serde_json::to_string_pretty(&self.data).context("Error serializando games.json")?;
        std::fs::write(&self.path, content)
            .with_context(|| format!("No se pudo escribir {}", self.path.display()))?;
        Ok(())
    }

    pub fn game(&mut self, app_key: &str) -> &mut GameEntry {
        self.data
            .games
            .entry(app_key.to_string())
            .or_insert_with(|| GameEntry {
                title: String::new(),
                sources: JsonMap::new(),
                categories: Vec::new(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn round_trip() {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("herocat_games_{ts}.json"));

        let mut store = GamesStore::load_from(path.clone()).unwrap();
        let entry = store.game("1133514031_gog");
        entry.title = "Prey".to_string();
        entry.sources.insert("igdb".into(), 15589.into());
        entry.categories = vec!["shooter".into(), "scifi".into()];
        store.save().unwrap();

        let reloaded = GamesStore::load_from(path.clone()).unwrap();
        let g = reloaded.data.games.get("1133514031_gog").unwrap();
        assert_eq!(g.title, "Prey");
        assert_eq!(g.sources.get("igdb"), Some(&15589.into()));
        assert_eq!(g.categories, vec!["shooter", "scifi"]);

        std::fs::remove_file(path).ok();
    }
}
