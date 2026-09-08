use anyhow::{Context, Result, anyhow, bail};
use std::time::{Duration, Instant};

use crate::models::{IgdbAuthResponse, IgdbGame};

const AUTH_URL: &str = "https://id.twitch.tv/oauth2/token";
const API_URL: &str = "https://api.igdb.com/v4/games";

pub struct IgdbClient {
    http: reqwest::Client,
    client_id: String,
    client_secret: String,
    access_token: Option<(String, Instant)>,
    token_lifetime: Duration,
    last_request: Instant,
    pub verbose: bool,
}

impl IgdbClient {
    pub fn new(client_id: String, client_secret: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            client_id,
            client_secret,
            access_token: None,
            token_lifetime: Duration::from_secs(0),
            last_request: Instant::now() - Duration::from_secs(1),
            verbose: false,
        }
    }

    async fn ensure_token(&mut self) -> Result<String> {
        let is_valid = self
            .access_token
            .as_ref()
            .map(|(_, at)| at.elapsed() < self.token_lifetime)
            .unwrap_or(false);

        if !is_valid {
            let resp = self
                .http
                .post(AUTH_URL)
                .form(&[
                    ("client_id", self.client_id.as_str()),
                    ("client_secret", self.client_secret.as_str()),
                    ("grant_type", "client_credentials"),
                ])
                .send()
                .await
                .context("Petición de token a Twitch falló")?;

            if !resp.status().is_success() {
                bail!("Auth IGDB falló con código {}", resp.status());
            }

            let auth: IgdbAuthResponse = resp
                .json()
                .await
                .context("Error parseando respuesta de autenticación")?;

            self.token_lifetime = Duration::from_secs(auth.expires_in - 60);
            self.access_token = Some((auth.access_token.clone(), Instant::now()));

            if self.verbose {
                eprintln!("> Token de IGDB obtenido ({}s)", auth.expires_in);
            }
            return Ok(auth.access_token);
        }

        Ok(self.access_token.as_ref().unwrap().0.clone())
    }

    pub async fn search_game(&mut self, name: &str) -> Result<Option<IgdbGame>> {
        let token = self.ensure_token().await?;

        self.rate_limit().await;

        let body = format!(
            "search \"{}\"; fields name,genres.*,themes.*; limit 1;",
            name.replace('"', "")
        );

        let resp = self
            .http
            .post(API_URL)
            .header("Client-ID", &self.client_id)
            .header("Authorization", format!("Bearer {}", token))
            .body(body)
            .send()
            .await
            .context("Petición a API IGDB falló")?;

        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            bail!("Rate limit excedido en IGDB");
        }
        if !resp.status().is_success() {
            bail!("IGDB respondió con código {}", resp.status());
        }

        let games: Vec<IgdbGame> = resp.json().await.context("Error parseando respuesta")?;
        Ok(games.into_iter().next())
    }

    async fn rate_limit(&mut self) {
        let elapsed = self.last_request.elapsed();
        let min_interval = Duration::from_millis(300);
        if elapsed < min_interval {
            tokio::time::sleep(min_interval - elapsed).await;
        }
        self.last_request = Instant::now();
    }
}

pub fn credentials_from_env() -> Result<(String, String)> {
    let client_id = std::env::var("IGDB_CLIENT_ID")
        .map_err(|_| anyhow!("La variable de entorno IGDB_CLIENT_ID no está definida"))?;
    let client_secret = std::env::var("IGDB_CLIENT_SECRET")
        .map_err(|_| anyhow!("La variable de entorno IGDB_CLIENT_SECRET no está definida"))?;
    Ok((client_id, client_secret))
}
