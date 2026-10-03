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
        online::server::fetch_detail(&ip, http_port, steam_id, &installed, cars_dir.as_deref())
    })
    .await
    .map_err(|e| e.to_string())?
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
        online::join::join(&conn, &cfg, &request, features)
    })
    .await
    .map_err(|e| e.to_string())?
}
