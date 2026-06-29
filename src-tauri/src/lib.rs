mod activation;
mod archive;
mod config;
mod detect;
mod export;
mod harmonize;
mod identity;
mod importer;
mod inspect;
mod kunos;
mod launch;
mod library;
mod maintenance;
mod modscan;
mod overlay;
mod profiles;
mod rules;
mod shared;
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
fn import_archives(app: AppHandle, db: State<Db>, paths: Vec<String>) -> Result<Vec<ArchiveResult>, String> {
    let cfg = config::load(&app);
    let rules = rules::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(importer::import_archives(&app, &conn, &cfg, &rules, &paths))
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
) -> Result<Vec<ArchiveResult>, String> {
    let cfg = config::load(&app);
    let rules = rules::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(importer::import_folders(&app, &conn, &cfg, &rules, &paths, copy))
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

/// Liste le contenu installé (`car` | `track`) pour les sélecteurs de lancement.
#[tauri::command]
fn list_installed(app: AppHandle, kind: String) -> Result<Vec<library::InstalledItem>, String> {
    let cfg = config::load(&app);
    let k = if kind == "track" { modscan::ModKind::Track } else { modscan::ModKind::Car };
    Ok(library::list_installed(&cfg, k))
}

/// Météos installées (pour le sélecteur de lancement).
#[tauri::command]
fn list_weather(app: AppHandle) -> Vec<String> {
    library::list_weather(&config::load(&app))
}

/// Skins (+ miniatures) d'une voiture installée (§8.6).
#[tauri::command]
fn list_skins(app: AppHandle, car_id: String) -> Vec<library::SkinItem> {
    library::list_car_skins(&config::load(&app), &car_id)
}

/// Skins de la version active d'un mod, lus dans la bibliothèque (fiche détail §6.3).
#[tauri::command]
fn list_mod_skins(db: State<Db>, id: String) -> Result<Vec<library::SkinItem>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(library::list_mod_skins(&conn, &id))
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

/// Température implicite (air/piste) pour une intention + heure (lecture seule).
#[tauri::command]
fn weather_temp(intent: String, hour: f32) -> weather::ImplicitTemp {
    weather::implicit_temp(&intent, hour)
}

/// Construit le race.ini et lance la session via Content Manager (§8.3).
#[tauri::command]
fn launch_session(app: AppHandle, db: State<Db>, setup: launch::RaceSetup) -> Result<(), String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    launch::launch(&conn, &cfg, &setup)
}

// --- Maintenance & export (L5) ----------------------------------------------

/// Analyse mods cassés + junctions orphelines, sans rien supprimer (§9.3).
#[tauri::command]
fn maintenance_scan(app: AppHandle, db: State<Db>) -> Result<maintenance::MaintenanceReport, String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    maintenance::scan(&conn, &cfg)
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

/// Exporte la version active d'un mod en archive autonome dans `dest_dir` (§9.1).
#[tauri::command]
fn export_mod(app: AppHandle, db: State<Db>, id: String, dest_dir: String) -> Result<export::ExportReport, String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    export::export_mod(&conn, &cfg, &id, std::path::Path::new(&dest_dir))
}

// --- Types de mods étendus (L6 / §12bis) ------------------------------------

/// Indexe le contenu de base Kunos présent dans content/ (§12bis.1).
#[tauri::command]
fn index_stock_content(app: AppHandle, db: State<Db>) -> Result<usize, String> {
    let cfg = config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    stock::index_stock_content(&conn, &cfg)
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let db_path = app.path().app_config_dir()?.join("overlay.sqlite");
            let conn = overlay::open(&db_path)?;
            app.manage(Db(std::sync::Mutex::new(conn)));
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
            list_library,
            get_mod_detail,
            activate_mod,
            deactivate_mod,
            list_profiles,
            create_profile,
            apply_profile,
            delete_profile,
            list_installed,
            list_weather,
            list_skins,
            list_mod_skins,
            weather_stack,
            weather_options,
            weather_temp,
            launch_session,
            maintenance_scan,
            delete_broken_mod,
            remove_orphan_junction,
            export_mod,
            index_stock_content,
            list_sub_mods,
            list_subs_by_type,
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
