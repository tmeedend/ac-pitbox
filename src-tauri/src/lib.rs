mod activation;
mod apps;
mod archive;
mod bulk;
mod cm_stats;
mod compose;
mod config;
mod detect;
mod export;
mod harmonize;
mod identity;
mod importer;
mod inspect;
mod kunos;
mod kunos_dates;
mod launch;
mod layers;
mod library;
mod maintenance;
mod modscan;
mod others;
mod overlay;
mod profiles;
mod rules;
mod shared;
mod showroom;
mod stock;
mod submods;
mod uijson;
mod weather;

use config::{AppConfig, ConfigValidation};
use detect::DetectedPaths;
use importer::ArchiveResult;
use library::{ModCard, ModDetail};
use overlay::Db;
use rules::Rules;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

// --- Configuration (§12) ----------------------------------------------------

#[tauri::command]
fn get_config(app: AppHandle) -> AppConfig {
    config::load(&app)
}

#[tauri::command]
fn save_config(app: AppHandle, config: AppConfig) -> Result<(), String> {
    config::save(&app, &config)
}

#[tauri::command]
fn validate_config(config: AppConfig) -> ConfigValidation {
    config::validate(&config)
}

#[tauri::command]
fn autodetect_paths() -> DetectedPaths {
    detect::autodetect()
}

// --- Bibliothèque & import (L1) ---------------------------------------------

#[tauri::command]
fn import_archives(
    app: AppHandle,
    db: State<Db>,
    paths: Vec<String>,
    // Décisions update/extension pour reprendre un import ambigu (§4.4). Vide au 1er appel.
    decisions: Option<Vec<importer::ImportDecision>>,
) -> Result<Vec<ArchiveResult>, String> {
    let cfg = config::load(&app);
    let rules = rules::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(importer::import_archives(&app, &conn, &cfg, &rules, &paths, &decisions.unwrap_or_default()))
}

// --- Tags & ontologie (L2) --------------------------------------------------

#[tauri::command]
fn get_rules(app: AppHandle) -> Rules {
    rules::load(&app)
}

/// Enregistre les règles et réapplique l'ontologie à toute la bibliothèque.
/// Renvoie le nombre de mods retraités.
#[tauri::command]
fn save_rules(app: AppHandle, db: State<Db>, rules: Rules) -> Result<usize, String> {
    rules::save(&app, &rules)?;
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    harmonize::harmonize_all(&conn, &rules).map_err(|e| e.to_string())
}

/// Aperçu d'impact : nombre de mods affectés par un jeu de règles candidat,
/// sans rien enregistrer (§5.4).
#[tauri::command]
fn rules_impact(db: State<Db>, rules: Rules) -> Result<usize, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    harmonize::count_affected(&conn, &rules).map_err(|e| e.to_string())
}

/// Réapplique les règles enregistrées à toute la bibliothèque.
#[tauri::command]
fn reapply_rules(app: AppHandle, db: State<Db>) -> Result<usize, String> {
    let rules = rules::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    harmonize::harmonize_all(&conn, &rules).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_favorite(db: State<Db>, id: String, favorite: bool) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    overlay::set_favorite(&conn, &id, favorite).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_manual_tags(db: State<Db>, id: String, tags: Vec<String>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    overlay::set_manual_tags(&conn, &id, &tags).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_mod_field(db: State<Db>, id: String, field: String, value: Option<String>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    overlay::set_mod_field(&conn, &id, &field, value.as_deref()).map_err(|e| e.to_string())
}

/// Import depuis des dossiers déjà décompressés (§4.5). `copy=true` préserve la
/// source, sinon déplacement adaptatif.
#[tauri::command]
fn import_folders(
    app: AppHandle,
    db: State<Db>,
    paths: Vec<String>,
    copy: bool,
    decisions: Option<Vec<importer::ImportDecision>>,
) -> Result<Vec<ArchiveResult>, String> {
    let cfg = config::load(&app);
    let rules = rules::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(importer::import_folders(&app, &conn, &cfg, &rules, &paths, copy, &decisions.unwrap_or_default()))
}

/// Analyse un dossier parent (§4.6) : classe chaque sous-dossier sans rien écrire.
#[tauri::command]
fn analyze_bulk_import(app: AppHandle, db: State<Db>, parent: String) -> Result<Vec<importer::BulkEntry>, String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    importer::analyze_bulk(&conn, &cfg, std::path::Path::new(&parent))
}

/// Exécute l'import en masse selon les décisions d'arbitrage (§4.6).
#[tauri::command]
fn execute_bulk_import(
    app: AppHandle,
    db: State<Db>,
    items: Vec<importer::BulkExecItem>,
    copy: bool,
) -> Result<Vec<ArchiveResult>, String> {
    let cfg = config::load(&app);
    let rules = rules::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(importer::execute_bulk(&app, &conn, &cfg, &rules, &items, copy))
}

/// Résout un conflit flou (§4.2) : action = "keep_both" | "replace".
#[tauri::command]
fn resolve_conflict(
    app: AppHandle,
    db: State<Db>,
    new_id: String,
    old_id: String,
    action: String,
) -> Result<(), String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    importer::resolve_conflict(&conn, &cfg, &new_id, &old_id, &action)
}

