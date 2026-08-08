//! Commandes d'import (§4) : archives, dossiers, import en masse et
//! arbitrage des conflits flous.

use super::prelude::*;
use tauri::Manager;

/// Importe une liste d'archives. `async` + `spawn_blocking` (§4.6bis) : un gros
/// lot (extraction 7-Zip incluse) peut prendre plusieurs minutes — exécuté
/// directement sur l'IPC, la commande partagerait le même thread que la
/// livraison des événements `import:progress`, qui n'arriveraient alors
/// jamais avant la toute fin (barre de progression muette, drop apparemment
/// sans effet). Sur un thread dédié, les événements sont émis et livrés au
/// fil de l'eau, sans jamais bloquer le reste de l'app (chaque écran lit
/// l'overlay via son propre verrou, repris entre deux archives — §9.3bis).
#[tauri::command]
pub async fn import_archives(
    app: AppHandle,
    paths: Vec<String>,
    // Décisions update/extension pour reprendre un import ambigu (§4.4). Vide au 1er appel.
    decisions: Option<Vec<crate::importer::ImportDecision>>,
) -> Result<Vec<ArchiveResult>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cfg = crate::config::load(&app);
        let rules = crate::rules::load(&app);
        let db = app.state::<Db>();
        crate::importer::import_archives(&app, db.inner(), &cfg, &rules, &paths, &decisions.unwrap_or_default())
    })
    .await
    .map_err(|e| e.to_string())
}

/// Import depuis des dossiers déjà décompressés (§4.5). `copy=true` préserve la
/// source, sinon déplacement adaptatif. Même raison qu'`import_archives` pour
/// `async` + `spawn_blocking`.
#[tauri::command]
pub async fn import_folders(
    app: AppHandle,
    paths: Vec<String>,
    copy: bool,
    decisions: Option<Vec<crate::importer::ImportDecision>>,
) -> Result<Vec<ArchiveResult>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cfg = crate::config::load(&app);
        let rules = crate::rules::load(&app);
        let db = app.state::<Db>();
        crate::importer::import_folders(
            &app,
            db.inner(),
            &cfg,
            &rules,
            &paths,
            copy,
            &decisions.unwrap_or_default(),
        )
    })
    .await
    .map_err(|e| e.to_string())
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

/// Exécute l'import en masse selon les décisions d'arbitrage (§4.6). Même
/// raison qu'`import_archives` pour `async` + `spawn_blocking`.
#[tauri::command]
pub async fn execute_bulk_import(
    app: AppHandle,
    items: Vec<crate::importer::BulkExecItem>,
    copy: bool,
) -> Result<Vec<ArchiveResult>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cfg = crate::config::load(&app);
        let rules = crate::rules::load(&app);
        let db = app.state::<Db>();
        crate::importer::execute_bulk(&app, db.inner(), &cfg, &rules, &items, copy)
    })
    .await
    .map_err(|e| e.to_string())
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
