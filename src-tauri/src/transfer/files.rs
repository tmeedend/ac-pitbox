//! What an export holds besides the base, and how it lands back (EXPORT§5).
//!
//! Two families. The **skeletons**: for every folder the library holds, what
//! the showcase keeps of it (ESPACE§3.1) and the manifest of what it does not
//! — written by the export for a complete mod too, so that at arrival every
//! one knows exactly what it lacks. And the **settings**: classification,
//! sessions, preferences, file by file, never a path of the machine.
//!
//! Only files this module names come back into `app_config_dir`: an archive
//! is a file someone may hand over, and an entry it carries must never be
//! able to replace the base or `config.json` behind the import's back.

use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use rusqlite::Connection;
use walkdir::WalkDir;

use super::{Archive, Part, Places};
use crate::config::AppConfig;
use crate::modscan::ModKind;
use crate::skeleton::{self, Manifest};

/// Where the skeletons sit in the zip: at the library's own relative paths.
const LIBRARY_PREFIX: &str = "bibliotheque/fichiers/";
const CLASSIFICATION_PREFIX: &str = "classement/";
const SESSIONS_PREFIX: &str = "sessions/";
const PREFERENCES_PREFIX: &str = "preferences/";
/// The classification's files in `app_config_dir` (REGLES§2, TAXO§4).
/// `tag-rules.json` only exists until it is migrated.
const CLASSIFICATION_FILES: &[&str] = &[
    "taxonomy.json",
    "rules-overlay.json",
    "tag-rules.json",
    "brand_logos.json",
];
const GRIDS_FILE: &str = "saved_grids.json";
/// Preferences that travel as they are. `config.json` travels as `prefs.json`
/// (its `prefs` object alone) and `music.json` without its folders.
const PREFERENCE_FILES: &[&str] = &["ui_prefs.json", "library_columns.json"];
const MUSIC_FILE: &str = "music.json";
const PREFS_ENTRY: &str = "prefs.json";
const CONFIG_FILE: &str = "config.json";
/// What a frozen image weighs, for the estimate: a few dozen KB (ESPACE§3.2).
const IMAGE_ESTIMATE: u64 = 60 * 1024;

/// One file of the zip.
pub(super) struct Entry {
    pub name: String,
    pub part: Part,
    source: Source,
}

enum Source {
    File(PathBuf),
    Bytes(Vec<u8>),
    /// Weighed, never written: an image the estimate does not produce.
    Estimated(u64),
}

impl Entry {
    fn file(part: Part, name: String, path: PathBuf) -> Self {
        Self {
            name,
            part,
            source: Source::File(path),
        }
    }

    fn bytes(part: Part, name: String, bytes: Vec<u8>) -> Self {
        Self {
            name,
            part,
            source: Source::Bytes(bytes),
        }
    }

    pub fn size(&self) -> u64 {
        match &self.source {
            Source::File(p) => std::fs::metadata(p).map(|m| m.len()).unwrap_or(0),
            Source::Bytes(b) => b.len() as u64,
            Source::Estimated(n) => *n,
        }
    }

    pub fn is_image(&self) -> bool {
        let lower = self.name.to_lowercase();
        [".png", ".jpg", ".jpeg", ".dds"].iter().any(|e| lower.ends_with(e))
    }

    pub fn write_to(&self, out: &mut impl Write) -> Result<(), String> {
        match &self.source {
            Source::File(p) => {
                let mut f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
                std::io::copy(&mut f, out).map_err(|e| e.to_string())?;
            }
            Source::Bytes(b) => out.write_all(b).map_err(|e| e.to_string())?,
            Source::Estimated(_) => return Err(format!("{}: weighed, never produced", self.name)),
        }
        Ok(())
    }
}

/// The files of `dir`, links not followed — a skin projected into a version
/// folder is a junction onto a skin stored apart (§8.3), not its file.
fn files_in(dir: &Path) -> Vec<(PathBuf, PathBuf)> {
    WalkDir::new(dir)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let rel = e.path().strip_prefix(dir).ok()?.to_path_buf();
            Some((rel, e.path().to_path_buf()))
        })
        .collect()
}

