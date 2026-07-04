//! Base d'overlay (§3.0) — SQLite. Source de vérité des **métadonnées produites
//! par l'app** (jamais des fichiers du mod). Indexée sur `id_interne` du mod.
//!
//! L1 peuple : mods, versions (avec snapshot lecture seule des tags du fichier,
//! features CSP, skins, layouts), historique. Les colonnes overlay-éditables
//! (car_class, year, category, is_favorite, tags règle/manuel) existent dès
//! maintenant mais seront pleinement exploitées en L2.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// État partagé Tauri : connexion SQLite protégée par un mutex.
pub struct Db(pub Mutex<Connection>);

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", &"WAL")?;
    conn.pragma_update(None, "foreign_keys", &"ON")?;
    init(&conn)?;
    migrate(&conn)?;
    Ok(conn)
}

/// Ajoute les colonnes L2 aux bases déjà créées en L1 (ALTER idempotent).
fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let cols = [
        "country TEXT",
        "tags_from_rule TEXT NOT NULL DEFAULT '[]'",
        "tags_manual TEXT NOT NULL DEFAULT '[]'",
        "drivetrain TEXT",
        "engine_pos TEXT",
        "aspiration TEXT",
        "engine_config TEXT",
        "gearbox TEXT",
        "source_pack TEXT",
        "source_url TEXT",
        "is_stock INTEGER NOT NULL DEFAULT 0",
    ];
    for col in cols {
        // Ignore l'erreur « duplicate column » si la colonne existe déjà.
        let _ = conn.execute(&format!("ALTER TABLE mods ADD COLUMN {col}"), []);
    }
    // Date de publication estimée depuis les dates de fichiers (§6.2).
    let _ = conn.execute("ALTER TABLE versions ADD COLUMN published_at TEXT", []);
    Ok(())
}

fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS mods (
            id_interne        TEXT PRIMARY KEY,
            kind              TEXT NOT NULL,          -- 'Car' | 'Track'
            brand             TEXT,
            display_name      TEXT,
            identity_hash     TEXT,
            car_class         TEXT,                   -- overlay-éditable (L2)
            year              INTEGER,
            category          TEXT,                   -- tag # principal (§5bis)
            country           TEXT,
            is_favorite       INTEGER NOT NULL DEFAULT 0,
            tags_from_rule    TEXT NOT NULL DEFAULT '[]',
            tags_manual       TEXT NOT NULL DEFAULT '[]',
            drivetrain        TEXT,
            engine_pos        TEXT,
            aspiration        TEXT,
            engine_config     TEXT,
            gearbox           TEXT,
            source_pack       TEXT,                   -- pack d'origine (§4.7)
            source_url        TEXT,                   -- URL d'origine (§4.7/§12ter)
            is_stock          INTEGER NOT NULL DEFAULT 0, -- contenu de base Kunos (§12bis.1)
            active_version_id TEXT,
            created_at        TEXT NOT NULL
        );

        -- Sous-éléments rattachés à une voiture/circuit (§12bis.2) : skins, sons.
        -- Ne polluent jamais la bibliothèque principale (mods de 1er niveau).
        CREATE TABLE IF NOT EXISTS sub_mods (
            id             TEXT PRIMARY KEY,
            sub_type       TEXT NOT NULL,          -- 'SKIN'|'SOUND'|'TRACK_SKIN'|'TRACK_MOD'
            parent_id      TEXT NOT NULL,          -- id_interne de la voiture/circuit cible (mod OU stock)
            name           TEXT NOT NULL,
            library_path   TEXT NOT NULL,
            source_archive TEXT,
            is_active      INTEGER NOT NULL DEFAULT 0, -- SOUND uniquement (exclusif)
            imported_at    TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_sub_parent ON sub_mods(parent_id);

        -- Apps Python (§12bis.4) : type autonome, activable par junction.
        CREATE TABLE IF NOT EXISTS apps (
            id             TEXT PRIMARY KEY,       -- nom du dossier de l'app
            library_path   TEXT NOT NULL,
            source_archive TEXT,
            imported_at    TEXT NOT NULL
        );

        -- Suivi d'usage propre à l'app (§6.5) : marqueur « déjà essayé » définitif
        -- posé au lancement d'une session. Fiabilise les faux zéros de CM.
        CREATE TABLE IF NOT EXISTS usage (
            mod_id        TEXT PRIMARY KEY,  -- id_interne de la voiture ou du circuit
            launched      INTEGER NOT NULL DEFAULT 0,
            launch_count  INTEGER NOT NULL DEFAULT 0,
            last_launched TEXT
        );

        CREATE TABLE IF NOT EXISTS versions (
            id                TEXT PRIMARY KEY,
            mod_id            TEXT NOT NULL REFERENCES mods(id_interne) ON DELETE CASCADE,
            version_label     TEXT,
            author            TEXT,
            imported_at       TEXT NOT NULL,
            library_path      TEXT NOT NULL,
            source_archive    TEXT,
            content_signature TEXT,
            csp_features      TEXT NOT NULL DEFAULT '[]',
            skins             TEXT NOT NULL DEFAULT '[]',
            layouts           TEXT NOT NULL DEFAULT '[]',
            tags_from_mod     TEXT NOT NULL DEFAULT '[]',
            published_at      TEXT                    -- date de publication estimée (§6.2)
        );

        CREATE TABLE IF NOT EXISTS history (
            id        INTEGER PRIMARY KEY AUTOINCREMENT,
            mod_id    TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            event     TEXT NOT NULL,
            details   TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS profiles (
            id         TEXT PRIMARY KEY,
            name       TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS profile_entries (
            profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
            mod_id     TEXT NOT NULL,
            version_id TEXT NOT NULL
        );

        -- Mods « autres » (§6.1bis) : ni voiture, circuit, skin, son, ni app —
        -- jamais perdus. Activables par junction (garde-fou habituel) ; en cas
        -- d'emplacement disputé avec un autre mod « autre », la priorité tranche.
        CREATE TABLE IF NOT EXISTS other_mods (
            id             TEXT PRIMARY KEY,
            library_path   TEXT NOT NULL,
            source_archive TEXT,
            imported_at    TEXT NOT NULL,
            is_priority    INTEGER NOT NULL DEFAULT 0,
            is_active      INTEGER NOT NULL DEFAULT 0,
            junctions      TEXT NOT NULL DEFAULT '[]'
        );

        CREATE INDEX IF NOT EXISTS idx_versions_mod ON versions(mod_id);
        CREATE INDEX IF NOT EXISTS idx_history_mod  ON history(mod_id);
        CREATE INDEX IF NOT EXISTS idx_mods_idhash  ON mods(identity_hash);
        CREATE INDEX IF NOT EXISTS idx_pe_profile   ON profile_entries(profile_id);
        "#,
    )
}

// --- Structures exposées au frontend ---------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModRow {
    pub id_interne: String,
    pub kind: String,
    pub brand: Option<String>,
    pub display_name: Option<String>,
    pub year: Option<i64>,
    pub car_class: Option<String>,
    pub category: Option<String>,
    pub country: Option<String>,
    pub is_favorite: bool,
    pub active_version_id: Option<String>,
    pub version_count: i64,
    pub created_at: String,
    /// Tags lus dans le fichier (origine « fichier mod », lecture seule).
    pub tags_from_mod: Vec<String>,
    /// Tags déduits par l'ontologie (origine « règle »).
    pub tags_from_rule: Vec<String>,
    /// Tags ajoutés à la main (origine « manuel »).
    pub tags_manual: Vec<String>,
    pub drivetrain: Option<String>,
    pub engine_pos: Option<String>,
    pub aspiration: Option<String>,
    pub engine_config: Option<String>,
    pub gearbox: Option<String>,
    /// Pack d'origine commun aux mods d'une même archive multi-voitures (§4.7).
    pub source_pack: Option<String>,
    /// URL d'origine (rempli plus tard par l'extension, §4.7/§12ter).
    pub source_url: Option<String>,
    /// Auteur de la version active (colonne §6.2).
    pub author: Option<String>,
    /// Label de version de la version active (colonne §6.2).
    pub active_version_label: Option<String>,
    /// Date de dernière mise à jour = import de la version la plus récente (§6.2).
    pub updated_at: Option<String>,
    /// Layouts de la version active (colonne circuits §6.2).
    pub layouts: Vec<String>,
    /// Extensions CSP de la version active (colonne circuits §6.2).
    pub csp_features: Vec<String>,
    /// Contenu de base Kunos : lecture seule, non désactivable (§12bis.1).
    pub is_stock: bool,
    /// Date de publication estimée de la version active (§6.2).
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionRow {
    pub id: String,
    pub mod_id: String,
    pub version_label: Option<String>,
    pub author: Option<String>,
    pub imported_at: String,
    pub library_path: String,
    pub source_archive: Option<String>,
    pub content_signature: Option<String>,
    pub csp_features: Vec<String>,
    pub skins: Vec<String>,
    pub layouts: Vec<String>,
    pub tags_from_mod: Vec<String>,
    /// Date de publication estimée depuis les dates de fichiers (§6.2).
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryRow {
    pub timestamp: String,
    pub event: String,
    pub details: String,
}

fn json_arr(s: &str) -> Vec<String> {
    serde_json::from_str(s).unwrap_or_default()
}

// --- Écritures --------------------------------------------------------------

/// Insère le mod s'il n'existe pas (ne touche pas aux champs overlay-éditables
/// existants en cas de ré-import).
#[allow(clippy::too_many_arguments)]
pub fn upsert_mod(
    conn: &Connection,
    id_interne: &str,
    kind: &str,
    brand: Option<&str>,
    display_name: Option<&str>,
    identity_hash: &str,
    year: Option<i64>,
    created_at: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO mods (id_interne, kind, brand, display_name, identity_hash, year, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
           ON CONFLICT(id_interne) DO UPDATE SET
               brand = COALESCE(excluded.brand, mods.brand),
               display_name = COALESCE(excluded.display_name, mods.display_name),
               identity_hash = excluded.identity_hash,
               year = COALESCE(mods.year, excluded.year)"#,
        params![id_interne, kind, brand, display_name, identity_hash, year, created_at],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn insert_version(
    conn: &Connection,
    id: &str,
    mod_id: &str,
    version_label: Option<&str>,
    author: Option<&str>,
    imported_at: &str,
    library_path: &str,
    source_archive: Option<&str>,
    content_signature: &str,
    csp_features: &[String],
    skins: &[String],
    layouts: &[String],
    tags_from_mod: &[String],
    published_at: Option<&str>,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO versions
           (id, mod_id, version_label, author, imported_at, library_path,
            source_archive, content_signature, csp_features, skins, layouts, tags_from_mod,
            published_at)
           VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)"#,
        params![
            id,
            mod_id,
            version_label,
            author,
            imported_at,
            library_path,
            source_archive,
            content_signature,
            serde_json::to_string(csp_features).unwrap_or_else(|_| "[]".into()),
            serde_json::to_string(skins).unwrap_or_else(|_| "[]".into()),
            serde_json::to_string(layouts).unwrap_or_else(|_| "[]".into()),
            serde_json::to_string(tags_from_mod).unwrap_or_else(|_| "[]".into()),
            published_at,
        ],
    )?;
    Ok(())
}

pub fn set_active_version(conn: &Connection, mod_id: &str, version_id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE mods SET active_version_id = ?2 WHERE id_interne = ?1",
        params![mod_id, version_id],
    )?;
    Ok(())
}

/// Écrit le résultat d'harmonisation (§5.4) dans l'overlay. brand/country et les
/// specs ne sont écrasés que si une valeur est fournie (préserve les complétions
/// manuelles) ; tags_from_rule/car_class/category reflètent toujours les règles.
#[allow(clippy::too_many_arguments)]
pub fn update_harmonization(
    conn: &Connection,
    id: &str,
    brand: Option<&str>,
    car_class: Option<&str>,
    category: Option<&str>,
    country: Option<&str>,
    tags_from_rule: &[String],
    drivetrain: Option<&str>,
    engine_pos: Option<&str>,
    aspiration: Option<&str>,
    engine_config: Option<&str>,
    gearbox: Option<&str>,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"UPDATE mods SET
               brand = COALESCE(?2, brand),
               car_class = ?3,
               category = ?4,
               country = COALESCE(?5, country),
               tags_from_rule = ?6,
               drivetrain = COALESCE(?7, drivetrain),
               engine_pos = COALESCE(?8, engine_pos),
               aspiration = COALESCE(?9, aspiration),
               engine_config = COALESCE(?10, engine_config),
               gearbox = COALESCE(?11, gearbox)
           WHERE id_interne = ?1"#,
        params![
            id,
            brand,
            car_class,
            category,
            country,
            serde_json::to_string(tags_from_rule).unwrap_or_else(|_| "[]".into()),
            drivetrain,
            engine_pos,
            aspiration,
            engine_config,
            gearbox,
        ],
    )?;
    Ok(())
}

