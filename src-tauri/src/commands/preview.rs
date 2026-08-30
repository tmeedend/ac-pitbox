//! Aperçu 3D des voitures (`docs/SPEC-preview-3d-kn5.md` §7.1).

use tauri::Manager;

use super::prelude::*;

/// Prépare l'aperçu 3D d'une voiture et renvoie l'URL de son `.glb`.
///
/// `driver` porte le réglage du frontend, où il vit (`ui_prefs.json`) : le
/// backend ne lit jamais ce fichier, dont le schéma appartient à l'UI. `None`
/// = pas de pilote ; `Some(angle)` = pilote, volant tourné de cet angle en
/// degrés. Les deux font partie de l'identité de l'entrée de cache — le
/// pilote est greffé dans le `.glb` et sa pose y est cuite — donc changer
/// l'un ou l'autre convertit une fois, après quoi les versions déjà vues se
/// rendent instantanément (§4.6).
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
    driver: Option<f32>,
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
        crate::preview::prepare(
            &app_for_task,
            &state,
            &car_dir,
            &car_id,
            skin_id.as_deref(),
            driver,
            token,
        )
    })
    .await
    .map_err(|e| format!("tâche d'aperçu interrompue : {e}"))?
}

/// Vide le cache d'aperçus et renvoie le nombre d'octets libérés (§5.3).
#[tauri::command]
pub fn clear_preview_cache(app: AppHandle) -> Result<u64, String> {
    crate::preview::clear_cache(&app)
}

/// Octets actuellement occupés par le cache d'aperçus (§5.3).
#[tauri::command]
pub fn preview_cache_size(app: AppHandle) -> Result<u64, String> {
    crate::preview::cache_usage(&app)
}

/// Fixe le plafond du cache et l'applique tout de suite (§5.3).
///
/// Le réglage vit dans `ui_prefs.json`, dont le schéma appartient au
/// frontend : c'est donc lui qui pousse la valeur ici, au démarrage et à
/// chaque changement, plutôt que le backend qui irait la lire.
#[tauri::command]
pub fn set_preview_cache_cap(
    app: AppHandle,
    state: State<'_, crate::preview::PreviewState>,
    bytes: u64,
) -> Result<(), String> {
    crate::preview::set_cache_cap(&app, &state, bytes)
}
