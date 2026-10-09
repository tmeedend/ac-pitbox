//! Exporting a library to a `.pitbox` file, and importing one into an empty
//! installation (EXPORT§).
//!
//! An export is a library whose mods are all in the showcase (ESPACE§): the
//! base, the skeletons the showcase defines, and the user's settings — never a
//! playable file, never a path of the machine it was made on (EXPORT§2). A
//! 300 GB library becomes a few MB, and the other machine gets back exactly
//! what cannot be downloaded again: names, notes, tags, decisions.
//!
//! Three files share the work:
//!
//! - `tables.rs`: what each table of the base becomes on the way out and on
//!   the way in, and the gate that fails when a table is not classified;
//! - `files.rs`: what lands in the zip besides the base — skeletons and
//!   settings — and how it lands back, with its undo;
//! - this one: the two journeys, in the spec's order.
//!
//! The container is a plain zip with a readable JSON manifest and an SQLite
//! base openable by any tool (EXPORT R4).

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::config::AppConfig;
use crate::overlay::{self, Db};

mod files;
mod tables;
#[cfg(test)]
mod tests;

/// The container's format. Bumped only when an older Pit Box could no longer
/// read the zip itself; what the base holds is versioned by `app_version`.
pub const FORMAT: u32 = 1;
const MANIFEST_ENTRY: &str = "pitbox-export.json";
const DATABASE_ENTRY: &str = "bibliotheque/overlay.sqlite";

/// What an export is cut into, each checked or not (EXPORT§3, R5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Part {
    /// The base and the skeletons.
    Library,
    /// Tag rules, taxonomies, logo choices and the user's own logos.
    Classification,
    /// Saved sessions (Content Manager presets) and saved grids.
    Sessions,
    /// Named sets of mods — only lists of ids, hence the library with them.
    Profiles,
    /// The application's and the screens' preferences, never a path.
    Preferences,
}

impl Part {
    pub const ALL: [Part; 5] = [
        Part::Library,
        Part::Classification,
        Part::Sessions,
        Part::Profiles,
        Part::Preferences,
    ];
}

/// The parts that can actually travel: Profiles names mods, and goes nowhere
/// without the library (EXPORT§3).
fn effective(parts: &[Part]) -> Vec<Part> {
    let mut out: Vec<Part> = Part::ALL.into_iter().filter(|p| parts.contains(p)).collect();
    if !out.contains(&Part::Library) {
        out.retain(|p| *p != Part::Profiles);
    }
    out
}

/// Where what is not in the base lives on this machine. Resolved by the
/// façade, which alone holds the `AppHandle`.
pub struct Places {
    /// `app_config_dir`: the base, `config.json`, `ui_prefs.json`…
    pub config_dir: PathBuf,
    /// Pit Box's own folder among Content Manager's presets (SESSION§3.6).
    pub presets_dir: Option<PathBuf>,
    /// This build's version, for the manifest and for R4.
    pub app_version: String,
}

/// What an export holds, part by part (EXPORT§6).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Counts {
    pub cars: usize,
    pub tracks: usize,
    pub layers: usize,
    pub skins: usize,
    pub sounds: usize,
    pub apps: usize,
    pub others: usize,
    /// Game content the user wrote something on (EXPORT§4.2).
    pub stock_with_user_data: usize,
    /// Mods installed outside Pit Box, which leave managed (EXPORT§4.2).
    pub unmanaged: usize,
    pub profiles: usize,
    pub sessions: usize,
    pub grids: usize,
}

/// `pitbox-export.json`, at the root of the zip (EXPORT§6).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub format: u32,
    pub app_version: String,
    pub exported_at: String,
    pub parts: Vec<Part>,
    #[serde(default)]
    pub counts: Counts,
    /// Mods, apps and "other" mods in the game at export time: what to
    /// propose activating again once their files are back.
    #[serde(default)]
    pub active_at_export: Vec<String>,
    /// What the library weighed, for one sentence at import.
    #[serde(default)]
    pub library_bytes_at_export: u64,
    /// `wiki::store::CONTENT_VERSION` of the cached articles: a cache written
    /// in another format is not imported, it is fetched again.
    #[serde(default)]
    pub wiki_content_version: u32,
}

