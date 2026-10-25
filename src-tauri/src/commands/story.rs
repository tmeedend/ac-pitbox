//! Story Mode (`docs/story/SPEC-story-mode.md`) — see `crate::story`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{Emitter, Manager};

use super::prelude::*;
use crate::story::holdings::{self, Holding};
use crate::story::library::{self, LibraryWrites};
use crate::story::{backups, packages, play, session, studio};
use story_engine::needs::AcKind;
use story_engine::scene::Shown;

/// What this machine holds of the cars and tracks asked
/// (`story::holdings`): the base locked for that one read, never while
/// files are read.
fn holdings(app: &AppHandle) -> impl Fn(&BTreeSet<(AcKind, String)>) -> Vec<Holding> + '_ {
    move |wanted| {
        let cfg = crate::config::load(app);
        let db = app.state::<Db>();
        let conn = db.0.lock().unwrap_or_else(|e| e.into_inner());
        holdings::look_up(&conn, &cfg, wanted)
    }
}

/// The library as Story Mode writes it (`story::library`): the base locked
/// for each write alone, under the game-write marker - a story's layers go
/// on and off by the same path as a sheet's switch.
struct AppLibrary<'a>(&'a AppHandle);

impl AppLibrary<'_> {
    fn with<T>(&self, write: impl FnOnce(&rusqlite::Connection, &AppConfig) -> Result<T, String>) -> Result<T, String> {
        let db = self.0.try_state::<Db>().ok_or("the base is not open yet")?;
        let cfg = crate::config::load(self.0);
        let _game_write = crate::gamestate::GameWrite::begin();
        let conn = db.0.lock().unwrap_or_else(|e| e.into_inner());
        write(&conn, &cfg)
    }
}

impl LibraryWrites for AppLibrary<'_> {
    fn add_layer(&self, kind: AcKind, host: &str, name: &str, files: &Path, origin: &str) -> Result<String, String> {
        self.with(|conn, cfg| library::add_layer(conn, cfg, kind, host, name, files, origin))
    }

    fn remove_layer(&self, id: &str) -> Result<(), String> {
        self.with(|conn, cfg| crate::compose::remove_layer(conn, cfg, id))
    }

    fn set_layer_active(&self, id: &str, active: bool) -> Result<(), String> {
        self.with(|conn, cfg| crate::compose::set_layer_active(conn, cfg, id, active))
    }
}

/// Whether the game runs, as the game watch last announced it: what the
/// guard of a story's session asks when its delay is over.
static GAME_RUNNING: AtomicBool = AtomicBool::new(false);

/// Hooked on the game watch (`startup.rs`), called on every change and once
/// at startup: when the game is not running, the layers a story's session
/// switched on go off (`story::session`). On its own thread: switching
/// recomposes, which must not hold the watch.
pub fn on_game_running(app: &AppHandle, running: bool) {
    GAME_RUNNING.store(running, Ordering::SeqCst);
    if running {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        if let Ok(dir) = crate::rules::config_dir(&app) {
            session::end(&dir, &AppLibrary(&app));
        }
    });
}

/// Before an event of a story is launched: the story's layers over its car
/// and track on, until the game closes - or, no game running when the guard's
/// delay is over, off again.
#[tauri::command]
pub async fn story_session_begin(app: AppHandle, source: play::Source, event: String) -> Result<(), String> {
    off_window(app, move |app| {
        let dir = crate::rules::config_dir(app)?;
        let layers = play::session_layers(&dir, &source, &event)?;
        if let Some(generation) = session::begin(&dir, &layers, &AppLibrary(app))? {
            let app = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(session::GUARD);
                let running = GAME_RUNNING.load(Ordering::SeqCst);
                session::end_if_no_game(&dir, generation, running, &AppLibrary(&app));
            });
        }
        Ok(())
    })
    .await
}

/// The Story switch asked for on this launch's command line, if any
/// (`--enable-story-studio` / `--disable-story-studio`). Read once at startup
/// by the frontend, which records it in `ui_prefs.json`: that file's schema
/// belongs to the frontend, so the backend never writes the key itself.
#[tauri::command]
pub fn story_studio_switch() -> Option<bool> {
    crate::story::studio_switch_from_args(std::env::args())
}