/// Renseigne le pack/URL d'origine d'un mod (§4.7). N'écrase une valeur
/// existante que si une nouvelle est fournie (COALESCE) — un ré-import ne
/// perd pas l'URL renseignée par ailleurs.
pub fn set_source(conn: &Connection, id: &str, pack: Option<&str>, url: Option<&str>) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE mods SET source_pack = COALESCE(?2, source_pack),
                         source_url  = COALESCE(?3, source_url)
         WHERE id_interne = ?1",
        params![id, pack, url],
    )?;
    Ok(())
}

pub fn set_favorite(conn: &Connection, id: &str, fav: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE mods SET is_favorite = ?2 WHERE id_interne = ?1",
        params![id, fav as i64],
    )?;
    Ok(())
}

pub fn set_manual_tags(conn: &Connection, id: &str, tags: &[String]) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE mods SET tags_manual = ?2 WHERE id_interne = ?1",
        params![id, serde_json::to_string(tags).unwrap_or_else(|_| "[]".into())],
    )?;
    Ok(())
}

/// Édite un champ overlay simple (liste blanche de colonnes pour éviter toute
/// injection). `value = None` met la colonne à NULL.
pub fn set_mod_field(conn: &Connection, id: &str, field: &str, value: Option<&str>) -> rusqlite::Result<()> {
    let column = match field {
        "category" => "category",
        "car_class" => "car_class",
        "country" => "country",
        "drivetrain" => "drivetrain",
        "engine_pos" => "engine_pos",
        "aspiration" => "aspiration",
        "engine_config" => "engine_config",
        "gearbox" => "gearbox",
        _ => return Err(rusqlite::Error::InvalidParameterName(field.into())),
    };
    conn.execute(
        &format!("UPDATE mods SET {column} = ?2 WHERE id_interne = ?1"),
        params![id, value],
    )?;
    Ok(())
}