fn zip_name(prefix: &str, rel: &str, file: &Path) -> String {
    format!("{prefix}{rel}/{}", file.to_string_lossy().replace('\\', "/"))
}

// --- Skeletons -----------------------------------------------------------------

/// The skeletons of a library, read from the export's copy of the base
/// **before** its rows are forced into the showcase: a row's state says
/// whether its folder is a skeleton already or must be made one.
pub(super) struct Skeletons<'a> {
    pub conn: &'a Connection,
    pub cfg: &'a AppConfig,
    pub library: &'a Path,
    pub scratch: &'a Path,
    pub stamp: &'a str,
    pub dry: bool,
}

/// One folder of the library on its way into the zip.
struct Folder {
    /// Relative to the library, `/`-separated: the zip's name for it.
    rel: String,
    dir: PathBuf,
    /// Already a skeleton: it leaves as it is.
    freed: bool,
}

impl Skeletons<'_> {
    /// Every folder the library holds: versions of managed mods, layers,
    /// attached skins and sounds, apps and "other" mods.
    pub fn of_library(&self) -> Result<Vec<Entry>, String> {
        let mut out = Vec::new();
        self.versions(&mut out).map_err(|e| e.to_string())?;
        self.layers(&mut out).map_err(|e| e.to_string())?;
        self.manifest_only(
            "SELECT id, parent_id, library_path, content_state, source_archive
             FROM sub_mods WHERE removable = 1",
            &mut out,
        )
        .map_err(|e| e.to_string())?;
        for table in ["apps", "other_mods"] {
            self.manifest_only(
                &format!("SELECT id, id, library_path, content_state, source_archive FROM {table}"),
                &mut out,
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(out)
    }

    fn folder(&self, stored: &str, state: &str) -> Option<Folder> {
        let rel = super::tables::relative(self.library, stored)?;
        Some(Folder {
            dir: self.library.join(&rel),
            rel,
            freed: state == crate::overlay::CONTENT_SKELETON,
        })
    }

    fn versions(&self, out: &mut Vec<Entry>) -> rusqlite::Result<()> {
        let mut stmt = self.conn.prepare(
            "SELECT v.id, v.mod_id, v.library_path, v.content_state, v.version_label,
                    v.content_signature, v.source_archive, v.source_site, v.source_file_name,
                    m.kind, COALESCE(m.active_version_id = v.id, 0)
             FROM versions v JOIN mods m ON m.id_interne = v.mod_id
             WHERE m.is_stock = 0",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                Manifest {
                    version_label: r.get(4)?,
                    content_signature: r.get(5)?,
                    source_archive: r.get(6)?,
                    source_site: r.get(7)?,
                    source_file_name: r.get(8)?,
                    ..Manifest::new(self.stamp.to_string(), String::new(), Vec::new())
                },
                r.get::<_, String>(9)?,
                r.get::<_, bool>(10)?,
            ))
        })?;
        for row in rows {
            let (mod_id, stored, state, labels, kind, active) = row?;
            let Some(folder) = self.folder(&stored, &state) else {
                continue;
            };
            if folder.freed {
                self.as_is(&folder, out);
                continue;
            }
            let kind = ModKind::from_column(&kind);
            let image = if self.dry {
                None
            } else if active {
                // The image the card shows, chosen by the card's own function
                // (ESPACE§3.3) — a preferred skin, a skin pack's livery.
                crate::overlay::get_mod(self.conn, &mod_id)?
                    .and_then(|m| crate::library::preview_for(self.conn, self.cfg, &m))
                    .map(PathBuf::from)
            } else {
                crate::showcase::own_preview(kind, &folder.dir)
            };
            let manifest = Manifest { mod_id, ..labels };
            self.skeleton_of(&folder, Some(kind), manifest, image, out);
        }
        Ok(())
    }

    fn layers(&self, out: &mut Vec<Entry>) -> rusqlite::Result<()> {
        let mut stmt = self
            .conn
            .prepare("SELECT parent_id, parent_kind, library_path, content_state, source_archive FROM layers")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })?;
        for row in rows {
            let (parent, parent_kind, stored, state, archive) = row?;
            let Some(folder) = self.folder(&stored, &state) else {
                continue;
            };
            if folder.freed {
                self.as_is(&folder, out);
                continue;
            }
            // A layer keeps its host's skeleton (ESPACE§5.4); an app's layer
            // keeps nothing.
            let kind = match parent_kind.as_str() {
                "Car" => Some(ModKind::Car),
                "Track" => Some(ModKind::Track),
                _ => None,
            };
            let mut manifest = Manifest::new(self.stamp.to_string(), parent, Vec::new());
            manifest.source_archive = archive;
            self.skeleton_of(&folder, kind, manifest, None, out);
        }
        Ok(())
    }

    /// Rows that keep no skeleton (ESPACE§5.4, ESPACE§5.6): `sql` selects an id,
    /// the id the manifest names, the folder, the state and the archive.
    fn manifest_only(&self, sql: &str, out: &mut Vec<Entry>) -> rusqlite::Result<()> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })?;
        for row in rows {
            let (owner, stored, state, archive) = row?;
            let Some(folder) = self.folder(&stored, &state) else {
                continue;
            };
            if folder.freed {
                self.as_is(&folder, out);
                continue;
            }
            let mut manifest = Manifest::new(self.stamp.to_string(), owner, Vec::new());
            manifest.source_archive = archive;
            self.skeleton_of(&folder, None, manifest, None, out);
        }
        Ok(())
    }

    /// A folder already in the showcase: its skeleton, as it is.
    fn as_is(&self, folder: &Folder, out: &mut Vec<Entry>) {
        for (rel, path) in files_in(&folder.dir) {
            out.push(Entry::file(
                Part::Library,
                zip_name(LIBRARY_PREFIX, &folder.rel, &rel),
                path,
            ));
        }
    }

    /// A complete folder made a skeleton in the zip only — the library is
    /// never touched: what `kind`'s whitelist keeps (nothing without one), the
    /// manifest of the rest, and the frozen image when there is one.
    fn skeleton_of(
        &self,
        folder: &Folder,
        kind: Option<ModKind>,
        manifest: Manifest,
        image: Option<PathBuf>,
        out: &mut Vec<Entry>,
    ) {
        let removed = match kind {
            Some(k) => skeleton::removable_files(k, &folder.dir),
            None => skeleton::all_files(&folder.dir),
        };
        let manifest = Manifest {
            removed_bytes: removed.iter().map(|f| f.size).sum(),
            removed,
            ..manifest
        };
        if let Some(kind) = kind {
            for (rel, path) in files_in(&folder.dir) {
                // A stale manifest or image of a former showcase is not this
                // export's: the ones written below are.
                if skeleton::is_showcase_file(&rel) || !skeleton::is_kept(kind, &rel) {
                    continue;
                }
                let name = zip_name(LIBRARY_PREFIX, &folder.rel, &rel);
                out.push(if skeleton::is_reduced(kind, &rel) {
                    self.reduced(name, &path)
                } else {
                    Entry::file(Part::Library, name, path)
                });
            }
        }
        match serde_json::to_vec_pretty(&manifest) {
            Ok(json) => out.push(Entry::bytes(
                Part::Library,
                zip_name(LIBRARY_PREFIX, &folder.rel, Path::new(skeleton::MANIFEST_NAME)),
                json,
            )),
            Err(e) => log::warn!("transfer: manifest of {} not written: {e}", folder.rel),
        }
        if self.dry && kind.is_some() {
            out.push(Entry {
                name: zip_name(LIBRARY_PREFIX, &folder.rel, Path::new(".pitbox-vitrine.jpg")),
                part: Part::Library,
                source: Source::Estimated(IMAGE_ESTIMATE),
            });
        } else if let Some(src) = image {
            match self.frozen(&src) {
                Ok(frozen) => {
                    let file = frozen.file_name().map(PathBuf::from).unwrap_or_default();
                    out.push(Entry::file(
                        Part::Library,
                        zip_name(LIBRARY_PREFIX, &folder.rel, &file),
                        frozen,
                    ));
                }
                Err(e) => log::warn!("transfer: card image of {} not frozen: {e}", folder.rel),
            }
        }
    }

    /// A track preview, reduced in a scratch copy (ESPACE§3.3).
    fn reduced(&self, name: String, path: &Path) -> Entry {
        if self.dry {
            let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            return Entry {
                name,
                part: Part::Library,
                source: Source::Estimated(size.min(IMAGE_ESTIMATE)),
            };
        }
        let copy = self.scratch_file(path.file_name().map(PathBuf::from).unwrap_or_default());
        let made = std::fs::copy(path, &copy)
            .map_err(|e| e.to_string())
            .and_then(|_| skeleton::reduce_in_place(&copy));
        match made {
            Ok(_) => Entry::file(Part::Library, name, copy),
            Err(e) => {
                log::warn!("transfer: {} not reduced, exported as is: {e}", path.display());
                Entry::file(Part::Library, name, path.to_path_buf())
            }
        }
    }

    fn frozen(&self, src: &Path) -> Result<PathBuf, String> {
        let dir = self.scratch_file(PathBuf::new());
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        skeleton::freeze_image(src, &dir)
    }

    /// A fresh place in the scratch folder: every reduced or frozen image
    /// has its own, so two of the same name never overwrite each other.
    fn scratch_file(&self, name: PathBuf) -> PathBuf {
        let dir = self.scratch.join(uuid::Uuid::new_v4().to_string());
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::warn!("transfer: scratch {} not created: {e}", dir.display());
        }
        dir.join(name)
    }

    /// Mods installed outside Pit Box leave **managed** (EXPORT§4.2): a
    /// version in the showcase is made of their folder in `content/` — its
    /// skeleton, a signature of its real files — under the library path the
    /// import would give them. At arrival they are mods like the others, and
    /// importing their archive recovers them. Returns how many, and their
    /// skeletons. Their game folder is only read.
    pub fn convert_unmanaged(&self) -> Result<(usize, Vec<Entry>), String> {
        let rows: Vec<(String, String, String, String)> = {
            let mut stmt = self
                .conn
                .prepare(
                    "SELECT m.id_interne, m.kind, v.id, v.library_path
                     FROM mods m JOIN versions v ON v.id = m.active_version_id
                     WHERE m.is_stock = 1 AND m.is_unmanaged = 1",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                .map_err(|e| e.to_string())?;
            rows.collect::<rusqlite::Result<_>>().map_err(|e| e.to_string())?
        };
        let mut out = Vec::new();
        let mut converted = 0;
        for (id, kind_col, version_id, content_dir) in rows {
            let dir = PathBuf::from(&content_dir);
            if !dir.is_dir() {
                log::warn!("transfer: unmanaged {id} has no folder at {content_dir}, left as game content");
                continue;
            }
            let kind = ModKind::from_column(&kind_col);
            let rel = format!("{}/{id}/export", kind.content_folder());
            let signature = crate::identity::content_signature(&dir);
            let size = crate::inspect::dir_size_bytes(&dir) as i64;
            let mut manifest = Manifest::new(self.stamp.to_string(), id.clone(), Vec::new());
            manifest.content_signature = Some(signature.clone());
            let image = (!self.dry).then(|| crate::showcase::own_preview(kind, &dir)).flatten();
            let folder = Folder {
                rel: rel.clone(),
                dir,
                freed: false,
            };
            self.skeleton_of(&folder, Some(kind), manifest, image, &mut out);
            self.conn
                .execute(
                    "UPDATE versions SET library_path = ?2, content_signature = ?3, size_bytes = ?4 WHERE id = ?1",
                    rusqlite::params![version_id, rel, signature, size],
                )
                .map_err(|e| e.to_string())?;
            self.conn
                .execute(
                    "UPDATE mods SET is_stock = 0, is_unmanaged = 0 WHERE id_interne = ?1",
                    [&id],
                )
                .map_err(|e| e.to_string())?;
            converted += 1;
        }
        Ok((converted, out))
    }
}

