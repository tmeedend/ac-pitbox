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

/// Ouvre la page Windows « Pour les développeurs » (§2, prérequis symlink).
#[tauri::command]
pub fn open_developer_mode_settings() -> Result<(), String> {
    crate::config::open_developer_mode_settings()
}
