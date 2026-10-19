//! Copies of a story the author can go back to (`docs/story/SPEC-story-mode.md`,
//! « Where a story lives, and how the editor writes it »): what survives a
//! restart, where the undo history (`history.rs`) does not.
//!
//! A copy is taken the first time a story is opened on a given day, and
//! before every restore. It holds the files the editor reads and writes -
//! `story.yaml`, `flow/`, `content/`, `events/`, `world/` - never the media,
//! which the editor does not touch. Copies live in Pit Box's own data
//! (`app_config_dir/story/backups/<story>/<date_time>/`), not in the story's
//! folder: a story is often a git repository, and copies inside it would
//! end up committed. The ten most recent are kept.

use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use serde::Serialize;
use story_engine::load::read_dir;

use super::history::{self, FileChange};
use super::studio::stamp;

const DIR: &str = "story/backups";
/// Ten copies: at one a day, the last ten days a story was worked on.
const KEEP: usize = 10;
/// The name of a copy: when it was taken, sortable as text.
const ID_FORMAT: &str = "%Y-%m-%d_%H-%M-%S";

/// A copy, as the editor lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Backup {
    /// When it was taken, `YYYY-MM-DD_HH-MM-SS`, local time.
    pub id: String,
    pub files: usize,
}

/// Where the copies of the story in `folder` live: its folder name, readable,
/// and a fingerprint of its full path, so two stories with the same folder
/// name never share their copies.
fn root(config_dir: &Path, folder: &Path) -> PathBuf {
    let name: String = (folder.file_name().map(|n| n.to_string_lossy().to_lowercase()))
        .unwrap_or_default()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let path = folder.to_string_lossy().to_lowercase();
    config_dir.join(DIR).join(format!("{name}-{}", &stamp(&path)[..8]))
}

/// The ids of the story's copies, newest first: their folder names alone,
/// which is all the daily check needs on every opening.
fn ids(root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out: Vec<String> = (entries.filter_map(|e| e.ok()))
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|id| is_id(id))
        .collect();
    out.sort_by(|a, b| b.cmp(a));
    out
}

/// The copies of the story, newest first.
pub fn list(config_dir: &Path, folder: &Path) -> Vec<Backup> {
    let root = root(config_dir, folder);
    (ids(&root).into_iter())
        .map(|id| Backup {
            files: read_tree(&root.join(&id)).len(),
            id,
        })
        .collect()
}

/// A copy's id as this module writes it: nothing that could climb out of the
/// copies' folder, whatever the frontend sends.
fn is_id(id: &str) -> bool {
    NaiveDateTime::parse_from_str(id, ID_FORMAT).is_ok()
}

/// Takes today's copy if the story has none from today yet. Returns its id,
/// or `None` when there already was one.
pub fn take_daily(config_dir: &Path, folder: &Path, now: NaiveDateTime) -> Result<Option<String>, String> {
    let today = now.format("%Y-%m-%d").to_string();
    if ids(&root(config_dir, folder)).iter().any(|id| id.starts_with(&today)) {
        return Ok(None);
    }
    take(config_dir, folder, now).map(Some)
}

/// Copies the story's files now, then drops the copies beyond the ten most
/// recent.
fn take(config_dir: &Path, folder: &Path, now: NaiveDateTime) -> Result<String, String> {
    let files = read_dir(folder).map_err(|e| e.to_string())?;
    let id = now.format(ID_FORMAT).to_string();
    let root = root(config_dir, folder);
    let dest = root.join(&id);
    for file in &files {
        let path = dest.join(&file.path);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, &file.text).map_err(|e| format!("writing {}: {e}", path.display()))?;
    }
    for old in ids(&root).into_iter().skip(KEEP) {
        // Only ever a copy this module took, inside its own folder.
        if let Err(e) = std::fs::remove_dir_all(root.join(&old)) {
            log::warn!("story backups: could not remove old copy {old}: {e}");
        }
    }
    Ok(id)
}

