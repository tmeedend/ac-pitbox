//! Commandes des mods « autres » (§7.3) : contenus hors voiture/circuit,
//! avec résolution de conflits de fichiers par priorité.

use super::prelude::*;

/// Liste les mods « autres » avec leurs conflits de fichiers détectés.
#[tauri::command]
pub fn list_other_mods(app: AppHandle, db: State<Db>) -> Result<Vec<crate::others::OtherModCard>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::others::list_others(&conn, &cfg).map_err(|e| e.to_string())
}

/// Ouvre le dossier de bibliothèque d'un mod « autre » dans l'explorateur.
/// Même rationale que `open_mod_folder` : le chemin est résolu côté Rust depuis
/// l'overlay, jamais reçu du front, donc le scope ACL du plugin `opener` reste
/// fermé.
#[tauri::command]
pub fn open_other_mod_folder(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let path = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        crate::others::folder_path(&conn, &cfg, &id)?
    };
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Marque/démarque un mod « autre » comme prioritaire (§7.3).
#[tauri::command]
pub fn set_other_priority(db: State<Db>, id: String, priority: bool) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::others::set_priority(&conn, &id, priority)
}

/// Active un mod « autre » par junction (§7.3).
#[tauri::command]
pub fn activate_other(app: AppHandle, db: State<Db>, id: String) -> Result<crate::others::ActivateOtherResult, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::others::activate_other(&conn, &cfg, &id)
}

/// Désactive un mod « autre » (§7.3).
#[tauri::command]
pub fn deactivate_other(db: State<Db>, id: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::others::deactivate_other(&conn, &id)
}

/// Supprime un mod « autre » : jonctions + fichiers + overlay (§7.3).
#[tauri::command]
pub fn delete_other_mod(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::others::delete_other(&conn, &cfg, &id)
}

// Cinquième exemplaire du même quintuplet que les mods/apps/packs/sons — voir
// la note dans `commands/addons.rs`.

/// Fichiers annexes d'un « autre mod » (§4.5.2) : notice, images livrées à
/// côté d'un mannequin de pilote nu, par exemple.
#[tauri::command]
pub fn list_other_resources(
    app: AppHandle,
    db: State<Db>,
    id: String,
) -> Result<Vec<crate::resources::ResourceFile>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(crate::resources::list_resources(&crate::others::resources_dir(
        &conn, &cfg, &id,
    )?))
}

/// Ouvre une annexe d'« autre mod » avec l'application par défaut de l'OS.
#[tauri::command]
pub fn open_other_resource(app: AppHandle, db: State<Db>, id: String, rel_path: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let dir = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        crate::others::resources_dir(&conn, &cfg, &id)?
    };
    let path = crate::resources::resolve_resource_path(&dir, &rel_path)?;
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Chemin absolu d'une annexe d'« autre mod », pour l'afficher via `asset://`.
#[tauri::command]
pub fn get_other_resource_path(app: AppHandle, db: State<Db>, id: String, rel_path: String) -> Result<String, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let dir = crate::others::resources_dir(&conn, &cfg, &id)?;
    Ok(crate::resources::resolve_resource_path(&dir, &rel_path)?
        .display()
        .to_string())
}

/// Contenu brut d'une annexe d'« autre mod », pour la prévisualisation.
#[tauri::command]
pub fn read_other_resource(
    app: AppHandle,
    db: State<Db>,
    id: String,
    rel_path: String,
) -> Result<tauri::ipc::Response, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let dir = crate::others::resources_dir(&conn, &cfg, &id)?;
    Ok(tauri::ipc::Response::new(crate::resources::read_resource(
        &dir, &rel_path,
    )?))
}