#[tauri::command]
fn list_library(app: AppHandle, db: State<Db>) -> Result<Vec<ModCard>, String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    library::list_cards(&conn, &cfg).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_mod_detail(app: AppHandle, db: State<Db>, id: String) -> Result<Option<ModDetail>, String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    library::detail(&conn, &cfg, &id).map_err(|e| e.to_string())
}

/// Ouvre le dossier réel d'un mod (voiture/circuit) dans l'explorateur.
/// Appelle directement `Opener::open_path` côté Rust (contourne le scope ACL
/// du plugin, qui refuse par défaut tout chemin non pré-autorisé) : le chemin
/// vient de notre propre résolution `entity_dir`, pas d'une entrée libre côté
/// front, donc pas besoin d'élargir la permission `opener:allow-open-path`
/// avec un scope large.
#[tauri::command]
fn open_mod_folder(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = config::load(&app);
    let path = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        library::folder_path(&conn, &cfg, &id)?
    };
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Fonctionnalités CSP effectivement détectées pour un mod (§6.4bis) : sert à
/// griser les réglages météo/saison non supportés sur l'écran de session.
#[tauri::command]
fn get_mod_csp_features(app: AppHandle, db: State<Db>, id: String) -> Result<Vec<String>, String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    library::mod_csp_features(&conn, &cfg, &id)
}

// --- Activation (L3) --------------------------------------------------------

/// Active un mod (crée la junction). `version_id` optionnel = change la version active.
#[tauri::command]
fn activate_mod(app: AppHandle, db: State<Db>, id: String, version_id: Option<String>) -> Result<(), String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    activation::activate(&conn, &cfg, &id, version_id.as_deref())
}

#[tauri::command]
fn deactivate_mod(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    activation::deactivate(&conn, &cfg, &id)
}

// --- Profils (L3) -----------------------------------------------------------

#[tauri::command]
fn list_profiles(db: State<Db>) -> Result<Vec<overlay::ProfileRow>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    overlay::list_profiles(&conn).map_err(|e| e.to_string())
}

/// Crée un profil capturant l'état actif courant.
#[tauri::command]
fn create_profile(app: AppHandle, db: State<Db>, name: String) -> Result<String, String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    profiles::create_from_active(&conn, &cfg, &name)
}

/// Applique un profil (réconciliation des junctions).
#[tauri::command]
fn apply_profile(app: AppHandle, db: State<Db>, id: String) -> Result<profiles::ApplyReport, String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    profiles::apply(&conn, &cfg, &id)
}

#[tauri::command]
fn delete_profile(db: State<Db>, id: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    overlay::delete_profile(&conn, &id).map_err(|e| e.to_string())
}

// --- Lancement (L4) ---------------------------------------------------------

/// Météos installées (pour le sélecteur de lancement).
#[tauri::command]
fn list_weather(app: AppHandle) -> Vec<String> {
    library::list_weather(&config::load(&app))
}

