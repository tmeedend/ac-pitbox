//! Story Mode (`docs/story/SPEC-story-mode.md`) — see `crate::story`.

/// The Story switch asked for on this launch's command line, if any
/// (`--enable-story-studio` / `--disable-story-studio`). Read once at startup
/// by the frontend, which records it in `ui_prefs.json`: that file's schema
/// belongs to the frontend, so the backend never writes the key itself.
#[tauri::command]
pub fn story_studio_switch() -> Option<bool> {
    crate::story::studio_switch_from_args(std::env::args())
}
