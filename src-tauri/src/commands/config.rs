//! Commandes de configuration (§12) : lecture, écriture, validation
//! et autodétection des chemins.

use super::prelude::*;

#[tauri::command]
pub fn get_config(app: AppHandle) -> AppConfig {
    crate::config::load(&app)
}

/// Enregistre la config, puis **réindexe le contenu de base si le dossier du
/// jeu vient d'être désigné ou a changé** (§12bis.1).
///
/// Sans ça, le scan du contenu Kunos ne tournait qu'au démarrage de l'app, et
/// seulement si un chemin d'AC était déjà connu — donc jamais au **premier**
/// démarrage, où la config n'existe pas encore quand `setup` s'exécute :
/// l'assistant renseignait les chemins, et la bibliothèque restait vide
/// jusqu'au lancement suivant. Même angle mort en changeant de dossier de jeu
/// depuis les Réglages, où l'index restait celui de l'ancienne install.
#[tauri::command]
pub fn save_config(app: AppHandle, db: State<Db>, config: AppConfig) -> Result<(), String> {
    let previous = crate::config::load(&app).ac_install_path;
    crate::config::save(&app, &config)?;

    let Some(ac) = config.ac_install_path.as_ref() else {
        return Ok(());
    };
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let indexed = crate::overlay::count_stock(&conn).unwrap_or(0).max(0) as usize;
    if !crate::stock::needs_reindex(previous.as_deref(), Some(ac.as_path()), indexed) {
        return Ok(());
    }
    // Best-effort : un contenu de base non indexé ne doit pas faire échouer
    // l'enregistrement des chemins, mais ne doit pas non plus passer inaperçu.
    let rules = crate::rules::load(&app);
    if let Err(e) = crate::stock::index_stock_content(&conn, &config, &rules) {
        log::warn!("index_stock_content after save_config: {e}");
    }
    Ok(())
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