// ------------------------------------------------------------- Story Mode
//
// Façades over `crate::story::play`. Each reads the story and its save from
// disk, hence off the window thread; the screen sends one move at a time, so
// two moves never race for the save.

#[tauri::command]
pub async fn story_play_list(app: AppHandle) -> Result<Vec<play::Playable>, String> {
    off_window(app, |app| {
        Ok(play::list(&crate::rules::config_dir(app)?, &holdings(app)))
    })
    .await
}

/// A new game, replacing the one under way; its draws come from the clock.
#[tauri::command]
pub async fn story_play_new(app: AppHandle, source: play::Source) -> Result<play::Game, String> {
    off_window(app, move |app| {
        let seed = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH))
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        play::new_game(&crate::rules::config_dir(app)?, &source, seed)
    })
    .await
}

#[tauri::command]
pub async fn story_play_resume(app: AppHandle, source: play::Source) -> Result<play::Game, String> {
    off_window(app, move |app| play::resume(&crate::rules::config_dir(app)?, &source)).await
}

#[tauri::command]
pub async fn story_play_move(app: AppHandle, source: play::Source, input: play::Move) -> Result<play::Game, String> {
    off_window(app, move |app| {
        play::play(&crate::rules::config_dir(app)?, &source, input)
    })
    .await
}

/// What a package install has downloaded, as `story:download` tells the
/// hub.
#[derive(Clone, serde::Serialize)]
struct DownloadProgress {
    received: u64,
    total: u64,
}

/// Installs a package from `origin`, reporting what it downloads as
/// `story:download` - at most ten events a second, the last always.
fn install_reporting(app: &AppHandle, origin: packages::Origin, lose_game: bool) -> Result<packages::Outcome, String> {
    let mut last: Option<std::time::Instant> = None;
    let mut progress = |received: u64, total: u64| {
        let due = last.is_none_or(|t| t.elapsed() >= std::time::Duration::from_millis(100));
        if due || received == total {
            last = Some(std::time::Instant::now());
            if let Err(e) = app.emit("story:download", DownloadProgress { received, total }) {
                log::warn!("story:download not delivered: {e}");
            }
        }
        true
    };
    packages::install(
        &crate::rules::config_dir(app)?,
        origin,
        lose_game,
        &holdings(app),
        &AppLibrary(app),
        &mut progress,
    )
}

/// Installs a story package (`.pitstory`), or updates the story it carries;
/// `lose_game` goes ahead when the game under way would not resume on it.
#[tauri::command]
pub async fn story_package_install(
    app: AppHandle,
    file: PathBuf,
    lose_game: bool,
) -> Result<packages::Outcome, String> {
    off_window(app, move |app| {
        install_reporting(app, packages::Origin::File(&file), lose_game)
    })
    .await
}

/// Installs the story whose package's manifest is at `url`.
#[tauri::command]
pub async fn story_package_install_url(
    app: AppHandle,
    url: String,
    lose_game: bool,
) -> Result<packages::Outcome, String> {
    off_window(app, move |app| {
        install_reporting(app, packages::Origin::Url(url.trim()), lose_game)
    })
    .await
}

/// Updates the installed story `id` from the URL it was installed from.
#[tauri::command]
pub async fn story_package_update(app: AppHandle, id: String, lose_game: bool) -> Result<packages::Outcome, String> {
    off_window(app, move |app| {
        let url = packages::update_url(&crate::rules::config_dir(app)?, &id)
            .ok_or(crate::errors::STORY_PACKAGE_DOWNLOAD_FAILED)?;
        install_reporting(app, packages::Origin::Url(&url), lose_game)
    })
    .await
}

/// The updates of the installed stories that came from a URL.
#[tauri::command]
pub async fn story_package_updates(app: AppHandle) -> Result<Vec<packages::Update>, String> {
    off_window(app, |app| {
        Ok(packages::updates(&crate::rules::config_dir(app)?, &holdings(app)))
    })
    .await
}

/// Stops the package download under way.
#[tauri::command]
pub fn story_package_cancel() {
    packages::cancel();
}

/// An archive an install set aside for the import, once the import is done
/// with it.
#[tauri::command]
pub async fn story_package_imported(app: AppHandle, archive: PathBuf) -> Result<(), String> {
    off_window(app, move |app| {
        packages::imported(&crate::rules::config_dir(app)?, &archive)
    })
    .await
}

