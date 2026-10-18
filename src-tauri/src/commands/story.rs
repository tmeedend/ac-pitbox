//! Story Mode (`docs/story/SPEC-story-mode.md`) — see `crate::story`.

use std::path::PathBuf;

use super::prelude::*;
use crate::story::studio;

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
    chapter_title: String,
) -> Result<(), String> {
    studio::create(
        &crate::rules::config_dir(&app)?,
        &folder,
        &title,
        &language,
        &chapter_title,
    )
}

#[tauri::command]
pub async fn story_open(app: AppHandle, folder: PathBuf) -> Result<studio::OpenStory, String> {
    off_window(app, move |_| studio::open(&folder)).await
}

#[tauri::command]
pub async fn story_check(app: AppHandle, folder: PathBuf) -> Result<Vec<story_engine::validate::Diagnostic>, String> {
    off_window(app, move |_| Ok(studio::check(&folder))).await
}

#[tauri::command]
pub fn story_save_chapter(
    folder: PathBuf,
    file: String,
    expected: Option<String>,
    chapter: serde_json::Value,
) -> Result<String, String> {
    let chapter = studio::chapter_from_json(chapter)?;
    studio::save_chapter(&folder, &file, expected.as_deref(), &chapter)
}
