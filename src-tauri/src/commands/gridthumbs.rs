//! Regenerated thumbnails for the library grid (GRILLE§5).
//!
//! Rendering happens in the frontend — that is where three.js lives — so the
//! round trip is: ask what this car needs (`grid_thumbnail`), convert it out of
//! the way (`prepare_grid_model`), render, hand the PNG back
//! (`save_grid_thumbnail`), drop the model (`release_grid_model`). The backend
//! keeps the two things the frontend must not decide: **where the file lands**
//! and **under what name**, since that name is what makes a thumbnail expire
//! exactly when the mod it shows changes.

use tauri::{Emitter, Manager};

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

/// The protected cars of the library under this template, in the livery
/// their card shows (GRILLE§7.1): what the in-game render can do in one go.
/// A few `stat` per car of the library, off the window thread.
#[tauri::command]
pub async fn protected_grid_cars(
    app: AppHandle,
    template: GridTemplate,
) -> Result<Vec<crate::gridthumbs::ProtectedCar>, String> {
    off_window(app, move |app| Ok(crate::gridthumbs::protected_cars(app, &template))).await
}

/// One car of a [`grid_thumbnails`] lookup.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridThumbQuery {
    car_id: String,
    skin_id: Option<String>,
}

/// [`grid_thumbnail`] for every card that just became visible, in one call
/// and **outside the render queue**: what exists on disk shows at once, and
/// the card never shows the mod's photo first only to switch to the thumbnail
/// a moment later (GRILLE§5.4). `None` for a car without a model folder.
///
/// The base first, for every car, then the disk, lock given back.
#[tauri::command]
pub async fn grid_thumbnails(
    app: AppHandle,
    cars: Vec<GridThumbQuery>,
    template: GridTemplate,
) -> Result<Vec<Option<GridThumb>>, String> {
    off_window(app, move |app| {
        let dirs: Vec<Option<std::path::PathBuf>> = {
            let cfg = crate::config::load(app);
            let db = app.state::<Db>();
            let conn = db.0.lock().map_err(|e| e.to_string())?;
            cars.iter()
                .map(|car| crate::preview::car_dir(&conn, &cfg, &car.car_id))
                .collect()
        };
        Ok(cars
            .iter()
            .zip(dirs)
            .map(|(car, dir)| {
                // No entry is an ordinary case (a folder without a usable
                // model): the card keeps its photo, as with `grid_thumbnail`.
                let stem = crate::preview::car_entry_stem(app, &dir?, &car.car_id, car.skin_id.as_deref()).ok()?;
                Some(crate::gridthumbs::look_up(
                    app,
                    &crate::gridthumbs::entry_stem(&stem, &template),
                ))
            })
            .collect())
    })
    .await
}

/// Converts one car **outside the preview cache** and returns the URL of the
/// model to render (GRILLE§5.3).
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

/// Remembers that this car will not render, and why (GRILLE§7).
///
/// `reason` is an i18n key — `errors.previewProtected` for an encrypted model,
/// which is the case this exists for. It is never retried until the mod itself
/// changes, since the fingerprint of the mod is in the entry name.
#[tauri::command]
pub fn mark_grid_thumbnail_failed(app: AppHandle, stem: String, reason: String) {
    crate::gridthumbs::mark_failed(&app, &stem, &reason);
}

/// Forgets one car thumbnail — image and failure marker alike — so the next
/// request converts again.
///
/// The manual escape hatch for a thumbnail that came out wrong without the mod
/// being at fault: a render interrupted by a lost WebGL context, a texture that
/// did not reach the page. Nothing distinguishes such an image from a good one
/// once it is written, so the judgement is the user's, and this is the button
/// behind it.
#[tauri::command]
pub fn forget_grid_thumbnail(app: AppHandle, stem: String) -> Result<bool, String> {
    crate::gridthumbs::forget(&app, &stem)
}

/// Drops the scratch model. Called once the render is done — and at startup,
/// where a brutal shutdown may have left one behind.
#[tauri::command]
pub fn release_grid_model(app: AppHandle) {
    crate::preview::release_scratch(&app);
}

/// Counters for the settings screen and the generation report (GRILLE§8.2).
#[tauri::command]
pub fn grid_thumbnail_stats(app: AppHandle) -> Result<GridThumbStats, String> {
    crate::gridthumbs::stats(&app)
}

/// Empties the store, failures included, and returns the bytes freed.
#[tauri::command]
pub fn clear_grid_thumbnails(app: AppHandle) -> Result<u64, String> {
    crate::gridthumbs::clear(&app)
}

