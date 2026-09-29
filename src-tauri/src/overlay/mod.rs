//! Base d'overlay (§3) — SQLite. Source de vérité des **métadonnées produites
//! par l'app** (jamais des fichiers du mod). Indexée sur `id_interne` du mod.
//!
//! L1 peuple : mods, versions (avec snapshot lecture seule des tags du fichier,
//! features CSP, skins, layouts), historique. Les colonnes overlay-éditables
//! (car_class, year, category, is_favorite, tags règle/manuel) existent dès
//! maintenant mais seront pleinement exploitées en L2.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};

use crate::layers::HostKind;
use serde::{Deserialize, Serialize};

// One file per table family; the facade below keeps every caller on
// `overlay::…`, as when all of it lived in a single file.
mod apps;
mod extras;
mod journal;
mod layers;
mod media;
mod mods;
mod others;
mod pending;
mod profiles;
mod schema;
mod stock;
mod submods;
#[cfg(test)]
mod tests;
mod usage;

pub use apps::*;
pub use extras::*;
pub use journal::*;
pub use layers::*;
pub use media::*;
pub use mods::*;
pub use others::*;
pub use pending::*;
pub use profiles::*;
use schema::{init, migrate};
pub use stock::*;
pub use submods::*;
pub use usage::*;

/// État partagé Tauri : connexion SQLite protégée par un mutex.
pub struct Db(pub Mutex<Connection>);

/// Version of the harmonisation engine that produced the stored overlay (§5).
pub const META_ENGINE_VERSION: &str = "engine_version";

/// Reads a `meta` entry; `None` when the key was never written.
pub fn get_meta(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    use rusqlite::OptionalExtension;
    conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
        .optional()
}

pub fn set_meta(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO meta(key, value) VALUES(?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    init(&conn)?;
    migrate(&conn)?;
    Ok(conn)
}

/// `PRAGMA quick_check(1)`: `"ok"`, or the first problem found. Milliseconds
/// on a library-sized base, so affordable at every startup.
pub fn quick_check(conn: &Connection) -> rusqlite::Result<String> {
    conn.query_row("PRAGMA quick_check(1)", [], |r| r.get(0))
}
