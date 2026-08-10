//! Commandes des sessions de lancement sauvegardées (§8.4bis) — voir
//! `saved_sessions.rs`.

use super::prelude::*;

#[tauri::command]
pub fn get_saved_sessions(app: AppHandle) -> serde_json::Value {
    crate::saved_sessions::load(&app)
}

#[tauri::command]
pub fn save_saved_sessions(app: AppHandle, all: serde_json::Value) -> Result<(), String> {
    crate::saved_sessions::save(&app, &all)
}
