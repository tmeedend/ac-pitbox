//! Commandes des add-ons (§12bis) : contenu de base Kunos, skins, skins de
//! circuit, sons et apps Python/Lua.

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
pub fn list_subs_by_type(
    app: AppHandle,
    db: State<Db>,
    sub_type: String,
) -> Result<Vec<crate::overlay::SubModRow>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::submods::list_by_type_sized(&conn, &cfg, &sub_type).map_err(|e| e.to_string())
}

/// Reconnaît les skins de circuit fournis avec le contenu initial du mod
/// (§8, lecture live du disque, best-effort) — à appeler avant de lister
/// les skins d'un circuit pour qu'ils y apparaissent.
#[tauri::command]
pub fn sync_track_skins(app: AppHandle, db: State<Db>, track_id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::submods::sync_bundled_track_skins(&conn, &cfg, &track_id);
    Ok(())
}

/// Skins de circuit actuellement actifs (§8, plusieurs possibles).
#[tauri::command]
pub fn list_active_track_skins(db: State<Db>, track_id: String) -> Result<Vec<String>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(crate::submods::list_active_track_skins(&conn, &track_id))
}

/// Skins de circuit avec image de prévisualisation résolue, pour le
/// sélecteur multi-choix de la barre latérale (§8).
#[tauri::command]
pub fn list_track_skin_options(
    app: AppHandle,
    db: State<Db>,
    track_id: String,
) -> Result<Vec<crate::submods::TrackSkinOption>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(crate::submods::list_track_skin_options(&conn, &cfg, &track_id))
}

/// Active/désactive un skin de circuit (§8, pas exclusif).
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

/// Liste les apps (Python ou Lua/CSP) avec leur état d'activation (§12bis.4).
#[tauri::command]
pub fn list_apps(app: AppHandle, db: State<Db>) -> Result<Vec<crate::apps::AppItem>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::apps::list_apps(&conn, &cfg)
}

/// Active une app (junction vers apps/python/ ou apps/lua/ selon le langage
/// détecté, §12bis.4).
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

/// Ouvre le dossier bibliothèque d'une app dans l'explorateur (même schéma
/// que `open_mod_folder` : chemin résolu côté serveur, pas de scope ACL large
/// à ouvrir sur le plugin opener).
#[tauri::command]
pub fn open_app_folder(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let path = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        crate::apps::app_folder_path(&conn, &cfg, &id)?
    };
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Liste les fichiers annexes d'une app (§4.5.2, même mécanisme que les mods
/// voiture/circuit) — lue en direct sur disque à chaque appel, jamais
/// mémorisée en base.
#[tauri::command]
pub fn list_app_resources(app: AppHandle, id: String) -> Result<Vec<crate::resources::ResourceFile>, String> {
    let cfg = crate::config::load(&app);
    let library = cfg.library_path.ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    Ok(crate::resources::list_resources(&crate::resources::resources_dir_for(
        &library,
        "apps",
        &[&id],
    )))
}

/// Ouvre un fichier annexe d'une app avec l'application par défaut de l'OS
/// (§4.5.2). `rel_path` résolu et validé côté serveur (garde-fou anti-traversée).
#[tauri::command]
pub fn open_app_resource(app: AppHandle, id: String, rel_path: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let library = cfg.library_path.ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    let dir = crate::resources::resources_dir_for(&library, "apps", &[&id]);
    let path = crate::resources::resolve_resource_path(&dir, &rel_path)?;
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}
