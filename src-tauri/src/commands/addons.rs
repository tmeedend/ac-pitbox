//! Commandes des add-ons (§12bis) : contenu de base Kunos, skins, skins de
//! circuit, sons et apps Python/Lua.

use super::prelude::*;

/// Indexe le contenu de base Kunos présent dans content/ (§12bis.1).
///
/// `reset_user_edits` : efface aussi ce que l'utilisateur a saisi sur ce
/// contenu (§9.3bis). Absent = préserver, le seul défaut acceptable — c'est le
/// contraire qui était fait, et il perdait un renommage sans prévenir.
#[tauri::command]
pub fn index_stock_content(app: AppHandle, db: State<Db>, reset_user_edits: Option<bool>) -> Result<usize, String> {
    let cfg = crate::config::load(&app);
    let rules = crate::rules::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::stock::index_stock_content(&conn, &cfg, &rules, reset_user_edits.unwrap_or(false))
}

/// Sous-éléments rattachés à une entité (skins/sons d'une voiture, §12bis.3).
#[tauri::command]
pub fn list_sub_mods(db: State<Db>, parent_id: String) -> Result<Vec<crate::overlay::SubModRow>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::overlay::list_subs_for_parent(&conn, &parent_id).map_err(|e| e.to_string())
}

/// Tous les sous-éléments d'un type, pour la vue transversale (§12bis.3) —
/// taille sur disque incluse (regroupements pesés côté UI).
#[tauri::command]
pub fn list_subs_by_type(
    app: AppHandle,
    db: State<Db>,
    sub_type: String,
) -> Result<Vec<crate::overlay::SubModRow>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::submods::list_by_type_sized(&conn, &cfg, &sub_type).map_err(|e| e.to_string())
}

/// Reconnaît les skins de circuit fournis avec le contenu initial du mod
/// (§8, lecture live du disque, best-effort) — à appeler avant de lister
/// les skins d'un circuit pour qu'ils y apparaissent.
#[tauri::command]
pub fn sync_track_skins(app: AppHandle, db: State<Db>, track_id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::submods::sync_bundled_track_skins(&conn, &cfg, &track_id);
    Ok(())
}

/// Skins de circuit actuellement actifs (§8, plusieurs possibles).
#[tauri::command]
pub fn list_active_track_skins(db: State<Db>, track_id: String) -> Result<Vec<String>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(crate::submods::list_active_track_skins(&conn, &track_id))
}

/// Skins de circuit avec image de prévisualisation résolue, pour le
/// sélecteur multi-choix de la barre latérale (§8).
#[tauri::command]
pub fn list_track_skin_options(
    app: AppHandle,
    db: State<Db>,
    track_id: String,
) -> Result<Vec<crate::submods::TrackSkinOption>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(crate::submods::list_track_skin_options(&conn, &cfg, &track_id))
}

/// Active/désactive un skin de circuit (§8, pas exclusif).
#[tauri::command]
pub fn set_track_skin_active(
    app: AppHandle,
    db: State<Db>,
    track_id: String,
    skin_name: String,
    active: bool,
) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::submods::set_track_skin_active(&conn, &cfg, &track_id, &skin_name, active)
}

/// Active un mod de son (bascule exclusive du sfx/, §12bis.2).
#[tauri::command]
pub fn activate_sound(app: AppHandle, db: State<Db>, sub_id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::submods::activate_sound(&conn, &cfg, &sub_id)
}

/// Écoute une entrée de la liste « Son du moteur » sans rien déployer.
///
/// `sub_id` à `null` désigne le son d'origine. Ne touche jamais à `content/` :
/// c'est `activate_sound` qui déploie, et les deux ne doivent pas se confondre.
#[tauri::command]
pub fn audition_engine_sound(
    app: AppHandle,
    db: State<Db>,
    parent_id: String,
    sub_id: Option<String>,
) -> Result<crate::enginesound::EngineClip, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::enginesound::audition(&conn, &cfg, &parent_id, sub_id.as_deref())
}

