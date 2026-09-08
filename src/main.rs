mod heroic;
mod heroic_config;
mod igdb;
mod models;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::heroic::HeroicLibrary;
use crate::heroic_config::HeroicConfig;
use crate::igdb::{IgdbClient, credentials_from_env};

#[derive(Parser)]
#[command(
    name = "herocat",
    about = "Categoriza los juegos de Heroic Games Launcher usando la base de datos IGDB"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Lista los juegos de la biblioteca de Heroic
    List {
        /// Filtrar por runner: legendary, gog, nile, sideload
        #[arg(long)]
        runner: Option<String>,
        /// Solo juegos instalados
        #[arg(long)]
        installed: bool,
        /// Buscar por título (insensible a mayúsculas, substring)
        #[arg(long)]
        search: Option<String>,
    },
    /// Busca cada juego en IGDB y asigna sus genres/themes como categorías
    Generate {
        /// Número máximo de juegos a procesar (0 = todos)
        #[arg(long, default_value_t = 0)]
        limit: usize,
    },
    /// Asigna manualmente un juego a una categoría
    Assign {
        /// Categoría (ej: "RPG", "Shooter")
        #[arg(long)]
        category: String,
        /// Clave del juego: app_name_runner (ej: "1133514031_gog")
        #[arg(long)]
        game: String,
    },
    /// Muestra las categorías existentes y cuántos juegos tiene cada una
    Categories,
    /// Muestra la ruta de configuración de Heroic detectada
    Config,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::List {
            runner,
            installed,
            search,
        } => cmd_list(runner, installed, search)?,
        Commands::Generate { limit } => cmd_generate(limit).await?,
        Commands::Assign { category, game } => cmd_assign(&category, &game)?,
        Commands::Categories => cmd_categories()?,
        Commands::Config => cmd_config()?,
    }
    Ok(())
}

fn cmd_list(runner: Option<String>, installed: bool, search: Option<String>) -> Result<()> {
    let library = HeroicLibrary::discover()?;
    let query = search.map(|s| s.to_lowercase());
    let games: Vec<_> = library
        .games
        .iter()
        .filter(|g| runner.as_deref().is_none_or(|r| g.runner == r))
        .filter(|g| !installed || g.is_installed)
        .filter(|g| {
            query
                .as_ref()
                .is_none_or(|q| g.title.to_lowercase().contains(q))
        })
        .collect();

    if games.is_empty() {
        println!("No se encontraron juegos.");
        return Ok(());
    }

    let width = games.iter().map(|g| g.title.len()).max().unwrap_or(0);
    let total = games.len();
    for g in games {
        let status = if g.is_installed { "✓" } else { "·" };
        println!(
            "{status} {:<width$}  {:<10}  {}",
            g.title, g.runner, g.app_name
        );
    }
    println!("\nTotal: {total}");
    Ok(())
}

fn cmd_assign(category: &str, game: &str) -> Result<()> {
    let library = HeroicLibrary::discover()?;
    let resolve = library
        .games
        .iter()
        .find(|g| format!("{}_{}", g.app_name, g.runner) == game)
        .or_else(|| {
            library
                .games
                .iter()
                .find(|g| g.title.eq_ignore_ascii_case(game))
        });

    let (key, name) = match resolve {
        Some(g) => (format!("{}_{}", g.app_name, g.runner), g.title.clone()),
        None => {
            println!("Juego no encontrado: {}", game);
            return Ok(());
        }
    };

    let mut config = HeroicConfig::load(&library.root)?;
    config.add_game_to_category(category, &key);
    config.save()?;
    println!("'{name}' añadido a la categoría '{category}'");
    Ok(())
}

async fn cmd_generate(limit: usize) -> Result<()> {
    let (client_id, client_secret) = credentials_from_env()?;
    let library = HeroicLibrary::discover()?;
    let mut igdb = IgdbClient::new(client_id, client_secret);
    igdb.verbose = true;

    let games: Vec<_> = library.games.clone();
    let count = if limit > 0 {
        limit.min(games.len())
    } else {
        games.len()
    };

    let mut config = HeroicConfig::load(&library.root)?;
    let mut categories = config.categories();
    let mut classified = 0usize;
    let mut misses: Vec<&models::LibraryGame> = Vec::new();

    println!("Procesando {count} juegos...");
    for game in &games[..count] {
        let key = format!("{}_{}", game.app_name, game.runner);
        match igdb.search_game(&game.title).await {
            Ok(Some(found)) => {
                if igdb.verbose {
                    eprintln!("> '{}' -> '{}'", game.title, found.name);
                }
                let cats: Vec<String> = found
                    .genres
                    .iter()
                    .chain(found.themes.iter())
                    .map(|c| c.name.clone())
                    .collect();
                if !cats.is_empty() {
                    for cat in cats {
                        let arr = categories
                            .entry(cat.clone())
                            .or_insert_with(|| serde_json::json!([]));
                        let arr = arr.as_array_mut().unwrap();
                        if !arr.iter().any(|v| v.as_str() == Some(&key)) {
                            arr.push(serde_json::Value::String(key.clone()));
                        }
                    }
                    classified += 1;
                } else {
                    misses.push(game);
                }
            }
            Ok(None) => misses.push(game),
            Err(e) => eprintln!("Error con '{}': {}", game.title, e),
        }
    }

    config.set_categories(categories);
    config.save()?;
    println!("\nClasificados: {classified} | Sin datos IGDB: {}", misses.len());

    if !misses.is_empty() {
        println!("\nJuegos sin datos en IGDB:");
        for g in &misses {
            println!("  {:<10}  {}", g.runner, g.title);
        }
    }

    println!("Config guardado en {}", config.path.display());
    Ok(())
}

fn cmd_categories() -> Result<()> {
    let library = HeroicLibrary::discover()?;
    let config = HeroicConfig::load(&library.root)?;
    let categories = config.categories();

    if categories.is_empty() {
        println!("No hay categorías. Usa 'herocat generate' o 'herocat assign'.");
        return Ok(());
    }

    let mut sorted: Vec<_> = categories.into_iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    for (name, keys) in &sorted {
        let n = keys.as_array().map_or(0, |a| a.len());
        println!("{name} ({n})");
    }
    Ok(())
}

fn cmd_config() -> Result<()> {
    let library = HeroicLibrary::discover()?;
    let config = HeroicConfig::load(&library.root)?;
    println!("Raíz config:  {}", library.root.display());
    println!("config.json:  {}", config.path.display());
    Ok(())
}
