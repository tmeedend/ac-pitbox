//! Commandes d'activation (§7) : déploiement dans `content/` et retrait.
//! Le garde-fou junction/hardlink vit dans `activation.rs`, jamais ici.

use super::prelude::*;

/// Active un mod (crée la junction). `version_id` optionnel = change la version active.
#[tauri::command]
pub fn activate_mod(app: AppHandle, db: State<Db>, id: String, version_id: Option<String>) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::activation::activate(&conn, &cfg, &id, version_id.as_deref())
}

#[tauri::command]
pub fn deactivate_mod(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::activation::deactivate(&conn, &cfg, &id)
}
