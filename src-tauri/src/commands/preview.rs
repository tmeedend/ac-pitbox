//! Aperçu 3D des voitures (`docs/SPEC-preview-3d-kn5.md` PREVIEW§7.1).

use tauri::Manager;

use super::prelude::*;

/// Prépare l'aperçu 3D d'une voiture et renvoie l'URL de son `.glb`.
///
/// **Aucun angle de braquage ici** : roues, volant et bras du pilote tournent
/// à l'affichage, à partir de ce que la conversion écrit dans le `.glb` — un
/// pivot et un axe pour les premiers, un squelette et une animation pour les
/// derniers. Le régler ne demande donc aucune conversion.
///
/// `driver` porte les réglages du frontend, où ils vivent (`ui_prefs.json`) :
/// le backend ne lit jamais ce fichier, dont le schéma appartient à l'UI.
/// `None` = pas de pilote ; sinon la tenue imposée. Tout cela fait partie de
/// l'identité de l'entrée de cache — le pilote est greffé dans le `.glb` et sa
/// pose y est cuite — donc changer l'un ou l'autre convertit une fois, après
/// quoi les versions déjà vues se rendent instantanément (PREVIEW§4.6).
///
/// La conversion est bloquante et gourmande en CPU : elle part sur
/// `spawn_blocking`, jamais sur le thread principal (PREVIEW§7.3). Le jeton de
/// génération est pris **avant** de céder la main, pour qu'une sélection
/// arrivée entre-temps rende bien celle-ci obsolète.
#[tauri::command]
pub async fn prepare_car_preview(
    app: AppHandle,
    db: State<'_, Db>,
    state: State<'_, crate::preview::PreviewState>,
    car_id: String,
    skin_id: Option<String>,
    driver: Option<crate::driver::DriverView>,
) -> Result<crate::preview::CarPreview, String> {
    let token = state.next_generation();

    let cfg = crate::config::load(&app);
    let car_dir = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        // The model left with the files (ESPACE§6): the detail page does not
        // ask, this is the net behind it.
        crate::skeleton::guard_mod(&conn, &car_id)?;
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
            &crate::preview::PreviewRequest {
                skin_id: skin_id.as_deref(),
                driver: driver.as_ref(),
            },
            token,
        )
    })
    .await
    .map_err(|e| format!("tâche d'aperçu interrompue : {e}"))?
}

/// Les tenues de pilote qui marcheront sur le mannequin de cette voiture
/// (`docs/SPEC-ecran-pilote.md` PREVIEW§6).
///
/// Rendu au frontend pour peupler les trois galeries de l'écran Pilote. La
/// compatibilité n'est pas devinée ni déduite d'autres voitures : un dossier
/// est retenu s'il contient une texture que le mannequin utilise comme couleur
/// de base — voir `driver::choices`.
///
/// `body` porte le corps substitué, quand l'utilisateur en impose un : c'est
/// lui qui commande les trois listes (PILOTE§1.3), pas celui que la voiture nomme.
///
/// Lit un KN5 de quatorze mégaoctets, donc `spawn_blocking` comme la
/// conversion, même si le parsing seul se compte en millisecondes.
#[tauri::command]
pub async fn list_driver_choices(
    app: AppHandle,
    db: State<'_, Db>,
    car_id: String,
    body: Option<String>,
) -> Result<Option<crate::driver::DriverChoices>, String> {
    let cfg = crate::config::load(&app);
    let Some(ac_root) = cfg.ac_install_path.clone() else {
        return Ok(None);
    };
    let car_dir = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        // The model left with the files (ESPACE§6): the detail page does not
        // ask, this is the net behind it.
        crate::skeleton::guard_mod(&conn, &car_id)?;
        crate::preview::car_dir(&conn, &cfg, &car_id).ok_or(crate::errors::PREVIEW_MODEL_NOT_FOUND)?
    };
    tauri::async_runtime::spawn_blocking(move || crate::driver::choices(&ac_root, &car_dir, &car_id, body.as_deref()))
        .await
        .map_err(|e| format!("tâche de tenues interrompue : {e}"))
}

/// Prépare le mannequin seul, habillé, pour le plateau d'essayage de l'écran
/// Pilote (`docs/SPEC-ecran-pilote.md` PREVIEW§5.1).
///
/// Le pilote y est **sans habitacle autour**, mais posé comme sa voiture le
/// pose : seul l'ancrage sur les yeux tombe, l'assise et l'animation de
/// braquage restent — sans elles le mannequin garde sa pose de modélisation.
/// La voiture est donc aussi la source du corps (`driver3d.ini`) et de la
/// tenue par défaut (`skin.ini` de la livrée), d'où `car_id` et `skin_id`.
///
/// `Ok(None)` — jamais une erreur — quand Assetto Corsa n'est pas configuré ou
/// que le corps n'est pas installé : le plateau retombe alors sur
/// l'échantillon plat, et la galerie reste entièrement utilisable (PILOTE§12.4).
#[tauri::command]
pub async fn prepare_driver_preview(
    app: AppHandle,
    db: State<'_, Db>,
    state: State<'_, crate::preview::PreviewState>,
    car_id: String,
    skin_id: Option<String>,
    outfit: crate::driver::OutfitOverride,
) -> Result<Option<crate::preview::DriverPreview>, String> {
    // Le plateau **prend** un jeton : une nouvelle tenue demandée rend
    // obsolète la conversion en cours, sinon parcourir la galerie vite
    // laisserait une file de conversions orphelines.
    let token = Some(state.next_generation());
    let cfg = crate::config::load(&app);
    let Some(ac_root) = cfg.ac_install_path.clone() else {
        return Ok(None);
    };
    let car_dir = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        // The model left with the files (ESPACE§6): the detail page does not
        // ask, this is the net behind it.
        crate::skeleton::guard_mod(&conn, &car_id)?;
        crate::preview::car_dir(&conn, &cfg, &car_id).ok_or(crate::errors::PREVIEW_MODEL_NOT_FOUND)?
    };
    convert_graft(app, token, move || {
        let skin_dir = kn5_gltf::resolve_skin(&car_dir, skin_id.as_deref());
        crate::driver::standalone(&ac_root, &car_dir, &car_id, skin_dir.as_deref(), &outfit)
    })
    .await
}

