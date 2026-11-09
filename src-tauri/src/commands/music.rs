//! Commandes du module musique (`docs/spec-module-musique_2.md`, SPEC §16).
//! Façades minces : toute la logique vit dans `music::{config,engine,scan,steam}`.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::music::engine::Context;
use crate::music::scan::{self, SourceInfo};
use crate::music::steam::{self, Soundtrack};
use crate::music::{self, EngineCommand, MusicConfig, MusicEngineHandle, PreviewHandle};

#[tauri::command]
pub fn get_music_config(app: AppHandle) -> MusicConfig {
    music::config::load(&app)
}

#[tauri::command]
pub fn save_music_config(app: AppHandle, config: MusicConfig, engine: State<MusicEngineHandle>) -> Result<(), String> {
    music::config::save(&app, &config)?;
    // Préchauffe les sources avant que le moteur en ait besoin : sans ça, la
    // première navigation vers une source qui vient de changer subit le scan
    // complet en pleine transition.
    music::index::warm(&app, config.clone());
    engine.send(EngineCommand::UpdateConfig(config));
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct DefaultMusicFolders {
    pub menu: PathBuf,
    pub grid: PathBuf,
}

/// Dossiers par défaut (MUSIQUE§3.1), pour affichage côté frontend quand
/// `menu_folder`/`grid_folder` valent `None` dans la config.
#[tauri::command]
pub fn get_default_music_folders(app: AppHandle) -> DefaultMusicFolders {
    DefaultMusicFolders {
        menu: music::config::default_menu_dir(&app),
        grid: music::config::default_grid_dir(&app),
    }
}

/// What a source holds — a folder or a file (§16.3) — for the pickers.
#[tauri::command]
pub fn scan_music_source(path: PathBuf) -> SourceInfo {
    scan::scan_source(&path)
}

/// The tracks of a source, sorted (§16.8): what the track count of a picker
/// unfolds, to pick one of them as a source of its own.
#[tauri::command]
pub fn list_music_tracks(path: PathBuf) -> Vec<PathBuf> {
    scan::list_tracks(&path)
}

#[tauri::command]
pub fn music_enter_big_picture(engine: State<MusicEngineHandle>) {
    engine.send(EngineCommand::EnterBigPicture);
}

#[tauri::command]
pub fn music_exit_big_picture(engine: State<MusicEngineHandle>) {
    engine.send(EngineCommand::ExitBigPicture);
}

/// What the screen on display resolved (§16.5); applied after the engine's
/// delay, and not at all when the source is the one already playing.
#[tauri::command]
pub fn music_set_context(context: Context, engine: State<MusicEngineHandle>) {
    engine.send(EngineCommand::SetContext(context));
}

#[tauri::command]
pub fn music_set_minimized(minimized: bool, engine: State<MusicEngineHandle>) {
    engine.send(EngineCommand::SetMinimized(minimized));
}

/// Écoute au clic (MUSIQUE§6) : une piste au hasard de la source, l'ambiance
/// en cours s'effaçant le temps de l'écoute (voir `engine::PreviewHandle`).
#[tauri::command]
pub fn music_preview_start(
    app: AppHandle,
    path: PathBuf,
    volume: f32,
    preview: State<PreviewHandle>,
) -> Result<(), String> {
    let tracks = scan::list_tracks(&path);
    if tracks.is_empty() {
        return Err(crate::errors::MUSIC_FOLDER_EMPTY.into());
    }
    let track = tracks[fastrand::usize(..tracks.len())].clone();
    let source = path.clone();
    preview.start(track, volume, move || {
        use tauri::Emitter;
        if let Err(e) = app.emit("music:preview-ended", source) {
            log::warn!("music: preview end not emitted: {e}");
        }
    });
    Ok(())
}

#[tauri::command]
pub fn music_preview_stop(preview: State<PreviewHandle>) {
    preview.stop();
}

/// Read once per run, at the first opening of a picker (§16.7).
static SOUNDTRACKS: Mutex<Option<Vec<Soundtrack>>> = Mutex::new(None);

/// The Steam soundtracks installed (§16.7). Off the window thread: it lists
/// folders on every Steam library, possibly on a slow disk.
#[tauri::command]
pub async fn list_steam_soundtracks(app: AppHandle) -> Result<Vec<Soundtrack>, String> {
    super::off_window(app, |_| {
        let mut cached = SOUNDTRACKS.lock().map_err(|e| e.to_string())?;
        Ok(cached.get_or_insert_with(steam::list).clone())
    })
    .await
}

/// Now-playing notification (MUSIQUE§5.5): reads the tags of the track that
/// just started and emits `music:track` to the front.
///
/// Called by the engine for every track it starts (`engine::spawn`), on its own
/// thread — so the tags and the cover thumbnail are read on a thread of their
/// own, off the 30 ms tick that drives the fades. Two quick changes in a row
/// spawn two readers that may finish out of order: a generation counter lets
/// only the latest one emit, or the notification would end on the track that
/// is no longer playing.
pub fn announce_track(app: &AppHandle, path: PathBuf) {
    use std::sync::atomic::{AtomicU64, Ordering};
    use tauri::Emitter;
    static LATEST: AtomicU64 = AtomicU64::new(0);

    let generation = LATEST.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    std::thread::spawn(move || {
        let info = music::tags::read(&path);
        if LATEST.load(Ordering::SeqCst) != generation {
            return;
        }
        if let Err(e) = app.emit("music:track", info) {
            log::warn!("music: now-playing event not emitted: {e}");
        }
    });
}
