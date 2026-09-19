//! Saved sessions as Content Manager presets (SESSION§3.6) — see
//! `sessionpreset.rs`.

use super::prelude::*;

/// Every `.cmpreset` under CM's `Quick Drive\`, ours and CM's alike. Never an
/// error: no Content Manager, no folder, or a corrupt file are non-results,
/// and each unreadable file is listed with its reason.
#[tauri::command]
pub fn list_session_presets(app: AppHandle) -> Vec<crate::sessionpreset::SessionPresetEntry> {
    crate::sessionpreset::list(&app)
}

/// Writes one session as a preset and returns its path. Not wrapped in a
/// silent fallback anywhere: this is a **write**, and a mute failure would
/// make "saved" out of a command that wrote nothing (golden rule 6).
#[tauri::command]
pub fn save_session_preset(
    app: AppHandle,
    name: String,
    setup: crate::launch::RaceSetup,
    snapshot: serde_json::Value,
) -> Result<String, String> {
    crate::sessionpreset::save(&app, &name, &setup, &snapshot)
}

/// Deletes one of **our** presets, identified by its path. A preset Content
/// Manager wrote is refused — listed by Pit Box, never removed by it.
#[tauri::command]
pub fn delete_session_preset(app: AppHandle, path: String) -> Result<(), String> {
    crate::sessionpreset::delete(&app, &path)
}
