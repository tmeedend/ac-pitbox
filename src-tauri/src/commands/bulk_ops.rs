//! Commandes groupées (§6.6) : appliquées à une sélection de mods.
//!
//! Les quatre lots qui touchent au disque (activer, désactiver, supprimer,
//! exporter) sont `async` + `spawn_blocking`, comme l'import et pour la même
//! raison (§4.2) : une commande synchrone s'exécute sur le thread principal,
//! donc supprimer quarante circuits y gèlerait la boucle d'événements — plus
//! aucun `invoke` ne répond, et Windows finit par marquer la fenêtre comme ne
//! répondant plus. Les événements `bulk:progress` ne partiraient d'ailleurs
//! qu'à la toute fin, ce qui rendrait la barre inutile.
//!
//! Les quatre autres (favori, catégorie, tags) restent synchrones : ce sont
//! quelques écritures SQLite, sans I/O de fichiers.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::prelude::*;
use tauri::{Emitter, Manager};

use crate::bulk::{BulkCtx, Progress};

/// Drapeau d'annulation partagé, même schéma que `ImportControl` : un seul lot
/// tourne à la fois (le panneau désactive ses boutons pendant), donc un
/// drapeau global suffit — remis à zéro au démarrage de chaque lot.
#[derive(Default)]
pub struct BulkControl(pub Arc<AtomicBool>);

/// Remet le drapeau d'annulation à zéro et rend de quoi émettre la
/// progression. La fermeture d'émission reste **ici** : `bulk.rs` ne connaît
/// pas Tauri, et pas seulement par principe — voir `ProgressSink`, l'import
/// y rend le binaire de test de la lib inexécutable.
fn begin(app: &AppHandle) -> (Arc<AtomicBool>, impl Fn(Progress) + '_) {
    let control = app.state::<BulkControl>();
    control.0.store(false, Ordering::Relaxed);
    let flag = control.0.clone();
    (flag, move |p: Progress| {
        let _ = app.emit("bulk:progress", p);
    })
}

/// Demande l'arrêt du lot en cours. Constaté **entre deux mods** : jamais au
/// milieu de l'un d'eux, qui laisserait une junction à moitié posée.
#[tauri::command]
pub fn cancel_bulk(control: State<BulkControl>) {
    control.0.store(true, Ordering::Relaxed);
}

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
pub async fn bulk_activate(app: AppHandle, ids: Vec<String>) -> Result<crate::bulk::BulkReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (cancel, emit) = begin(&app);
        let ctx = BulkCtx::new(&emit, "activate", cancel);
        let cfg = crate::config::load(&app);
        let db = app.state::<Db>();
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        Ok(crate::bulk::activate(&ctx, &conn, &cfg, &ids))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn bulk_deactivate(app: AppHandle, ids: Vec<String>) -> Result<crate::bulk::BulkReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (cancel, emit) = begin(&app);
        let ctx = BulkCtx::new(&emit, "deactivate", cancel);
        let cfg = crate::config::load(&app);
        let db = app.state::<Db>();
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        Ok(crate::bulk::deactivate(&ctx, &conn, &cfg, &ids))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Supprime en masse (fichiers + junction + overlay pour chacun, §9.3).
#[tauri::command]
pub async fn bulk_delete(app: AppHandle, ids: Vec<String>) -> Result<crate::bulk::BulkReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (cancel, emit) = begin(&app);
        let ctx = BulkCtx::new(&emit, "delete", cancel);
        let cfg = crate::config::load(&app);
        let db = app.state::<Db>();
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        Ok(crate::bulk::delete(&ctx, &conn, &cfg, &ids))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn bulk_export(
    app: AppHandle,
    ids: Vec<String>,
    dest_dir: String,
) -> Result<Vec<crate::bulk::BulkExportItem>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (cancel, emit) = begin(&app);
        let ctx = BulkCtx::new(&emit, "export", cancel);
        let cfg = crate::config::load(&app);
        let db = app.state::<Db>();
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        Ok(crate::bulk::export(
            &ctx,
            &conn,
            &cfg,
            &ids,
            std::path::Path::new(&dest_dir),
        ))
    })
    .await
    .map_err(|e| e.to_string())?
}
