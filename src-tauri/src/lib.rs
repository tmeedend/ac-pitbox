mod acpath;
mod activation;
mod apps;
mod archive;
mod backup;
mod bulk;
mod cm_stats;
mod commands;
mod compose;
mod config;
mod deploy;
mod detect;
mod errors;
mod export;
mod extras;
mod gamebackup;
mod harmonize;
mod identity;
mod import_bench;
mod import_progress;
mod importer;
mod inspect;
mod kunos;
mod kunos_dates;
mod launch;
mod layers;
mod libpath;
mod library;
mod library_columns;
mod maintenance;
mod media;
mod modscan;
mod music;
mod others;
mod overlay;
mod profiles;
mod quickdrive;
mod raceini;
mod resources;
mod rules;
mod saved_sessions;
mod session_state;
mod showroom;
mod stock;
mod submods;
#[cfg(test)]
mod testutil;
mod thumbnails;
mod ui_prefs;
mod uijson;
mod weather;

use overlay::Db;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        // Journal fichier (`%APPDATA%\com.pitbox.app\logs\`, §9.4) : seul moyen
        // de diagnostiquer un échec sur une install packagée (`.exe`, pas de
        // console). Niveau Warn : n'attrape que les échecs réels d'opérations
        // best-effort (`let _ = ...`) déjà silencieuses côté écran par design —
        // jamais un flux d'activité normale.
        .plugin(
            tauri_plugin_log::Builder::new()
                .target(tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                    file_name: Some("pitbox".into()),
                }))
                .level(log::LevelFilter::Warn)
                .build(),
        )
        // Mémorise taille/position/état agrandi de la fenêtre entre les
        // lancements (restauré automatiquement à l'ouverture, sauvegardé à la
        // fermeture et sur redimensionnement/déplacement).
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            // Sauvegarde de démarrage (§6.2/§9.4), avant toute ouverture de
            // connexion : on veut la base et les préférences exactement
            // telles que la session précédente les a laissées.
            backup::run_startup_backup(app.handle());

            let db_path = app.path().app_config_dir()?.join("overlay.sqlite");
            let conn = overlay::open(&db_path)?;

            // Filet de sécurité (§4.5.4) : un fichier du jeu remplacé par un mod
            // et que plus personne ne réclame redevient celui du jeu. Rattrape
            // une app tuée entre la sauvegarde et la pose, ou entre le retrait
            // et la restauration.
            gamebackup::restore_orphans(&conn);

            // Filet de sécurité (§8.7bis) : restaure video.ini si une
            // sauvegarde laissée par l'ancien aperçu 3D intégré traîne encore
            // (il forçait le mode fenêtré ; Pit Box n'y touche plus).
            showroom::restore_orphaned_video_ini();

            // Contenu de base Kunos jamais indexé : scan auto, pour que les
            // skins/sons puissent s'y rattacher tout de suite (§12bis.1).
            // Ne couvre PAS le premier démarrage — la config n'existe pas
            // encore à ce moment-là, l'assistant ne l'écrit qu'après. C'est
            // `save_config` qui s'en charge, dès que le dossier du jeu est
            // désigné. Best-effort ici aussi, mais tracé.
            let cfg = config::load(app.handle());
            if cfg.ac_install_path.is_some() && overlay::count_stock(&conn).unwrap_or(0) == 0 {
                let rules = rules::load(app.handle());
                if let Err(e) = stock::index_stock_content(&conn, &cfg, &rules) {
                    log::warn!("index_stock_content at startup: {e}");
                }
            }

            app.manage(Db(std::sync::Mutex::new(conn)));
            // Drapeau d'annulation d'un import en cours (§4.2bis).
            app.manage(commands::import::ImportControl::default());

            // Module musique du mode Big Picture (docs/spec-module-musique_2.md) :
            // dossiers par défaut créés au premier démarrage, peuplés du pack
            // embarqué (§16.1, `music/config.rs`), moteur audio + surveillance AC
            // démarrés pour toute la durée de vie de l'app.
            music::config::ensure_default_dirs(app.handle());
            let music_cfg = music::config::load(app.handle());
            // Préchauffe le cache d'index (§3.4/§16.3) en tâche de fond dès
            // le démarrage, pour que la première navigation Big Picture de
            // la session ne subisse pas le scan complet du dossier.
            music::index::warm(app.handle(), music_cfg.clone());
            let music_engine = music::engine::spawn(app.handle().clone(), music_cfg);
            music::watch::spawn(music_engine.clone_sender());
            app.manage(music_engine);
            app.manage(music::PreviewHandle::default());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::config::get_config,
            commands::config::save_config,
            commands::config::validate_config,
            commands::config::autodetect_paths,
            commands::config::open_developer_mode_settings,
            commands::import::import_archives,
            commands::import::import_folders,
            commands::import::analyze_bulk_import,
            commands::import::execute_bulk_import,
            commands::import::resolve_conflict,
            commands::import::cancel_import,
            commands::import::split_dropped_paths,
            commands::layers::list_layers,
            commands::layers::list_layers_by_kind,
            commands::layers::delete_layer,
            commands::layers::set_layer_active,
            commands::layers::reorder_layer,
            commands::library::list_library,
            commands::library::get_mod_detail,
            commands::library::open_mod_folder,
            commands::library::list_mod_resources,
            commands::library::list_mod_extras,
            commands::library::open_mod_resource,
            commands::library::get_mod_resource_path,
            commands::library::read_mod_resource,
            commands::library::get_mod_csp_features,
            commands::activation::activate_mod,
            commands::activation::deactivate_mod,
            commands::profiles::list_profiles,
            commands::profiles::create_profile,
            commands::profiles::apply_profile,
            commands::profiles::delete_profile,
            commands::session::list_weather,
            commands::library::list_mod_skins,
            commands::media::list_media_screenshots,
            commands::media::list_media_replays,
            commands::media::list_media_backgrounds,
            commands::media::link_media_manually,
            commands::media::open_media_folder,
            commands::media::trash_media_file,
            commands::media::get_session_background,
            commands::media::get_thumbnail,
            commands::session::weather_stack,
            commands::session::weather_options,
            commands::session::weather_conditions,
            commands::session::launch_session,
            commands::session::is_steam_running,
            commands::session::open_content_manager,
            commands::session::launch_replay,
            commands::session::open_native_showroom,
            commands::session::list_showrooms,
            commands::session_state::get_session_picks,
            commands::session_state::save_session_picks,
            commands::session_state::get_launch_state,
            commands::session_state::save_launch_state,
            commands::saved_sessions::get_saved_sessions,
            commands::saved_sessions::save_saved_sessions,
            commands::library_columns::get_library_columns,
            commands::library_columns::save_library_columns,
            commands::ui_prefs::get_ui_prefs,
            commands::ui_prefs::save_ui_prefs,
            commands::maintenance::maintenance_scan,
            commands::maintenance::reindex_library,
            commands::maintenance::delete_broken_mod,
            commands::maintenance::purge_orphan_subs,
            commands::maintenance::remove_orphan_junction,
            commands::maintenance::delete_pack,
            commands::maintenance::reinstall_from_archive,
            commands::maintenance::repair_all,
            commands::maintenance::relativize_library_paths,
            commands::maintenance::export_mod,
            commands::bulk_ops::bulk_set_favorite,
            commands::bulk_ops::bulk_set_category,
            commands::bulk_ops::bulk_add_tag,
            commands::bulk_ops::bulk_remove_tag,
            commands::bulk_ops::bulk_activate,
            commands::bulk_ops::bulk_deactivate,
            commands::bulk_ops::bulk_delete,
            commands::bulk_ops::bulk_export,
            commands::addons::index_stock_content,
            commands::addons::list_sub_mods,
            commands::addons::list_subs_by_type,
            commands::addons::sync_track_skins,
            commands::addons::list_active_track_skins,
            commands::addons::list_track_skin_options,
            commands::addons::set_track_skin_active,
            commands::addons::activate_sound,
            commands::addons::restore_sound,
            commands::addons::delete_sub_mod,
            commands::addons::list_apps,
            commands::addons::activate_app,
            commands::addons::deactivate_app,
            commands::addons::list_app_resources,
            commands::addons::open_app_resource,
            commands::addons::open_app_folder,
            commands::others::list_other_mods,
            commands::others::set_other_priority,
            commands::others::activate_other,
            commands::others::deactivate_other,
            commands::others::delete_other_mod,
            commands::addons::delete_app,
            commands::rules::get_rules,
            commands::rules::save_rules,
            commands::rules::rules_impact,
            commands::rules::reapply_rules,
            commands::library::set_favorite,
            commands::library::set_manual_tags,
            commands::library::set_mod_field,
            commands::music::get_music_config,
            commands::music::save_music_config,
            commands::music::get_default_music_folders,
            commands::music::scan_music_folder,
            commands::music::music_enter_big_picture,
            commands::music::music_exit_big_picture,
            commands::music::music_enter_menu,
            commands::music::music_enter_grid,
            commands::music::music_preview_start,
            commands::music::music_preview_stop,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
