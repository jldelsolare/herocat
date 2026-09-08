use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::models::{GameInfo, GamesWithArray, LibraryGame, LibraryWithArray};

pub const FLATPAK_CONFIG: &str = ".var/app/com.heroicgameslauncher.hgl/config/heroic";
pub const NATIVE_CONFIG: &str = ".config/heroic";

pub struct HeroicLibrary {
    pub root: PathBuf,
    pub games: Vec<LibraryGame>,
}

impl HeroicLibrary {
    pub fn discover() -> Result<Self> {
        let root = find_heroic_root().context(
            "No se encontró la configuración de Heroic. Asegúrate de que Heroic esté instalado.",
        )?;
        let games = read_all_libraries(&root)?;
        Ok(Self { root, games })
    }
}

pub fn find_heroic_root() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let flatpak = home.join(FLATPAK_CONFIG);
    if flatpak.join("store_cache").is_dir() {
        return Some(flatpak);
    }
    let native = home.join(NATIVE_CONFIG);
    if native.join("store_cache").is_dir() {
        return Some(native);
    }
    None
}

fn read_all_libraries(root: &Path) -> Result<Vec<LibraryGame>> {
    let mut games = Vec::new();

    let store_cache = root.join("store_cache");
    if let Ok(epic) = read_epic(&store_cache.join("legendary_library.json")) {
        games.extend(epic);
    }
    if let Ok(gog) = read_gog(&store_cache.join("gog_library.json")) {
        games.extend(gog);
    }
    if let Ok(nile) = read_nile(&store_cache.join("nile_library.json")) {
        games.extend(nile);
    }

    let sideload = read_sideload(&root.join("sideload_apps/library.json"));
    games.extend(sideload.unwrap_or_default());

    Ok(games)
}

fn read_epic(path: &Path) -> Result<Vec<LibraryGame>> {
    let content = std::fs::read_to_string(path)?;
    let data: LibraryWithArray =
        serde_json::from_str(&content).context("Error parseando legendary_library.json")?;
    Ok(data
        .library
        .iter()
        .map(|g: &GameInfo| g.to_library_game("legendary"))
        .collect())
}

fn read_gog(path: &Path) -> Result<Vec<LibraryGame>> {
    let content = std::fs::read_to_string(path)?;
    let data: GamesWithArray =
        serde_json::from_str(&content).context("Error parseando gog_library.json")?;
    Ok(data
        .games
        .iter()
        .map(|g: &GameInfo| g.to_library_game("gog"))
        .collect())
}

fn read_nile(path: &Path) -> Result<Vec<LibraryGame>> {
    let content = std::fs::read_to_string(path)?;
    let data: LibraryWithArray =
        serde_json::from_str(&content).context("Error parseando nile_library.json")?;
    Ok(data
        .library
        .iter()
        .map(|g: &GameInfo| g.to_library_game("nile"))
        .collect())
}

fn read_sideload(path: &Path) -> Result<Vec<LibraryGame>> {
    let content = std::fs::read_to_string(path)?;
    let data: GamesWithArray =
        serde_json::from_str(&content).context("Error parseando sideload library.json")?;
    Ok(data
        .games
        .iter()
        .map(|g: &GameInfo| g.to_library_game("sideload"))
        .collect())
}
