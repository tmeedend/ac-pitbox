//! Undo and redo in the story editor (`docs/story/SPEC-story-mode.md`,
//! « Where a story lives, and how the editor writes it »).
//!
//! Every edit the editor makes already writes whole files, so undoing one is
//! writing back what those files held before: an edit is recorded as the
//! text of each file it touched, before and after. A rename that changed
//! three files is one edit, undone at once.
//!
//! The same safeguard as a save: an edit is undone (or redone) only if every
//! file it touched still holds what the edit left there. A file changed
//! outside Pit Box meanwhile is never overwritten; the history of that story
//! is dropped instead, since every older edit stands on the same files.
//!
//! The history lives in memory, per story folder, for as long as the app
//! runs - closing Pit Box forgets it. The daily copies (`backups.rs`) are
//! what survives a restart.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;

use crate::errors;

/// What one edit did to one file. `None` is a file that did not exist: the
/// edit created it (`before`) or deleted it (`after`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub file: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// How far back an author can go. Each edit keeps whole files, so the cap
/// bounds memory; a hundred edits is a long working session.
const DEPTH: usize = 100;

#[derive(Debug, Default)]
pub struct History {
    undo: Vec<Vec<FileChange>>,
    redo: Vec<Vec<FileChange>>,
}

/// Whether there is something to undo or redo, for the editor's buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryStatus {
    pub can_undo: bool,
    pub can_redo: bool,
}

