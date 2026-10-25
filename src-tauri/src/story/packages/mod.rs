//! Story packages the player installed (SPEC-story-mode, Content: story
//! package and dependencies; the format is `story_engine::package`).
//!
//! An installed story is a folder of Pit Box's own data,
//! `app_config_dir/story/installed/<story id>/`, holding the story's files
//! as the package carried them. Beside it: `<story id>.package.json`, the
//! manifest it came with, what an update compares its items to;
//! `<story id>.source.json`, the URL it was installed from, where updates
//! are looked for (`updates.rs`); and in `.cache/<story id>/` the story's
//! own item, so that an update which leaves it as it was does not download
//! it again. A package comes from a file or a URL (`fetch.rs`).
//! The cars and tracks it carries go to the player's library through the
//! ordinary import, never straight into the game (`content.rs`).
//!
//! **Nothing lands before it is checked.** Every item is copied out of the
//! package while its SHA-256 is computed, and compared to the manifest; the
//! story is unpacked into a staging folder, read and checked there, and
//! only then put in place. A package that is damaged, from a newer Pit Box,
//! carrying a file a story is not made of, or whose story has errors,
//! changes nothing.
//!
//! **An update replaces the story whole**, once the game under way is known
//! to resume on the new version (`play::loses_game`): when it would not,
//! nothing changes until the player says to go ahead anyway. The game itself
//! is never touched - kept as it is, it says why it cannot resume.
//!
//! **A swap that stops half-way is put right.** The old version is renamed
//! aside (`.old-<id>`) before the new one takes its place, then removed; one
//! left behind by a crash comes back when its place is empty, and goes
//! otherwise. Names starting with a dot are Pit Box's own and never listed.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use story_engine::package::{self, Item, ItemKind, Manifest};
use story_engine::validate::{validate, Severity};
use story_engine::Story;

use super::holdings::Lookup;
use super::library::LibraryWrites;
use super::play::{self, Source};
use crate::errors;

mod content;
mod fetch;
mod updates;

pub use content::{OwnVersion, StoryLayer};
pub use fetch::{cancel, Origin, Progress};
pub use updates::{update_url, updates, Update};

/// One install or uninstall at a time, and no recovery while one runs: a
/// staging folder in use must not be taken for one left behind.
static BUSY: Mutex<()> = Mutex::new(());

fn root(config_dir: &Path) -> PathBuf {
    config_dir.join("story/installed")
}

/// The folder of the installed story `id`.
pub fn folder(config_dir: &Path, id: &str) -> Result<PathBuf, String> {
    if !package::valid_id(id) {
        return Err(format!("not an installed story: {id}"));
    }
    Ok(root(config_dir).join(id))
}

/// The installed stories' ids, in name order.
pub fn ids(config_dir: &Path) -> Vec<String> {
    let root = root(config_dir);
    // After an install under way, never in its middle: what it set aside is
    // its own.
    {
        let _busy = BUSY.lock().unwrap_or_else(|e| e.into_inner());
        recover(&root);
    }
    let Ok(entries) = std::fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = (entries.flatten())
        .filter(|e| e.path().join("story.yaml").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|id| package::valid_id(id))
        .collect();
    ids.sort();
    ids
}

/// What an install did.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outcome {
    Installed {
        id: String,
        title: String,
        version: String,
        /// The version it replaced, if one was installed.
        previous: Option<String>,
        /// The cars and tracks the player lacked, as archives for the
        /// ordinary import (`content.rs`).
        imports: Vec<PathBuf>,
    },
    /// The game under way would not resume on this version: nothing was
    /// changed.
    LosesGame {
        title: String,
        version: String,
        previous: String,
    },
}

