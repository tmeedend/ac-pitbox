//! Commandes groupées (§6.6) : appliquées à une sélection de mods.

use super::prelude::*;

#[tauri::command]
pub fn bulk_set_favorite(db: State<Db>, ids: Vec<String>, favorite: bool) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::bulk::set_favorite(&conn, &ids, favorite)
}

#[tauri::command]
pub fn bulk_set_category(db: State<Db>, ids: Vec<String>, category: Option<String>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::bulk::set_category(&conn, &ids, category.as_deref())
}

#[tauri::command]
pub fn bulk_add_tag(db: State<Db>, ids: Vec<String>, tag: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::bulk::add_tag(&conn, &ids, &tag)
}

#[tauri::command]
pub fn bulk_remove_tag(db: State<Db>, ids: Vec<String>, tag: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::bulk::remove_tag(&conn, &ids, &tag)
}

#[tauri::command]
pub fn bulk_activate(app: AppHandle, db: State<Db>, ids: Vec<String>) -> Result<crate::bulk::BulkReport, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(crate::bulk::activate(&conn, &cfg, &ids))
}

#[tauri::command]
pub fn bulk_deactivate(app: AppHandle, db: State<Db>, ids: Vec<String>) -> Result<crate::bulk::BulkReport, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(crate::bulk::deactivate(&conn, &cfg, &ids))
}

/// Supprime en masse (fichiers + junction + overlay pour chacun, §9.3).
#[tauri::command]
pub fn bulk_delete(app: AppHandle, db: State<Db>, ids: Vec<String>) -> Result<crate::bulk::BulkReport, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(crate::bulk::delete(&conn, &cfg, &ids))
}

#[tauri::command]
pub fn bulk_export(
    app: AppHandle,
    db: State<Db>,
    ids: Vec<String>,
    dest_dir: String,
) -> Result<Vec<crate::bulk::BulkExportItem>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(crate::bulk::export(&conn, &cfg, &ids, std::path::Path::new(&dest_dir)))
}
