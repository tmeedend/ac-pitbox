//! Mod updates (§4.7): the CUP registry check, one update's details, and the
//! download of its archive. The import that follows is the ordinary one
//! (`commands::import`), called by the frontend with the downloaded path.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::prelude::*;
use tauri::{Emitter, Manager};

/// Cancellation flag of the download in progress. One at a time, like the
/// import it feeds: the frontend runs download and import as one sequence.
#[derive(Default)]
pub struct UpdateDownloadControl(pub Arc<AtomicBool>);

/// Emitted as `update:progress`. Mirrored by `UpdateProgress` in
/// `src/lib/library/modUpdates.svelte.ts`.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProgress {
    id: String,
    received: u64,
    total: Option<u64>,
}

/// Every mod of the library with a newer version in the registry. Empty,
/// without a request, when the check is switched off (`mod_updates_online`).
///
/// The library is read first and the lock released before the request:
/// holding the SQLite mutex across a network call would freeze every screen
/// for as long as the registry takes to answer.
#[tauri::command]
pub async fn check_mod_updates(app: AppHandle) -> Result<Vec<crate::cup::ModUpdate>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cfg = crate::config::load(&app);
        if !cfg.prefs.mod_updates_online {
            return Ok(Vec::new());
        }
        let mods = {
            let db = app.state::<Db>();
            let conn = db.0.lock().map_err(|e| e.to_string())?;
            crate::overlay::list_mods(&conn).map_err(|e| e.to_string())?
        };
        let registry = crate::cup::fetch_registry().map_err(str::to_string)?;
        Ok(crate::cup::find_updates(&mods, &registry))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Changelog, author and information page of one update.
#[tauri::command]
pub async fn mod_update_details(kind: String, id: String) -> Result<crate::cup::UpdateDetails, String> {
    tauri::async_runtime::spawn_blocking(move || crate::cup::fetch_details(&kind, &id).map_err(str::to_string))
        .await
        .map_err(|e| e.to_string())?
}

/// Downloads one update's archive to the temp folder. `async` +
/// `spawn_blocking` like the import (§4.2): a download of several hundred
/// megabytes on the IPC thread would hold back the very `update:progress`
/// events that say it is moving.
#[tauri::command]
pub async fn download_mod_update(
    app: AppHandle,
    kind: String,
    id: String,
) -> Result<crate::cup::DownloadOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cancel = {
            let control = app.state::<UpdateDownloadControl>();
            control.0.store(false, Ordering::Relaxed);
            control.0.clone()
        };
        // At most ten events a second, like the import (§4.2bis): a fast
        // connection delivers thousands of chunks, and each one would be an
        // IPC message. The last chunk always goes out, so the bar ends full.
        let mut last_emit: Option<Instant> = None;
        let mut on_progress = |received: u64, total: Option<u64>| {
            let due = last_emit.is_none_or(|t| t.elapsed() >= Duration::from_millis(100));
            if due || total == Some(received) {
                last_emit = Some(Instant::now());
                let progress = UpdateProgress {
                    id: id.clone(),
                    received,
                    total,
                };
                if let Err(e) = app.emit("update:progress", progress) {
                    log::warn!("update:progress not delivered — {e}");
                }
            }
            !cancel.load(Ordering::Relaxed)
        };
        crate::cup::download_update(&kind, &id, &std::env::temp_dir(), &mut on_progress)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Stops the download in progress; the partial file is removed.
#[tauri::command]
pub fn cancel_mod_update_download(control: State<UpdateDownloadControl>) {
    control.0.store(true, Ordering::Relaxed);
}

/// Deletes a downloaded archive once the import has taken what it needed.
/// Refuses any path `download_mod_update` did not produce (`cup::discard_download`).
#[tauri::command]
pub fn discard_mod_update_download(path: String) -> Result<(), String> {
    crate::cup::discard_download(std::path::Path::new(&path), &std::env::temp_dir())
}
