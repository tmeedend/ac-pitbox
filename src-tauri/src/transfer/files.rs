//! What an export holds besides the base (EXPORT§5): the names its files take
//! in the zip, the [`Entry`] each one is, and the small readers the four
//! modules around it share.
//!
//! - `skeletons.rs`: for every folder the library holds, what the showcase
//!   keeps of it and the manifest of what it does not;
//! - `weigh.rs`: what an export would weigh, before making it;
//! - `settings.rs`: classification, sessions, preferences, without a path of
//!   the machine;
//! - `unpack.rs`: how all of it lands back at import, and its undo.
//!
//! Only files these modules name come back into `app_config_dir`: an archive
//! is a file someone may hand over, and an entry it carries must never be
//! able to replace the base or `config.json` behind the import's back.

use std::io::Write;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use super::Part;

/// Where the skeletons sit in the zip: at the library's own relative paths.
pub(super) const LIBRARY_PREFIX: &str = "bibliotheque/fichiers/";
pub(super) const CLASSIFICATION_PREFIX: &str = "classement/";
pub(super) const SESSIONS_PREFIX: &str = "sessions/";
pub(super) const PREFERENCES_PREFIX: &str = "preferences/";
/// The classification's files in `app_config_dir` (REGLES§2, TAXO§4).
/// `tag-rules.json` only exists until it is migrated.
pub(super) const CLASSIFICATION_FILES: &[&str] = &[
    "taxonomy.json",
    "rules-overlay.json",
    "tag-rules.json",
    "brand_logos.json",
];
pub(super) const GRIDS_FILE: &str = "saved_grids.json";
/// Preferences that travel as they are, but for their paths
/// (`settings::scrub_paths`).
/// `config.json` travels as `prefs.json`, its `prefs` object alone.
pub(super) const PREFERENCE_FILES: &[&str] = &["ui_prefs.json", "library_columns.json"];
pub(super) const MUSIC_FILE: &str = "music.json";
pub(super) const PREFS_ENTRY: &str = "prefs.json";
pub(super) const CONFIG_FILE: &str = "config.json";

/// One file of the zip.
pub(super) struct Entry {
    pub name: String,
    pub part: Part,
    source: Source,
}

enum Source {
    File(PathBuf),
    Bytes(Vec<u8>),
}

impl Entry {
    pub(super) fn file(part: Part, name: String, path: PathBuf) -> Self {
        Self {
            name,
            part,
            source: Source::File(path),
        }
    }

    pub(super) fn bytes(part: Part, name: String, bytes: Vec<u8>) -> Self {
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
        }
    }

    /// What it will weigh in the zip (`weigh::zipped`).
    pub fn zipped_size(&self) -> u64 {
        super::weigh::zipped(&self.name, self.size())
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
        }
        Ok(())
    }
}

/// The files of `dir`, links not followed — a skin projected into a version
/// folder is a junction onto a skin stored apart (§8.3), not its file.
pub(super) fn files_in(dir: &Path) -> Vec<(PathBuf, PathBuf)> {
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

pub(super) fn zip_name(prefix: &str, rel: &str, file: &Path) -> String {
    format!("{prefix}{rel}/{}", file.to_string_lossy().replace('\\', "/"))
}

pub(super) fn read_json(path: &Path) -> Option<serde_json::Value> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw)
        .inspect_err(|e| log::warn!("transfer: {} unreadable, not exported: {e}", path.display()))
        .ok()
}

pub(super) fn flat_files(dir: &Path) -> Vec<PathBuf> {
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
pub(super) fn logo_name(path: &Path) -> Option<String> {
    with_extension(path, &["png", "svg"])
}

pub(super) fn preset_name(path: &Path) -> Option<String> {
    with_extension(path, &["cmpreset"])
}
