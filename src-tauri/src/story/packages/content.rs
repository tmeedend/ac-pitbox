//! The cars and tracks a story package carries, on the player's side
//! (SPEC-story-mode, Content: story package and dependencies). The player's
//! own mods and versions stay untouched: nothing renamed, nothing
//! overwritten.
//!
//! - **What the player lacks** goes through the **ordinary import**, as a
//!   downloaded mod update does (§4.7): the item is an archive of the mod's
//!   folder, set aside in `story/installed/.imports/<story id>/` until the
//!   screen has imported it - with the report and the questions of any
//!   import.
//! - **What the player has** is never replaced: the package's copy is
//!   compared to theirs, file by file (`identity::content_differs`), and
//!   one that differs is recorded (`<story id>.content.json`), for Story
//!   Mode to say it plays the player's version.

use std::collections::BTreeSet;
use std::fs::File;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use story_engine::needs::AcKind;
use story_engine::package::Item;

use crate::errors;
use crate::story::holdings::Lookup;

/// What an install recorded of the cars and tracks it found here.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Record {
    /// Here in another version than the package's.
    other_version: Vec<(AcKind, String)>,
}

fn imports(root: &Path, story: &str) -> PathBuf {
    root.join(".imports").join(story)
}

fn record_path(root: &Path, story: &str) -> PathBuf {
    root.join(format!("{story}.content.json"))
}

/// A car's or track's folder id as a package names it: one plain folder
/// name, never a path.
pub(super) fn safe_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && !id.starts_with('.') && !id.contains(['/', '\\', ':'])
}

/// Whether the item's zip holds a mod's folder and nothing else: every
/// entry under `<id>/`, none leaving it.
pub(super) fn check_item(zip_path: &Path, id: &str) -> Result<(), String> {
    let damaged = |why: String| {
        log::warn!("story package: {id}: {why}");
        errors::STORY_PACKAGE_DAMAGED.to_string()
    };
    let zip = (File::open(zip_path).map_err(|e| e.to_string()))
        .and_then(|f| zip::ZipArchive::new(f).map_err(|e| e.to_string()))
        .map_err(&damaged)?;
    let prefix = format!("{id}/");
    for name in zip.file_names() {
        let inside = name.strip_prefix(&prefix).unwrap_or_default();
        let fits = !inside.is_empty() && inside.split('/').all(|p| p != ".." && !p.contains(['\\', ':']));
        if !fits && name != prefix {
            return Err(damaged(format!("`{name}` is not inside {prefix}")));
        }
    }
    Ok(())
}

/// After an install: each car and track item, checked already and copied to
/// `staged` under its file name, is set aside for the import when the player
/// lacks it, or compared to theirs. Returns the archives to import.
pub(super) fn place(root: &Path, story: &str, items: &[&Item], staged: &Path, lookup: Lookup) -> Vec<PathBuf> {
    let wanted: BTreeSet<(AcKind, String)> = (items.iter())
        .filter_map(|i| Some((i.kind.content()?, i.id.clone()?)))
        .collect();
    let held = lookup(&wanted);
    let dest = imports(root, story);
    let mut to_import = Vec::new();
    let mut record = Record::default();
    for item in items {
        let (Some(kind), Some(id)) = (item.kind.content(), item.id.as_deref()) else {
            continue;
        };
        let zip_path = staged.join(&item.file);
        match held
            .iter()
            .find(|h| h.kind == kind && h.id == id)
            .and_then(|h| h.folder.as_ref())
        {
            Some(theirs) => {
                if differs(&zip_path, id, theirs, staged) {
                    record.other_version.push((kind, id.to_string()));
                }
            }
            None => {
                let to = dest.join(&item.file);
                let moved = std::fs::create_dir_all(&dest).and_then(|_| std::fs::rename(&zip_path, &to));
                match moved {
                    Ok(()) => to_import.push(to),
                    Err(e) => log::warn!("story package: {id} not set aside for the import: {e}"),
                }
            }
        }
    }
    let path = record_path(root, story);
    let written = serde_json::to_string_pretty(&record)
        .map_err(|e| e.to_string())
        .and_then(|json| std::fs::write(&path, json).map_err(|e| e.to_string()));
    if let Err(e) = written {
        log::warn!("story package: {} not written: {e}", path.display());
    }
    to_import
}

/// Whether the package's copy of `id` differs from the player's. Unpacked
/// beside the item to compare; what cannot be read counts as the same - the
/// player's copy plays either way, and the log says why.
fn differs(zip_path: &Path, id: &str, theirs: &Path, staged: &Path) -> bool {
    let into = staged.join(format!("compare-{id}"));
    let result = super::extract(zip_path, &into, |_| true)
        .and_then(|_| crate::identity::content_differs(&into.join(id), theirs).map_err(|e| e.to_string()));
    if let Err(e) = std::fs::remove_dir_all(&into) {
        log::warn!("story package: {} not removed: {e}", into.display());
    }
    result.unwrap_or_else(|e| {
        log::warn!("story package: {id} not compared with {}: {e}", theirs.display());
        false
    })
}

/// The cars and tracks the installed story `story` found here in another
/// version than its package's.
pub fn other_versions(root: &Path, story: &str) -> Vec<(AcKind, String)> {
    (std::fs::read_to_string(record_path(root, story)).ok())
        .and_then(|t| serde_json::from_str::<Record>(&t).ok())
        .map(|r| r.other_version)
        .unwrap_or_default()
}

/// An archive set aside for the import, once the import is done with it.
/// Only ever one of `.imports/`.
pub fn imported(root: &Path, archive: &Path) -> Result<(), String> {
    if !archive.starts_with(root.join(".imports")) || archive.components().any(|c| c.as_os_str() == "..") {
        return Err(format!("not an archive set aside: {}", archive.display()));
    }
    if archive.is_file() {
        std::fs::remove_file(archive).map_err(|e| format!("removing {}: {e}", archive.display()))?;
    }
    Ok(())
}

/// Forgets what an earlier install of `story` set aside or recorded.
pub(super) fn clear(root: &Path, story: &str) {
    let dir = imports(root, story);
    if dir.is_dir() {
        if let Err(e) = std::fs::remove_dir_all(&dir) {
            log::warn!("story package: {} not removed: {e}", dir.display());
        }
    }
    let record = record_path(root, story);
    if record.is_file() {
        if let Err(e) = std::fs::remove_file(&record) {
            log::warn!("story package: {} not removed: {e}", record.display());
        }
    }
}