/// Écoute une entrée par le **vrai moteur FMOD du jeu** (§4.1).
///
/// Renvoie une erreur — jamais montrée — quand le chemin natif n'est pas
/// disponible : pas d'AC configuré, DLL introuvables, aucun événement moteur
/// dans les `GUIDs.txt`. C'est le signal pour l'appelant de retomber sur
/// `audition_engine_sound`, qui décode un échantillon lui-même. Les deux
/// chemins coexistent, le repli n'est pas une panne.
///
/// Comme `audition_engine_sound`, ne touche jamais à `content/`.
#[cfg(windows)]
#[tauri::command]
pub fn audition_engine_native(
    app: AppHandle,
    db: State<Db>,
    engine: State<crate::fmod::engine::FmodEngineHandle>,
    parent_id: String,
    sub_id: Option<String>,
    interior: Option<bool>,
    rev: Option<f32>,
) -> Result<NativeAudition, String> {
    let cfg = crate::config::load(&app);
    let view = if interior.unwrap_or(false) {
        crate::fmod::guids::EngineView::Interior
    } else {
        crate::fmod::guids::EngineView::Exterior
    };
    let target = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        crate::enginesound::native_target(&conn, &cfg, &parent_id, sub_id.as_deref(), view)?
    };
    let rev_ceiling = target.rev_ceiling;
    let play = engine.play(crate::fmod::engine::PlayRequest {
        ac_root: target.ac_root,
        bank: target.bank,
        guid: target.guid,
        event_path: target.event_path,
        // 900 tr/min au départ : le régime de ralenti exact vit dans
        // `data/engine.ini`, donc dans un `data.acd` chiffré la plupart du
        // temps. Le curseur rend la question sans objet (§4.4).
        rev: rev.unwrap_or(DEFAULT_REV),
        throttle: 0.0,
        rev_ceiling,
    })?;
    Ok(NativeAudition {
        play,
        rev_floor: crate::enginesound::REV_FLOOR,
        rev_ceiling,
        rev_start: rev
            .unwrap_or(DEFAULT_REV)
            .clamp(crate::enginesound::REV_FLOOR, rev_ceiling),
    })
}

/// Régime de départ, faute de mieux : le vrai ralenti est dans un `data.acd`
/// chiffré, et le curseur rend la question sans objet (§4.4).
#[cfg(windows)]
const DEFAULT_REV: f32 = 900.0;

/// Ce que l'écoute native renvoie à l'écran : le compte rendu du thread, plus
/// la plage du curseur, qui vient de la voiture et non de l'événement.
#[cfg(windows)]
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeAudition {
    #[serde(flatten)]
    pub play: crate::fmod::engine::PlayReport,
    pub rev_floor: f32,
    pub rev_ceiling: f32,
    pub rev_start: f32,
}

/// Règle le régime de l'écoute en cours. Sans effet si rien ne joue.
#[cfg(windows)]
#[tauri::command]
pub fn set_audition_rev(engine: State<crate::fmod::engine::FmodEngineHandle>, rev: f32) {
    engine.set_rev(rev);
}

/// Règle l'accélérateur de l'écoute en cours (0 = lâcher de gaz, 1 = pleine
/// charge). Sans effet si rien ne joue.
#[cfg(windows)]
#[tauri::command]
pub fn set_audition_throttle(engine: State<crate::fmod::engine::FmodEngineHandle>, throttle: f32) {
    engine.set_throttle(throttle);
}

/// Déplace l'oreille autour de la voiture, en degrés et en mètres.
///
/// Branché sur l'orbite de l'aperçu 3D : l'événement moteur d'AC est spatialisé
/// et expose `Event Cone Angle` en paramètre automatique, donc c'est FMOD qui
/// change le timbre entre l'avant et l'arrière — on ne fait que dire où on se
/// trouve. Envoyé sans réponse : ça suit une caméra qu'on fait tourner.
#[cfg(windows)]
#[tauri::command]
pub fn set_audition_listener(
    engine: State<crate::fmod::engine::FmodEngineHandle>,
    azimuth: f32,
    elevation: f32,
    distance: f32,
) {
    engine.set_listener(crate::fmod::engine::Listener {
        azimuth,
        elevation,
        distance,
    });
}

