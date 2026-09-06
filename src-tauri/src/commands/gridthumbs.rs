//! Regenerated thumbnails for the library grid (`docs/SPEC-grille.md` §5).
//!
//! Rendering happens in the frontend — that is where three.js lives — so the
//! round trip is: ask what this car needs (`grid_thumbnail`), convert it out of
//! the way (`prepare_grid_model`), render, hand the PNG back
//! (`save_grid_thumbnail`), drop the model (`release_grid_model`). The backend
//! keeps the two things the frontend must not decide: **where the file lands**
//! and **under what name**, since that name is what makes a thumbnail expire
//! exactly when the mod it shows changes.

use tauri::Manager;

use super::prelude::*;

use crate::gridthumbs::{GridTemplate, GridThumb, GridThumbStats};

/// What the grid already has for this car, **without converting anything**.
///
/// A few `stat` calls: the model, its date, the skin, the CSP configs. That is
/// what lets a card ask on every render without paying for it, and what tells
/// the queue whether this car is worth queuing at all.
#[tauri::command]
pub async fn grid_thumbnail(
    app: AppHandle,
    db: State<'_, Db>,
    car_id: String,
    skin_id: Option<String>,
    template: GridTemplate,
) -> Result<GridThumb, String> {
    let car_dir = car_dir(&app, &db, &car_id)?;
    let app_for_task = app.clone();
    let stem = tauri::async_runtime::spawn_blocking(move || {
        crate::preview::car_entry_stem(&app_for_task, &car_dir, &car_id, skin_id.as_deref())
    })
    .await
    .map_err(|e| format!("tâche de vignette interrompue : {e}"))??;
    Ok(crate::gridthumbs::look_up(
        &app,
        &crate::gridthumbs::entry_stem(&stem, &template),
    ))
}

/// Converts one car **outside the preview cache** and returns the URL of the
/// model to render (§5.3).
///
/// The obvious implementation would reuse `prepare_car_preview`. It must not:
/// the generation would fill the LRU cache with 312 cars the user will never
/// open, evicting the ones they actually consult — the cache would start
/// working against them. This path writes into a scratch folder that is emptied
/// before every conversion and dropped right after the render.
#[tauri::command]
pub async fn prepare_grid_model(
    app: AppHandle,
    db: State<'_, Db>,
    car_id: String,
    skin_id: Option<String>,
) -> Result<String, String> {
    let car_dir = car_dir(&app, &db, &car_id)?;
    let app_for_task = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_for_task.state::<crate::preview::PreviewState>();
        crate::preview::prepare_scratch(&app_for_task, &state, &car_dir, &car_id, skin_id.as_deref())
    })
    .await
    .map_err(|e| format!("tâche de vignette interrompue : {e}"))?
}

/// Stores the PNG the frontend just rendered, and returns its path.
#[tauri::command]
pub fn save_grid_thumbnail(app: AppHandle, stem: String, png: Vec<u8>) -> Result<String, String> {
    crate::gridthumbs::write(&app, &stem, &png).map(|path| path.to_string_lossy().into_owned())
}

/// Remembers that this car will not render, and why (§7).
///
/// `reason` is an i18n key — `errors.previewProtected` for an encrypted model,
/// which is the case this exists for. It is never retried until the mod itself
/// changes, since the fingerprint of the mod is in the entry name.
#[tauri::command]
pub fn mark_grid_thumbnail_failed(app: AppHandle, stem: String, reason: String) {
    crate::gridthumbs::mark_failed(&app, &stem, &reason);
}

/// Drops the scratch model. Called once the render is done — and at startup,
/// where a brutal shutdown may have left one behind.
#[tauri::command]
pub fn release_grid_model(app: AppHandle) {
    crate::preview::release_scratch(&app);
}

/// Counters for the settings screen and the generation report (§8.2).
#[tauri::command]
pub fn grid_thumbnail_stats(app: AppHandle) -> Result<GridThumbStats, String> {
    crate::gridthumbs::stats(&app)
}

/// Empties the store, failures included, and returns the bytes freed.
#[tauri::command]
pub fn clear_grid_thumbnails(app: AppHandle) -> Result<u64, String> {
    crate::gridthumbs::clear(&app)
}

/// Drops what a previous template produced, keeping only the one in use.
///
/// Called when a template is applied: nothing else would ever collect the old
/// set, the store having no eviction pass by design.
#[tauri::command]
pub fn sweep_grid_templates(app: AppHandle, template: GridTemplate) -> Result<u32, String> {
    crate::gridthumbs::sweep_other_templates(&app, &template)
}

/// Resolves a car folder: library first, `content/` next — same rule as the
/// 3D preview, and the same lock held for as little time as possible.
fn car_dir(app: &AppHandle, db: &State<'_, Db>, car_id: &str) -> Result<std::path::PathBuf, String> {
    let cfg = crate::config::load(app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::preview::car_dir(&conn, &cfg, car_id).ok_or_else(|| crate::errors::PREVIEW_MODEL_NOT_FOUND.to_string())
}