/// Installs the package at `origin`, or updates the story it carries. With
/// `lose_game`, an update goes ahead even when the game under way would not
/// resume on it. `lookup` says which of its cars and tracks the player has;
/// `library` stores its layers; `progress` follows what is downloaded.
pub fn install(
    config_dir: &Path,
    origin: Origin,
    lose_game: bool,
    lookup: Lookup,
    library: &dyn LibraryWrites,
    progress: Progress,
) -> Result<Outcome, String> {
    let _busy = BUSY.lock().unwrap_or_else(|e| e.into_inner());
    if super::session::under_way(config_dir) {
        return Err(errors::STORY_SESSION_UNDER_WAY.into());
    }
    let root = root(config_dir);
    recover(&root);
    let (mut fetcher, manifest) = fetch::Fetcher::open(origin, progress)?;
    let (stories, content): (Vec<&Item>, Vec<&Item>) = manifest.items.iter().partition(|i| i.kind == ItemKind::Story);
    let [story_item] = stories.as_slice() else {
        log::warn!(
            "story package {}: {} story items, one expected",
            manifest.id,
            stories.len()
        );
        return Err(errors::STORY_PACKAGE_UNREADABLE.into());
    };
    if let Some(bad) = content.iter().find(|i| !i.id.as_deref().is_some_and(content::safe_id)) {
        log::warn!("story package {}: item {} names no folder", manifest.id, bad.file);
        return Err(errors::STORY_PACKAGE_UNREADABLE.into());
    }

    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let id = manifest.id.clone();
    let staging = Staging::new(&root, &id)?;
    let plan = content::plan(&content, lookup);
    let story_zip = staging.work.join(&story_item.file);
    let cached = from_cache(&root, &id, story_item, &story_zip);
    let story_size = if cached { 0 } else { story_item.size };
    let others: u64 = (plan.fetch.iter().chain(&plan.layers).chain(&plan.versions))
        .map(|i| i.size)
        .sum();
    fetcher.set_total(story_size + others);
    if !cached {
        fetcher.fetch(story_item, &story_zip)?;
    }
    unpack(&story_zip, &staging.dir)?;
    let story = checked_story(&staging.dir, &manifest)?;
    // Every car, track, layer and version checked too, before anything lands.
    for item in plan.fetch.iter().chain(&plan.layers).chain(&plan.versions) {
        let to = staging.work.join(&item.file);
        fetcher.fetch(item, &to)?;
        content::check_item(&to, item.id.as_deref().unwrap_or_default())?;
    }

    let target = root.join(&id);
    let previous = Story::load(&target).ok().map(|s| s.manifest.version);
    if let (Some(previous), false) = (&previous, lose_game) {
        if play::loses_game(config_dir, &Source::Installed { id: id.clone() }, &story) {
            return Ok(Outcome::LosesGame {
                title: manifest.title,
                version: manifest.version,
                previous: previous.clone(),
            });
        }
    }
    swap(&staging.dir, &target)?;
    content::clear(&root, &id);
    let imports = content::place(&root, &id, &plan, &staging.work);
    content::place_layers(&root, &id, &manifest.title, &plan, &staging.work, library);
    content::place_versions(&root, &id, &plan, &staging.work);
    to_cache(&root, &id, &story_zip);
    keep(&root.join(format!("{id}.package.json")), Some(&manifest));
    let source = match origin {
        Origin::Url(url) if fetcher.downloads() => Some(SourceRecord { url: url.to_string() }),
        _ => None,
    };
    keep(&source_path(&root, &id), source.as_ref());
    Ok(Outcome::Installed {
        id,
        title: manifest.title,
        version: manifest.version,
        previous,
        imports,
    })
}

/// Where an installed story came from, when it was a URL.
#[derive(Debug, Serialize, Deserialize)]
struct SourceRecord {
    url: String,
}

fn source_path(root: &Path, id: &str) -> PathBuf {
    root.join(format!("{id}.source.json"))
}

/// The URL the installed story `id` was installed from, if any.
fn source_url(root: &Path, id: &str) -> Option<String> {
    let text = std::fs::read_to_string(source_path(root, id)).ok()?;
    serde_json::from_str::<SourceRecord>(&text).ok().map(|s| s.url)
}

/// The manifest an installed story came with.
fn installed_manifest(root: &Path, id: &str) -> Option<Manifest> {
    let text = std::fs::read_to_string(root.join(format!("{id}.package.json"))).ok()?;
    Manifest::from_json(&text).ok()
}

