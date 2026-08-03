//! Commandes d'import (§4) : archives, dossiers, import en masse et
//! arbitrage des conflits flous.

use super::prelude::*;

#[tauri::command]
pub fn import_archives(
    app: AppHandle,
    db: State<Db>,
    paths: Vec<String>,
    // Décisions update/extension pour reprendre un import ambigu (§4.4). Vide au 1er appel.
    decisions: Option<Vec<crate::importer::ImportDecision>>,
) -> Result<Vec<ArchiveResult>, String> {
    let cfg = crate::config::load(&app);
    let rules = crate::rules::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(crate::importer::import_archives(
        &app,
        &conn,
        &cfg,
        &rules,
        &paths,
        &decisions.unwrap_or_default(),
    ))
}

/// Import depuis des dossiers déjà décompressés (§4.5). `copy=true` préserve la
/// source, sinon déplacement adaptatif.
#[tauri::command]
pub fn import_folders(
    app: AppHandle,
    db: State<Db>,
    paths: Vec<String>,
    copy: bool,
    decisions: Option<Vec<crate::importer::ImportDecision>>,
) -> Result<Vec<ArchiveResult>, String> {
    let cfg = crate::config::load(&app);
    let rules = crate::rules::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(crate::importer::import_folders(
        &app,
        &conn,
        &cfg,
        &rules,
        &paths,
        copy,
        &decisions.unwrap_or_default(),
    ))
}

/// Analyse un dossier parent (§4.6) : classe chaque sous-dossier sans rien écrire.
#[tauri::command]
pub fn analyze_bulk_import(
    app: AppHandle,
    db: State<Db>,
    parent: String,
) -> Result<Vec<crate::importer::BulkEntry>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::importer::analyze_bulk(&conn, &cfg, std::path::Path::new(&parent))
}

/// Exécute l'import en masse selon les décisions d'arbitrage (§4.6).
#[tauri::command]
pub fn execute_bulk_import(
    app: AppHandle,
    db: State<Db>,
    items: Vec<crate::importer::BulkExecItem>,
    copy: bool,
) -> Result<Vec<ArchiveResult>, String> {
    let cfg = crate::config::load(&app);
    let rules = crate::rules::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(crate::importer::execute_bulk(&app, &conn, &cfg, &rules, &items, copy))
}

/// Résout un conflit flou (§4.2) : action = "keep_both" | "replace".
#[tauri::command]
pub fn resolve_conflict(
    app: AppHandle,
    db: State<Db>,
    new_id: String,
    old_id: String,
    action: String,
) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::importer::resolve_conflict(&conn, &cfg, &new_id, &old_id, &action)
}
