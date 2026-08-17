//! Aperçu 3D des voitures (`docs/SPEC-preview-3d-kn5.md` §7.1).

use tauri::Manager;

use super::prelude::*;

/// Prépare l'aperçu 3D d'une voiture et renvoie l'URL de son `.glb`.
///
/// La conversion est bloquante et gourmande en CPU : elle part sur
/// `spawn_blocking`, jamais sur le thread principal (§7.3). Le jeton de
/// génération est pris **avant** de céder la main, pour qu'une sélection
/// arrivée entre-temps rende bien celle-ci obsolète.
#[tauri::command]
pub async fn prepare_car_preview(
    app: AppHandle,
    db: State<'_, Db>,
    state: State<'_, crate::preview::PreviewState>,
    car_id: String,
    skin_id: Option<String>,
) -> Result<crate::preview::CarPreview, String> {
    let token = state.next_generation();

    let cfg = crate::config::load(&app);
    let car_dir = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        crate::preview::car_dir(&conn, &cfg, &car_id).ok_or(crate::errors::PREVIEW_MODEL_NOT_FOUND)?
    };

    let app_for_task = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app_for_task.state::<crate::preview::PreviewState>();
        crate::preview::prepare(&app_for_task, &state, &car_dir, skin_id.as_deref(), token)
    })
    .await
    .map_err(|e| format!("tâche d'aperçu interrompue : {e}"))?
}

/// Vide le cache d'aperçus et renvoie le nombre d'octets libérés (§5.3).
#[tauri::command]
pub fn clear_preview_cache(app: AppHandle) -> Result<u64, String> {
    crate::preview::clear_cache(&app)
}