/// What an export of every part would hold and weigh, before writing it
/// (EXPORT§7.2). The weight is per part, what the zip will hold: the screen
/// adds up the checked ones.
#[derive(Debug, Clone, Serialize)]
pub struct Estimate {
    pub counts: Counts,
    pub bytes: BTreeMap<Part, u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportReport {
    pub path: String,
    pub bytes: u64,
    pub counts: Counts,
    pub library_bytes: u64,
}

/// Why an export cannot be imported here, said with what to do (EXPORT§7.3).
/// `key` is an i18n key; `count` and `version` fill it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Refusal {
    pub key: &'static str,
    pub count: Option<usize>,
    pub version: Option<String>,
}

/// The first step of an import: the manifest alone, nothing written.
#[derive(Debug, Clone, Serialize)]
pub struct Inspection {
    pub manifest: Manifest,
    pub refusal: Option<Refusal>,
    /// What the file weighs, beside what the library weighed (EXPORT§7.3).
    pub file_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ImportReport {
    pub mods: usize,
    pub layers: usize,
    pub apps: usize,
    pub others: usize,
    /// Game content found here that got back what the user wrote on it.
    pub stock_applied: usize,
    /// Game content the user wrote on and this installation does not have
    /// (a DLC?): listed, never dropped in silence (EXPORT§4.2).
    pub stock_missing: Vec<String>,
    /// Mods of the export this machine already has, installed by hand in its
    /// `content/`: kept as they are, with the notes written on them.
    pub local_kept: Vec<String>,
    /// Presets renamed because one of the same name was already there.
    pub presets_renamed: usize,
    pub active_at_export: usize,
    pub library_bytes_at_export: u64,
}

// --- Export ------------------------------------------------------------------

/// A scratch folder, removed when dropped — even on an error.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Result<Self, String> {
        let dir = std::env::temp_dir().join(format!("pitbox-transfer-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        Ok(Self(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_dir_all(&self.0) {
            log::warn!("transfer: scratch folder {} not removed: {e}", self.0.display());
        }
    }
}

/// Everything an export holds, ready to be written — or only weighed.
struct Built {
    _scratch: Scratch,
    database: Option<PathBuf>,
    entries: Vec<files::Entry>,
    counts: Counts,
    active: Vec<String>,
    library_bytes: u64,
}

fn build(db: &Db, cfg: &AppConfig, places: &Places, parts: &[Part]) -> Result<Built, String> {
    let scratch = Scratch::new("export")?;
    let stamp = now();
    let mut entries = Vec::new();
    let mut counts = Counts::default();
    let mut active = Vec::new();
    let mut library_bytes = 0;
    let mut database = None;

    if parts.contains(&Part::Library) {
        let library = cfg
            .library_path
            .as_deref()
            .ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
        let copy_path = scratch.0.join("overlay.sqlite");
        {
            // The lock only for the snapshot: everything after reads the copy.
            let conn = db.0.lock().map_err(|e| e.to_string())?;
            // `VACUUM INTO` reads through the WAL, where recent commits live
            // (see `backup.rs`): a file copy would miss them.
            conn.execute("VACUUM INTO ?1", [copy_path.to_string_lossy()])
                .map_err(|e| e.to_string())?;
            active = tables::active_ids(&conn, cfg).map_err(|e| e.to_string())?;
        }
        let copy = Connection::open(&copy_path).map_err(|e| e.to_string())?;
        // The copy is reshaped table by table; a cascade would delete more
        // than each step means to.
        copy.pragma_update(None, "foreign_keys", "OFF")
            .map_err(|e| e.to_string())?;

        // Skeletons first: they read each row's state before it is forced.
        let preferred = files::preferred_previews(&places.config_dir);
        let skel = files::Skeletons {
            conn: &copy,
            cfg,
            library,
            scratch: &scratch.0,
            stamp: &stamp,
            preferred: &preferred,
        };
        entries.extend(skel.of_library()?);
        let (converted, unmanaged_entries) = skel.convert_unmanaged()?;
        entries.extend(unmanaged_entries);
        tables::drop_stock_versions(&copy).map_err(|e| e.to_string())?;
        tables::relativize(&copy, library).map_err(|e| e.to_string())?;
        tables::finish(&copy, parts, &stamp).map_err(|e| e.to_string())?;
        counts = tables::counts(&copy).map_err(|e| e.to_string())?;
        counts.unmanaged = converted;
        library_bytes = tables::library_bytes(&copy).map_err(|e| e.to_string())?;
        // One self-contained file: no `-wal` left beside it to forget. And
        // vacuumed: a deleted row or a rewritten path stays readable in the
        // file's free pages until then — the absolute paths R2 forbids were
        // found there by the round-trip test.
        copy.pragma_update(None, "journal_mode", "DELETE")
            .map_err(|e| e.to_string())?;
        copy.execute_batch("VACUUM").map_err(|e| e.to_string())?;
        drop(copy);
        database = Some(copy_path);
    }

    let settings = files::settings(places, parts)?;
    counts.sessions = settings.presets;
    counts.grids = settings.grids;
    entries.extend(settings.entries);

    Ok(Built {
        _scratch: scratch,
        database,
        entries,
        counts,
        active,
        library_bytes,
    })
}

