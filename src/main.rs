mod catalog;
mod games_store;
mod heroic;
mod heroic_config;
mod igdb;
mod models;

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::{Map, Value, json};

use crate::catalog::{CategoryRegistry, slugify};
use crate::games_store::GamesStore;
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
        /// Clave del juego a procesar (app_name_runner o título). Si se indica, solo ese juego
        #[arg(value_name = "GAME")]
        game: Option<String>,
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
        Commands::Generate { limit, game } => cmd_generate(limit, game.as_deref()).await?,
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

    let mut catalog = CategoryRegistry::load()?;
    let mut store = GamesStore::load()?;
    let mut config = HeroicConfig::load(&library.root)?;

    let slug = slugify(category);
    catalog.ensure_named(&slug, category);
    catalog.save()?;

    let entry = store.game(&key);
    if entry.title.is_empty() {
        entry.title = name.clone();
    }
    push_unique(&mut entry.categories, slug);
    store.save()?;

    let categories = rebuild_categories(&store, &catalog);
    config.set_categories(categories);
    config.save()?;

    println!("'{name}' añadido a la categoría '{category}'");
    Ok(())
}

async fn cmd_generate(limit: usize, game_filter: Option<&str>) -> Result<()> {
    let (client_id, client_secret) = credentials_from_env()?;
    let library = HeroicLibrary::discover()?;
    let mut igdb = IgdbClient::new(client_id, client_secret);
    igdb.verbose = true;

    let games: Vec<models::LibraryGame> = if let Some(filter) = game_filter {
        let found: Vec<models::LibraryGame> = library
            .games
            .iter()
            .filter(|g| {
                format!("{}_{}", g.app_name, g.runner) == filter
                    || g.title.eq_ignore_ascii_case(filter)
            })
            .cloned()
            .collect();
        if found.is_empty() {
            println!("Juego no encontrado: {filter}");
            return Ok(());
        }
        found
    } else {
        library.games.clone()
    };

    let count = if game_filter.is_some() {
        games.len()
    } else if limit > 0 {
        limit.min(games.len())
    } else {
        games.len()
    };

    let mut config = HeroicConfig::load(&library.root)?;
    let mut catalog = CategoryRegistry::load()?;
    let mut store = GamesStore::load()?;
    let mut classified = 0usize;
    let mut by_id = 0usize;
    let mut misses: Vec<&models::LibraryGame> = Vec::new();

    println!("Procesando {count} juegos...");
    for game in &games[..count] {
        let key = format!("{}_{}", game.app_name, game.runner);
        let mapped_id = store
            .data
            .games
            .get(&key)
            .and_then(|e| e.sources.get("igdb"))
            .and_then(serde_json::Value::as_u64);

        let result = if let Some(id) = mapped_id {
            by_id += 1;
            match igdb.get_game_by_id(id).await {
                Ok(Some(g)) => Ok(Some(g)),
                Ok(None) => igdb.search_game(&game.title).await,
                Err(e) => Err(e),
            }
        } else {
            igdb.search_game(&game.title).await
        };

        match result {
            Ok(Some(found)) => {
                if igdb.verbose {
                    eprintln!("> '{}' -> '{}'", game.title, found.name);
                }

                let mut slugs: Vec<String> = Vec::new();
                for c in found.genres.iter() {
                    let slug = catalog.register(
                        &slugify(&c.name),
                        &c.name,
                        "igdb",
                        "genres",
                        &c.id.to_string(),
                    );
                    push_unique(&mut slugs, slug);
                }
                for c in found.themes.iter() {
                    let slug = catalog.register(
                        &slugify(&c.name),
                        &c.name,
                        "igdb",
                        "themes",
                        &c.id.to_string(),
                    );
                    push_unique(&mut slugs, slug);
                }

                if !slugs.is_empty() {
                    classified += 1;
                } else {
                    misses.push(game);
                }

                let entry = store.game(&key);
                if entry.title.is_empty() {
                    entry.title = game.title.clone();
                }
                entry.sources.insert("igdb".into(), json!(found.id));
                entry.categories = slugs;
            }
            Ok(None) => misses.push(game),
            Err(e) => eprintln!("Error con '{}': {}", game.title, e),
        }
    }

    let healed = heal_names(&mut igdb, &mut catalog).await?;
    if healed > 0 {
        println!("Nombres canónicos corregidos desde IGDB: {healed}");
    }

    let categories = rebuild_categories(&store, &catalog);
    config.set_categories(categories);
    config.save()?;
    catalog.save()?;
    store.save()?;
    println!(
        "\nClasificados: {classified} | Actualizados por ID: {by_id} | Sin datos IGDB: {}",
        misses.len()
    );

    if !misses.is_empty() {
        println!("\nJuegos sin datos en IGDB:");
        for g in &misses {
            println!("  {:<10}  {}", g.runner, g.title);
        }
    }

    println!("Config guardado en {}", config.path.display());
    println!("games.json guardado en {}", store.path.display());
    println!("categories.json guardado en {}", catalog.path.display());
    Ok(())
}

fn push_unique(vec: &mut Vec<String>, value: String) {
    if !vec.iter().any(|v| v == &value) {
        vec.push(value);
    }
}

fn rebuild_categories(store: &GamesStore, catalog: &CategoryRegistry) -> Map<String, Value> {
    let mut cats: Map<String, Value> = Map::new();
    for (app_key, entry) in &store.data.games {
        for slug in &entry.categories {
            let name = display_name(catalog, slug);
            let arr = cats.entry(name).or_insert_with(|| json!([]));
            let arr = arr.as_array_mut().unwrap();
            if !arr.iter().any(|v| v.as_str() == Some(app_key)) {
                arr.push(Value::String(app_key.clone()));
            }
        }
    }
    cats
}

fn display_name(catalog: &CategoryRegistry, slug: &str) -> String {
    catalog
        .data
        .categories
        .get(slug)
        .map(|e| e.name.clone())
        .unwrap_or_else(|| slug.to_string())
}

async fn heal_names(igdb: &mut IgdbClient, catalog: &mut CategoryRegistry) -> Result<usize> {
    let pending: Vec<String> = catalog
        .data
        .categories
        .iter()
        .filter(|(slug, e)| e.sources.contains_key("igdb") && (e.name.is_empty() || e.name == **slug))
        .map(|(slug, _)| slug.clone())
        .collect();

    if pending.is_empty() {
        return Ok(0);
    }

    let slugs: Vec<&str> = pending.iter().map(String::as_str).collect();
    let names = igdb.canonical_names(&slugs).await?;
    let mut fixed = 0;
    for slug in &pending {
        if let Some(name) = names.get(slug) {
            let entry = catalog.data.categories.get_mut(slug).unwrap();
            entry.name = name.clone();
            fixed += 1;
        }
    }
    Ok(fixed)
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
