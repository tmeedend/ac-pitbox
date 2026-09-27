//! The tech sheet of a car (FICHE§): the user's corrections, and the one-off
//! reading of the cars a previous version of the app never read.

use super::prelude::*;
use tauri::{Emitter, Manager};

/// Applies the corrections of one "Enregistrer" (FICHE§8).
#[tauri::command]
pub fn save_tech_sheet(db: State<Db>, id: String, edits: Vec<crate::techsheet::Edit>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::techsheet::save_user(&conn, &id, edits)
}

/// Emitted once the backfill has read at least one car: the library then
/// reloads, its spec columns having changed under it.
pub const FILLED_EVENT: &str = "techsheet://filled";

/// Reads, in the background, every car whose files have no facts yet
/// (FICHE§9.3) — the whole library the first time this version starts, then
/// nothing. The lock is held per car, never across the file reading.
pub fn spawn_backfill(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let cfg = crate::config::load(&app);
        let db = app.state::<Db>();
        let pending = {
            let Ok(conn) = db.0.lock() else { return };
            crate::techsheet::pending_cars(&conn, &cfg)
        };
        let pending = match pending {
            Ok(p) => p,
            Err(e) => {
                log::warn!("techsheet backfill: listing failed — {e}");
                return;
            }
        };
        let mut done = 0usize;
        for p in &pending {
            let facts = crate::techsheet::read_files(&p.dir, &p.mod_id, p.stock);
            let Ok(conn) = db.0.lock() else { return };
            match crate::techsheet::store_files(&conn, &p.mod_id, &p.version, &facts) {
                Ok(()) => done += 1,
                Err(e) => log::warn!("techsheet backfill: {} not stored — {e}", p.mod_id),
            }
        }
        if done > 0 {
            log::warn!("techsheet backfill: {done} car(s) read");
            if let Err(e) = app.emit(FILLED_EVENT, done) {
                log::warn!("techsheet backfill: event not sent — {e}");
            }
        }
    });
}
