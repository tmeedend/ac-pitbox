mod acd;
mod acpath;
mod acreplay;
mod activation;
mod apps;
mod archive;
mod attach;
mod backup;
mod brands;
mod bulk;
mod cardata;
mod catalog_update;
mod cm_stats;
mod cmimport;
mod commands;
mod compose;
mod config;
mod cup;
mod deploy;
mod detect;
mod driver;
mod driverapply;
mod electronics;
mod enginesound;
mod errors;
mod export;
mod extras;
mod fmod;
mod fragment;
mod fsb5;
mod gamebackup;
mod gamestate;
mod gridthumbs;
mod harmonize;
mod http;
mod identity;
mod import_bench;
mod import_progress;
mod importer;
mod inspect;
mod inventory;
mod kunos;
mod kunos_dates;
mod launch;
mod layers;
mod libpath;
mod library;
mod library_columns;
mod lods;
mod logos;
mod maintenance;
mod media;
mod modscan;
mod music;
mod nationalities;
mod online;
mod others;
mod overlay;
mod packs;
mod pending;
mod preview;
mod profiles;
mod quickdrive;
mod raceini;
mod resources;
mod rule_overlay;
mod rules;
mod rules_share;
mod saved_grids;
mod saved_sessions;
mod session_state;
mod sessionpreset;
mod shadowdir;
mod showcase;
mod showroom;
mod skeleton;
mod startup;
mod steering;
mod stock;
mod submods;
mod sun;
mod survey;
mod taxonomy;
mod techsheet;
#[cfg(test)]
mod testutil;
mod thumbnails;
mod timing;
mod trackstates;
mod transfer;
mod ui_prefs;
mod uijson;
mod usermeta;
mod weather;
mod wiki;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    timing::start();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        // Journal fichier (`%APPDATA%\com.pitbox.app\logs\`, §10) : seul moyen
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
        // (PREVIEW§7.2). Sans lui, il faudrait faire
        // transiter le modèle par l'IPC : 30 Mo de binaire deviennent ~40 Mo
        // de base64 à parser côté JS, l'UI se fige. Ici la webview fetch un
        // fichier local, sans copie intermédiaire.
        .register_uri_scheme_protocol("carpreview", |ctx, request| {
            preview::serve_request(ctx.app_handle(), &request)
        })
        .setup(|app| {
            timing::mark("setup.begin", None);
            // The order of these four is the one that matters (`startup.rs`).
            let conn = startup::open_base(app)?;
            startup::safety_nets(app, &conn);
            let cfg = config::load(app.handle());
            startup::catch_up(app, &conn, &cfg);
            startup::start_services(app, conn);
            timing::mark("setup.end", None);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::timing::timing_enabled,
            commands::timing::timing_mark,
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
            commands::updates::check_mod_updates,
            commands::updates::recheck_mod_updates,
            commands::updates::mod_update_details,
            commands::updates::download_mod_update,
            commands::updates::cancel_mod_update_download,
            commands::updates::discard_mod_update_download,
            commands::bulk_ops::cancel_bulk,
            commands::import::split_dropped_paths,
            commands::import::list_pending_folders,
            commands::import::list_resource_folders,
            commands::import::remove_resource_folder,
            commands::import::resolve_pending_folder,
            commands::import::read_pending_document,
            commands::layers::list_layers,
            commands::layers::list_layers_by_kind,
            commands::layers::delete_layer,
            commands::layers::set_layer_active,
            commands::layers::reorder_layer,
            commands::layers::list_layer_files,
            commands::layers::layer_layout_origins,
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
            commands::session::car_factory_assists,
            commands::session::launch_session,
            commands::session::is_steam_running,
            commands::session::is_game_running,
            commands::session::open_content_manager,
            commands::session::launch_replay,
            commands::session::open_native_showroom,
            commands::session::list_showrooms,
            commands::preview::prepare_car_preview,
            commands::preview::list_driver_choices,
            commands::preview::list_driver_bodies,
            commands::preview::prepare_driver_preview,
            commands::preview::prepare_body_preview,
            commands::preview::body_thumbnail,
            commands::preview::save_body_thumbnail,
            commands::preview::clear_preview_cache,
            commands::preview::preview_cache_size,
            commands::preview::set_preview_cache_cap,
            commands::gridthumbs::grid_thumbnail,
            commands::gridthumbs::prepare_grid_model,
            commands::gridthumbs::save_grid_thumbnail,
            commands::gridthumbs::mark_grid_thumbnail_failed,
            commands::gridthumbs::forget_grid_thumbnail,
            commands::gridthumbs::release_grid_model,
            commands::gridthumbs::grid_thumbnail_stats,
            commands::gridthumbs::clear_grid_thumbnails,
            commands::gridthumbs::sweep_grid_templates,
            commands::session_state::get_session_picks,
            commands::session_state::save_session_picks,
            commands::session_state::get_launch_state,
            commands::session_state::save_launch_state,
            commands::cmimport::scan_cm_presets,
            commands::trackstate::track_states,
            commands::nationalities::nationalities,
            commands::saved_grids::get_saved_grids,
            commands::saved_grids::save_saved_grids,
            commands::sessionpreset::list_session_presets,
            commands::sessionpreset::save_session_preset,
            commands::sessionpreset::delete_session_preset,
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
            commands::gamestate::game_folder_status,
            commands::gamestate::scan_game_folder,
            commands::gamestate::game_folder_children,
            commands::gamestate::game_folder_detail,
            commands::gamestate::game_folder_search,
            commands::gamestate::game_folder_reveal,
            commands::gamestate::game_folder_owners,
            commands::gamestate::show_game_path,
            commands::bulk_ops::bulk_set_favorite,
            commands::bulk_ops::bulk_set_category,
            commands::bulk_ops::bulk_add_tag,
            commands::bulk_ops::bulk_remove_tag,
            commands::bulk_ops::bulk_activate,
            commands::bulk_ops::bulk_deactivate,
            commands::bulk_ops::bulk_delete,
            commands::bulk_ops::bulk_showcase,
            commands::showcase::showcase_plan,
            commands::showcase::showcase_sources,
            commands::transfer::transfer_estimate,
            commands::transfer::transfer_export,
            commands::transfer::transfer_inspect,
            commands::transfer::transfer_import,
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
            commands::addons::skin_detail,
            commands::addons::open_skin_folder,
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
            commands::online::online_servers,
            commands::online::online_server_detail,
            commands::online::online_join,
            commands::online::online_server_drivers,
            commands::online::online_slot_counts,
            commands::online::online_track_activity,
            commands::online::online_ping,
            commands::online::download_online_content,
            commands::online::get_online_store,
            commands::online::save_online_store,
            commands::others::list_other_mods,
            commands::others::list_inventory,
            commands::others::list_attached,
            commands::others::set_other_priority,
            commands::others::set_other_attachment,
            commands::others::activate_other,
            commands::others::deactivate_other,
            commands::others::delete_other_mod,
            commands::others::open_other_mod_folder,
            commands::others::list_other_resources,
            commands::others::open_other_resource,
            commands::others::get_other_resource_path,
            commands::others::read_other_resource,
            commands::addons::delete_app,
            commands::wiki::get_wiki_panel,
            commands::wiki::search_wiki_candidates,
            commands::wiki::set_wiki_link,
            commands::wiki::clear_wiki_link,
            commands::wiki::purge_wiki_cache,
            commands::wiki::export_wiki_links,
            commands::wiki::count_wiki_manual_links,
            commands::usermeta::set_entity_note,
            commands::usermeta::set_entity_display_name,
            commands::rules::get_rules,
            commands::rules::get_rules_view,
            commands::rules::save_rules_overlay,
            commands::rules::set_rule_enabled,
            commands::rules::export_rules,
            commands::rules::import_rules,
            commands::rules::export_survey,
            commands::rules::export_folder_survey,
            commands::rules::get_taxonomy,
            commands::rules::save_family_overlay,
            commands::rules::save_country_overlay,
            commands::rules::save_brand_overlay,
            commands::logos::get_brand_logos,
            commands::logos::save_brand_logo,
            commands::logos::import_brand_logo,
            commands::rules::get_catalog_report,
            commands::rules::set_catalog_reverted,
            commands::rules::dismiss_catalog_report,
            commands::rules::reapply_rules,
            commands::library::set_favorite,
            commands::library::set_manual_tags,
            commands::library::set_mod_field,
            commands::techsheet::save_tech_sheet,
            commands::techsheet::get_techsheet_report,
            commands::techsheet::dismiss_techsheet_report,
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
