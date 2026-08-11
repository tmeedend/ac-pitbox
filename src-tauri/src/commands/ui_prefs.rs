//! Commandes des réglages d'interface (§6.2/§8.6) — voir `ui_prefs.rs`.

use super::prelude::*;

#[tauri::command]
pub fn get_ui_prefs(app: AppHandle) -> serde_json::Value {
    crate::ui_prefs::load(&app)
}

#[tauri::command]
pub fn save_ui_prefs(app: AppHandle, prefs: serde_json::Value) -> Result<(), String> {
    crate::ui_prefs::save(&app, &prefs)
}