pub fn add_history(conn: &Connection, mod_id: &str, ts: &str, event: &str, details: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO history (mod_id, timestamp, event, details) VALUES (?1,?2,?3,?4)",
        params![mod_id, ts, event, details],
    )?;
    Ok(())
}

// --- Lectures ---------------------------------------------------------------

const MOD_SELECT: &str = r#"
    SELECT m.id_interne, m.kind, m.brand, m.display_name, m.year, m.car_class,
           m.category, m.country, m.is_favorite, m.active_version_id, m.created_at,
           m.tags_from_rule, m.tags_manual,
           m.drivetrain, m.engine_pos, m.aspiration, m.engine_config, m.gearbox,
           (SELECT COUNT(*) FROM versions v WHERE v.mod_id = m.id_interne) AS version_count,
           COALESCE((SELECT v.tags_from_mod FROM versions v
                     WHERE v.id = m.active_version_id), '[]') AS tags_from_mod,
           m.source_pack, m.source_url,
           -- Données de la version active (colonnes §6.2) + date de MAJ agrégée.
           (SELECT v.author FROM versions v WHERE v.id = m.active_version_id) AS author,
           (SELECT v.version_label FROM versions v WHERE v.id = m.active_version_id) AS version_label,
           COALESCE((SELECT v.layouts FROM versions v WHERE v.id = m.active_version_id), '[]') AS layouts,
           COALESCE((SELECT v.csp_features FROM versions v WHERE v.id = m.active_version_id), '[]') AS csp_features,
           (SELECT MAX(v.imported_at) FROM versions v WHERE v.mod_id = m.id_interne) AS updated_at,
           m.is_stock,
           (SELECT v.published_at FROM versions v WHERE v.id = m.active_version_id) AS published_at
    FROM mods m
"#;

fn map_mod(row: &rusqlite::Row) -> rusqlite::Result<ModRow> {
    let tags_rule: String = row.get(11)?;
    let tags_manual: String = row.get(12)?;
    let tags_mod: String = row.get(19)?;
    let layouts: String = row.get(24)?;
    let csp_features: String = row.get(25)?;
    Ok(ModRow {
        id_interne: row.get(0)?,
        kind: row.get(1)?,
        brand: row.get(2)?,
        display_name: row.get(3)?,
        year: row.get(4)?,
        car_class: row.get(5)?,
        category: row.get(6)?,
        country: row.get(7)?,
        is_favorite: row.get::<_, i64>(8)? != 0,
        active_version_id: row.get(9)?,
        created_at: row.get(10)?,
        tags_from_rule: json_arr(&tags_rule),
        tags_manual: json_arr(&tags_manual),
        drivetrain: row.get(13)?,
        engine_pos: row.get(14)?,
        aspiration: row.get(15)?,
        engine_config: row.get(16)?,
        gearbox: row.get(17)?,
        version_count: row.get(18)?,
        tags_from_mod: json_arr(&tags_mod),
        source_pack: row.get(20)?,
        source_url: row.get(21)?,
        author: row.get(22)?,
        active_version_label: row.get(23)?,
        layouts: json_arr(&layouts),
        csp_features: json_arr(&csp_features),
        updated_at: row.get(26)?,
        is_stock: row.get::<_, i64>(27)? != 0,
        published_at: row.get(28)?,
    })
}

