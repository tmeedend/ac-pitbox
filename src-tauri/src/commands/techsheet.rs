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

/// The report of the last reading of the cars' files (FICHE§9.3), until
/// closed.
#[tauri::command]
pub fn get_techsheet_report(app: AppHandle) -> Result<Option<crate::techsheet::report::Report>, String> {
    let dir = crate::rules::config_dir(&app)?;
    Ok(crate::techsheet::report::load(&dir))
}

#[tauri::command]
pub fn dismiss_techsheet_report(app: AppHandle) -> Result<(), String> {
    let dir = crate::rules::config_dir(&app)?;
    crate::techsheet::report::dismiss(&dir)
}

/// Emitted once the backfill has read at least one car: the library then
/// reloads, its spec columns having changed under it, and the report shows.
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
        let mut report = crate::techsheet::report::Report::default();
        for p in &pending {
            let facts = crate::techsheet::read_files(&p.stack.dirs, &p.mod_id, p.stock);
            let Ok(conn) = db.0.lock() else { return };
            if let Err(e) = crate::techsheet::store_pending(&conn, p, &facts, &mut report) {
                log::warn!("techsheet backfill: {} not stored — {e}", p.mod_id);
            }
        }
        let done = report.cars;
        if done > 0 {
            log::warn!("techsheet backfill: {done} car(s) read");
            // Said only when something moved: a reading that changes no
            // column (a new car, a fix elsewhere) is no news.
            if !report.is_empty() {
                match crate::rules::config_dir(&app) {
                    Ok(dir) => {
                        if let Err(e) = crate::techsheet::report::save(&dir, &report) {
                            log::warn!("techsheet backfill: report not written — {e}");
                        }
                    }
                    Err(e) => log::warn!("techsheet backfill: report not written — {e}"),
                }
            }
            if let Err(e) = app.emit(FILLED_EVENT, done) {
                log::warn!("techsheet backfill: event not sent — {e}");
            }
        }
    });
}
