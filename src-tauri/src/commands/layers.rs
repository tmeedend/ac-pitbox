//! Commandes de couches/extensions (§4.4) : contenus empilés sur une base,
//! activables et réordonnables.

use super::prelude::*;

/// Couches/extensions rattachées à une base (§4.4), pour la fiche détail.
#[tauri::command]
pub fn list_layers(db: State<Db>, parent_id: String) -> Result<Vec<crate::overlay::LayerRow>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::overlay::list_layers(&conn, &parent_id).map_err(|e| e.to_string())
}

/// Toutes les couches d'un type (Car|Track), vue transversale add-ons (§4.4).
#[tauri::command]
pub fn list_layers_by_kind(db: State<Db>, kind: String) -> Result<Vec<crate::overlay::LayerRow>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::overlay::list_layers_by_kind(&conn, &kind).map_err(|e| e.to_string())
}

/// Supprime une couche/extension : fichiers bibliothèque + overlay, puis
/// recompose le parent (§4.4).
#[tauri::command]
pub fn delete_layer(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::compose::remove_layer(&conn, &cfg, &id)
}

/// Active/désactive une couche puis recompose le contenu en jeu (§4.4).
#[tauri::command]
pub fn set_layer_active(app: AppHandle, db: State<Db>, id: String, active: bool) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::compose::set_layer_active(&conn, &cfg, &id, active)
}

/// Réordonne une couche (up = plus prioritaire) puis recompose (§4.4).
#[tauri::command]
pub fn reorder_layer(app: AppHandle, db: State<Db>, id: String, direction: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::compose::reorder_layer(&conn, &cfg, &id, &direction)
}

/// Fichiers apportés par une couche, avec ce que chacun fait à la base (§4.4).
#[tauri::command]
pub fn list_layer_files(app: AppHandle, db: State<Db>, id: String) -> Result<Vec<crate::layers::LayerFile>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::layers::list_files(&conn, &cfg, &id)
}

/// Ouvre le dossier d'une couche dans l'explorateur. Même rationale que
/// `open_mod_folder` : le chemin est résolu côté Rust depuis l'overlay, jamais
/// donné par le front, donc pas de scope ACL large à ouvrir.
#[tauri::command]
pub fn open_layer_folder(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let path = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        crate::layers::folder_path(&conn, &cfg, &id)?
    };
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}
