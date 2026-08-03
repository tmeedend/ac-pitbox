//! Commandes de configuration (§12) : lecture, écriture, validation
//! et autodétection des chemins.

use super::prelude::*;

#[tauri::command]
pub fn get_config(app: AppHandle) -> AppConfig {
    crate::config::load(&app)
}

#[tauri::command]
pub fn save_config(app: AppHandle, config: AppConfig) -> Result<(), String> {
    crate::config::save(&app, &config)
}

#[tauri::command]
pub fn validate_config(config: AppConfig) -> ConfigValidation {
    crate::config::validate(&config)
}

#[tauri::command]
pub fn autodetect_paths() -> DetectedPaths {
    crate::detect::autodetect()
}
