//! Commandes de profils (§7bis) : jeux de mods actifs mémorisés.

use super::prelude::*;

#[tauri::command]
pub fn list_profiles(db: State<Db>) -> Result<Vec<crate::overlay::ProfileRow>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::overlay::list_profiles(&conn).map_err(|e| e.to_string())
}

/// Crée un profil capturant l'état actif courant.
#[tauri::command]
pub fn create_profile(app: AppHandle, db: State<Db>, name: String) -> Result<String, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::profiles::create_from_active(&conn, &cfg, &name)
}

/// Applique un profil (réconciliation des junctions).
#[tauri::command]
pub fn apply_profile(app: AppHandle, db: State<Db>, id: String) -> Result<crate::profiles::ApplyReport, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::profiles::apply(&conn, &cfg, &id)
}

#[tauri::command]
pub fn delete_profile(db: State<Db>, id: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::overlay::delete_profile(&conn, &id).map_err(|e| e.to_string())
}