/// Drops the images no live preset claims any more.
///
/// Called when presets are saved: nothing else would ever collect the set of a
/// preset that was deleted or edited, the store having no eviction pass by
/// design. Takes every live template — the grid binds one preset per density,
/// so two sets are legitimately alive at once.
#[tauri::command]
pub fn sweep_grid_templates(app: AppHandle, templates: Vec<GridTemplate>) -> Result<u32, String> {
    crate::gridthumbs::sweep_other_templates(&app, &templates)
}

/// The frozen thumbnail a car in the showcase shows under `preset`
/// (ESPACE§3.3), or `None` when its card shows its frozen image. A car that
/// still has its files answers `None` too: its thumbnail is `grid_thumbnail`'s.
#[tauri::command]
pub async fn frozen_grid_thumbnail(
    app: AppHandle,
    db: State<'_, Db>,
    car_id: String,
    preset: String,
    template: GridTemplate,
) -> Result<Option<String>, String> {
    let dir = {
        let cfg = crate::config::load(&app);
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        if !crate::skeleton::is_showcase(&conn, &car_id).map_err(|e| e.to_string())? {
            return Ok(None);
        }
        crate::preview::car_dir(&conn, &cfg, &car_id)
    };
    let Some(dir) = dir else { return Ok(None) };
    let fingerprint = template.fingerprint();
    tauri::async_runtime::spawn_blocking(move || {
        crate::skeleton::pick_thumb(&dir, &preset, &fingerprint).map(|p| p.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| format!("frozen thumbnail task interrupted: {e}"))
}

/// Resolves a car folder: library first, `content/` next — same rule as the
/// 3D preview, and the same lock held for as little time as possible.
/// Renders the thumbnails of protected cars **in the game**, in one session
/// (`gameshot::CARS_PER_SESSION` at most), and stores them (GRILLE§7.1);
/// returns what became of each.
///
/// Lasts a whole game session, so off the window thread; the base is locked
/// only while the cars and the studio track are made available, never while
/// the game runs. The phase goes out as `gameshot://phase` for the task panel.
#[tauri::command]
pub async fn render_in_game(
    app: AppHandle,
    requests: Vec<crate::gameshot::ShotRequest>,
) -> Result<Vec<crate::gameshot::ShotResult>, String> {
    off_window(app, move |app| {
        let cfg = crate::config::load(app);
        let ac = cfg.ac_install_path.clone().ok_or(crate::errors::AC_NOT_CONFIGURED)?;
        // Before anything is activated: a car activated under a running game
        // would be in its folder for nothing.
        if crate::launch::game_running() {
            return Err(crate::errors::GAME_RUNNING.into());
        }
        crate::launch::steam_check()?;
        if requests.is_empty() || requests.len() > crate::gameshot::CARS_PER_SESSION {
            return Err(format!("{} voitures pour une session de prise de vue", requests.len()));
        }
        {
            let db = app.state::<Db>();
            let conn = db.0.lock().map_err(|e| e.to_string())?;
            use crate::modscan::ModKind;
            for request in &requests {
                crate::launch::ensure_available(&conn, &cfg, ModKind::Car, &request.car_id)?;
            }
            crate::launch::ensure_available(&conn, &cfg, ModKind::Track, crate::gameshot::STUDIO_TRACK)?;
        }
        let cars: Vec<(String, String)> = requests
            .iter()
            .map(|r| (r.car_id.clone(), crate::gameshot::session_skin(&ac, r)))
            .collect();
        let paths = crate::gameshot::Paths {
            ac,
            ac_documents: crate::gameshot::ac_documents().ok_or("dossier Documents introuvable")?,
            app_source: app
                .path()
                .app_config_dir()
                .map_err(|e| format!("dossier de l'app indisponible : {e}"))?
                .join(crate::gameshot::SOURCE_PARENT)
                .join(crate::gameshot::APP_ID),
            thumbs: crate::gridthumbs::dir(app)?,
        };
        let launch = || crate::launch::launch_studio(&cfg, &cars, crate::gameshot::STUDIO_TRACK);
        let progress = |phase: crate::gameshot::Phase| {
            if let Err(e) = app.emit("gameshot://phase", phase) {
                log::warn!("gameshot: phase not sent ({e})");
            }
        };
        // The next session of the queue starts only once Content Manager has
        // let go of this one (SESSION§2.7): asked once the game has gone.
        crate::gameshot::run(
            &paths,
            &requests,
            &launch,
            &crate::launch::game_running,
            &progress,
            &crate::cmwatch::release_owned,
        )
    })
    .await
}

fn car_dir(app: &AppHandle, db: &State<'_, Db>, car_id: &str) -> Result<std::path::PathBuf, String> {
    let cfg = crate::config::load(app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::preview::car_dir(&conn, &cfg, car_id).ok_or_else(|| crate::errors::PREVIEW_MODEL_NOT_FOUND.to_string())
}
