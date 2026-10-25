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
//! - **A layer the story carries** - its own change to a mod - is stored as
//!   a layer of the player's mod, off (`story::library`), and recorded
//!   (`<story id>.layers.json`) for the story's sessions to switch it on. An
//!   update replaces the story's layers; uninstalling removes them. One whose
//!   mod the player lacks is not fetched: the story does not start anyway.
//! - **What the player has** is never replaced, nor even fetched: the
//!   package names each car and track with its signature (paths and sizes,
//!   `package::folder_signature`), compared to the player's copy; one that
//!   differs is recorded (`<story id>.content.json`), for Story Mode to say
//!   it plays the player's version.

use std::collections::BTreeSet;
use std::fs::File;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use story_engine::needs::AcKind;
use story_engine::package::Item;

use crate::errors;
use crate::story::holdings::Lookup;
use crate::story::library::LibraryWrites;

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

/// What an install does with the cars and tracks of a package, decided
/// before anything is fetched: what the player lacks is fetched for the
/// import; what they have is compared without fetching it, by signature.
pub(super) struct Plan<'a> {
    pub fetch: Vec<&'a Item>,
    /// The layers whose car or track the player has.
    pub layers: Vec<&'a Item>,
    /// The cars and tracks the story needs in its own version, where the
    /// player has another: kept to stand in during the story's sessions.
    pub versions: Vec<&'a Item>,
    /// Here in another version than the package's.
    pub other_version: Vec<(AcKind, String)>,
}

/// The plan for `items`, the cars and tracks of a package. A copy whose
/// signature cannot be read, or an item without one, is taken as the same:
/// the player's copy plays either way, and the log says why.
pub(super) fn plan<'a>(items: &[&'a Item], lookup: Lookup) -> Plan<'a> {
    let wanted: BTreeSet<(AcKind, String)> = (items.iter())
        .filter_map(|i| Some((i.kind.content().or(i.host)?, i.id.clone()?)))
        .collect();
    let held = lookup(&wanted);
    let mut plan = Plan {
        fetch: Vec::new(),
        layers: Vec::new(),
        versions: Vec::new(),
        other_version: Vec::new(),
    };
    for item in items {
        if let (Some(host), Some(id)) = (item.host, item.id.as_deref()) {
            if held.iter().any(|h| h.kind == host && h.id == id && h.folder.is_some()) {
                plan.layers.push(item);
            }
            continue;
        }
        let (Some(kind), Some(id)) = (item.kind.content(), item.id.as_deref()) else {
            continue;
        };
        let theirs = (held.iter())
            .find(|h| h.kind == kind && h.id == id)
            .and_then(|h| h.folder.as_ref());
        let Some(theirs) = theirs else {
            plan.fetch.push(item);
            continue;
        };
        let differs = match (&item.signature, story_engine::package::folder_signature(theirs)) {
            (Some(ours), Ok(their)) => *ours != their,
            (None, _) => false,
            (_, Err(e)) => {
                log::warn!("story package: {} not compared: {e}", theirs.display());
                false
            }
        };
        if differs && item.own_version {
            plan.versions.push(item);
        } else if differs {
            plan.other_version.push((kind, id.to_string()));
        }
    }
    plan
}

/// After an install: each car and track fetched, checked already and in
/// `staged` under its file name, is set aside for the import; what the plan
/// found is recorded. Returns the archives to import.
pub(super) fn place(root: &Path, story: &str, plan: &Plan, staged: &Path) -> Vec<PathBuf> {
    let dest = imports(root, story);
    let mut to_import = Vec::new();
    for item in &plan.fetch {
        let (from, to) = (staged.join(&item.file), dest.join(&item.file));
        match std::fs::create_dir_all(&dest).and_then(|_| std::fs::rename(&from, &to)) {
            Ok(()) => to_import.push(to),
            Err(e) => log::warn!("story package: {} not set aside for the import: {e}", item.file),
        }
    }
    let record = Record {
        other_version: plan.other_version.clone(),
    };
    let path = record_path(root, story);
    let written = serde_json::to_string_pretty(&record)
        .map_err(|e| e.to_string())
        .and_then(|json| std::fs::write(&path, json).map_err(|e| e.to_string()));
    if let Err(e) = written {
        log::warn!("story package: {} not written: {e}", path.display());
    }
    to_import
}

/// A story's layer stored in the player's library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryLayer {
    /// The layer's id in the library.
    pub id: String,
    pub kind: AcKind,
    /// The car or track it lies over.
    pub host: String,
}

fn layers_path(root: &Path, story: &str) -> PathBuf {
    root.join(format!("{story}.layers.json"))
}