/// Skins d'une voiture pour la fiche détail (mod ou voiture de base, §6.3/§12bis).
#[tauri::command]
fn list_mod_skins(app: AppHandle, db: State<Db>, id: String) -> Result<Vec<library::SkinItem>, String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(library::list_mod_skins(&conn, &cfg, &id))
}

/// Stack météo détectée (CSP/SOL/vanilla) — §8.5.
#[tauri::command]
fn weather_stack(app: AppHandle) -> weather::WeatherStack {
    weather::detect_stack(&config::load(&app))
}

/// Intentions météo résolues selon la stack (dégradé gracieux).
#[tauri::command]
fn weather_options(app: AppHandle) -> Vec<weather::WeatherOption> {
    weather::options(&config::load(&app))
}

/// Température + vent implicites (air/piste/vent) pour une intention + heure
/// (lecture seule, §8.5/§8.6).
#[tauri::command]
fn weather_conditions(intent: String, hour: f32) -> weather::ImplicitConditions {
    weather::implicit_conditions(&intent, hour)
}

/// Construit le race.ini et lance la session via Content Manager (§8.3).
#[tauri::command]
fn launch_session(app: AppHandle, db: State<Db>, setup: launch::RaceSetup) -> Result<(), String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    launch::launch(&conn, &cfg, &setup)
}

/// Ouvre Content Manager sans argument (§12bis.5).
#[tauri::command]
fn open_content_manager(app: AppHandle) -> Result<(), String> {
    launch::open_content_manager(&config::load(&app))
}

/// Lance l'aperçu 3D natif (`acShowroom.exe`, distinct de Content Manager)
/// ciblé sur une voiture (+ skin optionnel). Bascule temporairement
/// `video.ini` en fenêtré, restauré automatiquement à la fermeture. Mémorise
/// le PID lancé pour une intégration ultérieure dans la page.
#[tauri::command]
fn open_native_showroom(
    app: AppHandle,
    showroom_state: State<showroom::ShowroomState>,
    car_id: String,
    skin_id: Option<String>,
) -> Result<(), String> {
    let pid = showroom::open_native_showroom(&config::load(&app), &car_id, skin_id.as_deref())?;
    *showroom_state.0.lock().map_err(|e| e.to_string())? = Some(showroom::ShowroomHandle { pid, overlay: None });
    Ok(())
}

