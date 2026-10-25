//! Story packages the player installed (SPEC-story-mode, Content: story
//! package and dependencies; the format is `story_engine::package`).
//!
//! An installed story is a folder of Pit Box's own data,
//! `app_config_dir/story/installed/<story id>/`, holding the story's files
//! as the package carried them, and beside it `<story id>.package.json`, the
//! manifest it came with: what an update compares its items to (L4.4).
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
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use sha2::{Digest, Sha256};
use story_engine::package::{self, ItemKind, Manifest};
use story_engine::validate::{validate, Severity};
use story_engine::Story;

use super::holdings::Lookup;
use super::play::{self, Source};
use crate::errors;

mod content;

/// One install or uninstall at a time, and no recovery while one runs: a
/// staging folder in use must not be taken for one left behind.
static BUSY: Mutex<()> = Mutex::new(());

/// A manifest is a few lines; anything bigger is not one.
const MANIFEST_LIMIT: u64 = 1024 * 1024;

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

/// Installs the package `file`, or updates the story it carries. With
/// `lose_game`, an update goes ahead even when the game under way would not
/// resume on it. `lookup` says which of its cars and tracks the player has.
pub fn install(config_dir: &Path, file: &Path, lose_game: bool, lookup: Lookup) -> Result<Outcome, String> {
    let _busy = BUSY.lock().unwrap_or_else(|e| e.into_inner());
    let root = root(config_dir);
    recover(&root);
    let mut zip = File::open(file)
        .map_err(|e| e.to_string())
        .and_then(|f| zip::ZipArchive::new(f).map_err(|e| e.to_string()))
        .map_err(|e| {
            log::warn!("story package {}: not a zip: {e}", file.display());
            errors::STORY_PACKAGE_UNREADABLE.to_string()
        })?;
    let manifest = read_manifest(&mut zip, file)?;
    let (stories, content): (Vec<&package::Item>, Vec<&package::Item>) =
        manifest.items.iter().partition(|i| i.kind == ItemKind::Story);
    let [story_item] = stories.as_slice() else {
        log::warn!(
            "story package {}: {} story items, one expected",
            file.display(),
            stories.len()
        );
        return Err(errors::STORY_PACKAGE_UNREADABLE.into());
    };
    if let Some(bad) = content.iter().find(|i| !i.id.as_deref().is_some_and(content::safe_id)) {
        log::warn!("story package {}: item {} names no folder", file.display(), bad.file);
        return Err(errors::STORY_PACKAGE_UNREADABLE.into());
    }

    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let staging = Staging::new(&root, &manifest.id)?;
    let story_zip = staging.work.join(&story_item.file);
    copy_checked(&mut zip, story_item, &story_zip)?;
    unpack(&story_zip, &staging.dir)?;
    let story = checked_story(&staging.dir, &manifest)?;
    // Every car and track checked too, before anything lands.
    for item in &content {
        let to = staging.work.join(&item.file);
        copy_checked(&mut zip, item, &to)?;
        content::check_item(&to, item.id.as_deref().unwrap_or_default())?;
    }

    let id = manifest.id.clone();
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
    let imports = content::place(&root, &id, &content, &staging.work, lookup);
    let kept = root.join(format!("{id}.package.json"));
    let json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    if let Err(e) = std::fs::write(&kept, json) {
        // The story plays without it; an update then takes every item again.
        log::warn!("story package: {} not written: {e}", kept.display());
    }
    Ok(Outcome::Installed {
        id,
        title: manifest.title,
        version: manifest.version,
        previous,
        imports,
    })
}

/// Removes the installed story `id`. Its game stays where it is: installed
/// again, the story resumes it.
pub fn uninstall(config_dir: &Path, id: &str) -> Result<(), String> {
    let _busy = BUSY.lock().unwrap_or_else(|e| e.into_inner());
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
    content::clear(&root(config_dir), id);
    Ok(())
}

/// The cars and tracks the installed story `id` found here in another
/// version than its package's.
pub fn other_versions(config_dir: &Path, id: &str) -> Vec<(story_engine::needs::AcKind, String)> {
    content::other_versions(&root(config_dir), id)
}

/// An archive an install set aside for the import, once the import is done
/// with it.
pub fn imported(config_dir: &Path, archive: &Path) -> Result<(), String> {
    content::imported(&root(config_dir), archive)
}

fn read_manifest(zip: &mut zip::ZipArchive<File>, file: &Path) -> Result<Manifest, String> {
    let unreadable = |why: String| {
        log::warn!("story package {}: {why}", file.display());
        errors::STORY_PACKAGE_UNREADABLE.to_string()
    };
    let mut text = String::new();
    (zip.by_name(package::MANIFEST).map_err(|e| unreadable(e.to_string()))?)
        .take(MANIFEST_LIMIT)
        .read_to_string(&mut text)
        .map_err(|e| unreadable(e.to_string()))?;
    // The format first: a newer manifest may well not read here at all.
    let format = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| v.get("format")?.as_u64());
    if format.is_some_and(|f| f > u64::from(package::FORMAT)) {
        return Err(errors::STORY_PACKAGE_TOO_NEW.into());
    }
    let manifest = Manifest::from_json(&text).map_err(unreadable)?;
    if manifest.items.iter().any(|i| i.kind == ItemKind::Unknown) {
        return Err(errors::STORY_PACKAGE_TOO_NEW.into());
    }
    if !package::valid_id(&manifest.id) {
        return Err(unreadable(format!("id `{}` cannot name a folder", manifest.id)));
    }
    Ok(manifest)
}

/// The item's file copied to `to`, refused unless it has the size and the
/// fingerprint the manifest gives it.
fn copy_checked(zip: &mut zip::ZipArchive<File>, item: &package::Item, to: &Path) -> Result<(), String> {
    let damaged = |why: String| {
        log::warn!("story package: item {}: {why}", item.file);
        errors::STORY_PACKAGE_DAMAGED.to_string()
    };
    let mut entry = (zip.by_name(&format!("{}{}", package::ITEMS, item.file))).map_err(|e| damaged(e.to_string()))?;
    let mut out = File::create(to).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = entry.read(&mut buf).map_err(|e| damaged(e.to_string()))?;
        if n == 0 {
            break;
        }
        size += n as u64;
        if size > item.size {
            return Err(damaged(format!("longer than its {} bytes", item.size)));
        }
        hasher.update(&buf[..n]);
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
    }
    if size != item.size {
        return Err(damaged(format!("{size} bytes, {} declared", item.size)));
    }
    let sha256 = package::hex(&hasher.finalize());
    if sha256 != item.sha256 {
        return Err(damaged(format!("sha256 {sha256}, {} declared", item.sha256)));
    }
    Ok(())
}

/// The story item's files into `dir`. Every entry must be a file a story is
/// made of (`package::carried`): one that is not - a path leaving the
/// folder among them - refuses the whole package.
fn unpack(item: &Path, dir: &Path) -> Result<(), String> {
    extract(item, dir, package::carried)
}

/// The files of the zip `item` into `dir`, each name `fits` accepts; one it
/// refuses, or one that could leave `dir`, refuses the whole zip.
pub(super) fn extract(item: &Path, dir: &Path, fits: impl Fn(&str) -> bool) -> Result<(), String> {
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
