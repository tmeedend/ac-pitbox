//! Facades of the game folder screen (DOSSIER§9.1). All of them read: the scan
//! reads the disk and a snapshot of the database, the queries read the index in
//! memory.

use std::sync::Arc;

use tauri::{Emitter, Manager};

use super::prelude::*;
use crate::gamestate::{
    self, ChildrenPage, Detail, Filters, NodeId, OwnerOption, SearchLimits, SearchResults, Snapshot, Status, Store,
};

fn index(store: &Store) -> Result<Arc<gamestate::Index>, String> {
    store
        .index()
        .ok_or_else(|| crate::errors::GAME_FOLDER_NOT_SCANNED.to_string())
}

#[tauri::command]
pub fn game_folder_status(store: State<Store>) -> Status {
    store.status()
}

/// Scans the game folder in the background (DOSSIER§5.2): `spawn_blocking`,
/// never the main thread - which is what froze the general repair. Progress
/// goes out as `gamefolder://progress`, the end as `gamefolder://done`. A scan
/// already running is not doubled: the call returns at once and the screen
/// follows the events of the one in flight.
#[tauri::command]
pub async fn scan_game_folder(app: AppHandle) -> Result<(), String> {
    if !app.state::<Store>().begin() {
        return Ok(());
    }
    let cfg = crate::config::load(&app);
    let snapshot = {
        let db = app.state::<Db>();
        let conn = db.0.lock().map_err(|e| e.to_string());
        conn.and_then(|conn| Snapshot::load(&conn, &cfg, gamestate::game_generation()))
    };
    let snapshot = match snapshot {
        Ok(s) => s,
        Err(e) => {
            app.state::<Store>().end(Some(e.clone()));
            let _ = app.emit("gamefolder://done", ());
            return Err(e);
        }
    };
    let worker = app.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let store = worker.state::<Store>();
        gamestate::run_scan(&store, snapshot, &|p| {
            let _ = worker.emit("gamefolder://progress", p);
        });
    })
    .await
    .map_err(|e| e.to_string());
    let error = outcome.as_ref().err().cloned();
    if let Some(e) = &error {
        log::warn!("game folder scan failed: {e}");
    }
    app.state::<Store>().end(error);
    let _ = app.emit("gamefolder://done", ());
    outcome
}

#[tauri::command]
pub fn game_folder_children(
    store: State<Store>,
    node: NodeId,
    filters: Filters,
    members: bool,
    offset: usize,
    limit: usize,
) -> Result<ChildrenPage, String> {
    Ok(index(&store)?.children(node, &filters, members, offset, limit))
}

#[tauri::command]
pub fn game_folder_detail(store: State<Store>, node: NodeId) -> Result<Option<Detail>, String> {
    Ok(index(&store)?.detail(node))
}

#[tauri::command]
pub fn game_folder_search(
    store: State<Store>,
    query: String,
    filters: Filters,
    limits: SearchLimits,
) -> Result<SearchResults, String> {
    Ok(index(&store)?.search(&query, &filters, limits))
}

/// What the Provenance chip offers (DOSSIER§6.2).
#[tauri::command]
pub fn game_folder_owners(store: State<Store>) -> Result<Vec<OwnerOption>, String> {
    Ok(index(&store)?.owner_options())
}

/// The chain of nodes down to `path` (relative to the game folder), or `null`
/// when the current index does not have it.
#[tauri::command]
pub fn game_folder_reveal(store: State<Store>, path: String) -> Result<Option<Vec<NodeId>>, String> {
    Ok(index(&store)?.reveal(&path))
}

/// "Show in Explorer" (DOSSIER§8.2): the path itself - for a junction, the
/// junction and not its target -, or its nearest ancestor on the disk when it
/// is missing.
#[tauri::command]
pub fn show_game_path(app: AppHandle, store: State<Store>, node: NodeId) -> Result<(), String> {
    let path = index(&store)?
        .explorer_target(node)
        .ok_or_else(|| crate::errors::GAME_FOLDER_NOT_SCANNED.to_string())?;
    app.opener().reveal_item_in_dir(path).map_err(|e| e.to_string())
}
