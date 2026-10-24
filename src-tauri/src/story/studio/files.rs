//! The files the editor writes, removes and renames, each only when it
//! still holds what the editor read, and recorded in the undo history.

use std::path::Path;

use serde::de::DeserializeOwned;
use story_engine::credits::Credit;
use story_engine::frame::FRAMES;
use story_engine::load::{read_dir, ROSTER};
use story_engine::model::{Chapter, ContentFile, Event};
use story_engine::refs::{self, NameKind};
use story_engine::scene::SCENES;
use story_engine::skin::Skin;
use story_engine::write;

use super::super::history::{self, FileChange};
use crate::errors;

/// What a file held when the editor read it: a write is refused if the file
/// no longer holds it. Also the fingerprint of a story's folder in the name
/// of its save (`play.rs`).
pub(in crate::story) fn stamp(text: &str) -> String {
    // FNV-1a: a fingerprint, not a security measure.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in text.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

pub(super) const MANIFEST: &str = "story.yaml";
pub(super) const CHARACTERS: &str = "world/characters.yaml";
pub(super) const VARIABLES: &str = "world/variables.yaml";

/// `flow/<name>.yaml`, directly in `flow/`. With `story.yaml`, the two
/// world files, the roster, the events and the content, the only files the
/// editor writes: anything else - a path climbing out of the story, another
/// folder - is refused, whatever the frontend sends.
pub(super) fn is_chapter_path(path: &str) -> bool {
    is_yaml_in("flow/", path)
}

/// `events/<name>.yaml`, directly in `events/`, with the same care.
pub(super) fn is_event_path(path: &str) -> bool {
    is_yaml_in("events/", path)
}

/// `content/…/<name>.story`, in `content/` or a folder of it: an author
/// files content as they like (`content/friend/`). No part of the path can
/// climb out of the story or hide a file.
pub(super) fn is_content_path(path: &str) -> bool {
    path.strip_prefix("content/").is_some_and(|rest| {
        rest.ends_with(".story")
            && !rest.contains(['\\', ':'])
            && rest.split('/').all(|part| !part.is_empty() && !part.starts_with('.'))
            && rest.rsplit('/').next().is_some_and(|name| name.len() > ".story".len())
    })
}

/// A `.yaml` file directly in `dir`, whose name cannot climb out of it.
fn is_yaml_in(dir: &str, path: &str) -> bool {
    path.strip_prefix(dir).is_some_and(|name| {
        name.ends_with(".yaml")
            && name.len() > ".yaml".len()
            && !name.contains(['/', '\\', ':'])
            && !name.starts_with('.')
    })
}

/// The canonical text of `file` from the value the editor sends, in the
/// shape of that file (`story_engine::model`). Read here rather than in the
/// command's signature: a sentence the engine cannot read is then reported in
/// the engine's words alone, without Tauri's "invalid args" wrapping, and the
/// author sees what to fix.
pub fn render(file: &str, value: serde_json::Value) -> Result<String, String> {
    fn from<T: DeserializeOwned>(value: serde_json::Value) -> Result<T, String> {
        serde_json::from_value(value).map_err(|e| e.to_string())
    }
    match file {
        MANIFEST => write::manifest(&from(value)?),
        CHARACTERS => write::characters(&from(value)?),
        VARIABLES => write::variables(&from(value)?),
        ROSTER => write::roster(&from(value)?),
        SCENES => write::scenes(&from(value)?),
        FRAMES => write::frames(&from(value)?),
        _ if story_engine::skin::skin_id(file).is_some() => write::skin(file, &from::<Skin>(value)?),
        _ if story_engine::credits::is_register(file) => write::credits(file, &from::<Vec<Credit>>(value)?),
        _ if is_chapter_path(file) => write::chapter(file, &from::<Chapter>(value)?),
        _ if is_event_path(file) => write::event(file, &from::<Event>(value)?),
        _ if is_content_path(file) => write::content_file(file, &from::<ContentFile>(value)?),
        _ => Err(format!("not a file the editor writes: {file}")),
    }
}

/// Writes one file of the story. `expected` is the stamp the editor read it
/// with, `None` for a file the story does not have yet (a new chapter, a
/// first character), which must not exist. Returns the new stamp.
pub fn save_file(
    folder: &Path,
    file: &str,
    expected: Option<&str>,
    value: serde_json::Value,
) -> Result<String, String> {
    let writes = matches!(file, MANIFEST | CHARACTERS | VARIABLES | ROSTER | SCENES | FRAMES)
        || story_engine::skin::skin_id(file).is_some()
        || story_engine::credits::is_register(file)
        || is_chapter_path(file)
        || is_event_path(file)
        || is_content_path(file);
    if !writes {
        return Err(format!("not a file the editor writes: {file}"));
    }
    write_guarded(folder, file, expected, || render(file, value))
}

/// Deletes an event's file, or a `.story` file whose last item goes, only
/// if it still holds what the editor read. What cited it is left as it is,
/// for the checks to report. Recorded in the undo history, so Ctrl+Z brings
/// it back.
pub fn remove_file(folder: &Path, file: &str, expected: &str) -> Result<(), String> {
    if !is_event_path(file) && !is_content_path(file) && story_engine::skin::skin_id(file).is_none() {
        return Err(format!("not a file the editor removes: {file}"));
    }
    let path = folder.join(file);
    let on_disk = std::fs::read_to_string(&path).ok();
    if on_disk.as_deref().map(stamp).as_deref() != Some(expected) {
        return Err(errors::STORY_FILE_CHANGED.into());
    }
    std::fs::remove_file(&path).map_err(|e| format!("removing {}: {e}", path.display()))?;
    history::with(folder, |h| {
        h.record(vec![FileChange {
            file: file.into(),
            before: on_disk,
            after: None,
        }])
    });
    Ok(())
}

/// Renames a character, flag, counter, relation step, event or driver, and every
/// sentence citing it, in every file of the story (`story_engine::refs`).
/// Unlike a save it takes no stamp: it reads the files as they are now and
/// changes only the cited words, so an edit made meanwhile in a text editor
/// is kept, not overwritten. Nothing is written unless the whole story reads
/// back. An event's file named after it moves to its new name.
pub fn rename(folder: &Path, kind: NameKind, from: &str, to: &str) -> Result<(), String> {
    let files = read_dir(folder).map_err(|e| e.to_string())?;
    let changes = refs::rename(&files, kind, from, to)?;
    let mut edit = Vec::new();
    for file in changes.written {
        let path = folder.join(&file.path);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, &file.text).map_err(|e| format!("writing {}: {e}", path.display()))?;
        edit.push(FileChange {
            before: files.iter().find(|f| f.path == file.path).map(|f| f.text.clone()),
            after: Some(file.text),
            file: file.path,
        });
    }
    // Removed last: the moved file is already written under its new name.
    for file in changes.removed {
        let path = folder.join(&file);
        let before = std::fs::read_to_string(&path).ok();
        std::fs::remove_file(&path).map_err(|e| format!("removing {}: {e}", path.display()))?;
        edit.push(FileChange {
            file,
            before,
            after: None,
        });
    }
    // One edit, however many files: Ctrl+Z undoes the rename whole.
    history::with(folder, |h| h.record(edit));
    Ok(())
}

/// Writes `file` with the text `render` gives, only if the file still holds
/// what the editor read (`expected`, its stamp; `None`: the file must not
/// exist yet). The text is rendered after the check, so a refused write costs
/// nothing. Recorded in the undo history; returns the new stamp.
pub(super) fn write_guarded(
    folder: &Path,
    file: &str,
    expected: Option<&str>,
    render: impl FnOnce() -> Result<String, String>,
) -> Result<String, String> {
    let path = folder.join(file);
    let on_disk = std::fs::read_to_string(&path).ok();
    let unchanged = match (expected, &on_disk) {
        (None, None) => true,
        (Some(expected), Some(text)) => stamp(text) == expected,
        _ => false,
    };
    if !unchanged {
        return Err(errors::STORY_FILE_CHANGED.into());
    }
    let text = render()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, &text).map_err(|e| format!("writing {}: {e}", path.display()))?;
    let new_stamp = stamp(&text);
    history::with(folder, |h| {
        h.record(vec![FileChange {
            file: file.into(),
            before: on_disk,
            after: Some(text),
        }])
    });
    Ok(new_stamp)
}