/// Writes `value` to `path` as JSON, or removes the file for `None`. A
/// failure is logged: the story plays without these files, an update then
/// downloads what it would have skipped.
fn keep<T: Serialize>(path: &Path, value: Option<&T>) {
    let done = match value {
        Some(v) => (serde_json::to_string_pretty(v).map_err(|e| e.to_string()))
            .and_then(|json| std::fs::write(path, json).map_err(|e| e.to_string())),
        None if path.is_file() => std::fs::remove_file(path).map_err(|e| e.to_string()),
        None => Ok(()),
    };
    if let Err(e) = done {
        log::warn!("story package: {} not written: {e}", path.display());
    }
}

fn cache_dir(root: &Path, id: &str) -> PathBuf {
    root.join(".cache").join(id)
}

/// The story's item copied from the cache to `to`, when the cache holds the
/// very item - named by its fingerprint, checked again.
fn from_cache(root: &Path, id: &str, item: &Item, to: &Path) -> bool {
    let cached = cache_dir(root, id).join(&item.file);
    cached.is_file() && std::fs::copy(&cached, to).is_ok() && fetch::check_file(to, item).is_ok()
}

/// Keeps the story's item for the next update, in place of the last one.
fn to_cache(root: &Path, id: &str, story_zip: &Path) {
    let dir = cache_dir(root, id);
    if dir.is_dir() {
        if let Err(e) = std::fs::remove_dir_all(&dir) {
            log::warn!("story package: {} not removed: {e}", dir.display());
        }
    }
    let name = story_zip.file_name().unwrap_or_default();
    let kept = std::fs::create_dir_all(&dir).and_then(|_| std::fs::rename(story_zip, dir.join(name)));
    if let Err(e) = kept {
        log::warn!("story package: the story's item of {id} not kept: {e}");
    }
}

/// Removes the installed story `id`, and its layers from the library. Its
/// game stays where it is: installed again, the story resumes it.
pub fn uninstall(config_dir: &Path, id: &str, library: &dyn LibraryWrites) -> Result<(), String> {
    let _busy = BUSY.lock().unwrap_or_else(|e| e.into_inner());
    if super::session::under_way(config_dir) {
        return Err(errors::STORY_SESSION_UNDER_WAY.into());
    }
    let dir = folder(config_dir, id)?;
    if dir.is_dir() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("removing {}: {e}", dir.display()))?;
    }
    let kept = root(config_dir).join(format!("{id}.package.json"));
    if kept.is_file() {
        if let Err(e) = std::fs::remove_file(&kept) {
            log::warn!("story package: {} not removed: {e}", kept.display());
        }
    }
    // The cars and tracks it brought stay: they are the player's mods now.
    let root = root(config_dir);
    content::clear(&root, id);
    content::remove_layers(&root, id, library);
    content::remove_versions(&root, id);
    keep::<SourceRecord>(&source_path(&root, id), None);
    let cache = cache_dir(&root, id);
    if cache.is_dir() {
        if let Err(e) = std::fs::remove_dir_all(&cache) {
            log::warn!("story package: {} not removed: {e}", cache.display());
        }
    }
    Ok(())
}

/// The cars and tracks the installed story `id` found here in another
/// version than its package's.
pub fn other_versions(config_dir: &Path, id: &str) -> Vec<(story_engine::needs::AcKind, String)> {
    content::other_versions(&root(config_dir), id)
}

/// The layers the installed story `id` stored in the library.
pub fn story_layers(config_dir: &Path, id: &str) -> Vec<StoryLayer> {
    content::story_layers(&root(config_dir), id)
}

/// The cars and tracks the installed story `id` keeps in its own version.
pub fn own_versions(config_dir: &Path, id: &str) -> Vec<OwnVersion> {
    content::own_versions(&root(config_dir), id)
}

/// An archive an install set aside for the import, once the import is done
/// with it.
pub fn imported(config_dir: &Path, archive: &Path) -> Result<(), String> {
    content::imported(&root(config_dir), archive)
}

/// The story item's files into `dir`. Every entry must be a file a story is
/// made of (`package::carried`): one that is not - a path leaving the
/// folder among them - refuses the whole package.
fn unpack(item: &Path, dir: &Path) -> Result<(), String> {
    extract(item, dir, package::carried)
}