/// Counts and weight of every part, nothing prepared nor written
/// (EXPORT§7.2): counts from the base, weights from the files a skeleton
/// keeps and the compression measured on a real export ([`files::weigh`]).
pub fn estimate(db: &Db, cfg: &AppConfig, places: &Places) -> Result<Estimate, String> {
    let (mut counts, folders, base_bytes) = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let counts = tables::counts(&conn).map_err(|e| e.to_string())?;
        let folders = match cfg.library_path.as_deref() {
            Some(library) => files::folders_to_weigh(&conn, library).map_err(|e| e.to_string())?,
            None => Vec::new(),
        };
        let base_bytes: i64 = conn
            .query_row(
                "SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        (counts, folders, base_bytes.max(0) as u64)
    };
    let mut bytes: BTreeMap<Part, u64> = BTreeMap::new();
    if cfg.library_path.is_some() {
        let base = (base_bytes as f64 * files::BASE_RATIO) as u64;
        bytes.insert(Part::Library, base + files::weigh(&folders));
    }
    let settings = files::settings(places, &[Part::Classification, Part::Sessions, Part::Preferences])?;
    counts.sessions = settings.presets;
    counts.grids = settings.grids;
    for e in &settings.entries {
        *bytes.entry(e.part).or_default() += e.zipped_size();
    }
    Ok(Estimate { counts, bytes })
}

/// Writes the export to `dest` (EXPORT§4 to §6). Written beside `dest` first
/// and renamed at the end: a failure never leaves half a file under the
/// name the user chose.
pub fn export(db: &Db, cfg: &AppConfig, places: &Places, parts: &[Part], dest: &Path) -> Result<ExportReport, String> {
    let parts = effective(parts);
    let built = build(db, cfg, places, &parts)?;
    let manifest = Manifest {
        format: FORMAT,
        app_version: places.app_version.clone(),
        exported_at: now(),
        parts: parts.clone(),
        counts: built.counts.clone(),
        active_at_export: built.active.clone(),
        library_bytes_at_export: built.library_bytes,
        wiki_content_version: crate::wiki::store::CONTENT_VERSION,
    };

    let partial = dest.with_extension("pitbox-partial");
    let written = write_zip(&partial, &manifest, built.database.as_deref(), &built.entries);
    if let Err(e) = written {
        if let Err(rm) = std::fs::remove_file(&partial) {
            log::warn!("transfer: partial export {} not removed: {rm}", partial.display());
        }
        return Err(e);
    }
    std::fs::rename(&partial, dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    Ok(ExportReport {
        path: dest.to_string_lossy().into_owned(),
        bytes: std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0),
        counts: built.counts,
        library_bytes: built.library_bytes,
    })
}