// --- Settings --------------------------------------------------------------------

pub(super) struct Settings {
    pub entries: Vec<Entry>,
    pub presets: usize,
    pub grids: usize,
}

fn read_json(path: &Path) -> Option<serde_json::Value> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw)
        .inspect_err(|e| log::warn!("transfer: {} unreadable, not exported: {e}", path.display()))
        .ok()
}

/// The settings of the parts asked for, file by file (EXPORT§5.2).
pub(super) fn settings(places: &Places, parts: &[Part]) -> Result<Settings, String> {
    let dir = &places.config_dir;
    let mut entries = Vec::new();
    let mut presets = 0;
    let mut grids = 0;

    if parts.contains(&Part::Classification) {
        for name in CLASSIFICATION_FILES {
            let path = dir.join(name);
            if path.is_file() {
                entries.push(Entry::file(
                    Part::Classification,
                    format!("{CLASSIFICATION_PREFIX}{name}"),
                    path,
                ));
            }
        }
        for path in flat_files(&dir.join(crate::logos::LOGOS_DIR)) {
            if let Some(name) = logo_name(&path) {
                entries.push(Entry::file(
                    Part::Classification,
                    format!("{CLASSIFICATION_PREFIX}{}/{name}", crate::logos::LOGOS_DIR),
                    path,
                ));
            }
        }
    }

    if parts.contains(&Part::Sessions) {
        let path = dir.join(GRIDS_FILE);
        if let Some(value) = read_json(&path) {
            grids = value.as_object().map_or(0, |o| o.len());
            entries.push(Entry::file(
                Part::Sessions,
                format!("{SESSIONS_PREFIX}{GRIDS_FILE}"),
                path,
            ));
        }
        if let Some(presets_dir) = &places.presets_dir {
            for path in flat_files(presets_dir) {
                if let Some(name) = preset_name(&path) {
                    entries.push(Entry::file(
                        Part::Sessions,
                        format!("{SESSIONS_PREFIX}presets/{name}"),
                        path,
                    ));
                    presets += 1;
                }
            }
        }
    }

    if parts.contains(&Part::Preferences) {
        // `prefs` alone: the six paths of `config.json` are this machine's (R2).
        if let Some(prefs) = read_json(&dir.join(CONFIG_FILE)).and_then(|c| c.get("prefs").cloned()) {
            let json = serde_json::to_vec_pretty(&prefs).map_err(|e| e.to_string())?;
            entries.push(Entry::bytes(
                Part::Preferences,
                format!("{PREFERENCES_PREFIX}{PREFS_ENTRY}"),
                json,
            ));
        }
        for name in PREFERENCE_FILES {
            let path = dir.join(name);
            if path.is_file() {
                entries.push(Entry::file(
                    Part::Preferences,
                    format!("{PREFERENCES_PREFIX}{name}"),
                    path,
                ));
            }
        }
        // The music's own folders are paths of this machine: they go, and the
        // other one plays its defaults until the user picks its own.
        if let Some(mut music) = read_json(&dir.join(MUSIC_FILE)) {
            if let Some(o) = music.as_object_mut() {
                o.insert("menu_folder".into(), serde_json::Value::Null);
                o.insert("grid_folder".into(), serde_json::Value::Null);
            }
            let json = serde_json::to_vec_pretty(&music).map_err(|e| e.to_string())?;
            entries.push(Entry::bytes(
                Part::Preferences,
                format!("{PREFERENCES_PREFIX}{MUSIC_FILE}"),
                json,
            ));
        }
    }
    Ok(Settings {
        entries,
        presets,
        grids,
    })
}