#[tauri::command]
pub async fn story_package_uninstall(app: AppHandle, id: String) -> Result<(), String> {
    off_window(app, move |app| {
        packages::uninstall(&crate::rules::config_dir(app)?, &id, &AppLibrary(app))
    })
    .await
}

/// Builds the package of the story in `folder` into `out`, and says its
/// size; refused while the story or its permission register has an error.
#[tauri::command]
pub async fn story_build_package(app: AppHandle, folder: PathBuf, out: PathBuf) -> Result<u64, String> {
    off_window(app, move |app| studio::build_package(&folder, &out, &holdings(app))).await
}

/// Writes into `dir` what a host serves for the story in `folder`: its
/// items and `manifest.json`. Says the items' size.
#[tauri::command]
pub async fn story_publish_package(app: AppHandle, folder: PathBuf, dir: PathBuf) -> Result<u64, String> {
    off_window(app, move |app| studio::publish_package(&folder, &dir, &holdings(app))).await
}

/// Keeps the line the dialogue under way is read at, beside the save.
#[tauri::command]
pub async fn story_play_reading(app: AppHandle, source: play::Source, reading: play::Reading) -> Result<(), String> {
    off_window(app, move |app| {
        play::set_reading(&crate::rules::config_dir(app)?, &source, &reading)
    })
    .await
}

/// Copies a file the author picked into the story, at `into` (a folder of
/// `media/` or a skin's), and says the path a story names it by.
#[tauri::command]
pub async fn story_import_file(
    app: AppHandle,
    folder: PathBuf,
    source: PathBuf,
    into: String,
) -> Result<String, String> {
    off_window(app, move |_| crate::story::frames::import_file(&folder, &source, &into)).await
}

/// The starter kit's sheets, as the engine draws them: SVG by file name.
#[tauri::command]
pub fn story_kit_sheets() -> Vec<(String, String)> {
    story_engine::kit::sheets()
}

/// Saves the starter kit in a folder the author picked: the SVG sheets and
/// the PNGs the page made of them.
#[tauri::command]
pub async fn story_save_kit(
    app: AppHandle,
    dir: PathBuf,
    pngs: Vec<crate::story::frames::KitPng>,
) -> Result<usize, String> {
    off_window(app, move |_| crate::story::frames::save_kit(&dir, &pngs)).await
}

/// The bytes of a skin's font, for the page to load (`story/media.rs`).
#[tauri::command]
pub async fn story_skin_font(
    app: AppHandle,
    source: play::Source,
    path: String,
) -> Result<tauri::ipc::Response, String> {
    off_window(app, move |app| {
        crate::story::media::skin_font(&crate::rules::config_dir(app)?, &source, &path)
    })
    .await
    .map(tauri::ipc::Response::new)
}

// ------------------------------------------------- Create Your Own (studio)
//
// Façades over `crate::story::studio`. The story files live in a folder of
// the author's choosing; only the list of those folders is Pit Box's
// (`app_config_dir/story/studio.json`). Reads of a story folder run off the
// window thread; a write stays synchronous, so two edits in a row reach the
// disk in the order they were made.

#[tauri::command]
pub async fn story_list(app: AppHandle) -> Result<Vec<studio::StorySummary>, String> {
    off_window(app, |app| Ok(studio::list(&crate::rules::config_dir(app)?))).await
}

#[tauri::command]
pub fn story_register(app: AppHandle, folder: PathBuf) -> Result<(), String> {
    studio::register(&crate::rules::config_dir(&app)?, &folder)
}

#[tauri::command]
pub fn story_forget(app: AppHandle, folder: PathBuf) -> Result<(), String> {
    studio::forget(&crate::rules::config_dir(&app)?, &folder)
}

#[tauri::command]
pub fn story_create(
    app: AppHandle,
    folder: PathBuf,
    title: String,
    language: String,
    frame: story_engine::frame::Frame,
    chapter_title: String,
) -> Result<(), String> {
    studio::create(
        &crate::rules::config_dir(&app)?,
        &folder,
        &title,
        &language,
        frame,
        &chapter_title,
    )
}

/// Copies the sample story into an empty folder of the author's.
#[tauri::command]
pub fn story_copy_sample(app: AppHandle, folder: PathBuf) -> Result<(), String> {
    studio::copy_sample(&crate::rules::config_dir(&app)?, &folder)
}