/// Every file under `dir`, as `(path relative to it, text)`.
fn read_tree(dir: &Path) -> Vec<(String, String)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for path in entries.filter_map(|e| e.ok()).map(|e| e.path()) {
            if path.is_dir() {
                walk(root, &path, out);
            } else if let Ok(text) = std::fs::read_to_string(&path) {
                let rel = path.strip_prefix(root).unwrap_or(&path);
                let rel = (rel.components())
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                out.push((rel, text));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out
}

/// Puts the story back as it was in copy `id`: its files as they were, and
/// the story files added since removed. The story as it is now is copied
/// first, and the restore is one edit of the undo history. Returns the files
/// written or removed.
pub fn restore(config_dir: &Path, folder: &Path, id: &str, now: NaiveDateTime) -> Result<Vec<String>, String> {
    if !is_id(id) {
        return Err(format!("not a copy: {id}"));
    }
    // Read before the copy of the present is taken: that copy may push the
    // one being restored out of the ten kept.
    let copy = read_tree(&root(config_dir, folder).join(id));
    if copy.is_empty() {
        return Err(format!("copy {id} not found"));
    }
    take(config_dir, folder, now)?;
    let current = read_dir(folder).map_err(|e| e.to_string())?;
    let mut edit: Vec<FileChange> = copy
        .iter()
        .map(|(file, text)| FileChange {
            file: file.clone(),
            before: current.iter().find(|f| &f.path == file).map(|f| f.text.clone()),
            after: Some(text.clone()),
        })
        .collect();
    edit.extend(
        (current.iter())
            .filter(|f| !copy.iter().any(|(file, _)| file == &f.path))
            .map(|f| FileChange {
                file: f.path.clone(),
                before: Some(f.text.clone()),
                after: None,
            }),
    );
    edit.retain(|c| c.before != c.after);
    for c in &edit {
        let path = folder.join(&c.file);
        match &c.after {
            Some(text) => {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
                }
                std::fs::write(&path, text).map_err(|e| format!("writing {}: {e}", path.display()))?;
            }
            None => std::fs::remove_file(&path).map_err(|e| format!("removing {}: {e}", path.display()))?,
        }
    }
    let files = edit.iter().map(|c| c.file.clone()).collect();
    history::with(folder, |h| h.record(edit));
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::story::studio::create;
    use crate::testutil::temp_dir;

    fn at(text: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(text, ID_FORMAT).unwrap()
    }

    fn new_story(base: &Path, config: &Path) -> PathBuf {
        let folder = base.join("s");
        create(config, &folder, "S", "en", "C").unwrap();
        folder
    }

    // One copy the first time a story is opened on a day, none after: an
    // author who reopens it ten times a day still has ten days behind them.
    #[test]
    fn a_copy_is_taken_once_a_day() {
        let (config, base) = (temp_dir("backups-config"), temp_dir("backups-story"));
        let folder = new_story(&base, &config);
        assert_eq!(
            take_daily(&config, &folder, at("2026-10-19_08-00-00"))
                .unwrap()
                .as_deref(),
            Some("2026-10-19_08-00-00")
        );
        assert_eq!(
            take_daily(&config, &folder, at("2026-10-19_17-30-00")).unwrap(),
            None,
            "same day"
        );
        take_daily(&config, &folder, at("2026-10-20_09-00-00")).unwrap();
        let copies = list(&config, &folder);
        assert_eq!(copies.len(), 2);
        assert_eq!(copies[0].id, "2026-10-20_09-00-00", "newest first");
        assert_eq!(copies[0].files, 2, "story.yaml and the first chapter");
        assert!(
            root(&config, &folder).starts_with(&*config),
            "copies live in Pit Box's data, not in the story"
        );
    }

    #[test]
    fn only_the_ten_most_recent_copies_are_kept() {
        let (config, base) = (temp_dir("backups-config"), temp_dir("backups-story"));
        let folder = new_story(&base, &config);
        for day in 1..=13 {
            take_daily(&config, &folder, at(&format!("2026-10-{day:02}_08-00-00"))).unwrap();
        }
        let copies = list(&config, &folder);
        assert_eq!(copies.len(), KEEP);
        assert_eq!(
            copies.last().unwrap().id,
            "2026-10-04_08-00-00",
            "the oldest three dropped"
        );
    }

    // Restoring puts the story back as it was, removes the story files added
    // since, copies the present first, and is undone by Ctrl+Z.
    #[test]
    fn a_restore_brings_the_story_back_and_can_be_undone() {
        let (config, base) = (temp_dir("backups-config"), temp_dir("backups-story"));
        let folder = new_story(&base, &config);
        let story_then = std::fs::read_to_string(folder.join("story.yaml")).unwrap();
        take_daily(&config, &folder, at("2026-10-19_08-00-00")).unwrap();
        std::fs::write(
            folder.join("story.yaml"),
            story_then.replace("title: S", "title: Later"),
        )
        .unwrap();
        std::fs::write(folder.join("flow/02-chapter.yaml"), "id: ch02\nnodes: []\n").unwrap();
        std::fs::write(folder.join("notes.txt"), "the author's own file").unwrap();

        let changed = restore(&config, &folder, "2026-10-19_08-00-00", at("2026-10-19_18-00-00")).unwrap();
        assert_eq!(
            changed.len(),
            2,
            "story.yaml back, the new chapter removed: {changed:?}"
        );
        assert_eq!(std::fs::read_to_string(folder.join("story.yaml")).unwrap(), story_then);
        assert!(
            !folder.join("flow/02-chapter.yaml").exists(),
            "a chapter added since is removed"
        );
        assert!(
            folder.join("notes.txt").exists(),
            "a file that is not the story's is left alone"
        );
        assert_eq!(
            list(&config, &folder)[0].id,
            "2026-10-19_18-00-00",
            "the present was copied first"
        );

        history::with(&folder, |h| h.undo(&folder)).unwrap();
        assert!(
            std::fs::read_to_string(folder.join("story.yaml"))
                .unwrap()
                .contains("title: Later"),
            "Ctrl+Z undoes the restore"
        );
        assert!(folder.join("flow/02-chapter.yaml").exists(), "the chapter is back too");
    }

    #[test]
    fn a_restore_refuses_what_is_not_a_copy() {
        let (config, base) = (temp_dir("backups-config"), temp_dir("backups-story"));
        let folder = new_story(&base, &config);
        let now = at("2026-10-19_18-00-00");
        assert!(
            restore(&config, &folder, "../../s", now).is_err(),
            "no path climbing out"
        );
        assert!(
            restore(&config, &folder, "2026-01-01_00-00-00", now).is_err(),
            "no such copy"
        );
        assert!(list(&config, &folder).is_empty(), "nothing taken for a refused restore");
    }
}
