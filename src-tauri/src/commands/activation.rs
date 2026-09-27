//! Commandes d'activation (§7) : déploiement dans `content/` et retrait.
//! Le garde-fou junction/hardlink vit dans `activation.rs`, jamais ici.

use super::prelude::*;

/// Active un mod (crée la junction). `version_id` optionnel = change la version active.
#[tauri::command]
pub fn activate_mod(app: AppHandle, db: State<Db>, id: String, version_id: Option<String>) -> Result<(), String> {
    let _game_write = crate::gamestate::GameWrite::begin();
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::activation::activate(&conn, &cfg, &id, version_id.as_deref())?;
    // Another version made active: its tags are re-read by the rules, as its
    // physics already was through `recompose` (FICHE§6.2). Best-effort — the
    // version is switched either way, and the next harmonisation catches up.
    if version_id.is_some() {
        let rules = crate::rules::load(&app);
        if let Err(e) = crate::harmonize::harmonize_mod(&conn, &cfg, &rules, &id) {
            log::warn!("harmonisation of {id} after a version switch: {e}");
        }
    }
    Ok(())
}

#[tauri::command]
pub fn deactivate_mod(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let _game_write = crate::gamestate::GameWrite::begin();
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::activation::deactivate(&conn, &cfg, &id)
}