/// Written file by file, never the whole archive in memory (EXPORT§8.1).
fn write_zip(
    path: &Path,
    manifest: &Manifest,
    database: Option<&Path>,
    entries: &[files::Entry],
) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(file));
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(true);
    // Images are compressed already: deflating them again costs time for
    // nothing.
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let err = |e: zip::result::ZipError| e.to_string();
    let io = |e: std::io::Error| e.to_string();

    zip.start_file(MANIFEST_ENTRY, deflated).map_err(err)?;
    let json = serde_json::to_vec_pretty(manifest).map_err(|e| e.to_string())?;
    zip.write_all(&json).map_err(io)?;
    if let Some(db_file) = database {
        zip.start_file(DATABASE_ENTRY, deflated).map_err(err)?;
        let mut f = std::fs::File::open(db_file).map_err(io)?;
        std::io::copy(&mut f, &mut zip).map_err(io)?;
    }
    for e in entries {
        let options = if e.is_image() { stored } else { deflated };
        zip.start_file(e.name.as_str(), options).map_err(err)?;
        e.write_to(&mut zip)?;
    }
    let mut inner = zip.finish().map_err(err)?;
    inner.flush().map_err(io)
}

// --- Import ------------------------------------------------------------------

type Archive = zip::ZipArchive<std::io::BufReader<std::fs::File>>;

fn open_archive(path: &Path) -> Result<Archive, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    zip::ZipArchive::new(std::io::BufReader::new(file)).map_err(|_| crate::errors::TRANSFER_NOT_AN_EXPORT.to_string())
}

fn read_manifest(zip: &mut Archive) -> Result<Manifest, String> {
    let mut entry = zip
        .by_name(MANIFEST_ENTRY)
        .map_err(|_| crate::errors::TRANSFER_NOT_AN_EXPORT.to_string())?;
    let mut raw = String::new();
    entry.read_to_string(&mut raw).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|_| crate::errors::TRANSFER_NOT_AN_EXPORT.to_string())
}

/// Reads the manifest alone, and says whether this installation can take it
/// (EXPORT§7.3, steps 1 and 2).
pub fn inspect(conn: &Connection, cfg: &AppConfig, places: &Places, path: &Path) -> Result<Inspection, String> {
    let manifest = read_manifest(&mut open_archive(path)?)?;
    let refusal = check_importable(conn, cfg, &manifest, &places.app_version).map_err(|e| e.to_string())?;
    Ok(Inspection {
        manifest,
        refusal,
        file_bytes: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
    })
}

/// R3 and R4: a format this build knows, a version not newer than it, and an
/// installation with nothing in it yet.
pub fn check_importable(
    conn: &Connection,
    cfg: &AppConfig,
    manifest: &Manifest,
    app_version: &str,
) -> rusqlite::Result<Option<Refusal>> {
    let refuse = |key| Refusal {
        key,
        count: None,
        version: None,
    };
    if manifest.format > FORMAT {
        return Ok(Some(refuse("transfer.refuseFormat")));
    }
    if is_newer(&manifest.app_version, app_version) {
        return Ok(Some(Refusal {
            version: Some(manifest.app_version.clone()),
            ..refuse("transfer.refuseNewer")
        }));
    }
    if cfg.library_path.is_none() || cfg.ac_install_path.is_none() {
        return Ok(Some(refuse("transfer.refusePaths")));
    }
    let held = tables::held(conn)?;
    if held > 0 {
        return Ok(Some(Refusal {
            count: Some(held),
            ..refuse("transfer.refuseNotEmpty")
        }));
    }
    Ok(None)
}

/// `a` strictly newer than `b`, both `x.y.z`. A version that does not read
/// as one counts as newer: refusing is the safe side of not knowing (R4).
fn is_newer(a: &str, b: &str) -> bool {
    fn parts(v: &str) -> Option<Vec<u64>> {
        v.trim()
            .split(['.', '-', '+'])
            .take(3)
            .map(|p| p.parse::<u64>().ok())
            .collect()
    }
    match (parts(a), parts(b)) {
        (Some(a), Some(b)) => a > b,
        _ => true,
    }
}

