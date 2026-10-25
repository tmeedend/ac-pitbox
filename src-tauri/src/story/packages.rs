//! Story packages the player installed (SPEC-story-mode, Content: story
//! package and dependencies; the format is `story_engine::package`).
//!
//! An installed story is a folder of Pit Box's own data,
//! `app_config_dir/story/installed/<story id>/`, holding the story's files
//! as the package carried them, and beside it `<story id>.package.json`, the
//! manifest it came with: what an update compares its items to (L4.4).
//! The cars and tracks it carries go to the player's library through the
//! ordinary import, never straight into the game (`package_content.rs`).
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
use super::package_content;
use super::play::{self, Source};
use crate::errors;

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
        /// ordinary import (`package_content.rs`).
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
    if let Some(bad) = content
        .iter()
        .find(|i| !i.id.as_deref().is_some_and(package_content::safe_id))
    {
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
        package_content::check_item(&to, item.id.as_deref().unwrap_or_default())?;
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
    package_content::clear(&root, &id);
    let imports = package_content::place(&root, &id, &content, &staging.work, lookup);
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
    package_content::clear(&root(config_dir), id);
    Ok(())
}

/// The cars and tracks the installed story `id` found here in another
/// version than its package's.
pub fn other_versions(config_dir: &Path, id: &str) -> Vec<(story_engine::needs::AcKind, String)> {
    package_content::other_versions(&root(config_dir), id)
}