fn flat_files(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect()
}

/// A file name alone — no folder, no `..` —, or `None`.
fn plain_name(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    (Path::new(name).components().count() == 1 && name != ".." && name != ".").then(|| name.to_string())
}

fn with_extension(path: &Path, allowed: &[&str]) -> Option<String> {
    let name = plain_name(path)?;
    let ext = Path::new(&name).extension()?.to_str()?.to_lowercase();
    allowed.contains(&ext.as_str()).then_some(name)
}

/// A logo of the user's (TAXO§9): PNG or SVG.
fn logo_name(path: &Path) -> Option<String> {
    with_extension(path, &["png", "svg"])
}

fn preset_name(path: &Path) -> Option<String> {
    with_extension(path, &["cmpreset"])
}

// --- Import ----------------------------------------------------------------------

/// `rel` under `base`, or `None` for anything that could leave it: a zip
/// entry is data, and `..` or an absolute path in one must never reach
/// outside the folder it lands in.
fn under(base: &Path, rel: &str) -> Option<PathBuf> {
    let rel = Path::new(rel);
    if rel.as_os_str().is_empty() || !rel.components().all(|c| matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(base.join(rel))
}

/// The skeleton entries of the zip, and where each lands in the library.
fn library_targets(zip: &mut Archive, library: &Path) -> Result<Vec<(usize, PathBuf)>, String> {
    let mut out = Vec::new();
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let Some(rel) = entry.name().strip_prefix(LIBRARY_PREFIX) else {
            continue;
        };
        let target = under(library, rel).ok_or(crate::errors::TRANSFER_NOT_AN_EXPORT)?;
        out.push((i, target));
    }
    Ok(out)
}

