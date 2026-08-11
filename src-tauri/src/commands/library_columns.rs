//! Commandes des colonnes de la bibliothèque (§6.2) — voir `library_columns.rs`.

use super::prelude::*;

#[tauri::command]
pub fn get_library_columns(app: AppHandle) -> serde_json::Value {
    crate::library_columns::load(&app)
}

#[tauri::command]
pub fn save_library_columns(app: AppHandle, prefs: serde_json::Value) -> Result<(), String> {
    crate::library_columns::save(&app, &prefs)
}
