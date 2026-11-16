//! The sessions driven (RESULTAT§): the end of a game, the screen shown on
//! return, starting a session again, and CM's thumbnail copies.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use super::prelude::*;
use crate::overlay::Launch;
use crate::sessionlog::{self, LaunchKind};
use tauri::{Emitter, Manager};

/// When the running game was seen starting, in ms; 0 when none was seen since
/// Pit Box started — the watch's first announcement, at startup, is no game
/// closing.
static GAME_STARTED_AT: AtomicI64 = AtomicI64::new(0);

/// The last server joined, password included, for "Again" on the return
/// screen. Kept in memory only: a password does not go into the base.
static LAST_JOIN: Mutex<Option<(i64, crate::online::join::JoinRequest)>> = Mutex::new(None);

/// When CM's copy is looked for after a game: it writes it a second or so
/// after the game, and a second pass catches a slow one.
const SCANS_AFTER_GAME: [Duration; 2] = [Duration::from_secs(5), Duration::from_secs(25)];

/// What the window hears when a session of the user's has ended.
#[derive(Clone, serde::Serialize)]
struct SessionEnded {
    launch_id: i64,
}

fn with_db(app: &AppHandle, f: &mut dyn FnMut(&rusqlite::Connection)) {
    let Some(db) = app.try_state::<Db>() else {
        log::warn!("results: the base is not open yet");
        return;
    };
    match db.0.lock() {
        Ok(conn) => f(&conn),
        Err(e) => log::warn!("results: base unavailable ({e})"),
    };
}

/// Notes a launch in the journal (RESULTAT§2); `None` when it could not be.
pub(crate) fn note(app: &AppHandle, launch: Launch) -> Option<i64> {
    let mut id = None;
    let mut launch = Some(launch);
    with_db(app, &mut |conn| {
        if let Some(l) = launch.take() {
            id = sessionlog::note_launch(conn, l);
        }
    });
    id
}

/// Notes a thumbnail session: its cars, on the studio track.
pub(crate) fn note_gameshot(app: &AppHandle, cars: &[String], started_at: i64) {
    note(
        app,
        Launch {
            kind: LaunchKind::Gameshot.as_str().into(),
            started_at,
            car_id: cars.first().cloned(),
            track_id: Some(crate::gameshot::STUDIO_TRACK.into()),
            cars: cars.to_vec(),
            ..Default::default()
        },
    );
}

/// Remembers the server just joined, for "Again".
pub(crate) fn remember_join(launch_id: Option<i64>, request: &crate::online::join::JoinRequest) {
    if let Some(id) = launch_id {
        *LAST_JOIN.lock().unwrap_or_else(|e| e.into_inner()) = Some((id, request.clone()));
    }
}

fn scan_cm(app: &AppHandle) {
    let Some(dir) = sessionlog::cmcopy::sessions_dir() else {
        return;
    };
    let read = sessionlog::scan(&dir, &|f| with_db(app, f));
    if read > 0 {
        log::info!("results: {read} copies of Content Manager read");
    }
}

/// Hooked on the game watch (`startup.rs`). The game closing ends the open
/// launch, records the session, brings Pit Box back and tells the window;
/// then CM's copies are read. At startup, the copies are read once.
pub fn on_game_running(app: &AppHandle, running: bool) {
    if running {
        GAME_STARTED_AT.store(sessionlog::now_ms(), Ordering::SeqCst);
        return;
    }
    let app = app.clone();
    let started = GAME_STARTED_AT.swap(0, Ordering::SeqCst);
    std::thread::spawn(move || {
        if started == 0 {
            // Startup: a launch left open belongs to a game closed while Pit
            // Box was not there; it ends, unannounced.
            with_db(&app, &mut |conn| {
                if let Err(e) = crate::overlay::end_open_launches(conn, sessionlog::now_ms()) {
                    log::warn!("results: open launches not closed ({e})");
                }
            });
            scan_cm(&app);
            return;
        }
        let file = crate::raceresult::result_path();
        let mut closed = None;
        with_db(&app, &mut |conn| {
            closed = sessionlog::game_closed(conn, file.as_deref(), sessionlog::now_ms(), Some(started))
                .unwrap_or_else(|e| {
                    log::warn!("results: the end of the session not recorded ({e})");
                    None
                });
        });
        if let Some(c) = closed.filter(|c| LaunchKind::is_recorded(&c.launch.kind)) {
            bring_back(&app);
            if let Err(e) = app.emit("session://ended", SessionEnded { launch_id: c.launch.id }) {
                log::warn!("results: session://ended not delivered ({e})");
            }
        }
        for delay in SCANS_AFTER_GAME {
            std::thread::sleep(delay);
            scan_cm(&app);
        }
    });
}

/// Pit Box back in front once the game has gone (RESULTAT§4). Windows may
/// refuse the foreground to a window that does not have it: the taskbar
/// button flashes instead.
fn bring_back(app: &AppHandle) {
    let Some(win) = app
        .get_webview_window("main")
        .or_else(|| app.webview_windows().into_values().next())
    else {
        return;
    };
    if let Err(e) = win.unminimize().and_then(|_| win.show()).and_then(|_| win.set_focus()) {
        log::warn!("results: window not brought back ({e})");
    }
    std::thread::sleep(Duration::from_millis(400));
    if !win.is_focused().unwrap_or(false) {
        if let Err(e) = win.request_user_attention(Some(tauri::UserAttentionType::Critical)) {
            log::warn!("results: taskbar not flashed ({e})");
        }
    }
}

