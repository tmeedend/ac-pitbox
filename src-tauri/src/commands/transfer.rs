//! Exporting and importing a library (EXPORT§). Every command runs off the
//! window: an export walks every folder of the library, an import writes
//! hundreds of skeletons.

use std::path::Path;

use super::prelude::*;
use crate::transfer::{self, Part, Places};
use tauri::Manager;

/// What the transfer needs to know of this machine, resolved here: the
/// business module never sees the `AppHandle`.
fn places(app: &AppHandle) -> Result<Places, String> {
    Ok(Places {
        config_dir: app.path().app_config_dir().map_err(|e| e.to_string())?,
        presets_dir: crate::sessionpreset::own_dir(app),
        app_version: app.package_info().version.to_string(),
    })
}

/// Counts and weight of each part, before anything is written (EXPORT§7.2).
#[tauri::command]
pub async fn transfer_estimate(app: AppHandle) -> Result<transfer::Estimate, String> {
    off_window(app, |app| {
        let cfg = crate::config::load(app);
        transfer::estimate(&app.state::<Db>(), &cfg, &places(app)?)
    })
    .await
}

#[tauri::command]
pub async fn transfer_export(app: AppHandle, path: String, parts: Vec<Part>) -> Result<transfer::ExportReport, String> {
    off_window(app, move |app| {
        let cfg = crate::config::load(app);
        transfer::export(&app.state::<Db>(), &cfg, &places(app)?, &parts, Path::new(&path))
    })
    .await
}

/// The manifest of an export, and whether this installation can take it.
#[tauri::command]
pub async fn transfer_inspect(app: AppHandle, path: String) -> Result<transfer::Inspection, String> {
    off_window(app, move |app| {
        let cfg = crate::config::load(app);
        let places = places(app)?;
        let db = app.state::<Db>();
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        transfer::inspect(&conn, &cfg, &places, Path::new(&path))
    })
    .await
}

#[tauri::command]
pub async fn transfer_import(app: AppHandle, path: String, parts: Vec<Part>) -> Result<transfer::ImportReport, String> {
    off_window(app, move |app| {
        let cfg = crate::config::load(app);
        let rules = crate::rules::load(app);
        transfer::import(
            &app.state::<Db>(),
            &cfg,
            &places(app)?,
            &rules,
            Path::new(&path),
            &parts,
        )
    })
    .await
}
