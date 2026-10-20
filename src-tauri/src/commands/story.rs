//! Story Mode (`docs/story/SPEC-story-mode.md`) — see `crate::story`.

use std::path::PathBuf;

use super::prelude::*;
use crate::story::{backups, studio};

/// The Story switch asked for on this launch's command line, if any
/// (`--enable-story-studio` / `--disable-story-studio`). Read once at startup
/// by the frontend, which records it in `ui_prefs.json`: that file's schema
/// belongs to the frontend, so the backend never writes the key itself.
#[tauri::command]
pub fn story_studio_switch() -> Option<bool> {
    crate::story::studio_switch_from_args(std::env::args())
}

// ------------------------------------------------- Create Your Own (studio)
//
// Façades over `crate::story::studio`. The story files live in a folder of
// the author's choosing; only the list of those folders is Pit Box's
// (`app_config_dir/story/studio.json`). Reads of a story folder run off the
// window thread; a write stays synchronous, so two edits in a row reach the
// disk in the order they were made.

#[tauri::command]
pub async fn story_list(app: AppHandle) -> Result<Vec<studio::StorySummary>, String> {
    off_window(app, |app| Ok(studio::list(&crate::rules::config_dir(app)?))).await
}

#[tauri::command]
pub fn story_register(app: AppHandle, folder: PathBuf) -> Result<(), String> {
    studio::register(&crate::rules::config_dir(&app)?, &folder)
}

#[tauri::command]
pub fn story_forget(app: AppHandle, folder: PathBuf) -> Result<(), String> {
    studio::forget(&crate::rules::config_dir(&app)?, &folder)
}

#[tauri::command]
pub fn story_create(
    app: AppHandle,
    folder: PathBuf,
    title: String,
    language: String,
    frame: story_engine::frame::Frame,
    chapter_title: String,
) -> Result<(), String> {
    studio::create(
        &crate::rules::config_dir(&app)?,
        &folder,
        &title,
        &language,
        frame,
        &chapter_title,
    )
}

/// Opens a story, and takes its copy of the day if it has none yet
/// (`story::backups`). The copy is best-effort: a story that cannot be
/// copied still opens.
#[tauri::command]
pub async fn story_open(app: AppHandle, folder: PathBuf) -> Result<studio::OpenStory, String> {
    off_window(app, move |app| {
        let now = chrono::Local::now().naive_local();
        if let Err(e) = backups::take_daily(&crate::rules::config_dir(app)?, &folder, now) {
            log::warn!("story backups: no copy of {}: {e}", folder.display());
        }
        studio::open(&folder)
    })
    .await
}

#[tauri::command]
pub async fn story_check(app: AppHandle, folder: PathBuf) -> Result<studio::Checked, String> {
    off_window(app, move |_| Ok(studio::check(&folder))).await
}

/// Writes one file the editor edits - a chapter, an event, `story.yaml` or
/// a world file - from the value it sends, in the shape of that file.
#[tauri::command]
pub fn story_save_file(
    folder: PathBuf,
    file: String,
    expected: Option<String>,
    value: serde_json::Value,
) -> Result<String, String> {
    studio::save_file(&folder, &file, expected.as_deref(), value)
}

/// Deletes an event's file, guarded by the stamp it was read with.
#[tauri::command]
pub fn story_remove_file(folder: PathBuf, file: String, expected: String) -> Result<(), String> {
    studio::remove_file(&folder, &file, &expected)
}

/// Renames a character, flag, counter, relation step or event everywhere it
/// is cited. Off the window thread: it reads the whole story folder. The
/// frontend's queue of edits keeps it in order with the saves around it.
#[tauri::command]
pub async fn story_rename(
    app: AppHandle,
    folder: PathBuf,
    kind: story_engine::refs::NameKind,
    from: String,
    to: String,
) -> Result<(), String> {
    off_window(app, move |_| studio::rename(&folder, kind, &from, &to)).await
}

/// Ctrl+Z in the editor: puts back the files of the last edit. Resolves to
/// the files written, empty when there was nothing to undo. A short write,
/// synchronous like a save.
#[tauri::command]
pub fn story_undo(folder: PathBuf) -> Result<Vec<String>, String> {
    crate::story::history::with(&folder, |h| h.undo(&folder))
}

/// Ctrl+Y: writes again the files of the last edit undone.
#[tauri::command]
pub fn story_redo(folder: PathBuf) -> Result<Vec<String>, String> {
    crate::story::history::with(&folder, |h| h.redo(&folder))
}

/// The story's copies, newest first.
#[tauri::command]
pub async fn story_backups(app: AppHandle, folder: PathBuf) -> Result<Vec<backups::Backup>, String> {
    off_window(app, move |app| {
        Ok(backups::list(&crate::rules::config_dir(app)?, &folder))
    })
    .await
}

/// Puts the story back as it was in a copy, after copying it as it is now.
#[tauri::command]
pub async fn story_restore(app: AppHandle, folder: PathBuf, id: String) -> Result<Vec<String>, String> {
    off_window(app, move |app| {
        let now = chrono::Local::now().naive_local();
        backups::restore(&crate::rules::config_dir(app)?, &folder, &id, now)
    })
    .await
}