pub fn list_mods(conn: &Connection) -> rusqlite::Result<Vec<ModRow>> {
    let sql = format!("{MOD_SELECT} ORDER BY m.display_name COLLATE NOCASE");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], map_mod)?;
    rows.collect()
}

pub fn get_mod(conn: &Connection, id: &str) -> rusqlite::Result<Option<ModRow>> {
    let sql = format!("{MOD_SELECT} WHERE m.id_interne = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([id], map_mod)?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

pub fn get_versions(conn: &Connection, mod_id: &str) -> rusqlite::Result<Vec<VersionRow>> {
    let mut stmt = conn.prepare(
        r#"SELECT id, mod_id, version_label, author, imported_at, library_path,
                  source_archive, content_signature, csp_features, skins, layouts, tags_from_mod,
                  published_at
           FROM versions WHERE mod_id = ?1 ORDER BY imported_at DESC"#,
    )?;
    let rows = stmt.query_map([mod_id], |row| {
        let csp: String = row.get(8)?;
        let skins: String = row.get(9)?;
        let layouts: String = row.get(10)?;
        let tags: String = row.get(11)?;
        Ok(VersionRow {
            id: row.get(0)?,
            mod_id: row.get(1)?,
            version_label: row.get(2)?,
            author: row.get(3)?,
            imported_at: row.get(4)?,
            library_path: row.get(5)?,
            source_archive: row.get(6)?,
            content_signature: row.get(7)?,
            csp_features: json_arr(&csp),
            skins: json_arr(&skins),
            layouts: json_arr(&layouts),
            tags_from_mod: json_arr(&tags),
            published_at: row.get(12)?,
        })
    })?;
    rows.collect()
}

pub fn get_history(conn: &Connection, mod_id: &str) -> rusqlite::Result<Vec<HistoryRow>> {
    let mut stmt = conn.prepare(
        "SELECT timestamp, event, details FROM history WHERE mod_id = ?1 ORDER BY id DESC",
    )?;
    let rows = stmt.query_map([mod_id], |row| {
        Ok(HistoryRow {
            timestamp: row.get(0)?,
            event: row.get(1)?,
            details: row.get(2)?,
        })
    })?;
    rows.collect()
}

/// Rapprochement flou : mods de même type ayant le même brand+name normalisé
/// mais un `id_interne` différent (§4.2 « match flou »).
pub fn find_fuzzy(
    conn: &Connection,
    kind: &str,
    brand: &str,
    name: &str,
    exclude_id: &str,
) -> rusqlite::Result<Vec<ModRow>> {
    let sql = format!(
        "{MOD_SELECT} WHERE m.kind = ?1 AND m.id_interne <> ?2
         AND LOWER(TRIM(COALESCE(m.brand,''))) = LOWER(TRIM(?3))
         AND LOWER(TRIM(COALESCE(m.display_name,''))) = LOWER(TRIM(?4))"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![kind, exclude_id, brand, name], map_mod)?;
    rows.collect()
}

/// Signature de contenu de la version active d'un mod (doublon vs mise à jour).
pub fn active_signature(conn: &Connection, mod_id: &str) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT v.content_signature FROM versions v
         JOIN mods m ON m.active_version_id = v.id WHERE m.id_interne = ?1",
    )?;
    let mut rows = stmt.query_map([mod_id], |r| r.get::<_, Option<String>>(0))?;
    match rows.next() {
        Some(r) => Ok(r?),
        None => Ok(None),
    }
}

/// Chemin bibliothèque d'une version donnée (pour calculer la preview).
pub fn get_version_path(conn: &Connection, version_id: &str) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT library_path FROM versions WHERE id = ?1")?;
    let mut rows = stmt.query_map([version_id], |r| r.get::<_, String>(0))?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

/// Supprime un mod et ses données overlay (versions cascade + historique).
/// N'agit que sur l'overlay : les fichiers bibliothèque sont gérés par l'appelant.
pub fn delete_mod(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM history WHERE mod_id = ?1", [id])?;
    conn.execute("DELETE FROM mods WHERE id_interne = ?1", [id])?;
    Ok(())
}

// --- Profils (L3) -----------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ProfileRow {
    pub id: String,
    pub name: String,
    pub entry_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileEntry {
    pub mod_id: String,
    pub version_id: String,
}

pub fn create_profile(conn: &Connection, id: &str, name: &str, created_at: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO profiles (id, name, created_at) VALUES (?1, ?2, ?3)",
        params![id, name, created_at],
    )?;
    Ok(())
}