/// What the screen shown on return needs (RESULTAT§4).
#[tauri::command]
pub fn session_return(db: State<Db>, launch_id: i64) -> Result<Option<sessionlog::ReturnView>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let file = crate::raceresult::result_path();
    // An online "Again" needs the join request kept in memory: gone with a
    // restart of Pit Box.
    let online_again =
        matches!(LAST_JOIN.lock().unwrap_or_else(|e| e.into_inner()).as_ref(), Some((id, _)) if *id == launch_id);
    sessionlog::return_view(&conn, launch_id, file.as_deref(), online_again).map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
pub struct SessionMedia {
    replay: Option<crate::media::ReplayFile>,
    screenshots: Vec<crate::media::ScreenshotFile>,
}

/// The replay and the screenshots of a session, computed from its times and
/// its combo (RESULTAT§6). Reads the disk: off the window.
#[tauri::command]
pub async fn session_media(app: AppHandle, launch_id: i64) -> Result<SessionMedia, String> {
    off_window(app, move |app| {
        let launch = {
            let db = app.state::<Db>();
            let conn = db.0.lock().map_err(|e| e.to_string())?;
            crate::overlay::get_launch(&conn, launch_id).map_err(|e| e.to_string())?
        };
        let empty = SessionMedia {
            replay: None,
            screenshots: Vec::new(),
        };
        let Some(l) = launch else { return Ok(empty) };
        let (Some(car), Some(track)) = (l.car_id.as_deref(), l.track_id.as_deref()) else {
            return Ok(empty);
        };
        let end = l.ended_at.unwrap_or_else(sessionlog::now_ms);
        Ok(SessionMedia {
            replay: crate::media::session_replay(car, track, l.started_at, end),
            screenshots: crate::media::session_screenshots(car, track, l.started_at, end),
        })
    })
    .await
}

/// "Again": the same session, started again (RESULTAT§4.3). A session of
/// the session screen goes with its own setup and preset; an online one joins
/// the same server again.
#[tauri::command]
pub async fn launch_again(app: AppHandle, launch_id: i64) -> Result<(), String> {
    let launch = {
        let db = app.state::<Db>();
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        crate::overlay::get_launch(&conn, launch_id).map_err(|e| e.to_string())?
    }
    .ok_or(crate::errors::SESSION_NOT_RELAUNCHABLE)?;
    match launch.kind.as_str() {
        "session" => {
            let raw: serde_json::Value = launch
                .setup
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .ok_or(crate::errors::SESSION_NOT_RELAUNCHABLE)?;
            let setup: crate::launch::RaceSetup =
                serde_json::from_value(raw.clone()).map_err(|_| crate::errors::SESSION_NOT_RELAUNCHABLE)?;
            let started_at = sessionlog::now_ms();
            {
                let _game_write = crate::gamestate::GameWrite::begin();
                let cfg = crate::config::load(&app);
                let db = app.state::<Db>();
                let conn = db.0.lock().map_err(|e| e.to_string())?;
                crate::launch::launch(&conn, &cfg, &setup)?;
            }
            // The same preset as the first time, block included.
            let mut again = sessionlog::session_launch(LaunchKind::Session, started_at, &raw, &setup, None);
            again.preset = launch.preset.clone();
            note(&app, again);
            Ok(())
        }
        "online" => {
            let request = match LAST_JOIN.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                Some((id, r)) if *id == launch_id => r.clone(),
                _ => return Err(crate::errors::SESSION_NOT_RELAUNCHABLE.into()),
            };
            super::online::online_join(app, request).await
        }
        _ => Err(crate::errors::SESSION_NOT_RELAUNCHABLE.into()),
    }
}

/// The copies of CM's that look like a thumbnail session (RESULTAT§5.2).
#[tauri::command]
pub async fn list_thumbnail_copies(app: AppHandle) -> Result<Vec<sessionlog::thumbs::ThumbnailCopy>, String> {
    off_window(app, |_| {
        Ok(sessionlog::cmcopy::sessions_dir()
            .map(|dir| sessionlog::thumbs::list_past(&dir))
            .unwrap_or_default())
    })
    .await
}

/// Puts the confirmed ones in the recycle bin; returns those that went.
#[tauri::command]
pub async fn trash_thumbnail_copies(app: AppHandle, files: Vec<String>) -> Result<Vec<String>, String> {
    off_window(app, move |app| {
        let Some(dir) = sessionlog::cmcopy::sessions_dir() else {
            return Ok(Vec::new());
        };
        let gone = sessionlog::thumbs::trash(&dir, &files, &|p| crate::media::trash_file(p));
        mark_trashed(app, &gone);
        Ok(gone)
    })
    .await
}

fn mark_trashed(app: &AppHandle, files: &[String]) {
    with_db(app, &mut |conn| {
        for file in files {
            if let Err(e) = crate::overlay::mark_cm_file(conn, file, "trashed", sessionlog::now_ms()) {
                log::warn!("results: {file} not marked as trashed ({e})");
            }
        }
    });
}

/// After a thumbnail session: CM's copy of it goes to the recycle bin, when
/// it is certain which one it is (RESULTAT§5.1). Blocking: run off the window.
pub(crate) fn retire_thumbnail_copy(app: &AppHandle, cars: &[String], from_ms: i64, to_ms: i64) {
    let Some(dir) = sessionlog::cmcopy::sessions_dir() else {
        return;
    };
    // Up to ten seconds for CM to write it.
    let Some(file) = sessionlog::thumbs::wait_job_copy(&dir, cars, from_ms, to_ms, 6, Duration::from_secs(2)) else {
        return;
    };
    match crate::media::trash_file(&dir.join(&file)) {
        Ok(()) => {
            log::info!("results: CM's copy {file} of the thumbnail session sent to the recycle bin");
            mark_trashed(app, &[file]);
        }
        Err(e) => log::warn!("results: CM's copy {file} of the thumbnail session left ({e})"),
    }
}