/// Intègre la fenêtre du dernier showroom lancé dans la page, à la place de
/// la preview image (`x`/`y`/`width`/`height` en pixels physiques, relatifs
/// à la zone cliente de l'app). Passe par une fenêtre overlay séparée — voir
/// `showroom::attach` pour pourquoi un enfant direct de la fenêtre
/// principale reste invisible (WebView2 compose son rendu par-dessus).
#[tauri::command]
fn attach_native_showroom(
    app: AppHandle,
    showroom_state: State<showroom::ShowroomState>,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<(), String> {
    let pid = {
        let guard = showroom_state.0.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("aucun aperçu 3D en cours")?.pid
    };
    let overlay = showroom::attach(&app, pid, x, y, width, height)?;
    let mut guard = showroom_state.0.lock().map_err(|e| e.to_string())?;
    if let Some(handle) = guard.as_mut() {
        handle.overlay = Some(overlay);
    }
    Ok(())
}

/// Repositionne/redimensionne le showroom déjà intégré (suivi resize/scroll).
#[tauri::command]
fn reposition_native_showroom(
    app: AppHandle,
    showroom_state: State<showroom::ShowroomState>,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<(), String> {
    let overlay = {
        let guard = showroom_state.0.lock().map_err(|e| e.to_string())?;
        guard.as_ref().and_then(|h| h.overlay).ok_or("aperçu 3D pas encore intégré")?
    };
    let win = app.get_webview_window("main").ok_or("fenêtre principale introuvable")?;
    let host = win.hwnd().map_err(|e| e.to_string())?;
    showroom::reposition(host, overlay, x, y, width, height)
}

/// Ferme proprement le showroom en cours (attaché ou flottant).
#[tauri::command]
fn close_native_showroom(showroom_state: State<showroom::ShowroomState>) -> Result<(), String> {
    let handle = showroom_state.0.lock().map_err(|e| e.to_string())?.take();
    match handle {
        Some(h) => showroom::close(h.pid, h.overlay),
        None => Ok(()),
    }
}

// --- Maintenance & export (L5) ----------------------------------------------

/// Analyse mods cassés + junctions orphelines, sans rien supprimer (§9.3).
#[tauri::command]
fn maintenance_scan(app: AppHandle, db: State<Db>) -> Result<maintenance::MaintenanceReport, String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    maintenance::scan(&conn, &cfg)
}

/// Relit sur le disque les champs cache (nom, auteur, tags fichier, CSP,
/// skins/layouts) de tous les mods déjà importés, puis réapplique l'ontologie
/// (même effet que « Réappliquer les règles »). Sert à rattraper un mod dont
/// le fichier source a été corrigé/édité après import, sans le réimporter.
/// `recalc_size` (§9.4, option décochée par défaut côté UI) : recalcule aussi
/// la taille sur disque de chaque version — parcourt tous les fichiers de la
/// bibliothèque, potentiellement lent, d'où l'opt-in explicite.
/// Renvoie le nombre de mods traités.
#[tauri::command]
fn reindex_library(app: AppHandle, db: State<Db>, recalc_size: bool) -> Result<usize, String> {
    let rules = rules::load(&app);
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let n = maintenance::reindex_all(&conn, &cfg, recalc_size)?;
    harmonize::harmonize_all(&conn, &rules).map_err(|e| e.to_string())?;
    Ok(n)
}

/// Supprime un mod cassé (fichiers + junction + overlay).
#[tauri::command]
fn delete_broken_mod(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    maintenance::delete_broken(&conn, &cfg, &id)
}

/// Retire une junction orpheline (garde-fou junction).
#[tauri::command]
fn remove_orphan_junction(app: AppHandle, kind: String, id: String) -> Result<(), String> {
    maintenance::remove_orphan(&config::load(&app), &kind, &id)
}

/// Désinstalle tout un pack (§4.7) : supprime chaque mod du pack. Renvoie le nb supprimé.
#[tauri::command]
fn delete_pack(app: AppHandle, db: State<Db>, pack: String) -> Result<usize, String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    maintenance::delete_pack(&conn, &cfg, &pack)
}

/// Exporte la version active d'un mod en archive autonome dans `dest_dir` (§9.1).
#[tauri::command]
fn export_mod(app: AppHandle, db: State<Db>, id: String, dest_dir: String) -> Result<export::ExportReport, String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    export::export_mod(&conn, &cfg, &id, std::path::Path::new(&dest_dir))
}

// --- Édition groupée (§6.3bis) -----------------------------------------------

#[tauri::command]
fn bulk_set_favorite(db: State<Db>, ids: Vec<String>, favorite: bool) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    bulk::set_favorite(&conn, &ids, favorite)
}

#[tauri::command]
fn bulk_set_category(db: State<Db>, ids: Vec<String>, category: Option<String>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    bulk::set_category(&conn, &ids, category.as_deref())
}

#[tauri::command]
fn bulk_add_tag(db: State<Db>, ids: Vec<String>, tag: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    bulk::add_tag(&conn, &ids, &tag)
}

#[tauri::command]
fn bulk_remove_tag(db: State<Db>, ids: Vec<String>, tag: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    bulk::remove_tag(&conn, &ids, &tag)
}

#[tauri::command]
fn bulk_activate(app: AppHandle, db: State<Db>, ids: Vec<String>) -> Result<bulk::BulkReport, String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(bulk::activate(&conn, &cfg, &ids))
}

#[tauri::command]
fn bulk_deactivate(app: AppHandle, db: State<Db>, ids: Vec<String>) -> Result<bulk::BulkReport, String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(bulk::deactivate(&conn, &cfg, &ids))
}