/// R3, on the disk: no folder the import would write is there already.
pub(super) fn check_free(zip: &mut Archive, library: &Path) -> Result<(), String> {
    for (_, target) in library_targets(zip, library)? {
        if target.exists() {
            log::warn!("transfer: {} is already there, import refused", target.display());
            return Err(crate::errors::TRANSFER_FOLDER_NOT_EMPTY.into());
        }
    }
    Ok(())
}

/// Step 5 of EXPORT§7.3: the skeletons, at the library's relative paths.
pub(super) fn unpack_library(zip: &mut Archive, library: &Path, undo: &mut Undo) -> Result<(), String> {
    for (i, target) in library_targets(zip, library)? {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        undo.write(&target, |f| std::io::copy(&mut entry, f).map(|_| ()))?;
    }
    Ok(())
}

fn read_entry(zip: &mut Archive, name: &str) -> Result<Vec<u8>, String> {
    let mut entry = zip.by_name(name).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

/// Step 6 and 7 of EXPORT§7.3: the settings of the parts asked for, into
/// this machine's folders. Returns how many presets were renamed — a preset
/// of the same name already there is never overwritten.
pub(super) fn unpack_settings(
    zip: &mut Archive,
    places: &Places,
    parts: &[Part],
    undo: &mut Undo,
) -> Result<usize, String> {
    let names: Vec<String> = zip.file_names().map(str::to_string).collect();
    let dir = &places.config_dir;
    let mut renamed = 0;
    for name in names {
        if let Some(rest) = name.strip_prefix(CLASSIFICATION_PREFIX) {
            if !parts.contains(&Part::Classification) {
                continue;
            }
            let target = if CLASSIFICATION_FILES.contains(&rest) {
                Some(dir.join(rest))
            } else {
                rest.strip_prefix(&format!("{}/", crate::logos::LOGOS_DIR))
                    .and_then(|f| logo_name(Path::new(f)))
                    .map(|f| dir.join(crate::logos::LOGOS_DIR).join(f))
            };
            if let Some(target) = target {
                let bytes = read_entry(zip, &name)?;
                undo.write(&target, |f| f.write_all(&bytes))?;
            }
        } else if let Some(rest) = name.strip_prefix(SESSIONS_PREFIX) {
            if !parts.contains(&Part::Sessions) {
                continue;
            }
            if rest == GRIDS_FILE {
                let bytes = read_entry(zip, &name)?;
                undo.write(&dir.join(GRIDS_FILE), |f| f.write_all(&bytes))?;
            } else if let Some(file) = rest.strip_prefix("presets/").and_then(|f| preset_name(Path::new(f))) {
                let Some(presets) = &places.presets_dir else {
                    log::warn!("transfer: no presets folder here, {file} not imported");
                    continue;
                };
                let (target, was_renamed) = free_name(presets, &file);
                renamed += usize::from(was_renamed);
                let bytes = read_entry(zip, &name)?;
                undo.write(&target, |f| f.write_all(&bytes))?;
            }
        } else if let Some(rest) = name.strip_prefix(PREFERENCES_PREFIX) {
            if !parts.contains(&Part::Preferences) {
                continue;
            }
            let bytes = read_entry(zip, &name)?;
            if rest == PREFS_ENTRY {
                merge_prefs(dir, &bytes, undo)?;
            } else if PREFERENCE_FILES.contains(&rest) || rest == MUSIC_FILE {
                undo.write(&dir.join(rest), |f| f.write_all(&bytes))?;
            }
        }
    }
    Ok(renamed)
}

/// The preferences laid into this machine's `config.json`: its paths do not
/// move (EXPORT§7.3, step 7).
fn merge_prefs(dir: &Path, bytes: &[u8], undo: &mut Undo) -> Result<(), String> {
    let prefs: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let path = dir.join(CONFIG_FILE);
    let mut config = read_json(&path).unwrap_or_else(|| serde_json::json!({}));
    if let Some(o) = config.as_object_mut() {
        o.insert("prefs".into(), prefs);
    }
    let json = serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?;
    undo.write(&path, |f| f.write_all(&json))
}

/// `name` in `dir`, or `name (2)`, `(3)`… when taken: never a preset of the
/// user's overwritten. The suffix has no word in it — the backend does not
/// know the user's language.
fn free_name(dir: &Path, name: &str) -> (PathBuf, bool) {
    let first = dir.join(name);
    if !first.exists() {
        return (first, false);
    }
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut n = 2;
    loop {
        let candidate = dir.join(format!("{stem} ({n}).{ext}"));
        if !candidate.exists() {
            return (candidate, true);
        }
        n += 1;
    }
}

/// Everything an import wrote, to take it back if it fails on the way
/// (EXPORT§7.3): files created are removed, files replaced get their former
/// bytes back, and folders created empty go.
#[derive(Default)]
pub(super) struct Undo {
    created: Vec<PathBuf>,
    replaced: Vec<(PathBuf, Vec<u8>)>,
    folders: Vec<PathBuf>,
}

impl Undo {
    fn write(
        &mut self,
        target: &Path,
        fill: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
    ) -> Result<(), String> {
        if let Some(parent) = target.parent() {
            let missing: Vec<PathBuf> = parent
                .ancestors()
                .take_while(|d| !d.exists())
                .map(Path::to_path_buf)
                .collect();
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
            self.folders.extend(missing);
        }
        if target.exists() {
            let former = std::fs::read(target).map_err(|e| format!("{}: {e}", target.display()))?;
            self.replaced.push((target.to_path_buf(), former));
        } else {
            self.created.push(target.to_path_buf());
        }
        let mut file = std::fs::File::create(target).map_err(|e| format!("{}: {e}", target.display()))?;
        fill(&mut file).map_err(|e| format!("{}: {e}", target.display()))
    }

    pub fn undo(self) {
        for path in &self.created {
            if let Err(e) = std::fs::remove_file(path) {
                log::warn!("transfer undo: {} not removed: {e}", path.display());
            }
        }
        for (path, former) in &self.replaced {
            if let Err(e) = std::fs::write(path, former) {
                log::warn!("transfer undo: {} not restored: {e}", path.display());
            }
        }
        // Deepest first: a folder empties before its parent is tried.
        let mut folders = self.folders;
        folders.sort_by(|a, b| b.components().count().cmp(&a.components().count()).then(a.cmp(b)));
        folders.dedup();
        for dir in folders {
            if dir.is_dir() {
                if let Err(e) = std::fs::remove_dir(&dir) {
                    log::warn!("transfer undo: {} not removed: {e}", dir.display());
                }
            }
        }
    }
}
