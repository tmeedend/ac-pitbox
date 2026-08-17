//! Commandes de maintenance et d'export (§9) : diagnostic, réindexation,
//! réparation et archive autonome.

use super::prelude::*;

/// Analyse mods cassés + junctions orphelines, sans rien supprimer (§9.3).
#[tauri::command]
pub fn maintenance_scan(app: AppHandle, db: State<Db>) -> Result<crate::maintenance::MaintenanceReport, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::maintenance::scan(&conn, &cfg)
}

/// Relit sur le disque les champs cache (nom, auteur, tags fichier, CSP,
/// skins/layouts) de tous les mods déjà importés, puis réapplique l'ontologie
/// (même effet que « Réappliquer les règles »). Sert à rattraper un mod dont
/// le fichier source a été corrigé/édité après import, sans le réimporter.
/// `recalc_size` (§9.4, option décochée par défaut côté UI) : recalcule aussi
/// la taille sur disque de chaque version — parcourt tous les fichiers de la
/// bibliothèque, potentiellement lent, d'où l'opt-in explicite.
/// Renvoie le nombre de mods traités.
#[tauri::command]
pub fn reindex_library(app: AppHandle, db: State<Db>, recalc_size: bool) -> Result<usize, String> {
    let rules = crate::rules::load(&app);
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let n = crate::maintenance::reindex_all(&conn, &cfg, recalc_size)?;
    crate::harmonize::harmonize_all(&conn, &cfg, &rules).map_err(|e| e.to_string())?;
    Ok(n)
}

/// Supprime un mod cassé (fichiers + junction + overlay).
#[tauri::command]
pub fn delete_broken_mod(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::maintenance::delete_broken(&conn, &cfg, &id)
}

/// Efface les skins/sons dont le mod parent n'existe plus (§9.3).
#[tauri::command]
pub fn purge_orphan_subs(app: AppHandle, db: State<Db>) -> Result<usize, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::maintenance::purge_orphan_subs(&conn, &cfg)
}

/// Retire une junction orpheline (garde-fou junction).
#[tauri::command]
pub fn remove_orphan_junction(app: AppHandle, kind: String, id: String) -> Result<(), String> {
    crate::maintenance::remove_orphan(&crate::config::load(&app), &kind, &id)
}

/// Désinstalle tout un pack (§4.4) : supprime chaque mod du pack. Renvoie le nb supprimé.
#[tauri::command]
pub fn delete_pack(app: AppHandle, db: State<Db>, pack: String) -> Result<usize, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::maintenance::delete_pack(&conn, &cfg, &pack)
}

/// Réinstalle un mod depuis son archive/dossier source conservé (§10/§11).
#[tauri::command]
pub fn reinstall_from_archive(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::maintenance::reinstall_from_archive(&conn, &cfg, &id)
}

/// Réparation générale (§9.3) : recrée les projections skin/circuit cassées,
/// et si `reinstall_broken`, réinstalle depuis l'archive source conservée
/// chaque mod détecté cassé qui en a une.
#[tauri::command]
pub fn repair_all(
    app: AppHandle,
    db: State<Db>,
    reinstall_broken: bool,
) -> Result<crate::maintenance::RepairAllReport, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::maintenance::repair_all(&conn, &cfg, reinstall_broken)
}

/// Exporte la version active d'un mod en archive autonome dans `dest_dir` (§9.1).
#[tauri::command]
pub fn export_mod(
    app: AppHandle,
    db: State<Db>,
    id: String,
    dest_dir: String,
) -> Result<crate::export::ExportReport, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::export::export_mod(&conn, &cfg, &id, std::path::Path::new(&dest_dir))
}