/// Supprime en masse (fichiers + junction + overlay pour chacun, §9.3).
#[tauri::command]
fn bulk_delete(app: AppHandle, db: State<Db>, ids: Vec<String>) -> Result<bulk::BulkReport, String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(bulk::delete(&conn, &cfg, &ids))
}

#[tauri::command]
fn bulk_export(app: AppHandle, db: State<Db>, ids: Vec<String>, dest_dir: String) -> Result<Vec<bulk::BulkExportItem>, String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(bulk::export(&conn, &cfg, &ids, std::path::Path::new(&dest_dir)))
}

// --- Types de mods étendus (L6 / §12bis) ------------------------------------

/// Indexe le contenu de base Kunos présent dans content/ (§12bis.1).
#[tauri::command]
fn index_stock_content(app: AppHandle, db: State<Db>) -> Result<usize, String> {
    let cfg = config::load(&app);
    let rules = rules::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    stock::index_stock_content(&conn, &cfg, &rules)
}

/// Couches/extensions rattachées à une base (§4.4), pour la fiche détail.
#[tauri::command]
fn list_layers(db: State<Db>, parent_id: String) -> Result<Vec<overlay::LayerRow>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    overlay::list_layers(&conn, &parent_id).map_err(|e| e.to_string())
}

/// Supprime une couche/extension : fichiers bibliothèque + overlay, puis
/// recompose le parent (§4.4).
#[tauri::command]
fn delete_layer(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    compose::remove_layer(&conn, &cfg, &id)
}

/// Active/désactive une couche puis recompose le contenu en jeu (§4.4).
#[tauri::command]
fn set_layer_active(app: AppHandle, db: State<Db>, id: String, active: bool) -> Result<(), String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    compose::set_layer_active(&conn, &cfg, &id, active)
}

/// Réordonne une couche (up = plus prioritaire) puis recompose (§4.4).
#[tauri::command]
fn reorder_layer(app: AppHandle, db: State<Db>, id: String, direction: String) -> Result<(), String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    compose::reorder_layer(&conn, &cfg, &id, &direction)
}

/// Sous-éléments rattachés à une entité (skins/sons d'une voiture, §12bis.3).
#[tauri::command]
fn list_sub_mods(db: State<Db>, parent_id: String) -> Result<Vec<overlay::SubModRow>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    overlay::list_subs_for_parent(&conn, &parent_id).map_err(|e| e.to_string())
}

/// Tous les sous-éléments d'un type, pour la vue transversale (§12bis.3).
#[tauri::command]
fn list_subs_by_type(db: State<Db>, sub_type: String) -> Result<Vec<overlay::SubModRow>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    overlay::list_subs_by_type(&conn, &sub_type).map_err(|e| e.to_string())
}

/// Active un mod de son (bascule exclusive du sfx/, §12bis.2).
#[tauri::command]
fn activate_sound(app: AppHandle, db: State<Db>, sub_id: String) -> Result<(), String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    submods::activate_sound(&conn, &cfg, &sub_id)
}

/// Restaure le son d'origine d'une voiture (§12bis.2).
#[tauri::command]
fn restore_sound(app: AppHandle, db: State<Db>, parent_id: String) -> Result<(), String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    submods::restore_sound(&conn, &cfg, &parent_id)
}

/// Supprime proprement un sous-élément (skin/son) : junction + fichiers + overlay (§12bis.3).
#[tauri::command]
fn delete_sub_mod(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    submods::remove_sub(&conn, &cfg, &id)
}

/// Supprime proprement une app : junction + fichiers + overlay (§12bis.4).
#[tauri::command]
fn delete_app(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    apps::remove_app(&conn, &cfg, &id)
}

/// Liste les apps Python avec leur état d'activation (§12bis.4).
#[tauri::command]
fn list_apps(app: AppHandle, db: State<Db>) -> Result<Vec<apps::AppItem>, String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    apps::list_apps(&conn, &cfg)
}

/// Active une app (junction vers apps/python/, §12bis.4).
#[tauri::command]
fn activate_app(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    apps::activate_app(&conn, &cfg, &id)
}