pub fn add_profile_entry(conn: &Connection, profile_id: &str, mod_id: &str, version_id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO profile_entries (profile_id, mod_id, version_id) VALUES (?1, ?2, ?3)",
        params![profile_id, mod_id, version_id],
    )?;
    Ok(())
}

pub fn list_profiles(conn: &Connection) -> rusqlite::Result<Vec<ProfileRow>> {
    let mut stmt = conn.prepare(
        r#"SELECT p.id, p.name,
                  (SELECT COUNT(*) FROM profile_entries e WHERE e.profile_id = p.id) AS entry_count
           FROM profiles p ORDER BY p.name COLLATE NOCASE"#,
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ProfileRow { id: r.get(0)?, name: r.get(1)?, entry_count: r.get(2)? })
    })?;
    rows.collect()
}

pub fn get_profile_entries(conn: &Connection, profile_id: &str) -> rusqlite::Result<Vec<ProfileEntry>> {
    let mut stmt = conn.prepare(
        "SELECT mod_id, version_id FROM profile_entries WHERE profile_id = ?1",
    )?;
    let rows = stmt.query_map([profile_id], |r| {
        Ok(ProfileEntry { mod_id: r.get(0)?, version_id: r.get(1)? })
    })?;
    rows.collect()
}

pub fn delete_profile(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM profiles WHERE id = ?1", [id])?;
    Ok(())
}

/// Ids des mods partageant un même pack d'origine (§4.7).
pub fn list_pack_ids(conn: &Connection, pack: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT id_interne FROM mods WHERE source_pack = ?1")?;
    let rows = stmt.query_map([pack], |r| r.get::<_, String>(0))?;
    rows.collect()
}

pub fn mod_exists(conn: &Connection, id: &str) -> rusqlite::Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM mods WHERE id_interne = ?1",
        [id],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

// --- Contenu de base Kunos (§12bis.1) ---------------------------------------

/// Indexe une voiture/circuit de base : ligne minimale `is_stock=1` (lecture
/// seule). Ne touche pas un mod déjà présent (un vrai mod n'est jamais « stock »).
pub fn upsert_stock_mod(
    conn: &Connection,
    id: &str,
    kind: &str,
    brand: Option<&str>,
    name: Option<&str>,
    created_at: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO mods (id_interne, kind, brand, display_name, is_stock, created_at)
           VALUES (?1, ?2, ?3, ?4, 1, ?5)
           ON CONFLICT(id_interne) DO NOTHING"#,
        params![id, kind, brand, name, created_at],
    )?;
    Ok(())
}

/// Supprime toutes les entrées de contenu de base (ré-indexation depuis zéro).
/// Les versions associées tombent par CASCADE. Les vrais mods ne sont pas touchés.
pub fn clear_stock(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM history WHERE mod_id IN (SELECT id_interne FROM mods WHERE is_stock = 1)", [])?;
    let n = conn.execute("DELETE FROM mods WHERE is_stock = 1", [])?;
    Ok(n)
}

/// Nombre d'entrées de contenu de base déjà indexées — sert à déclencher un
/// scan automatique au premier démarrage (§12bis.1).
pub fn count_stock(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("SELECT COUNT(*) FROM mods WHERE is_stock = 1", [], |r| r.get(0))
}

// --- Sous-éléments rattachés (§12bis.2) -------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubModRow {
    pub id: String,
    pub sub_type: String,
    pub parent_id: String,
    pub name: String,
    pub library_path: String,
    pub source_archive: Option<String>,
    pub is_active: bool,
    pub imported_at: String,
}

