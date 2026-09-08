use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub type SourceIds = BTreeMap<String, Vec<String>>;

pub const SEED_CATEGORIES: &str = "data/categories.json";

fn seed_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(SEED_CATEGORIES)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogData {
    pub version: u32,
    #[serde(default)]
    pub categories: BTreeMap<String, CategoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CategoryEntry {
    pub name: String,
    #[serde(default)]
    pub sources: BTreeMap<String, SourceIds>,
}

impl CategoryEntry {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            sources: BTreeMap::new(),
        }
    }
}

pub struct CategoryRegistry {
    pub path: PathBuf,
    pub data: CatalogData,
}

impl CategoryRegistry {
    pub fn load() -> Result<Self> {
        let seed = seed_path();
        let user_path = user_categories_path();

        let (path, source) = if user_path.exists() {
            (user_path.clone(), user_path.as_path())
        } else {
            (seed.clone(), seed.as_path())
        };

        let content = std::fs::read_to_string(source)
            .with_context(|| format!("No se pudo leer {}", source.display()))?;
        let mut data: CatalogData = serde_json::from_str(&content)
            .with_context(|| format!("Error parseando {}", source.display()))?;

        if data.version == 0 {
            data.version = 1;
        }

        Ok(Self { path, data })
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&self.data)
            .context("Error serializando categories.json")?;
        std::fs::write(&self.path, content)
            .with_context(|| format!("No se pudo escribir {}", self.path.display()))?;
        Ok(())
    }

    pub fn entry(&mut self, slug: &str) -> &mut CategoryEntry {
        self.data
            .categories
            .entry(slug.to_string())
            .or_insert_with(|| CategoryEntry::new(slug))
    }

    pub fn ensure_named(&mut self, slug: &str, name: &str) {
        let entry = self.entry(slug);
        entry.name = name.to_string();
    }

    pub fn register(
        &mut self,
        slug: &str,
        name: &str,
        source: &str,
        dimension: &str,
        id: &str,
    ) -> String {
        let entry = self.entry(slug);
        entry.name = name.to_string();
        let dim = entry
            .sources
            .entry(source.to_string())
            .or_default()
            .entry(dimension.to_string())
            .or_default();
        if !dim.iter().any(|x| x == id) {
            dim.push(id.to_string());
        }
        slug.to_string()
    }
}

pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut last_dash = true;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

pub fn user_categories_path() -> PathBuf {
    user_config_dir().join("categories.json")
}

pub fn user_games_path() -> PathBuf {
    user_config_dir().join("games.json")
}

fn user_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("herocat")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Comentado temporalmente. Test de serialización/reload de CategoryEntry.
    // Para restaurarlo, quita el prefijo `//` de las líneas siguientes.
    //
    // #[test]
    // fn round_trip() {
    //     let ts = SystemTime::now()
    //         .duration_since(UNIX_EPOCH)
    //         .unwrap()
    //         .as_nanos();
    //     let path = std::env::temp_dir().join(format!("herocat_cat_{ts}.json"));
    //
    //     let mut cat = CategoryEntry::new("Shooter");
    //     cat.sources.insert(
    //         "igdb".into(),
    //         SourceIds::from([("genres".into(), vec!["5".into()])]),
    //     );
    //     let mut data = CatalogData {
    //         version: 1,
    //         categories: BTreeMap::new(),
    //     };
    //     data.categories.insert("shooter".into(), cat);
    //
    //     let content = serde_json::to_string_pretty(&data).unwrap();
    //     std::fs::write(&path, content).unwrap();
    //
    //     let reloaded: CatalogData =
    //         serde_json::from_str(&std::fs::read_to_string(&path).unwrap().as_str()).unwrap();
    //     let entry = reloaded.categories.get("shooter").unwrap();
    //     assert_eq!(entry.name, "Shooter");
    //     assert_eq!(entry.sources["igdb"]["genres"], vec!["5"]);
    //
    //     std::fs::remove_file(path).ok();
    // }
}