/// Lance ou coupe les coups d'accélérateur (§6bis) : quelques secondes de
/// ralenti, puis une rafale de brefs coups de gaz, en boucle.
#[cfg(windows)]
#[tauri::command]
pub fn set_audition_showcase(engine: State<crate::fmod::engine::FmodEngineHandle>, on: bool) {
    engine.set_showcase(on);
}

/// Coupe l'écoute native. Sans effet si rien ne joue.
#[cfg(windows)]
#[tauri::command]
pub fn stop_audition_native(engine: State<crate::fmod::engine::FmodEngineHandle>) {
    engine.stop();
}

/// Fiche d'un mod de son : ce qu'il vise, ce qu'il pèse, et ce que son bank
/// contient réellement (§8). Tout est lu à la demande — rien de ce qui décrit
/// un fichier n'est mémorisé en base, il peut changer sous nos pieds.
#[tauri::command]
pub fn sound_detail(app: AppHandle, db: State<Db>, sub_id: String) -> Result<crate::enginesound::SoundDetail, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::enginesound::detail(&conn, &cfg, &sub_id)
}

/// Saisit l'auteur d'un mod de son. Vide efface.
#[tauri::command]
pub fn set_sound_author(db: State<Db>, sub_id: String, author: Option<String>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::overlay::set_sub_author(&conn, &sub_id, author.as_deref()).map_err(|e| e.to_string())
}

// Les cinq commandes de ressources ci-dessous sont le quatrième exemplaire du
// même quintuplet (mods, apps, packs, sons). La duplication est assumée pour
// rester cohérente avec l'existant plutôt que d'entamer ici un refactor non
// demandé ; le regroupement — un jeu de commandes prenant la portée en
// paramètre, ou un `ResourcesBlock` prenant ses fonctions plutôt qu'un nom de
// source — mérite son propre commit.

/// Fichiers annexes d'un mod de son (§4.5.2) : notice, changelog livrés à côté
/// du dossier `sfx/`.
#[tauri::command]
pub fn list_sound_resources(
    app: AppHandle,
    db: State<Db>,
    sub_id: String,
) -> Result<Vec<crate::resources::ResourceFile>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(crate::resources::list_resources(&crate::enginesound::resources_dir(
        &conn, &cfg, &sub_id,
    )?))
}

/// Ouvre une annexe de mod de son avec l'application par défaut de l'OS.
#[tauri::command]
pub fn open_sound_resource(app: AppHandle, db: State<Db>, sub_id: String, rel_path: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let dir = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        crate::enginesound::resources_dir(&conn, &cfg, &sub_id)?
    };
    let path = crate::resources::resolve_resource_path(&dir, &rel_path)?;
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Chemin absolu d'une annexe de mod de son, pour l'afficher via `asset://`.
#[tauri::command]
pub fn get_sound_resource_path(
    app: AppHandle,
    db: State<Db>,
    sub_id: String,
    rel_path: String,
) -> Result<String, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let dir = crate::enginesound::resources_dir(&conn, &cfg, &sub_id)?;
    Ok(crate::resources::resolve_resource_path(&dir, &rel_path)?
        .display()
        .to_string())
}

/// Contenu brut d'une annexe de mod de son, pour la prévisualisation.
#[tauri::command]
pub fn read_sound_resource(
    app: AppHandle,
    db: State<Db>,
    sub_id: String,
    rel_path: String,
) -> Result<tauri::ipc::Response, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let dir = crate::enginesound::resources_dir(&conn, &cfg, &sub_id)?;
    Ok(tauri::ipc::Response::new(crate::resources::read_resource(
        &dir, &rel_path,
    )?))
}

/// Restaure le son d'origine d'une voiture (§12bis.2).
#[tauri::command]
pub fn restore_sound(app: AppHandle, db: State<Db>, parent_id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::submods::restore_sound(&conn, &cfg, &parent_id)
}

/// Supprime proprement un sous-élément (skin/son) : junction + fichiers + overlay (§12bis.3).
#[tauri::command]
pub fn delete_sub_mod(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::submods::remove_sub(&conn, &cfg, &id)
}