#[allow(clippy::too_many_arguments)]
pub fn insert_sub_mod(
    conn: &Connection,
    id: &str,
    sub_type: &str,
    parent_id: &str,
    name: &str,
    library_path: &str,
    source_archive: Option<&str>,
    imported_at: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO sub_mods (id, sub_type, parent_id, name, library_path, source_archive, imported_at)
           VALUES (?1,?2,?3,?4,?5,?6,?7)"#,
        params![id, sub_type, parent_id, name, library_path, source_archive, imported_at],
    )?;
    Ok(())
}

fn map_sub(row: &rusqlite::Row) -> rusqlite::Result<SubModRow> {
    Ok(SubModRow {
        id: row.get(0)?,
        sub_type: row.get(1)?,
        parent_id: row.get(2)?,
        name: row.get(3)?,
        library_path: row.get(4)?,
        source_archive: row.get(5)?,
        is_active: row.get::<_, i64>(6)? != 0,
        imported_at: row.get(7)?,
    })
}

const SUB_SELECT: &str =
    "SELECT id, sub_type, parent_id, name, library_path, source_archive, is_active, imported_at FROM sub_mods";

/// Sous-éléments rattachés à une entité (fiche détail, §12bis.3).
pub fn list_subs_for_parent(conn: &Connection, parent_id: &str) -> rusqlite::Result<Vec<SubModRow>> {
    let mut stmt = conn.prepare(&format!("{SUB_SELECT} WHERE parent_id = ?1 ORDER BY name COLLATE NOCASE"))?;
    let rows = stmt.query_map([parent_id], map_sub)?;
    rows.collect()
}

/// Tous les sous-éléments d'un type (vue transversale, §12bis.3).
pub fn list_subs_by_type(conn: &Connection, sub_type: &str) -> rusqlite::Result<Vec<SubModRow>> {
    let mut stmt = conn.prepare(&format!("{SUB_SELECT} WHERE sub_type = ?1 ORDER BY parent_id, name COLLATE NOCASE"))?;
    let rows = stmt.query_map([sub_type], map_sub)?;
    rows.collect()
}

pub fn get_sub_mod(conn: &Connection, id: &str) -> rusqlite::Result<Option<SubModRow>> {
    let mut stmt = conn.prepare(&format!("{SUB_SELECT} WHERE id = ?1"))?;
    let mut rows = stmt.query_map([id], map_sub)?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

/// Existe-t-il déjà un sous-élément de ce type/parent/nom ? (idempotence import).
pub fn sub_exists(conn: &Connection, sub_type: &str, parent_id: &str, name: &str) -> rusqlite::Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sub_mods WHERE sub_type = ?1 AND parent_id = ?2 AND name = ?3",
        params![sub_type, parent_id, name],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

pub fn delete_sub_mod(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM sub_mods WHERE id = ?1", [id])?;
    Ok(())
}

/// Bascule exclusive du son actif d'une voiture (§12bis.2) : un seul SOUND actif
/// par parent. `id = None` désactive tout (retour au son d'origine).
pub fn set_active_sound(conn: &Connection, parent_id: &str, id: Option<&str>) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE sub_mods SET is_active = 0 WHERE parent_id = ?1 AND sub_type = 'SOUND'",
        [parent_id],
    )?;
    if let Some(id) = id {
        conn.execute("UPDATE sub_mods SET is_active = 1 WHERE id = ?1", [id])?;
    }
    Ok(())
}

// --- Apps (§12bis.4) --------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppRow {
    pub id: String,
    pub library_path: String,
    pub source_archive: Option<String>,
    pub imported_at: String,
}

pub fn insert_app(
    conn: &Connection,
    id: &str,
    library_path: &str,
    source_archive: Option<&str>,
    imported_at: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO apps (id, library_path, source_archive, imported_at)
           VALUES (?1, ?2, ?3, ?4)
           ON CONFLICT(id) DO UPDATE SET library_path = excluded.library_path"#,
        params![id, library_path, source_archive, imported_at],
    )?;
    Ok(())
}

pub fn list_apps(conn: &Connection) -> rusqlite::Result<Vec<AppRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, library_path, source_archive, imported_at FROM apps ORDER BY id COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(AppRow {
            id: r.get(0)?,
            library_path: r.get(1)?,
            source_archive: r.get(2)?,
            imported_at: r.get(3)?,
        })
    })?;
    rows.collect()
}

