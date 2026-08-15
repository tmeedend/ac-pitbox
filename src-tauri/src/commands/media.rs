//! Commandes de l'onglet Médias (§6.1) : screenshots/replays personnels
//! rattachés par nom de fichier, backgrounds officiels CSP, et fond photo de
//! l'écran de réglages (§6.2/§9.3).

use std::path::{Path, PathBuf};

use super::prelude::*;

const SCREENSHOT_KIND: &str = "SCREENSHOT";
const REPLAY_KIND: &str = "REPLAY";

/// Ids de l'« autre » type de la bibliothèque (circuits pour une voiture, et
/// inversement) — sert à `media::list_screenshots`/`list_replays` à retrouver
/// la contrepartie dans un nom de fichier (§6.1).
fn counterpart_ids(conn: &rusqlite::Connection, own_kind: &str) -> Result<std::collections::HashSet<String>, String> {
    let other_kind = if own_kind == "Track" { "Car" } else { "Track" };
    Ok(crate::overlay::list_mod_ids_by_kind(conn, other_kind)
        .map_err(|e| e.to_string())?
        .into_iter()
        .collect())
}

#[tauri::command]
pub fn list_media_screenshots(db: State<Db>, id: String) -> Result<Vec<crate::media::ScreenshotFile>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let m = crate::overlay::get_mod(&conn, &id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::MOD_NOT_FOUND)?;
    let counterparts = counterpart_ids(&conn, &m.kind)?;
    let auto = crate::media::list_screenshots(&id, &counterparts);
    let manual = crate::overlay::list_media_links(&conn, &id, SCREENSHOT_KIND).map_err(|e| e.to_string())?;
    Ok(crate::media::merge_screenshot_links(auto, &manual))
}

#[tauri::command]
pub fn list_media_replays(db: State<Db>, id: String) -> Result<Vec<crate::media::ReplayFile>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let m = crate::overlay::get_mod(&conn, &id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::MOD_NOT_FOUND)?;
    let counterparts = counterpart_ids(&conn, &m.kind)?;
    let auto = crate::media::list_replays(&id, &counterparts);
    let manual = crate::overlay::list_media_links(&conn, &id, REPLAY_KIND).map_err(|e| e.to_string())?;
    Ok(crate::media::merge_replay_links(auto, &manual))
}

/// Backgrounds officiels CSP (§6.1, onglet dispo seulement pour un circuit) —
/// `id` est directement l'id du circuit, `layout_id` le layout sélectionné sur
/// la fiche (`None` = tous les backgrounds du circuit).
#[tauri::command]
pub fn list_media_backgrounds(
    app: AppHandle,
    id: String,
    layout_id: Option<String>,
) -> Result<Vec<crate::media::BackgroundFile>, String> {
    let cfg = crate::config::load(&app);
    let ac_install = cfg.ac_install_path.ok_or(crate::errors::AC_NOT_CONFIGURED)?;
    Ok(crate::media::list_backgrounds(&ac_install, &id, layout_id.as_deref()))
}

/// Rattachement manuel d'un screenshot/replay (§6.1) : repli quand le
/// matching automatique par nom de fichier ne trouve pas l'entité.
#[tauri::command]
pub fn link_media_manually(db: State<Db>, id: String, kind: String, file_path: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::overlay::add_media_link(&conn, &file_path, &id, &kind).map_err(|e| e.to_string())
}

/// Envoie un screenshot/replay à la corbeille Windows (§6.1) — bouton corbeille
/// des galeries et touche Suppr. Récupérable par l'utilisateur depuis la
/// corbeille, c'est ce qui justifie l'absence de confirmation côté UI.
#[tauri::command]
pub fn trash_media_file(db: State<Db>, path: String) -> Result<(), String> {
    crate::media::trash_file(Path::new(&path))?;
    // Best-effort : le fichier est déjà parti, échouer ici n'annulerait rien.
    // Mais une ligne `media_links` orpheline referait apparaître le média
    // supprimé à la prochaine ouverture de l'onglet — d'où la trace.
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    if let Err(e) = crate::overlay::remove_media_links_for_path(&conn, &path) {
        log::warn!("media link cleanup failed for {path}: {e}");
    }
    Ok(())
}

/// Ouvre `screens/` ou `replay/` (Documents AC) dans l'explorateur — bouton
/// « Ouvrir le dossier » de l'onglet Médias, et point de départ pratique pour
/// retrouver un fichier à rattacher manuellement.
#[tauri::command]
pub fn open_media_folder(app: AppHandle, kind: String) -> Result<(), String> {
    let base = crate::media::documents_ac_dir().ok_or(crate::errors::DOCUMENTS_NOT_FOUND)?;
    let sub = if kind == REPLAY_KIND { "replay" } else { "screens" };
    app.opener()
        .open_path(base.join(sub).display().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Miniature mise en cache d'un screenshot/background (§6.1) — voir
/// `thumbnails.rs`. `max_dim` par défaut couvre confortablement les cartes de
/// galerie actuelles (150–180px CSS, avec marge pour les écrans haute
/// densité).
///
/// `async` + `spawn_blocking` (§4.2) : la galerie déclenche un appel par
/// fichier en parallèle (`MediaScreenshots.svelte`), parfois des dizaines à la
/// fois sur des captures AC en pleine résolution jeu — décoder/redimensionner/
/// réencoder en JPEG directement sur le thread IPC synchrone y bloquait tout
/// le runtime async de Tauri (mêmes threads que les autres commandes),
/// figeant l'app entière le temps du lot au lieu de juste la galerie.
#[tauri::command]
pub async fn get_thumbnail(app: AppHandle, path: PathBuf, max_dim: Option<u32>) -> Result<PathBuf, String> {
    tauri::async_runtime::spawn_blocking(move || crate::thumbnails::get_or_create(&app, &path, max_dim.unwrap_or(320)))
        .await
        .map_err(|e| e.to_string())?
}

/// Fond photo de l'écran de réglages (§6.2/§9.3) : combo exact → même circuit
/// → background officiel → `None` (fond neutre côté front).
#[tauri::command]
pub fn get_session_background(
    app: AppHandle,
    car_id: String,
    track_id: String,
    layout_id: Option<String>,
) -> Result<Option<String>, String> {
    let cfg = crate::config::load(&app);
    // Chemin par défaut (vide) si l'install AC n'est pas configurée :
    // `list_backgrounds` ne trouve alors aucun dossier (`is_dir()` faux),
    // l'étape « background officiel » de la chaîne de repli est simplement
    // sautée — jamais bloquant pour cet écran, cohérent avec le reste de
    // l'onglet Médias (§6.1, agrément non critique).
    let ac_install = cfg.ac_install_path.unwrap_or_default();
    Ok(crate::media::resolve_session_background(
        &ac_install,
        &car_id,
        &track_id,
        layout_id.as_deref(),
    ))
}
