mod acd;
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
mod driver;
mod enginesound;
mod errors;
mod export;
mod extras;
mod fmod;
mod fragment;
mod fsb5;
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
mod packs;
mod pending;
mod preview;
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
mod sun;
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
        // Protocole servant les `.glb` d'aperçu 3D depuis le cache disque
        // (docs/SPEC-preview-3d-kn5.md §7.2). Sans lui, il faudrait faire
        // transiter le modèle par l'IPC : 30 Mo de binaire deviennent ~40 Mo
        // de base64 à parser côté JS, l'UI se fige. Ici la webview fetch un
        // fichier local, sans copie intermédiaire.
        .register_uri_scheme_protocol("carpreview", |ctx, request| {
            preview::serve_request(ctx.app_handle(), &request)
        })
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
                if let Err(e) = stock::index_stock_content(&conn, &cfg, &rules, false) {
                    log::warn!("index_stock_content at startup: {e}");
                }
            }
            // Contenu de base vs mod installé hors Pit Box (§12bis.1bis) : une
            // base écrite avant cette distinction range les seconds avec les
            // premiers, ce qui autorise dessus des écritures qu'ils ne doivent
            // pas subir. Le scan complet ci-dessus ne se relancerait jamais
            // (il exige un index vide), donc cette passe — sans disque, une
            // comparaison par entrée — rattrape le classement à chaque
            // démarrage.
            match stock::reclassify_indexed_content(&conn) {
                Ok(n) if n > 0 => log::warn!("reclassified {n} indexed entries as unmanaged mods"),
                Err(e) => log::warn!("reclassify_indexed_content at startup: {e}"),
                _ => {}
            }

            // Reprise (§12bis.4) : les ajouts au jeu qui visaient l'intérieur du
            // dossier d'une app deviennent des couches de cette app. Rangés
            // avant que les couches d'app n'existent, ils créaient le dossier de
            // l'app en vrai dossier — ce qui bloquait ensuite son installation.
            // Idempotente par construction (plus aucun chemin `apps/<lang>/…`
            // ne subsiste après coup), donc sans drapeau à mémoriser.
            match extras::migrate_app_extras_to_layers(&conn, &cfg) {
                0 => {}
                n => log::warn!("migrated {n} app extra tree(s) to app layers"),
            }

            // L'harmonisation (§5) est calculée une fois à l'import puis
            // stockée : faire évoluer le moteur ne change donc rien à ce qui
            // est déjà en base, et personne ne devinerait qu'il faut rouvrir
            // l'écran Règles et réenregistrer pour la recalculer. Une passe au
            // démarrage, **une seule fois par version de moteur**, rattrape les
            // bases écrites par la précédente. Marqueur posé seulement en cas
            // de succès, et rien n'est tenté sans bibliothèque accessible :
            // sinon un disque externe non monté ferait passer un balayage à
            // vide pour un rattrapage fait, et la base resterait périmée pour
            // toujours.
            let lib_ready = cfg.library_path.as_deref().is_some_and(|p| p.is_dir());
            let engine = rules::ENGINE_VERSION.to_string();
            let stamped = overlay::get_meta(&conn, overlay::META_ENGINE_VERSION).unwrap_or(None);
            if lib_ready && stamped.as_deref() != Some(engine.as_str()) {
                let rules = rules::load(app.handle());
                match harmonize::harmonize_all(&conn, &cfg, &rules) {
                    Ok(n) => {
                        log::warn!("re-harmonized {n} mods for engine v{engine}");
                        if let Err(e) = overlay::set_meta(&conn, overlay::META_ENGINE_VERSION, &engine) {
                            log::warn!("stamping engine version failed, catch-up will run again: {e}");
                        }
                    }
                    Err(e) => log::warn!("re-harmonize at startup: {e}"),
                }
            }

            app.manage(Db(std::sync::Mutex::new(conn)));
            // Drapeau d'annulation d'un import en cours (§4.2bis).
            app.manage(commands::import::ImportControl::default());
            app.manage(commands::bulk_ops::BulkControl::default());

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

            // Aperçu 3D des voitures : jeton de génération + créneau unique de
            // conversion (docs/SPEC-preview-3d-kn5.md §7.3).
            app.manage(preview::PreviewState::default());
            // Thread propriétaire du système FMOD (§4.3). Rien n'est chargé
            // ici : les DLL du jeu ne sont touchées qu'à la première écoute,
            // donc une install sans Assetto Corsa ne paie rien.
            #[cfg(windows)]
            app.manage(fmod::engine::spawn());

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
            commands::bulk_ops::cancel_bulk,
            commands::import::split_dropped_paths,
            commands::import::list_pending_folders,
            commands::import::resolve_pending_folder,
            commands::import::read_pending_document,
            commands::layers::list_layers,
            commands::layers::list_layers_by_kind,
            commands::layers::delete_layer,
            commands::layers::set_layer_active,
            commands::layers::reorder_layer,
            commands::layers::list_layer_files,
            commands::layers::open_layer_folder,
            commands::library::list_library,
            commands::library::get_mod_detail,
            commands::library::open_mod_folder,
            commands::library::list_mod_resources,
            commands::library::list_mod_extras,
            commands::library::force_mod_extra,
            commands::library::open_mod_resource,
            commands::library::list_import_decisions,
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
            commands::session::track_sun,
            commands::session::launch_session,
            commands::session::is_steam_running,
            commands::session::open_content_manager,
            commands::session::launch_replay,
            commands::session::open_native_showroom,
            commands::session::list_showrooms,
            commands::preview::prepare_car_preview,
            commands::preview::list_driver_choices,
            commands::preview::list_driver_bodies,
            commands::preview::prepare_driver_preview,
            commands::preview::clear_preview_cache,
            commands::preview::preview_cache_size,
            commands::preview::set_preview_cache_cap,
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
            commands::maintenance::delete_mod_version,
            commands::maintenance::profiles_using_version,
            commands::maintenance::purge_orphan_subs,
            commands::maintenance::remove_orphan_junction,
            commands::maintenance::delete_pack,
            commands::packs::get_pack_detail,
            commands::packs::list_pack_resources,
            commands::packs::open_pack_resource,
            commands::packs::get_pack_resource_path,
            commands::packs::read_pack_resource,
            commands::packs::list_pack_extras,
            commands::maintenance::reinstall_from_archive,
            commands::maintenance::repair_all,
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
            commands::addons::audition_engine_sound,
            #[cfg(windows)]
            commands::addons::audition_engine_native,
            #[cfg(windows)]
            commands::addons::set_audition_rev,
            #[cfg(windows)]
            commands::addons::set_audition_pedal,
            #[cfg(windows)]
            commands::addons::set_audition_listener,
            #[cfg(windows)]
            commands::addons::set_audition_showcase,
            #[cfg(windows)]
            commands::addons::stop_audition_native,
            commands::addons::sound_detail,
            commands::addons::set_sound_author,
            commands::addons::list_sound_resources,
            commands::addons::open_sound_resource,
            commands::addons::get_sound_resource_path,
            commands::addons::read_sound_resource,
            commands::addons::restore_sound,
            commands::addons::delete_sub_mod,
            commands::addons::list_apps,
            commands::addons::activate_app,
            commands::addons::deactivate_app,
            commands::addons::get_app_resource_path,
            commands::addons::read_app_resource,
            commands::addons::list_app_extras,
            commands::addons::list_app_resources,
            commands::addons::open_app_resource,
            commands::addons::open_app_folder,
            commands::others::list_other_mods,
            commands::others::set_other_priority,
            commands::others::activate_other,
            commands::others::deactivate_other,
            commands::others::delete_other_mod,
            commands::others::open_other_mod_folder,
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