pub fn get_app(conn: &Connection, id: &str) -> rusqlite::Result<Option<AppRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, library_path, source_archive, imported_at FROM apps WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map([id], |r| {
        Ok(AppRow {
            id: r.get(0)?,
            library_path: r.get(1)?,
            source_archive: r.get(2)?,
            imported_at: r.get(3)?,
        })
    })?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

#[allow(dead_code)] // utilisé par les tests d'apps
pub fn app_exists(conn: &Connection, id: &str) -> rusqlite::Result<bool> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM apps WHERE id = ?1", [id], |r| r.get(0))?;
    Ok(n > 0)
}

pub fn delete_app(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM apps WHERE id = ?1", [id])?;
    Ok(())
}

// --- Mods « autres » (§6.1bis) -----------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtherModRow {
    pub id: String,
    pub library_path: String,
    pub source_archive: Option<String>,
    pub imported_at: String,
    pub is_priority: bool,
    pub is_active: bool,
    /// Chemins absolus des jonctions créées lors de la dernière activation.
    pub junctions: Vec<String>,
}

pub fn insert_other_mod(
    conn: &Connection,
    id: &str,
    library_path: &str,
    source_archive: Option<&str>,
    imported_at: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO other_mods (id, library_path, source_archive, imported_at)
           VALUES (?1, ?2, ?3, ?4)"#,
        params![id, library_path, source_archive, imported_at],
    )?;
    Ok(())
}

fn map_other(row: &rusqlite::Row) -> rusqlite::Result<OtherModRow> {
    let junctions: String = row.get(6)?;
    Ok(OtherModRow {
        id: row.get(0)?,
        library_path: row.get(1)?,
        source_archive: row.get(2)?,
        imported_at: row.get(3)?,
        is_priority: row.get::<_, i64>(4)? != 0,
        is_active: row.get::<_, i64>(5)? != 0,
        junctions: json_arr(&junctions),
    })
}

const OTHER_SELECT: &str =
    "SELECT id, library_path, source_archive, imported_at, is_priority, is_active, junctions FROM other_mods";

pub fn list_other_mods(conn: &Connection) -> rusqlite::Result<Vec<OtherModRow>> {
    let mut stmt = conn.prepare(&format!("{OTHER_SELECT} ORDER BY id COLLATE NOCASE"))?;
    let rows = stmt.query_map([], map_other)?;
    rows.collect()
}

pub fn get_other_mod(conn: &Connection, id: &str) -> rusqlite::Result<Option<OtherModRow>> {
    let mut stmt = conn.prepare(&format!("{OTHER_SELECT} WHERE id = ?1"))?;
    let mut rows = stmt.query_map([id], map_other)?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

pub fn other_exists(conn: &Connection, id: &str) -> rusqlite::Result<bool> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM other_mods WHERE id = ?1", [id], |r| r.get(0))?;
    Ok(n > 0)
}

pub fn set_other_priority(conn: &Connection, id: &str, priority: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE other_mods SET is_priority = ?2 WHERE id = ?1",
        params![id, priority as i64],
    )?;
    Ok(())
}

/// Bascule active/inactive + mémorise les jonctions créées (pour une
/// désactivation exacte plus tard).
pub fn set_other_active(conn: &Connection, id: &str, active: bool, junctions: &[String]) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE other_mods SET is_active = ?2, junctions = ?3 WHERE id = ?1",
        params![id, active as i64, serde_json::to_string(junctions).unwrap_or_else(|_| "[]".into())],
    )?;
    Ok(())
}

pub fn delete_other_mod(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM other_mods WHERE id = ?1", [id])?;
    Ok(())
}

// --- Suivi d'usage (§6.5) ---------------------------------------------------

/// Pose/incrémente le marqueur « essayé » d'un mod au lancement d'une session.
pub fn mark_launched(conn: &Connection, mod_id: &str, ts: &str) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO usage (mod_id, launched, launch_count, last_launched)
           VALUES (?1, 1, 1, ?2)
           ON CONFLICT(mod_id) DO UPDATE SET
               launched = 1,
               launch_count = usage.launch_count + 1,
               last_launched = ?2"#,
        params![mod_id, ts],
    )?;
    Ok(())
}

/// Ids des mods déjà lancés au moins une fois par l'app.
pub fn launched_ids(conn: &Connection) -> rusqlite::Result<std::collections::HashSet<String>> {
    let mut stmt = conn.prepare("SELECT mod_id FROM usage WHERE launched = 1")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    rows.collect()
}
