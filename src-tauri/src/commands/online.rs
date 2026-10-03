//! Multiplayer servers (`docs/SPEC-play-online.md`). Every command here waits on
//! the network, so each runs in `spawn_blocking`, and the SQLite lock is held
//! only for the read that judges servers against the library — never across a
//! request, which would freeze every other screen for as long as a server
//! takes to answer.

use super::prelude::*;
use tauri::Manager;

use crate::online::{self, Installed};

fn steam_id() -> Result<u64, String> {
    online::active_steam_id().ok_or_else(|| crate::errors::STEAM_NOT_SIGNED_IN.to_string())
}

fn installed(app: &AppHandle) -> Result<Installed, String> {
    let cfg = crate::config::load(app);
    let db = app.state::<Db>();
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(Installed::scan(&conn, &cfg))
}

/// The public server list, each judged against what can be driven here, with
/// the names and pictures of what they reference.
#[tauri::command]
pub async fn online_servers(app: AppHandle) -> Result<online::ServerList, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let servers = online::lobby::fetch_lobby(steam_id()?)?;
        let cfg = crate::config::load(&app);
        let db = app.state::<Db>();
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        Ok(online::judge_list(&conn, &cfg, servers))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// One server's live state, car slots and drivers, asked from the server.
#[tauri::command]
pub async fn online_server_detail(
    app: AppHandle,
    ip: String,
    http_port: u16,
) -> Result<online::server::ServerDetail, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let steam_id = steam_id()?;
        let installed = installed(&app)?;
        let cars_dir = crate::config::load(&app)
            .ac_install_path
            .map(|ac| ac.join("content").join("cars"));
        let mut detail = online::server::fetch_detail(&ip, http_port, steam_id, &installed, cars_dir.as_deref())?;
        // The network is done: the lock only for what the library says.
        let cfg = crate::config::load(&app);
        let db = app.state::<Db>();
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        detail.attach_library(&conn, &cfg);
        Ok(detail)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Downloads the archive of missing content `id` from where a server says it
/// is (`online/content.rs`), with the progress and the cancel of a mod update
/// — the same rule follows: an archive goes to the import, anything else to
/// the browser. Into the temp folder only; the import that follows is the
/// ordinary one, as for a file dropped on the window.
#[tauri::command]
pub async fn download_online_content(
    app: AppHandle,
    url: String,
    id: String,
) -> Result<crate::cup::DownloadOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || {
        super::updates::download_reporting(&app, &id, |on_progress| {
            crate::cup::download_archive(&url, &id, &std::env::temp_dir(), on_progress)
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The connected drivers of each of `servers`, asked from the servers
/// themselves — to find friends (`online/drivers.rs`).
#[tauri::command]
pub async fn online_server_drivers(
    servers: Vec<online::drivers::ServerAddr>,
) -> Result<Vec<online::drivers::ServerDrivers>, String> {
    tauri::async_runtime::spawn_blocking(move || Ok(online::drivers::scan(&servers, steam_id()?)))
        .await
        .map_err(|e| e.to_string())?
}

/// The free slots of one server, per car, from its `/JSON` alone — what the
/// "notify me" watch asks every 30 s (SPEC-play-online.md, v2).
#[tauri::command]
pub async fn online_slot_counts(ip: String, http_port: u16) -> Result<Vec<online::server::SlotCount>, String> {
    tauri::async_runtime::spawn_blocking(move || online::server::fetch_slot_counts(&ip, http_port, steam_id()?))
        .await
        .map_err(|e| e.to_string())?
}

/// The round trip to each of `servers`, in milliseconds (`online/ping.rs`).
/// Servers that do not answer are absent.
#[tauri::command]
pub async fn online_ping(servers: Vec<online::drivers::ServerAddr>) -> Result<Vec<online::ping::Ping>, String> {
    tauri::async_runtime::spawn_blocking(move || online::ping::ping(&servers))
        .await
        .map_err(|e| e.to_string())
}

/// Hooked on the game watch (`lib.rs`), which calls it on every change and
/// once at startup: when the game is not running, gives back the layers an
/// online session set aside (`online/session_layers.rs`), and tells the screen
/// which. On its own thread — a recomposition must not hold the watch, which
/// also drives the music.
pub fn on_game_running(app: &AppHandle, running: bool) {
    if running {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let Some(db) = app.try_state::<Db>() else {
            log::warn!("online: the base is not open yet, set-aside layers wait for the next check");
            return;
        };
        let Ok(dir) = config_dir(&app) else { return };
        let cfg = crate::config::load(&app);
        let restored = {
            let _game_write = crate::gamestate::GameWrite::begin();
            let Ok(conn) = db.0.lock() else { return };
            online::session_layers::restore(&conn, &cfg, &dir)
        };
        if !restored.is_empty() {
            use tauri::Emitter;
            if let Err(e) = app.emit("online://layers-restored", restored) {
                log::warn!("online://layers-restored not delivered — {e}");
            }
        }
    });
}

fn config_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path().app_config_dir().map_err(|e| e.to_string())
}

/// Favourites and recent joins (`online/store.rs`); an empty object when
/// there are none yet.
#[tauri::command]
pub fn get_online_store(app: AppHandle) -> Result<serde_json::Value, String> {
    Ok(online::store::load(&config_dir(&app)?))
}

/// Writes the whole store, synchronously (golden rule 6).
#[tauri::command]
pub fn save_online_store(app: AppHandle, store: serde_json::Value) -> Result<(), String> {
    online::store::save(&config_dir(&app)?, &store)
}

/// Joins a server with the chosen car, through Content Manager.
#[tauri::command]
pub async fn online_join(app: AppHandle, request: online::join::JoinRequest) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let features = online::join::prepare(&request)?;
        let _game_write = crate::gamestate::GameWrite::begin();
        let cfg = crate::config::load(&app);
        let db = app.state::<Db>();
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        online::join::join(&conn, &cfg, &config_dir(&app)?, &request, features)
    })
    .await
    .map_err(|e| e.to_string())?
}
