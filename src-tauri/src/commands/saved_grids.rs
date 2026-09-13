//! Commandes des grilles d'adversaires enregistrées (§5) — voir
//! `saved_grids.rs`.

use super::prelude::*;

#[tauri::command]
pub fn get_saved_grids(app: AppHandle) -> serde_json::Value {
    crate::saved_grids::load(&app)
}

#[tauri::command]
pub fn save_saved_grids(app: AppHandle, all: serde_json::Value) -> Result<(), String> {
    crate::saved_grids::save(&app, &all)
}