/// Désactive une app (§12bis.4).
#[tauri::command]
fn deactivate_app(app: AppHandle, id: String) -> Result<(), String> {
    apps::deactivate_app(&config::load(&app), &id)
}

// --- Mods « autres » (§6.1bis) -----------------------------------------------

/// Liste les mods « autres » avec leurs conflits de fichiers détectés.
#[tauri::command]
fn list_other_mods(db: State<Db>) -> Result<Vec<others::OtherModCard>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    others::list_others(&conn).map_err(|e| e.to_string())
}

/// Marque/démarque un mod « autre » comme prioritaire (§6.1bis).
#[tauri::command]
fn set_other_priority(db: State<Db>, id: String, priority: bool) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    others::set_priority(&conn, &id, priority)
}

/// Active un mod « autre » par junction (§6.1bis).
#[tauri::command]
fn activate_other(app: AppHandle, db: State<Db>, id: String) -> Result<others::ActivateOtherResult, String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    others::activate_other(&conn, &cfg, &id)
}

/// Désactive un mod « autre » (§6.1bis).
#[tauri::command]
fn deactivate_other(db: State<Db>, id: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    others::deactivate_other(&conn, &id)
}

/// Supprime un mod « autre » : jonctions + fichiers + overlay (§6.1bis).
#[tauri::command]
fn delete_other_mod(db: State<Db>, id: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    others::delete_other(&conn, &id)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        // Mémorise taille/position/état agrandi de la fenêtre entre les
        // lancements (restauré automatiquement à l'ouverture, sauvegardé à la
        // fermeture et sur redimensionnement/déplacement).
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            let db_path = app.path().app_config_dir()?.join("overlay.sqlite");
            let conn = overlay::open(&db_path)?;

            // Filet de sécurité (§8.7bis) : restaure video.ini si une
            // sauvegarde traîne suite à une fermeture anormale d'une session
            // d'aperçu 3D natif précédente (process tué, crash…).
            showroom::restore_orphaned_video_ini();

            // Premier démarrage (ou contenu jamais indexé) : scan auto du
            // contenu de base Kunos, pour que les skins/sons puissent s'y
            // rattacher tout de suite (§12bis.1). Best-effort.
            let cfg = config::load(app.handle());
            if cfg.ac_install_path.is_some() && overlay::count_stock(&conn).unwrap_or(0) == 0 {
                let rules = rules::load(app.handle());
                let _ = stock::index_stock_content(&conn, &cfg, &rules);
            }

            app.manage(Db(std::sync::Mutex::new(conn)));
            app.manage(showroom::ShowroomState(std::sync::Mutex::new(None)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            validate_config,
            autodetect_paths,
            import_archives,
            import_folders,
            analyze_bulk_import,
            execute_bulk_import,
            resolve_conflict,
            list_layers,
            delete_layer,
            set_layer_active,
            reorder_layer,
            list_library,
            get_mod_detail,
            open_mod_folder,
            get_mod_csp_features,
            activate_mod,
            deactivate_mod,
            list_profiles,
            create_profile,
            apply_profile,
            delete_profile,
            list_weather,
            list_mod_skins,
            weather_stack,
            weather_options,
            weather_conditions,
            launch_session,
            open_content_manager,
            open_native_showroom,
            attach_native_showroom,
            reposition_native_showroom,
            close_native_showroom,
            maintenance_scan,
            reindex_library,
            delete_broken_mod,
            remove_orphan_junction,
            delete_pack,
            export_mod,
            bulk_set_favorite,
            bulk_set_category,
            bulk_add_tag,
            bulk_remove_tag,
            bulk_activate,
            bulk_deactivate,
            bulk_delete,
            bulk_export,
            index_stock_content,
            list_sub_mods,
            list_subs_by_type,
            activate_sound,
            restore_sound,
            delete_sub_mod,
            list_apps,
            activate_app,
            deactivate_app,
            list_other_mods,
            set_other_priority,
            activate_other,
            deactivate_other,
            delete_other_mod,
            delete_app,
            get_rules,
            save_rules,
            rules_impact,
            reapply_rules,
            set_favorite,
            set_manual_tags,
            set_mod_field,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
