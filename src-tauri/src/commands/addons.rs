//! Commandes des add-ons (§12bis) : contenu de base Kunos, skins, skins de
//! circuit, sons et apps Python.

use super::prelude::*;

/// Indexe le contenu de base Kunos présent dans content/ (§12bis.1).
#[tauri::command]
pub fn index_stock_content(app: AppHandle, db: State<Db>) -> Result<usize, String> {
    let cfg = crate::config::load(&app);
    let rules = crate::rules::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::stock::index_stock_content(&conn, &cfg, &rules)
}

/// Sous-éléments rattachés à une entité (skins/sons d'une voiture, §12bis.3).
#[tauri::command]
pub fn list_sub_mods(db: State<Db>, parent_id: String) -> Result<Vec<crate::overlay::SubModRow>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::overlay::list_subs_for_parent(&conn, &parent_id).map_err(|e| e.to_string())
}

/// Tous les sous-éléments d'un type, pour la vue transversale (§12bis.3) —
/// taille sur disque incluse (regroupements pesés côté UI).
#[tauri::command]
pub fn list_subs_by_type(db: State<Db>, sub_type: String) -> Result<Vec<crate::overlay::SubModRow>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::submods::list_by_type_sized(&conn, &sub_type).map_err(|e| e.to_string())
}

/// Reconnaît les skins de circuit fournis avec le contenu initial du mod
/// (§4.6bis, lecture live du disque, best-effort) — à appeler avant de lister
/// les skins d'un circuit pour qu'ils y apparaissent.
#[tauri::command]
pub fn sync_track_skins(app: AppHandle, db: State<Db>, track_id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::submods::sync_bundled_track_skins(&conn, &cfg, &track_id);
    Ok(())
}

/// Skins de circuit actuellement actifs (§4.6bis, plusieurs possibles).
#[tauri::command]
pub fn list_active_track_skins(db: State<Db>, track_id: String) -> Result<Vec<String>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(crate::submods::list_active_track_skins(&conn, &track_id))
}

/// Skins de circuit avec image de prévisualisation résolue, pour le
/// sélecteur multi-choix de la barre latérale (§4.6bis).
#[tauri::command]
pub fn list_track_skin_options(
    db: State<Db>,
    track_id: String,
) -> Result<Vec<crate::submods::TrackSkinOption>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(crate::submods::list_track_skin_options(&conn, &track_id))
}

/// Active/désactive un skin de circuit (§4.6bis, pas exclusif).
#[tauri::command]
pub fn set_track_skin_active(
    app: AppHandle,
    db: State<Db>,
    track_id: String,
    skin_name: String,
    active: bool,
) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::submods::set_track_skin_active(&conn, &cfg, &track_id, &skin_name, active)
}

/// Active un mod de son (bascule exclusive du sfx/, §12bis.2).
#[tauri::command]
pub fn activate_sound(app: AppHandle, db: State<Db>, sub_id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::submods::activate_sound(&conn, &cfg, &sub_id)
}

/// Restaure le son d'origine d'une voiture (§12bis.2).
#[tauri::command]
pub fn restore_sound(app: AppHandle, db: State<Db>, parent_id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::submods::restore_sound(&conn, &cfg, &parent_id)
}

/// Supprime proprement un sous-élément (skin/son) : junction + fichiers + overlay (§12bis.3).
#[tauri::command]
pub fn delete_sub_mod(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::submods::remove_sub(&conn, &cfg, &id)
}

/// Supprime proprement une app : junction + fichiers + overlay (§12bis.4).
#[tauri::command]
pub fn delete_app(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::apps::remove_app(&conn, &cfg, &id)
}

/// Liste les apps Python avec leur état d'activation (§12bis.4).
#[tauri::command]
pub fn list_apps(app: AppHandle, db: State<Db>) -> Result<Vec<crate::apps::AppItem>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::apps::list_apps(&conn, &cfg)
}

/// Active une app (junction vers apps/python/, §12bis.4).
#[tauri::command]
pub fn activate_app(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::apps::activate_app(&conn, &cfg, &id)
}

/// Désactive une app (§12bis.4).
#[tauri::command]
pub fn deactivate_app(app: AppHandle, id: String) -> Result<(), String> {
    crate::apps::deactivate_app(&crate::config::load(&app), &id)
}