/// Imports `path` into this installation (EXPORT§7.3, step 4), only the
/// `parts` asked for among those it holds. All or nothing: a failure on the
/// way rolls the base back and removes every file written, and the
/// installation is empty again.
pub fn import(
    db: &Db,
    cfg: &AppConfig,
    places: &Places,
    rules: &crate::rules::Rules,
    path: &Path,
    parts: &[Part],
) -> Result<ImportReport, String> {
    let mut zip = open_archive(path)?;
    let manifest = read_manifest(&mut zip)?;
    let parts: Vec<Part> = effective(parts)
        .into_iter()
        .filter(|p| manifest.parts.contains(p))
        .collect();
    let library = cfg.library_path.clone().ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;

    let conn = db.0.lock().map_err(|e| e.to_string())?;
    if let Some(refusal) = check_importable(&conn, cfg, &manifest, &places.app_version).map_err(|e| e.to_string())? {
        return Err(refusal_error(&refusal).into());
    }
    let library_part = parts.contains(&Part::Library);
    if library_part {
        files::check_free(&mut zip, &library)?;
    }

    // 1. The current base saved, even empty, by the startup net's mechanism.
    crate::backup::backup_now(&places.config_dir, places.presets_dir.as_deref())?;
    // 2. This machine's own game content, indexed first: the export's notes
    //    on it are then laid on the ids that exist here (EXPORT§4.2).
    if library_part {
        crate::stock::index_stock_content(&conn, cfg, rules, false)?;
    }

    let scratch = Scratch::new("import")?;
    let mut undo = files::Undo::default();
    let mut report = ImportReport {
        active_at_export: manifest.active_at_export.len(),
        library_bytes_at_export: manifest.library_bytes_at_export,
        ..Default::default()
    };
    let attached = if library_part {
        Some(attach(&conn, &mut zip, &scratch.0)?)
    } else {
        None
    };
    let result = (|| -> Result<(), String> {
        if attached.is_some() {
            conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;
            let wiki = manifest.wiki_content_version == crate::wiki::store::CONTENT_VERSION;
            let merged =
                tables::merge(&conn, parts.contains(&Part::Profiles), wiki, &now()).map_err(|e| e.to_string())?;
            report.mods = merged.mods;
            report.layers = merged.layers;
            report.apps = merged.apps;
            report.others = merged.others;
            report.stock_applied = merged.stock_applied;
            report.stock_missing = merged.stock_missing;
            report.local_kept = merged.local_kept;
            files::unpack_library(&mut zip, &library, &merged.skipped_folders, &mut undo)?;
        }
        report.presets_renamed = files::unpack_settings(&mut zip, places, &parts, &mut undo)?;
        if attached.is_some() {
            conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    if result.is_err() && attached.is_some() {
        if let Err(e) = conn.execute_batch("ROLLBACK") {
            log::warn!("transfer: rollback failed: {e}");
        }
    }
    if attached.is_some() {
        if let Err(e) = conn.execute_batch("DETACH DATABASE src") {
            log::warn!("transfer: export base not detached: {e}");
        }
    }
    if let Err(e) = result {
        undo.undo();
        return Err(e);
    }
    Ok(report)
}

/// Extracts the export's base, brings it up to this build's schema — the
/// migrations of `overlay::open`, exactly as an old local base (R4) — and
/// attaches it as `src`.
fn attach(conn: &Connection, zip: &mut Archive, scratch: &Path) -> Result<(), String> {
    let file = scratch.join("export.sqlite");
    {
        let mut entry = zip
            .by_name(DATABASE_ENTRY)
            .map_err(|_| crate::errors::TRANSFER_NOT_AN_EXPORT.to_string())?;
        let mut out = std::fs::File::create(&file).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    }
    let migrated = overlay::open(&file).map_err(|e| e.to_string())?;
    migrated
        .pragma_update(None, "journal_mode", "DELETE")
        .map_err(|e| e.to_string())?;
    drop(migrated);
    conn.execute("ATTACH DATABASE ?1 AS src", [file.to_string_lossy()])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// The error an import returns when the inspection would have refused it:
/// the same refusal, without its count.
fn refusal_error(r: &Refusal) -> &'static str {
    match r.key {
        "transfer.refuseFormat" | "transfer.refuseNewer" => crate::errors::TRANSFER_INCOMPATIBLE,
        "transfer.refusePaths" => crate::errors::LIBRARY_NOT_CONFIGURED,
        _ => crate::errors::TRANSFER_NOT_EMPTY,
    }
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}
