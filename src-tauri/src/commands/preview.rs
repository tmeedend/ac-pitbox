//! Aperçu 3D des voitures (`docs/SPEC-preview-3d-kn5.md` §7.1).

use tauri::Manager;

use super::prelude::*;

/// Prépare l'aperçu 3D d'une voiture et renvoie l'URL de son `.glb`.
///
/// `steer` est l'angle du volant en degrés : il tourne les roues avant, le
/// volant du poste de pilotage et — quand il y en a un — les bras du pilote,
/// que l'animation de la voiture pose au même angle. Il vaut donc avec ou sans
/// mannequin, d'où sa place hors de `driver`.
///
/// `driver` porte les réglages du frontend, où ils vivent (`ui_prefs.json`) :
/// le backend ne lit jamais ce fichier, dont le schéma appartient à l'UI.
/// `None` = pas de pilote ; sinon la tenue imposée. Tout cela fait partie de
/// l'identité de l'entrée de cache — le pilote est greffé dans le `.glb` et sa
/// pose y est cuite — donc changer l'un ou l'autre convertit une fois, après
/// quoi les versions déjà vues se rendent instantanément (§4.6).
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
    steer: Option<f32>,
    driver: Option<crate::driver::DriverView>,
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
            &crate::preview::PreviewRequest {
                skin_id: skin_id.as_deref(),
                steer_degrees: steer.unwrap_or(0.0),
                driver: driver.as_ref(),
            },
            token,
        )
    })
    .await
    .map_err(|e| format!("tâche d'aperçu interrompue : {e}"))?
}

/// Les tenues de pilote qui marcheront sur le mannequin de cette voiture
/// (`docs/SPEC-ecran-pilote.md` §6).
///
/// Rendu au frontend pour peupler les trois galeries de l'écran Pilote. La
/// compatibilité n'est pas devinée ni déduite d'autres voitures : un dossier
/// est retenu s'il contient une texture que le mannequin utilise comme couleur
/// de base — voir `driver::choices`.
///
/// `body` porte le corps substitué, quand l'utilisateur en impose un : c'est
/// lui qui commande les trois listes (§1.3), pas celui que la voiture nomme.
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
        crate::preview::car_dir(&conn, &cfg, &car_id).ok_or(crate::errors::PREVIEW_MODEL_NOT_FOUND)?
    };
    tauri::async_runtime::spawn_blocking(move || crate::driver::choices(&ac_root, &car_dir, &car_id, body.as_deref()))
        .await
        .map_err(|e| format!("tâche de tenues interrompue : {e}"))
}

/// Prépare le mannequin seul, habillé, pour le plateau d'essayage de l'écran
/// Pilote (`docs/SPEC-ecran-pilote.md` §5.1).
///
/// Le pilote y est **sans habitacle autour**, mais posé comme sa voiture le
/// pose : seul l'ancrage sur les yeux tombe, l'assise et l'animation de
/// braquage restent — sans elles le mannequin garde sa pose de modélisation.
/// La voiture est donc aussi la source du corps (`driver3d.ini`) et de la
/// tenue par défaut (`skin.ini` de la livrée), d'où `car_id` et `skin_id`.
///
/// `Ok(None)` — jamais une erreur — quand Assetto Corsa n'est pas configuré ou
/// que le corps n'est pas installé : le plateau retombe alors sur
/// l'échantillon plat, et la galerie reste entièrement utilisable (§12.4).
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
    driver_glb(app, db, car_id, skin_id, outfit, token).await
}

/// Le même mannequin, pour la **vignette** d'un corps dans la galerie (§9.1).
///
/// Deux différences avec le plateau, et une seule raison derrière les deux :
/// il y en a quarante-cinq à produire. Pas de jeton de génération, donc — une
/// vignette ne périme pas le plateau et ne se périme pas elle-même — et une
/// tenue vide, pour que toutes les vignettes d'une même voiture montrent les
/// corps dans la même tenue et se comparent.
#[tauri::command]
pub async fn prepare_body_preview(
    app: AppHandle,
    db: State<'_, Db>,
    car_id: String,
    skin_id: Option<String>,
    body: String,
) -> Result<Option<crate::preview::DriverPreview>, String> {
    let outfit = crate::driver::OutfitOverride {
        model: Some(body),
        ..Default::default()
    };
    driver_glb(app, db, car_id, skin_id, outfit, None).await
}