/// The files of the zip `item` into `dir`, each name `fits` accepts; one it
/// refuses, or one that could leave `dir`, refuses the whole zip.
fn extract(item: &Path, dir: &Path, fits: impl Fn(&str) -> bool) -> Result<(), String> {
    let damaged = |why: String| {
        log::warn!("story package: {why}");
        errors::STORY_PACKAGE_DAMAGED.to_string()
    };
    let mut zip = (File::open(item).map_err(|e| e.to_string()))
        .and_then(|f| zip::ZipArchive::new(f).map_err(|e| e.to_string()))
        .map_err(&damaged)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| damaged(e.to_string()))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if !fits(&name) || entry.enclosed_name().is_none() {
            return Err(damaged(format!("`{name}` does not belong here")));
        }
        let path = dir.join(&name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = File::create(&path).map_err(|e| format!("writing {}: {e}", path.display()))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| damaged(format!("{name}: {e}")))?;
    }
    Ok(())
}

/// The unpacked story, if it reads, has no error, and is the one the
/// manifest says.
fn checked_story(dir: &Path, manifest: &Manifest) -> Result<Story, String> {
    let story = Story::load(dir).map_err(|errors| {
        let first = errors.first().map(ToString::to_string).unwrap_or_default();
        log::warn!("story package {}: does not load: {first}", manifest.id);
        errors::STORY_PACKAGE_HAS_ERRORS.to_string()
    })?;
    if let Some(e) = validate(&story).into_iter().find(|d| d.severity == Severity::Error) {
        log::warn!("story package {}: {e}", manifest.id);
        return Err(errors::STORY_PACKAGE_HAS_ERRORS.into());
    }
    let m = &story.manifest;
    if (&m.id, &m.version) != (&manifest.id, &manifest.version) {
        log::warn!(
            "story package: the manifest says {} v{}, the story {} v{}",
            manifest.id,
            manifest.version,
            m.id,
            m.version
        );
        return Err(errors::STORY_PACKAGE_DAMAGED.into());
    }
    Ok(story)
}

/// Puts `staging` in the place of `target`, the old version aside until the
/// new one is there (module doc).
fn swap(staging: &Path, target: &Path) -> Result<(), String> {
    let aside = aside(target);
    if target.exists() {
        std::fs::rename(target, &aside).map_err(|e| format!("moving {} aside: {e}", target.display()))?;
    }
    if let Err(e) = std::fs::rename(staging, target) {
        if aside.exists() {
            if let Err(back) = std::fs::rename(&aside, target) {
                log::warn!("story package: {} not put back: {back}", aside.display());
            }
        }
        return Err(format!("installing {}: {e}", target.display()));
    }
    if aside.exists() {
        if let Err(e) = std::fs::remove_dir_all(&aside) {
            // Taken away by the next recovery.
            log::warn!("story package: {} not removed: {e}", aside.display());
        }
    }
    Ok(())
}

fn aside(target: &Path) -> PathBuf {
    let id = target.file_name().unwrap_or_default().to_string_lossy();
    target.with_file_name(format!(".old-{id}"))
}

/// What an install stopped half-way left behind (module doc).
fn recover(root: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let gone = if let Some(id) = name.strip_prefix(".old-") {
            let target = root.join(id);
            if !target.exists() && package::valid_id(id) {
                match std::fs::rename(&path, &target) {
                    Ok(()) => continue,
                    Err(e) => log::warn!("story package: {} not put back: {e}", path.display()),
                }
            }
            true
        } else {
            name.starts_with(".staging-")
        };
        if gone {
            let removed = if path.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
            if let Err(e) = removed {
                log::warn!("story package: {} not removed: {e}", path.display());
            }
        }
    }
}

/// The staging folder of one install - the story unpacked, then put in
/// place - and beside it the items copied out of the package, removed when
/// the install ends, whichever way.
struct Staging {
    dir: PathBuf,
    work: PathBuf,
}

impl Staging {
    fn new(root: &Path, id: &str) -> Result<Staging, String> {
        let staging = Staging {
            dir: root.join(format!(".staging-{id}")),
            work: root.join(format!(".staging-{id}.items")),
        };
        staging.clear();
        for dir in [&staging.dir, &staging.work] {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        Ok(staging)
    }

    fn clear(&self) {
        for dir in [&self.dir, &self.work] {
            if dir.is_dir() {
                if let Err(e) = std::fs::remove_dir_all(dir) {
                    log::warn!("story package: {} not removed: {e}", dir.display());
                }
            }
        }
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests;