/// Opens a story, and takes its copy of the day if it has none yet
/// (`story::backups`). The copy is best-effort: a story that cannot be
/// copied still opens.
#[tauri::command]
pub async fn story_open(app: AppHandle, folder: PathBuf) -> Result<studio::OpenStory, String> {
    off_window(app, move |app| {
        let now = chrono::Local::now().naive_local();
        if let Err(e) = backups::take_daily(&crate::rules::config_dir(app)?, &folder, now) {
            log::warn!("story backups: no copy of {}: {e}", folder.display());
        }
        studio::open(&folder, &holdings(app))
    })
    .await
}

#[tauri::command]
pub async fn story_check(app: AppHandle, folder: PathBuf) -> Result<studio::Checked, String> {
    off_window(app, move |app| Ok(studio::check(&folder, &holdings(app)))).await
}

/// Plays a content item for the Content screen's preview. Reads the story
/// from disk, hence off the window thread.
#[tauri::command]
pub async fn story_preview(
    app: AppHandle,
    folder: PathBuf,
    content: String,
    setup: Vec<String>,
    replies: Vec<String>,
) -> Result<studio::PreviewView, String> {
    off_window(app, move |_| studio::preview(&folder, &content, &setup, &replies)).await
}

/// The variant of a place the Scenes screen's preview shows, at the moment
/// the author set.
#[tauri::command]
pub fn story_scene_pick(place: serde_json::Value, moment: studio::SceneMoment) -> Result<Option<Shown>, String> {
    studio::scene_pick(place, &moment)
}

/// Reads a content item's text into the tree the Form view edits.
#[tauri::command]
pub fn story_script_read(text: String) -> studio::ScriptRead {
    studio::read_script(&text)
}

/// Writes the tree the Form view edited back as Ink.
#[tauri::command]
pub fn story_script_write(script: serde_json::Value) -> Result<String, String> {
    studio::write_script(script)
}

/// Writes one file the editor edits - a chapter, an event, `story.yaml` or
/// a world file - from the value it sends, in the shape of that file.
#[tauri::command]
pub fn story_save_file(
    folder: PathBuf,
    file: String,
    expected: Option<String>,
    value: serde_json::Value,
) -> Result<String, String> {
    studio::save_file(&folder, &file, expected.as_deref(), value)
}

/// Deletes an event's file, guarded by the stamp it was read with.
#[tauri::command]
pub fn story_remove_file(folder: PathBuf, file: String, expected: String) -> Result<(), String> {
    studio::remove_file(&folder, &file, &expected)
}

/// Renames a character, flag, counter, relation step or event everywhere it
/// is cited. Off the window thread: it reads the whole story folder. The
/// frontend's queue of edits keeps it in order with the saves around it.
#[tauri::command]
pub async fn story_rename(
    app: AppHandle,
    folder: PathBuf,
    kind: story_engine::refs::NameKind,
    from: String,
    to: String,
) -> Result<(), String> {
    off_window(app, move |_| studio::rename(&folder, kind, &from, &to)).await
}

/// Ctrl+Z in the editor: puts back the files of the last edit. Resolves to
/// the files written, empty when there was nothing to undo. A short write,
/// synchronous like a save.
#[tauri::command]
pub fn story_undo(folder: PathBuf) -> Result<Vec<String>, String> {
    crate::story::history::with(&folder, |h| h.undo(&folder))
}

/// Ctrl+Y: writes again the files of the last edit undone.
#[tauri::command]
pub fn story_redo(folder: PathBuf) -> Result<Vec<String>, String> {
    crate::story::history::with(&folder, |h| h.redo(&folder))
}

/// The story's copies, newest first.
#[tauri::command]
pub async fn story_backups(app: AppHandle, folder: PathBuf) -> Result<Vec<backups::Backup>, String> {
    off_window(app, move |app| {
        Ok(backups::list(&crate::rules::config_dir(app)?, &folder))
    })
    .await
}

/// Puts the story back as it was in a copy, after copying it as it is now.
#[tauri::command]
pub async fn story_restore(app: AppHandle, folder: PathBuf, id: String) -> Result<Vec<String>, String> {
    off_window(app, move |app| {
        let now = chrono::Local::now().naive_local();
        backups::restore(&crate::rules::config_dir(app)?, &folder, &id, now)
    })
    .await
}
