//! Commandes d'import (§4) : archives, dossiers, import en masse, arbitrage
//! des conflits flous et annulation d'un lot en cours.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::prelude::*;
use tauri::Manager;

/// Drapeau d'annulation partagé (§4.2bis). Un seul import tourne à la fois
/// (le frontend désactive les boutons pendant), donc un drapeau global suffit :
/// il est remis à zéro au démarrage de chaque lot, jamais avant.
#[derive(Default)]
pub struct ImportControl(pub Arc<AtomicBool>);

/// Prépare un lot : remet le drapeau d'annulation à zéro et construit le
/// contexte de progression (émission + benchmark persistant).
fn begin(app: &AppHandle) -> crate::import_progress::ImportCtx {
    let control = app.state::<ImportControl>();
    control.0.store(false, Ordering::Relaxed);
    crate::import_progress::ImportCtx::new(app, control.0.clone())
}

/// Demande l'arrêt de l'import en cours (§4.2bis). L'arrêt est constaté
/// **entre deux items** — et 7-Zip est tué s'il décompresse : jamais au milieu
/// du rangement d'un mod, qui laisserait une bibliothèque à moitié écrite.
#[tauri::command]
pub fn cancel_import(control: State<ImportControl>) {
    control.0.store(true, Ordering::Relaxed);
}

/// Importe une liste d'archives. `async` + `spawn_blocking` (§4.2) : un gros
/// lot (extraction 7-Zip incluse) peut prendre plusieurs minutes — exécuté
/// directement sur l'IPC, la commande partagerait le même thread que la
/// livraison des événements `import:progress`, qui n'arriveraient alors
/// jamais avant la toute fin (barre de progression muette, drop apparemment
/// sans effet). Sur un thread dédié, les événements sont émis et livrés au
/// fil de l'eau, sans jamais bloquer le reste de l'app.
#[tauri::command]
pub async fn import_archives(
    app: AppHandle,
    paths: Vec<String>,
    // Décisions update/extension pour reprendre un import ambigu (§4.4). Vide au 1er appel.
    decisions: Option<Vec<crate::importer::ImportDecision>>,
) -> Result<Vec<ArchiveResult>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = begin(&app);
        let cfg = crate::config::load(&app);
        let rules = crate::rules::load(&app);
        let db = app.state::<Db>();
        let out =
            crate::importer::import_archives(&ctx, db.inner(), &cfg, &rules, &paths, &decisions.unwrap_or_default());
        ctx.finish_batch(ctx.cancelled());
        out
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Import depuis des dossiers déjà décompressés (§4.2). `copy=true` préserve la
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
        let ctx = begin(&app);
        let cfg = crate::config::load(&app);
        let rules = crate::rules::load(&app);
        let db = app.state::<Db>();
        let out = crate::importer::import_folders(
            &ctx,
            db.inner(),
            &cfg,
            &rules,
            &paths,
            copy,
            &decisions.unwrap_or_default(),
        );
        ctx.finish_batch(ctx.cancelled());
        out
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Analyse un dossier parent (§4.2) : classe chaque sous-dossier sans rien
/// écrire. `async` + `spawn_blocking` comme les imports : le scan d'un dossier
/// parent de plusieurs dizaines de mods tient le verrou base tout du long, et
/// le tenir depuis le thread IPC gèlerait en plus la livraison des événements.
#[tauri::command]
pub async fn analyze_bulk_import(app: AppHandle, parent: String) -> Result<Vec<crate::importer::BulkEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cfg = crate::config::load(&app);
        let db = app.state::<Db>();
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        crate::importer::analyze_bulk(&conn, &cfg, std::path::Path::new(&parent))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Exécute l'import en masse selon les décisions d'arbitrage (§4.2). Même
/// raison qu'`import_archives` pour `async` + `spawn_blocking`.
#[tauri::command]
pub async fn execute_bulk_import(
    app: AppHandle,
    items: Vec<crate::importer::BulkExecItem>,
    copy: bool,
) -> Result<Vec<ArchiveResult>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = begin(&app);
        let cfg = crate::config::load(&app);
        let rules = crate::rules::load(&app);
        let db = app.state::<Db>();
        let out = crate::importer::execute_bulk(&ctx, db.inner(), &cfg, &rules, &items, copy);
        ctx.finish_batch(ctx.cancelled());
        out
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Ce qu'un glisser-déposer contient réellement (§4.2). Le frontend ne peut pas
/// distinguer un dossier d'un fichier à partir du seul chemin que lui donne
/// Tauri — il filtrait donc sur l'extension, et **ignorait un dossier de mod en
/// silence**, sans le moindre retour.
#[derive(serde::Serialize)]
pub struct DroppedPaths {
    pub archives: Vec<String>,
    pub folders: Vec<String>,
}

#[tauri::command]
pub fn split_dropped_paths(paths: Vec<String>) -> DroppedPaths {
    let mut archives = Vec::new();
    let mut folders = Vec::new();
    for p in paths {
        let path = std::path::Path::new(&p);
        if path.is_dir() {
            folders.push(p);
        } else if path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
            crate::importer::NESTED_ARCHIVE_EXTS
                .iter()
                .any(|a| a.eq_ignore_ascii_case(e))
        }) {
            archives.push(p);
        }
        // Tout le reste (un .txt lâché par mégarde) est ignoré : c'est le seul
        // cas où ne rien faire est la bonne réponse.
    }
    DroppedPaths { archives, folders }
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

/// Dossiers proposés par l'auteur et en attente d'une décision (§4.6ter).
/// Lus en base, pas dans le rapport en mémoire : ne rien décider est une
/// réponse valable, donc ce qui attend doit survivre à une fermeture de l'app.
#[tauri::command]
pub fn list_pending_folders(app: AppHandle, db: State<Db>) -> Result<Vec<crate::pending::PendingFolder>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::pending::list(&conn, &cfg)
}

/// Applique le sort choisi pour un dossier proposé (§4.6ter) :
/// "game" | "layer" | "resources" | "other" | "discard".
#[tauri::command]
pub fn resolve_pending_folder(app: AppHandle, db: State<Db>, id: String, action: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::pending::resolve(&conn, &cfg, &id, &action)
}

/// Contenu texte de la notice d'un dossier proposé (§4.6ter), pour la lire sans
/// quitter l'écran d'arbitrage : c'est elle qui porte l'information que le
/// disque n'a pas. Même garde-fou anti-traversée et même plafond de taille que
/// la prévisualisation des ressources (§4.5.2).
#[tauri::command]
pub fn read_pending_document(app: AppHandle, db: State<Db>, id: String, name: String) -> Result<String, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::pending::read_document(&conn, &cfg, &id, &name)
}