/// The mannequin for a body's **thumbnail** in the gallery (SESSION§5).
///
/// The body alone, seated by the reference car and in its own textures
/// ([`crate::driver::thumbnail_body`]): nothing about the session, so one
/// conversion in a body's life rather than one per car picked. No generation
/// token either — a thumbnail neither supersedes the fitting stage nor another
/// thumbnail.
#[tauri::command]
pub async fn prepare_body_preview(
    app: AppHandle,
    body: String,
) -> Result<Option<crate::preview::DriverPreview>, String> {
    let Some(ac_root) = crate::config::load(&app).ac_install_path else {
        return Ok(None);
    };
    convert_graft(app, None, move || crate::driver::thumbnail_body(&ac_root, &body)).await
}

/// The thumbnail already rendered for this body, or `None` when it has to be
/// produced (SESSION§5).
///
/// **Converts nothing**: it only recomputes the mannequin's entry name — a few
/// `stat`s — and looks for the PNG. That is what lets every cell ask without
/// paying anything when the answer is yes.
#[tauri::command]
pub async fn body_thumbnail(app: AppHandle, body: String) -> Result<Option<String>, String> {
    Ok(body_entry_stem(&app, &body)
        .and_then(|stem| crate::preview::body_thumb(&app, &stem))
        .map(|path| path.to_string_lossy().into_owned()))
}

/// Stores the thumbnail the frontend has just rendered, and returns its path.
///
/// The rendering happens in the frontend — three.js lives there — but it does
/// not choose where the file lands nor its name: a thumbnail's identity is
/// the mannequin's cache entry, so it is computed here with the rest.
#[tauri::command]
pub async fn save_body_thumbnail(app: AppHandle, body: String, png: Vec<u8>) -> Result<Option<String>, String> {
    let Some(stem) = body_entry_stem(&app, &body) else {
        return Ok(None);
    };
    crate::preview::write_body_thumb(&app, &stem, &png).map(|path| Some(path.to_string_lossy().into_owned()))
}

/// A body thumbnail's entry name, without converting anything.
fn body_entry_stem(app: &AppHandle, body: &str) -> Option<String> {
    let ac_root = crate::config::load(app).ac_install_path?;
    crate::driver::thumbnail_body(&ac_root, body).map(|graft| crate::preview::driver_entry_stem(&graft))
}

/// Grafts, then converts, off the async thread: building the graft reads a
/// `data.acd` and the conversion reads KN5s.
async fn convert_graft(
    app: AppHandle,
    token: Option<u64>,
    make_graft: impl FnOnce() -> Option<kn5_gltf::DriverGraft> + Send + 'static,
) -> Result<Option<crate::preview::DriverPreview>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(graft) = make_graft() else {
            return Ok(None);
        };
        let state = app.state::<crate::preview::PreviewState>();
        crate::preview::prepare_driver(&app, &state, &graft, token).map(Some)
    })
    .await
    .map_err(|e| format!("driver task interrupted: {e}"))?
}

/// Les mannequins installés, pour la galerie des corps (SESSION§5).
///
/// Liste vide — jamais une erreur — quand Assetto Corsa n'est pas configuré :
/// l'écran Pilote reste ouvrable, il n'a simplement rien à proposer.
///
/// Parcourt tout `content/driver/`, soit une cinquantaine de KN5 de quinze
/// mégaoctets : `spawn_blocking` obligatoire.
#[tauri::command]
pub async fn list_driver_bodies(app: AppHandle) -> Result<crate::driver::BodyList, String> {
    let Some(ac_root) = crate::config::load(&app).ac_install_path else {
        return Ok(crate::driver::BodyList {
            bodies: Vec::new(),
            discarded: 0,
        });
    };
    tauri::async_runtime::spawn_blocking(move || crate::driver::bodies(&ac_root))
        .await
        .map_err(|e| format!("tâche de corps interrompue : {e}"))
}

/// Vide le cache d'aperçus et renvoie le nombre d'octets libérés (PREVIEW§5.3).
#[tauri::command]
pub fn clear_preview_cache(app: AppHandle) -> Result<u64, String> {
    crate::preview::clear_cache(&app)
}

/// Octets actuellement occupés par le cache d'aperçus (PREVIEW§5.3).
#[tauri::command]
pub fn preview_cache_size(app: AppHandle) -> Result<u64, String> {
    crate::preview::cache_usage(&app)
}

/// Fixe le plafond du cache et l'applique tout de suite (PREVIEW§5.3).
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