/// The layers the installed story `story` stored in the library.
pub fn story_layers(root: &Path, story: &str) -> Vec<StoryLayer> {
    (std::fs::read_to_string(layers_path(root, story)).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Removes from the library the layers an earlier install of `story`
/// stored. One already gone is forgotten; one that cannot be removed is
/// logged and forgotten too - it stays off, a layer like any other.
pub(super) fn remove_layers(root: &Path, story: &str, library: &dyn LibraryWrites) {
    for layer in story_layers(root, story) {
        match library.remove_layer(&layer.id) {
            Err(e) if e != crate::errors::LAYER_NOT_FOUND => {
                log::warn!("story package: layer {} of {story} not removed: {e}", layer.id)
            }
            _ => {}
        }
    }
    let path = layers_path(root, story);
    if path.is_file() {
        if let Err(e) = std::fs::remove_file(&path) {
            log::warn!("story package: {} not removed: {e}", path.display());
        }
    }
}

/// Stores the story's layers, checked already and in `staged` under their
/// file name, in the library, off, in place of an earlier install's.
/// `title` names them on the mod's sheet, beside their own name.
pub(super) fn place_layers(
    root: &Path,
    story: &str,
    title: &str,
    plan: &Plan,
    staged: &Path,
    library: &dyn LibraryWrites,
) {
    remove_layers(root, story, library);
    let mut stored = Vec::new();
    for item in &plan.layers {
        let (Some(kind), Some(host), Some(name)) = (item.host, item.id.as_deref(), item.name.as_deref()) else {
            continue;
        };
        let into = staged.join(format!("{}.files", item.file));
        let added = super::extract(&staged.join(&item.file), &into, |_| true)
            .and_then(|_| library.add_layer(kind, host, &format!("{name} ({title})"), &into.join(host), story));
        match added {
            Ok(id) => stored.push(StoryLayer {
                id,
                kind,
                host: host.to_string(),
            }),
            Err(e) => log::warn!("story package: layer {name} of {story} not stored over {host}: {e}"),
        }
    }
    let path = layers_path(root, story);
    let written = serde_json::to_string_pretty(&stored)
        .map_err(|e| e.to_string())
        .and_then(|json| std::fs::write(&path, json).map_err(|e| e.to_string()));
    if let Err(e) = written {
        log::warn!("story package: {} not written: {e}", path.display());
    }
}

/// A car or track the story keeps in its own version, to stand in for the
/// player's during its sessions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnVersion {
    pub kind: AcKind,
    pub id: String,
    /// The version's folder, named after the car or track.
    pub folder: PathBuf,
}

fn versions_dir(root: &Path, story: &str) -> PathBuf {
    root.join(".versions").join(story)
}

fn versions_path(root: &Path, story: &str) -> PathBuf {
    root.join(format!("{story}.versions.json"))
}

/// The versions the installed story `story` keeps.
pub fn own_versions(root: &Path, story: &str) -> Vec<OwnVersion> {
    (std::fs::read_to_string(versions_path(root, story)).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Forgets the versions an earlier install of `story` kept.
pub(super) fn remove_versions(root: &Path, story: &str) {
    let dir = versions_dir(root, story);
    if dir.is_dir() {
        if let Err(e) = std::fs::remove_dir_all(&dir) {
            log::warn!("story package: {} not removed: {e}", dir.display());
        }
    }
    let path = versions_path(root, story);
    if path.is_file() {
        if let Err(e) = std::fs::remove_file(&path) {
            log::warn!("story package: {} not removed: {e}", path.display());
        }
    }
}

/// Keeps the story's own versions, checked already and in `staged` under
/// their file name, unpacked in Pit Box's data, in place of an earlier
/// install's.
pub(super) fn place_versions(root: &Path, story: &str, plan: &Plan, staged: &Path) {
    remove_versions(root, story);
    let mut kept = Vec::new();
    for item in &plan.versions {
        let (Some(kind), Some(id)) = (item.kind.content(), item.id.as_deref()) else {
            continue;
        };
        let into = versions_dir(root, story).join(kind.folder());
        match super::extract(&staged.join(&item.file), &into, |_| true) {
            Ok(()) => kept.push(OwnVersion {
                kind,
                id: id.to_string(),
                folder: into.join(id),
            }),
            Err(e) => log::warn!("story package: the story's version of {id} not kept: {e}"),
        }
    }
    let path = versions_path(root, story);
    let written = serde_json::to_string_pretty(&kept)
        .map_err(|e| e.to_string())
        .and_then(|json| std::fs::write(&path, json).map_err(|e| e.to_string()));
    if let Err(e) = written {
        log::warn!("story package: {} not written: {e}", path.display());
    }
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