/// An archive an install set aside for the import, once the import is done
/// with it.
pub fn imported(config_dir: &Path, archive: &Path) -> Result<(), String> {
    package_content::imported(&root(config_dir), archive)
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
mod tests {
    use super::*;
    use crate::story::holdings::all_held;
    use crate::testutil::temp_dir;
    use std::io::Cursor;

    /// The sample's files, `edit` applied to the one at `path`.
    fn sample_with(path: &str, edit: impl Fn(&str) -> String) -> Vec<(String, Vec<u8>)> {
        let mut files = story_engine::sample::folder_files();
        let file = files.iter_mut().find(|(p, _)| p == path).expect("a file of the sample");
        file.1 = edit(std::str::from_utf8(&file.1).unwrap()).into_bytes();
        files
    }

    fn version(v: &str) -> impl Fn(&str) -> String + '_ {
        move |t| t.replace("version: \"1\"", &format!("version: \"{v}\""))
    }

    /// A `.pitstory` of `files`, written under `dir`.
    fn packed(dir: &Path, name: &str, files: &[(String, Vec<u8>)]) -> PathBuf {
        let sources = (files.iter())
            .filter(|(p, _)| p.ends_with(".yaml") || p.ends_with(".story"))
            .map(|(p, b)| story_engine::load::SourceFile::new(p, std::str::from_utf8(b).unwrap()))
            .collect();
        let manifest = Story::from_sources(sources).expect("reads").manifest;
        let path = dir.join(name);
        std::fs::write(&path, package::pack(&manifest, files).unwrap()).unwrap();
        path
    }

    /// A `.pitstory` written by hand: a manifest and items as given, for
    /// what `package::pack` never makes.
    fn by_hand(dir: &Path, manifest: &str, items: &[(&str, &[u8])]) -> PathBuf {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file(package::MANIFEST, options).unwrap();
        zip.write_all(manifest.as_bytes()).unwrap();
        for (name, bytes) in items {
            zip.start_file(format!("{}{name}", package::ITEMS), options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        let path = dir.join("by_hand.pitstory");
        std::fs::write(&path, zip.finish().unwrap().into_inner()).unwrap();
        path
    }

    fn one_item_manifest(item: &[u8]) -> String {
        format!(
            r#"{{"format":1,"id":"sample","title":"Sample story","version":"1",
                "items":[{{"kind":"story","file":"s.zip","size":{},"sha256":"{}"}}]}}"#,
            item.len(),
            package::sha256_hex(item)
        )
    }

    fn installed(id: &str, version: &str, previous: Option<&str>) -> Outcome {
        Outcome::Installed {
            id: id.into(),
            title: "Sample story".into(),
            version: version.into(),
            previous: previous.map(String::from),
            imports: Vec::new(),
        }
    }

    // One action installs the story, and Story Mode offers it beside the
    // sample, from Pit Box's own data (SPEC-story-mode, Story Mode: player
    // side, stories offered).
    #[test]
    fn a_package_installs_and_plays() {
        let base = temp_dir("story-pkg-install");
        let config = base.join("config");
        let file = packed(&base, "sample.pitstory", &story_engine::sample::folder_files());

        assert_eq!(
            install(&config, &file, false, &all_held).unwrap(),
            installed("sample", "1", None)
        );
        let dir = config.join("story/installed/sample");
        assert!(
            dir.join("media/scenes/bedroom.webp").is_file(),
            "media unpacked with the story"
        );
        assert!(
            config.join("story/installed/sample.package.json").is_file(),
            "the manifest kept"
        );
        let leftovers: Vec<_> = (std::fs::read_dir(config.join("story/installed")).unwrap().flatten())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with('.'))
            .collect();
        assert!(leftovers.is_empty(), "nothing left of the staging: {leftovers:?}");

        let source = Source::Installed { id: "sample".into() };
        let listed = play::list(&config, &all_held);
        let p = listed.iter().find(|p| p.source == source).expect("offered");
        assert_eq!((p.errors, p.version.as_deref()), (0, Some("1")));
        let game = play::new_game(&config, &source, 7).expect("starts");
        assert_eq!(
            game.folder.as_deref(),
            Some(dir.as_path()),
            "media read from the installed folder"
        );
        assert!(
            config.join("story/saves/installed/sample.json").is_file(),
            "its own save"
        );
    }

    // An update keeps the game that resumes on it; one that would lose it
    // asks first and changes nothing until told to (SPEC-story-mode,
    // Content: installation and updates).
    #[test]
    fn an_update_keeps_the_game_or_asks_before_losing_it() {
        let base = temp_dir("story-pkg-update");
        let config = base.join("config");
        install(
            &config,
            &packed(&base, "v1.pitstory", &story_engine::sample::folder_files()),
            false,
            &all_held,
        )
        .unwrap();
        let source = Source::Installed { id: "sample".into() };
        play::new_game(&config, &source, 7).unwrap();

        let v2 = packed(&base, "v2.pitstory", &sample_with("story.yaml", version("2")));
        assert_eq!(
            install(&config, &v2, false, &all_held).unwrap(),
            installed("sample", "2", Some("1"))
        );
        play::resume(&config, &source).expect("the game resumes on the update");

        // The game waits on `invite`, which version 3 renames.
        let mut v3 = sample_with("story.yaml", version("3"));
        let flow = "flow/01-first-race.yaml";
        let renamed = sample_with(flow, |t| t.replace("  - id: invite\n", "  - id: invite_again\n"));
        let at = v3.iter().position(|(p, _)| p == flow).unwrap();
        v3[at] = renamed.into_iter().find(|(p, _)| p == flow).unwrap();
        let v3 = packed(&base, "v3.pitstory", &v3);
        match install(&config, &v3, false, &all_held).unwrap() {
            Outcome::LosesGame { previous, version, .. } => {
                assert_eq!((previous.as_str(), version.as_str()), ("2", "3"));
            }
            other => panic!("asked first, got {other:?}"),
        }
        let listed = play::list(&config, &all_held);
        let p = listed.iter().find(|p| p.source == source).unwrap();
        assert_eq!(p.version.as_deref(), Some("2"), "nothing changed before the answer");
        play::resume(&config, &source).expect("the game still resumes");

        assert_eq!(
            install(&config, &v3, true, &all_held).unwrap(),
            installed("sample", "3", Some("2"))
        );
        let listed = play::list(&config, &all_held);
        let p = listed.iter().find(|p| p.source == source).unwrap();
        assert_eq!(p.version.as_deref(), Some("3"));
        let game = p.game.as_ref().expect("the game is kept");
        assert_eq!(
            game.problem.as_deref(),
            Some(errors::STORY_SAVE_STALE),
            "and says why it cannot resume"
        );
    }

    // Files are checked after download (SPEC-story-mode, Content): a
    // package whose item is not what the manifest says installs nothing.
    #[test]
    fn a_damaged_package_installs_nothing() {
        let base = temp_dir("story-pkg-damaged");
        let config = base.join("config");
        let item = package::story_item(&story_engine::sample::folder_files()).unwrap();
        let mut bad = item.clone();
        let last = bad.len() - 1;
        bad[last] ^= 0xff;
        let file = by_hand(&base, &one_item_manifest(&item), &[("s.zip", &bad)]);
        assert_eq!(
            install(&config, &file, false, &all_held),
            Err(errors::STORY_PACKAGE_DAMAGED.into())
        );
        let file = by_hand(&base, &one_item_manifest(&item), &[("other.zip", &item)]);
        assert_eq!(
            install(&config, &file, false, &all_held),
            Err(errors::STORY_PACKAGE_DAMAGED.into()),
            "item missing"
        );
        assert!(
            play::list(&config, &all_held)
                .iter()
                .all(|p| p.source == Source::Sample),
            "nothing offered"
        );

        let ok = by_hand(&base, &one_item_manifest(&item), &[("s.zip", &item)]);
        assert_eq!(
            install(&config, &ok, false, &all_held).unwrap(),
            installed("sample", "1", None),
            "the same, whole"
        );
    }

    // A package can only bring a story's own files: a path leaving the
    // folder, or a file a story is not made of, refuses all of it.
    #[test]
    fn a_package_writes_nothing_outside_the_story() {
        let base = temp_dir("story-pkg-escape");
        let config = base.join("config");
        for evil in ["../../evil.txt", "notes.txt", "media/../../evil.txt"] {
            let mut files = story_engine::sample::folder_files();
            let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
            let options = zip::write::SimpleFileOptions::default();
            files.push((evil.into(), b"x".to_vec()));
            for (p, b) in &files {
                zip.start_file(p.as_str(), options).unwrap();
                zip.write_all(b).unwrap();
            }
            let item = zip.finish().unwrap().into_inner();
            let file = by_hand(&base, &one_item_manifest(&item), &[("s.zip", &item)]);
            assert_eq!(
                install(&config, &file, false, &all_held),
                Err(errors::STORY_PACKAGE_DAMAGED.into()),
                "{evil}"
            );
            assert!(
                !config.join("story/installed/sample").exists(),
                "{evil}: nothing installed"
            );
            assert!(
                !config.join("evil.txt").exists() && !base.join("evil.txt").exists(),
                "{evil}"
            );
        }
    }

    #[test]
    fn a_package_from_a_newer_pit_box_says_so() {
        let base = temp_dir("story-pkg-newer");
        let config = base.join("config");
        let newer = r#"{"format":2,"id":"sample","something":"new"}"#;
        assert_eq!(
            install(&config, &by_hand(&base, newer, &[]), false, &all_held),
            Err(errors::STORY_PACKAGE_TOO_NEW.into())
        );
        let item = package::story_item(&story_engine::sample::folder_files()).unwrap();
        let unknown_kind = one_item_manifest(&item).replace(r#""kind":"story""#, r#""kind":"livery""#);
        assert_eq!(
            install(
                &config,
                &by_hand(&base, &unknown_kind, &[("s.zip", &item)]),
                false,
                &all_held
            ),
            Err(errors::STORY_PACKAGE_TOO_NEW.into()),
            "an item kind unknown here"
        );
        let not_a_package = base.join("a.pitstory");
        std::fs::write(&not_a_package, b"hello").unwrap();
        assert_eq!(
            install(&config, &not_a_package, false, &all_held),
            Err(errors::STORY_PACKAGE_UNREADABLE.into())
        );
    }

    // A story the checks find errors in would stop half-way: it is not
    // installed (SPEC-story-mode, Create Your Own: errors block the build).
    #[test]
    fn a_story_with_errors_is_not_installed() {
        let base = temp_dir("story-pkg-errors");
        let config = base.join("config");
        let files = sample_with("story.yaml", |t| t.replace("node: intro", "node: nowhere"));
        let item = package::story_item(&files).unwrap();
        let file = by_hand(&base, &one_item_manifest(&item), &[("s.zip", &item)]);
        assert_eq!(
            install(&config, &file, false, &all_held),
            Err(errors::STORY_PACKAGE_HAS_ERRORS.into())
        );
        assert!(!config.join("story/installed/sample").exists());
    }

    // Uninstalling leaves the game: installed again, the story resumes it.
    #[test]
    fn uninstalling_keeps_the_game_for_a_new_install() {
        let base = temp_dir("story-pkg-uninstall");
        let config = base.join("config");
        let file = packed(&base, "sample.pitstory", &story_engine::sample::folder_files());
        install(&config, &file, false, &all_held).unwrap();
        let source = Source::Installed { id: "sample".into() };
        play::new_game(&config, &source, 7).unwrap();

        uninstall(&config, "sample").unwrap();
        assert!(!config.join("story/installed/sample").exists(), "the story is gone");
        assert!(
            !config.join("story/installed/sample.package.json").exists(),
            "its manifest too"
        );
        assert!(
            play::list(&config, &all_held).iter().all(|p| p.source != source),
            "no longer offered"
        );
        assert!(uninstall(&config, "../config").is_err(), "an id is never a path");

        install(&config, &file, false, &all_held).unwrap();
        play::resume(&config, &source).expect("the game comes back with the story");
    }

    // A crash between moving the old version aside and putting the new one
    // in place leaves the story out of its folder: it comes back.
    #[test]
    fn a_swap_stopped_half_way_is_put_right() {
        let base = temp_dir("story-pkg-recover");
        let config = base.join("config");
        install(
            &config,
            &packed(&base, "s.pitstory", &story_engine::sample::folder_files()),
            false,
            &all_held,
        )
        .unwrap();
        let root = config.join("story/installed");
        std::fs::rename(root.join("sample"), root.join(".old-sample")).unwrap();
        std::fs::create_dir_all(root.join(".staging-sample")).unwrap();

        assert_eq!(ids(&config), vec!["sample".to_string()], "the old version is back");
        assert!(!root.join(".old-sample").exists() && !root.join(".staging-sample").exists());
    }
}
