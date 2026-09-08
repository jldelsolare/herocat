# herocat

CLI en Rust para clasificar automáticamente los juegos de tu biblioteca de **Heroic Games Launcher** usando la API de **IGDB**.

Cada juego se busca en IGDB, se extraen sus `genres` (mecánica) y `themes` (ambientación), y se escriben como categorías en `games.customCategories` del `config.json` de Heroic. Las categorías aparecen en la interfaz de Heroic sin configuración adicional.

## Requisitos

- Rust (edición 2024)
- Heroic Games Launcher instalado (config en `~/.config/heroic/` o Flatpak)
- Cuenta de Twitch con 2FA + app registrada en [dev.twitch.tv](https://dev.twitch.tv/console/apps) con **Client Type: Confidential**

## Instalación

```bash
cargo build --release
# el binario queda en target/release/herocat
```

## Uso

### Credenciales IGDB

```bash
export IGDB_CLIENT_ID="tu_client_id"
export IGDB_CLIENT_SECRET="tu_client_secret"
```

### Subcomandos

| Comando | Descripción |
|---------|-------------|
| `herocat list` | Lista los juegos de la biblioteca |
| `herocat generate` | Clasifica todos los juegos automáticamente desde IGDB |
| `herocat assign` | Asigna manualmente un juego a una categoría |
| `herocat categories` | Muestra las categorías y cuántos juegos tiene cada una |
| `herocat config` | Muestra la ruta de configuración de Heroic detectada |

### Ejemplos

```bash
# Listar todos los juegos (✓ = instalado)
herocat list

# Filtrar por plataforma y/o instalados
herocat list --runner gog
herocat list --runner legendary --installed

# Buscar por título (insensible a mayúsculas, substring)
herocat list --search witcher
herocat list --search lego --runner gog

# Clasificar todos los juegos desde IGDB
herocat generate

# Probar con los primeros 5 juegos (útil para validar antes de procesar todo)
herocat generate --limit 5

# Asignación manual (acepta app_name_runner o título)
herocat assign --category "RPG" --game "1133514031_gog"
herocat assign --category "RPG" --game "Prey"
```

## Cómo funciona

```
┌──────────────┐   título   ┌───────────┐  genres + themes  ┌──────────────────┐
│  library.json ├──────────►│  IGDB API  ├──────────────────►│   config.json    │
│ (4 runners)   │           │  v4/games  │                   │ games.customCat  │
└──────────────┘            └───────────┘                   └──────────────────┘
```

1. **`src/heroic.rs`** — Descubre la ruta de Heroic (nativa o Flatpak) y lee las bibliotecas de Epic (`legendary`), GOG, Amazon (`nile`) y juegos sideload.
2. **`src/igdb.rs`** — Autentica con OAuth de Twitch (token cachead, 4 req/seg límite respetado) y busca cada título con `search "X"; fields name,genres.*,themes.*; limit 1;`.
3. **`src/heroic_config.rs`** — Lee/escribe `customCategories` del `store/config.json` de Heroic (anidado bajo `games`), preservando el resto del archivo.
4. **`src/main.rs`** — CLI con clap y la orquestación de los subcomandos.

### Categorías

Se usan **genres + themes** de IGDB como categorías. Ejemplo de juego "Halo: Combat Evolved":

- genres → `Shooter`, `RPG`
- themes → `Action`, `Science fiction`

Cada juego se guarda bajo su clave compuesta `{app_name}_{runner}` (formato interno de Heroic). Los juegos sin datos en IGDB se listan al final de `generate` para corregirlos manualmente con `assign`.

### Categorías detectadas al generar

- Los juegos que IGDB no encuentra, o que devuelven sin genres/themes, aparecen al final de `generate` bajo "Juegos sin datos en IGDB".
- Los errores de red se imprimen por separado y no cuentan como "sin datos".

## Estructura

```
src/
├── main.rs           # CLI (clap) y orquestación
├── models.rs         # Estructuras de datos (biblioteca e IGDB)
├── heroic.rs         # Descubrimiento y lectura de bibliotecas de Heroic
├── igdb.rs           # Cliente HTTP de la API IGDB (auth + búsqueda)
└── heroic_config.rs  # Lectura/escritura de config.json de Heroic
```

## Limitaciones

- La búsqueda en IGDB es difusa (fuzzy) por nombre: ediciones (Remastered, Complete Edition) y juegos con el mismo nombre pueden mapear a la versión base que IGDB considera más relevante. El binario no valida todavía la coincidencia del título devuelto.
- El token de Twitch se solicita en cada ejecución cuya caché en memoria haya expirado (no persiste entre ejecuciones).

## Seguridad

- Las credenciales **no** se guardan en el código ni en el repositorio: se leen de `IGDB_CLIENT_ID` y `IGDB_CLIENT_SECRET`.
- No introduzcas el `client_secret` en chats, logs o la línea de comando compartida; regenera el secret en Twitch si se expuso.