impl History {
    /// Records an edit just written. A new edit drops what could be redone,
    /// as everywhere else.
    pub fn record(&mut self, edit: Vec<FileChange>) {
        let edit: Vec<FileChange> = edit.into_iter().filter(|c| c.before != c.after).collect();
        if edit.is_empty() {
            return;
        }
        self.undo.push(edit);
        if self.undo.len() > DEPTH {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn status(&self) -> HistoryStatus {
        HistoryStatus {
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
        }
    }

    /// Puts back the files of the last edit. Returns the files it wrote,
    /// empty when there was nothing to undo.
    pub fn undo(&mut self, folder: &Path) -> Result<Vec<String>, String> {
        step(folder, &mut self.undo, &mut self.redo, Direction::Back)
    }

    /// Writes again the files of the last edit undone.
    pub fn redo(&mut self, folder: &Path) -> Result<Vec<String>, String> {
        step(folder, &mut self.redo, &mut self.undo, Direction::Forward)
    }
}

#[derive(Clone, Copy)]
enum Direction {
    Back,
    Forward,
}

fn step(
    folder: &Path,
    from: &mut Vec<Vec<FileChange>>,
    to: &mut Vec<Vec<FileChange>>,
    direction: Direction,
) -> Result<Vec<String>, String> {
    let Some(edit) = from.last() else {
        return Ok(Vec::new());
    };
    // What each file must hold now, and what it gets.
    let ends = |c: &FileChange| match direction {
        Direction::Back => (c.after.clone(), c.before.clone()),
        Direction::Forward => (c.before.clone(), c.after.clone()),
    };
    // Checked for every file before any is written: an edit goes back whole
    // or not at all.
    for c in edit {
        let on_disk = std::fs::read_to_string(folder.join(&c.file)).ok();
        if on_disk != ends(c).0 {
            from.clear();
            to.clear();
            return Err(errors::STORY_HISTORY_OUTDATED.into());
        }
    }
    for c in edit {
        let path = folder.join(&c.file);
        match ends(c).1 {
            Some(text) => {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
                }
                std::fs::write(&path, text).map_err(|e| format!("writing {}: {e}", path.display()))?;
            }
            None => std::fs::remove_file(&path).map_err(|e| format!("removing {}: {e}", path.display()))?,
        }
    }
    let edit = from.pop().expect("checked above");
    let files = edit.iter().map(|c| c.file.clone()).collect();
    to.push(edit);
    Ok(files)
}

/// The history of every story opened since the app started, by folder.
static HISTORIES: Mutex<BTreeMap<PathBuf, History>> = Mutex::new(BTreeMap::new());

/// Runs `f` on the history of the story in `folder`.
pub fn with<T>(folder: &Path, f: impl FnOnce(&mut History) -> T) -> T {
    // A panic while the lock was held leaves a history that is still a
    // valid one: keep using it rather than poison every later edit.
    let mut all = HISTORIES.lock().unwrap_or_else(|e| e.into_inner());
    f(all.entry(folder.to_path_buf()).or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;

    fn change(file: &str, before: Option<&str>, after: Option<&str>) -> FileChange {
        FileChange {
            file: file.into(),
            before: before.map(String::from),
            after: after.map(String::from),
        }
    }

    fn read(folder: &Path, file: &str) -> Option<String> {
        std::fs::read_to_string(folder.join(file)).ok()
    }

    // Ctrl+Z puts back every file of the last edit at once - a rename that
    // touched two files included - and Ctrl+Y writes them again.
    #[test]
    fn an_edit_is_undone_and_redone_whole() {
        let folder = temp_dir("story-history");
        std::fs::create_dir_all(folder.join("world")).unwrap();
        std::fs::write(folder.join("a.yaml"), "new a").unwrap();
        std::fs::write(folder.join("world/b.yaml"), "new b").unwrap();
        let mut h = History::default();
        h.record(vec![
            change("a.yaml", Some("old a"), Some("new a")),
            change("world/b.yaml", Some("old b"), Some("new b")),
        ]);
        assert_eq!(
            h.status(),
            HistoryStatus {
                can_undo: true,
                can_redo: false
            }
        );

        assert_eq!(h.undo(&folder).unwrap(), ["a.yaml", "world/b.yaml"], "both files");
        assert_eq!(read(&folder, "a.yaml").as_deref(), Some("old a"));
        assert_eq!(read(&folder, "world/b.yaml").as_deref(), Some("old b"));
        assert_eq!(
            h.status(),
            HistoryStatus {
                can_undo: false,
                can_redo: true
            }
        );

        h.redo(&folder).unwrap();
        assert_eq!(read(&folder, "a.yaml").as_deref(), Some("new a"), "written again");
        assert!(
            h.undo(&folder).is_ok() && h.undo(&folder).unwrap().is_empty(),
            "nothing more to undo"
        );
    }

    // A chapter the edit created is removed by undoing it, and comes back by
    // redoing it.
    #[test]
    fn undoing_a_new_file_removes_it() {
        let folder = temp_dir("story-history");
        std::fs::create_dir_all(folder.join("flow")).unwrap();
        std::fs::write(folder.join("flow/02.yaml"), "chapter").unwrap();
        let mut h = History::default();
        h.record(vec![change("flow/02.yaml", None, Some("chapter"))]);
        h.undo(&folder).unwrap();
        assert!(!folder.join("flow/02.yaml").exists(), "the new chapter is gone");
        h.redo(&folder).unwrap();
        assert_eq!(read(&folder, "flow/02.yaml").as_deref(), Some("chapter"), "and back");
    }

    // A file changed outside Pit Box since the edit is never overwritten:
    // nothing is written, and the history, which stands on that file, is
    // dropped.
    #[test]
    fn a_file_changed_outside_stops_the_undo() {
        let folder = temp_dir("story-history");
        std::fs::write(folder.join("a.yaml"), "new a").unwrap();
        std::fs::write(folder.join("b.yaml"), "typed by hand").unwrap();
        let mut h = History::default();
        h.record(vec![
            change("a.yaml", Some("old a"), Some("new a")),
            change("b.yaml", Some("old b"), Some("new b")),
        ]);
        assert_eq!(h.undo(&folder).unwrap_err(), errors::STORY_HISTORY_OUTDATED);
        assert_eq!(read(&folder, "a.yaml").as_deref(), Some("new a"), "not half undone");
        assert_eq!(
            read(&folder, "b.yaml").as_deref(),
            Some("typed by hand"),
            "the hand edit kept"
        );
        assert_eq!(
            h.status(),
            HistoryStatus {
                can_undo: false,
                can_redo: false
            },
            "history dropped"
        );
    }

    // A new edit drops what could be redone; an edit that changed nothing is
    // not one; the history keeps the last hundred edits.
    #[test]
    fn recording_keeps_a_bounded_linear_history() {
        let folder = temp_dir("story-history");
        std::fs::write(folder.join("a.yaml"), "1").unwrap();
        let mut h = History::default();
        h.record(vec![change("a.yaml", Some("0"), Some("1"))]);
        h.undo(&folder).unwrap();
        h.record(vec![change("a.yaml", Some("0"), Some("2"))]);
        assert!(!h.status().can_redo, "a new edit drops the redo");
        h.record(vec![change("a.yaml", Some("2"), Some("2"))]);
        assert_eq!(h.undo.len(), 1, "an edit that changed nothing is not recorded");
        for n in 0..150 {
            h.record(vec![change("a.yaml", Some(&n.to_string()), Some(&(n + 1).to_string()))]);
        }
        assert_eq!(h.undo.len(), DEPTH, "bounded");
    }
}
