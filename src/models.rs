use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct GameInfo {
    pub app_name: String,
    pub title: String,
    #[serde(default)]
    pub is_installed: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibraryWithArray {
    pub library: Vec<GameInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GamesWithArray {
    pub games: Vec<GameInfo>,
}

#[derive(Debug, Clone)]
pub struct LibraryGame {
    pub app_name: String,
    pub title: String,
    pub runner: String,
    pub is_installed: bool,
}

impl GameInfo {
    pub fn to_library_game(&self, runner: &str) -> LibraryGame {
        LibraryGame {
            app_name: self.app_name.clone(),
            title: self.title.clone(),
            runner: runner.to_string(),
            is_installed: self.is_installed,
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct IgdbIdName {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IgdbSlugName {
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IgdbGame {
    #[serde(default)]
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub genres: Vec<IgdbIdName>,
    #[serde(default)]
    pub themes: Vec<IgdbIdName>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IgdbAuthResponse {
    pub access_token: String,
    pub expires_in: u64,
}
