//! Commandes des mods « autres » (§6.1bis) : contenus hors voiture/circuit,
//! avec résolution de conflits de fichiers par priorité.

use super::prelude::*;

/// Liste les mods « autres » avec leurs conflits de fichiers détectés.
#[tauri::command]
pub fn list_other_mods(app: AppHandle, db: State<Db>) -> Result<Vec<crate::others::OtherModCard>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::others::list_others(&conn, &cfg).map_err(|e| e.to_string())
}

/// Marque/démarque un mod « autre » comme prioritaire (§6.1bis).
#[tauri::command]
pub fn set_other_priority(db: State<Db>, id: String, priority: bool) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::others::set_priority(&conn, &id, priority)
}

/// Active un mod « autre » par junction (§6.1bis).
#[tauri::command]
pub fn activate_other(app: AppHandle, db: State<Db>, id: String) -> Result<crate::others::ActivateOtherResult, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::others::activate_other(&conn, &cfg, &id)
}

/// Désactive un mod « autre » (§6.1bis).
#[tauri::command]
pub fn deactivate_other(db: State<Db>, id: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::others::deactivate_other(&conn, &id)
}

/// Supprime un mod « autre » : jonctions + fichiers + overlay (§6.1bis).
#[tauri::command]
pub fn delete_other_mod(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::others::delete_other(&conn, &cfg, &id)
}