/// Supprime proprement une app : junction + fichiers + overlay (§12bis.4).
#[tauri::command]
pub fn delete_app(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::apps::remove_app(&conn, &cfg, &id)
}

/// Liste les apps (Python ou Lua/CSP) avec leur état d'activation (§12bis.4).
#[tauri::command]
pub fn list_apps(app: AppHandle, db: State<Db>) -> Result<Vec<crate::apps::AppItem>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::apps::list_apps(&conn, &cfg)
}

/// Active une app (junction vers apps/python/ ou apps/lua/ selon le langage
/// détecté, §12bis.4).
#[tauri::command]
pub fn activate_app(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::apps::activate_app(&conn, &cfg, &id)
}

/// Désactive une app (§12bis.4).
#[tauri::command]
pub fn deactivate_app(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::apps::deactivate_app(&conn, &cfg, &id)
}

/// Ouvre le dossier bibliothèque d'une app dans l'explorateur (même schéma
/// que `open_mod_folder` : chemin résolu côté serveur, pas de scope ACL large
/// à ouvrir sur le plugin opener).
#[tauri::command]
pub fn open_app_folder(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let path = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        crate::apps::app_folder_path(&conn, &cfg, &id)?
    };
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Liste les fichiers annexes d'une app (§4.5.2, même mécanisme que les mods
/// voiture/circuit) — lue en direct sur disque à chaque appel, jamais
/// mémorisée en base.
#[tauri::command]
pub fn list_app_resources(app: AppHandle, id: String) -> Result<Vec<crate::resources::ResourceFile>, String> {
    let cfg = crate::config::load(&app);
    let library = cfg.library_path.ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    Ok(crate::resources::list_resources(&crate::resources::resources_dir_for(
        &library,
        "apps",
        &[&id],
    )))
}

/// Ouvre un fichier annexe d'une app avec l'application par défaut de l'OS
/// (§4.5.2). `rel_path` résolu et validé côté serveur (garde-fou anti-traversée).
#[tauri::command]
pub fn open_app_resource(app: AppHandle, id: String, rel_path: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let library = cfg.library_path.ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    let dir = crate::resources::resources_dir_for(&library, "apps", &[&id]);
    let path = crate::resources::resolve_resource_path(&dir, &rel_path)?;
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Chemin absolu d'une ressource d'app, pour l'afficher via `asset://`
/// (§4.5.2, prévisualisation des images) — jumeau de
/// `get_mod_resource_path`. Le front ne construit jamais ce chemin lui-même :
/// il passe par ici pour bénéficier du garde-fou anti-traversée.
#[tauri::command]
pub fn get_app_resource_path(app: AppHandle, id: String, rel_path: String) -> Result<String, String> {
    let cfg = crate::config::load(&app);
    let library = cfg.library_path.ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    let dir = crate::resources::resources_dir_for(&library, "apps", &[&id]);
    Ok(crate::resources::resolve_resource_path(&dir, &rel_path)?
        .display()
        .to_string())
}

/// Contenu brut d'une ressource d'app, pour la prévisualisation dans la fiche
/// (§4.5.2) — jumeau de `read_mod_resource`, mêmes garde-fous et même plafond.
#[tauri::command]
pub fn read_app_resource(app: AppHandle, id: String, rel_path: String) -> Result<tauri::ipc::Response, String> {
    let cfg = crate::config::load(&app);
    let library = cfg.library_path.ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    let dir = crate::resources::resources_dir_for(&library, "apps", &[&id]);
    Ok(tauri::ipc::Response::new(crate::resources::read_resource(
        &dir, &rel_path,
    )?))
}

/// Ce qu'une app installe hors de son dossier `apps/<langue>/<id>` (§4.5.3) —
/// onglet « Ajouts au jeu » de sa fiche. Une app en a autant qu'une voiture :
/// configs CSP, textures, fichiers de `cfg/` livrés à côté de son dossier.
#[tauri::command]
pub fn list_app_extras(app: AppHandle, db: State<Db>, id: String) -> Result<Vec<crate::extras::ExtraFile>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(crate::extras::list(&conn, &cfg, crate::extras::OwnerKind::App, &id))
}
