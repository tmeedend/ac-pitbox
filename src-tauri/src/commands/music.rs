//! Commandes du module musique Big Picture (`docs/spec-module-musique_2.md`).
//! Façades minces : toute la logique vit dans `music::{config,engine,scan}`.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::music::scan::{self, FolderInfo};
use crate::music::{self, EngineCommand, MusicConfig, MusicEngineHandle, PreviewHandle};

#[tauri::command]
pub fn get_music_config(app: AppHandle) -> MusicConfig {
    music::config::load(&app)
}

#[tauri::command]
pub fn save_music_config(app: AppHandle, config: MusicConfig, engine: State<MusicEngineHandle>) -> Result<(), String> {
    music::config::save(&app, &config)?;
    // Préchauffe le nouveau dossier avant que le moteur en ait besoin :
    // sans ça, la première navigation vers l'ambiance dont le dossier vient
    // de changer subit le scan complet en pleine transition.
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

#[tauri::command]
pub fn scan_music_folder(path: PathBuf) -> FolderInfo {
    scan::scan_folder(&path)
}

#[tauri::command]
pub fn music_enter_big_picture(engine: State<MusicEngineHandle>) {
    engine.send(EngineCommand::EnterBigPicture);
}

#[tauri::command]
pub fn music_exit_big_picture(engine: State<MusicEngineHandle>) {
    engine.send(EngineCommand::ExitBigPicture);
}

#[tauri::command]
pub fn music_enter_menu(engine: State<MusicEngineHandle>) {
    engine.send(EngineCommand::EnterMenu);
}

#[tauri::command]
pub fn music_enter_grid(engine: State<MusicEngineHandle>) {
    engine.send(EngineCommand::EnterGrid);
}

/// Écoute au clic (MUSIQUE§6) : tire une piste au hasard dans le dossier, en dehors
/// de l'ambiance en cours (voir `engine::PreviewHandle`).
#[tauri::command]
pub fn music_preview_start(path: PathBuf, volume: f32, preview: State<PreviewHandle>) -> Result<(), String> {
    let tracks = scan::list_tracks(&path);
    if tracks.is_empty() {
        return Err(crate::errors::MUSIC_FOLDER_EMPTY.into());
    }
    let track = tracks[fastrand::usize(..tracks.len())].clone();
    preview.start(track, volume);
    Ok(())
}

#[tauri::command]
pub fn music_preview_stop(preview: State<PreviewHandle>) {
    preview.stop();
}

/// Now-playing notification (MUSIQUE§5.5): reads the tags of the track that
/// just started and emits `music:track` to the front.
///
/// Called by the engine for every track it starts (`engine::spawn`), on its own
/// thread — so the tags and the cover thumbnail are read on a thread of their
/// own, off the 30 ms tick that drives the fades. Two quick changes in a row
/// (menu → grid → menu) spawn two readers that may finish out of order: a
/// generation counter lets only the latest one emit, or the notification
/// would end on the track that is no longer playing.
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