/// La vignette déjà rendue pour ce corps, ou `None` s'il faut la produire
/// (§9.1).
///
/// **Ne convertit rien** : elle ne fait que recalculer le nom d'entrée du
/// mannequin — quelques `stat` sur des fichiers — et regarder si le PNG est
/// là. C'est ce qui permet de le demander pour chaque case sans payer quoi que
/// ce soit quand la réponse est oui.
#[tauri::command]
pub async fn body_thumbnail(
    app: AppHandle,
    db: State<'_, Db>,
    car_id: String,
    skin_id: Option<String>,
    body: String,
) -> Result<Option<String>, String> {
    let stem = body_entry_stem(&app, &db, &car_id, skin_id.as_deref(), &body)?;
    Ok(stem
        .and_then(|stem| crate::preview::body_thumb(&app, &stem))
        .map(|path| path.to_string_lossy().into_owned()))
}

/// Range la vignette que le frontend vient de rendre, et renvoie son chemin.
///
/// Le rendu se fait côté frontend — c'est là que vit three.js — mais il ne
/// choisit pas où le fichier atterrit ni sous quel nom : l'identité d'une
/// vignette est celle de l'entrée de cache du mannequin, donc elle se calcule
/// ici, avec le reste.
#[tauri::command]
pub async fn save_body_thumbnail(
    app: AppHandle,
    db: State<'_, Db>,
    car_id: String,
    skin_id: Option<String>,
    body: String,
    png: Vec<u8>,
) -> Result<Option<String>, String> {
    let Some(stem) = body_entry_stem(&app, &db, &car_id, skin_id.as_deref(), &body)? else {
        return Ok(None);
    };
    crate::preview::write_body_thumb(&app, &stem, &png).map(|path| Some(path.to_string_lossy().into_owned()))
}

/// Le nom d'entrée du mannequin d'un corps, sans conversion.
fn body_entry_stem(
    app: &AppHandle,
    db: &State<'_, Db>,
    car_id: &str,
    skin_id: Option<&str>,
    body: &str,
) -> Result<Option<String>, String> {
    let cfg = crate::config::load(app);
    let Some(ac_root) = cfg.ac_install_path.clone() else {
        return Ok(None);
    };
    let car_dir = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        crate::preview::car_dir(&conn, &cfg, car_id).ok_or(crate::errors::PREVIEW_MODEL_NOT_FOUND)?
    };
    let skin_dir = kn5_gltf::resolve_skin(&car_dir, skin_id);
    let outfit = crate::driver::OutfitOverride {
        model: Some(body.to_string()),
        ..Default::default()
    };
    Ok(
        crate::driver::standalone(&ac_root, &car_dir, car_id, skin_dir.as_deref(), &outfit)
            .map(|graft| crate::preview::driver_entry_stem(&graft)),
    )
}

/// Le tronc commun des deux : résoudre la voiture, greffer, convertir.
async fn driver_glb(
    app: AppHandle,
    db: State<'_, Db>,
    car_id: String,
    skin_id: Option<String>,
    outfit: crate::driver::OutfitOverride,
    token: Option<u64>,
) -> Result<Option<crate::preview::DriverPreview>, String> {
    let cfg = crate::config::load(&app);
    let Some(ac_root) = cfg.ac_install_path.clone() else {
        return Ok(None);
    };
    let car_dir = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        crate::preview::car_dir(&conn, &cfg, &car_id).ok_or(crate::errors::PREVIEW_MODEL_NOT_FOUND)?
    };

    let app_for_task = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let skin_dir = kn5_gltf::resolve_skin(&car_dir, skin_id.as_deref());
        let Some(graft) = crate::driver::standalone(&ac_root, &car_dir, &car_id, skin_dir.as_deref(), &outfit) else {
            return Ok(None);
        };
        let state = app_for_task.state::<crate::preview::PreviewState>();
        crate::preview::prepare_driver(&app_for_task, &state, &graft, token).map(Some)
    })
    .await
    .map_err(|e| format!("tâche de pilote interrompue : {e}"))?
}

/// Les mannequins installés, pour la galerie des corps (§9.1).
///
/// Liste vide — jamais une erreur — quand Assetto Corsa n'est pas configuré :
/// l'écran Pilote reste ouvrable, il n'a simplement rien à proposer.
///
/// Parcourt tout `content/driver/`, soit une cinquantaine de KN5 de quinze
/// mégaoctets : `spawn_blocking` obligatoire.
#[tauri::command]
pub async fn list_driver_bodies(app: AppHandle) -> Result<Vec<crate::driver::BodyOption>, String> {
    let Some(ac_root) = crate::config::load(&app).ac_install_path else {
        return Ok(Vec::new());
    };
    tauri::async_runtime::spawn_blocking(move || crate::driver::bodies(&ac_root))
        .await
        .map_err(|e| format!("tâche de corps interrompue : {e}"))
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
