//! Steam soundtracks (§16.7): the folders of `<library>/steamapps/music/`,
//! where Steam installs a soundtrack bought on its own, offered as shortcuts in
//! the music pickers. Nothing else of Steam is scanned — not `steamapps/common`,
//! where a game's own files live — and nothing is assigned automatically.
//!
//! The libraries come from the Steam install the registry names (the usual
//! folders otherwise) and its `libraryfolders.vdf`, read by `detect.rs` — no
//! crate for it: those two reads were already in the app.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::scan;

/// What `rodio` actually decodes (`scan::AUDIO_EXTENSIONS` also counts `.m4a`,
/// which it does not): a soundtrack of `.m4a` alone would be offered and play
/// nothing.
const PLAYABLE: [&str; 4] = ["mp3", "flac", "ogg", "wav"];

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Soundtrack {
    /// The folder's name, or `<name> — <subfolder>` when the tracks are one
    /// level down (`MP3/`, `FLAC/`: a common layout).
    pub name: String,
    pub path: PathBuf,
    pub track_count: usize,
}

fn playable_count(dir: &Path) -> usize {
    scan::list_tracks(dir)
        .iter()
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| PLAYABLE.iter().any(|x| x.eq_ignore_ascii_case(e)))
        })
        .count()
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|it| {
            it.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default();
    dirs.sort();
    dirs
}

fn dir_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The soundtracks of one `steamapps/music` folder. A soundtrack whose tracks
/// sit at its root is one source; one that splits them in subfolders (one per
/// format) gives one source per subfolder that plays — a source plays the
/// files of its own folder, not those below it.
pub fn soundtracks_in(music_dir: &Path) -> Vec<Soundtrack> {
    let mut out = Vec::new();
    for album in subdirs(music_dir) {
        let n = playable_count(&album);
        if n > 0 {
            out.push(Soundtrack {
                name: dir_name(&album),
                path: album,
                track_count: n,
            });
            continue;
        }
        for sub in subdirs(&album) {
            let n = playable_count(&sub);
            if n > 0 {
                out.push(Soundtrack {
                    name: format!("{} — {}", dir_name(&album), dir_name(&sub)),
                    path: sub,
                    track_count: n,
                });
            }
        }
    }
    out
}

/// The Steam install the registry names (`HKCU\Software\Valve\Steam`,
/// `SteamPath`, written with forward slashes), then the usual folders.
#[cfg(windows)]
fn steam_installs() -> Vec<PathBuf> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let mut out: Vec<PathBuf> = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Valve\Steam")
        .and_then(|k| k.get_value::<String, _>("SteamPath"))
        .ok()
        .map(|p| PathBuf::from(p.replace('/', "\\")))
        .filter(|p| p.is_dir())
        .into_iter()
        .collect();
    out.extend(crate::detect::usual_steam_dirs());
    out
}

#[cfg(not(windows))]
fn steam_installs() -> Vec<PathBuf> {
    Vec::new()
}

/// Every soundtrack of every Steam library, by name. Reads the disk: called
/// off the window thread, once, at the first opening of a picker.
pub fn list() -> Vec<Soundtrack> {
    let mut roots: Vec<PathBuf> = steam_installs()
        .iter()
        .flat_map(|s| crate::detect::library_roots(s))
        .collect();
    // The same library reached twice (the registry and a usual folder, or a
    // spelling of the path in different case) is listed once.
    roots.sort_by_key(|p| p.to_string_lossy().to_lowercase());
    roots.dedup_by_key(|p| p.to_string_lossy().to_lowercase());
    let mut all: Vec<Soundtrack> = roots
        .iter()
        .flat_map(|r| soundtracks_in(&r.join("steamapps").join("music")))
        .collect();
    all.sort_by_key(|s| s.name.to_lowercase());
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    // Rule (§16.7): a soundtrack is offered only if it holds a track that
    // plays; tracks one level down give one entry per subfolder.
    #[test]
    fn only_folders_with_a_playable_track_are_offered() {
        let base = crate::testutil::temp_dir("music-steam");
        let music = base.join("music");
        let flat = music.join("Flat OST");
        let split = music.join("Split OST");
        let empty = music.join("Art Book");
        let m4a = music.join("Apple OST");
        for d in [&flat, &split.join("MP3"), &split.join("FLAC"), &empty, &m4a] {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(flat.join("01.mp3"), b"x").unwrap();
        std::fs::write(flat.join("02.ogg"), b"x").unwrap();
        std::fs::write(split.join("MP3").join("01.mp3"), b"x").unwrap();
        std::fs::write(split.join("FLAC").join("01.flac"), b"x").unwrap();
        std::fs::write(empty.join("page.pdf"), b"x").unwrap();
        std::fs::write(m4a.join("01.m4a"), b"x").unwrap();

        let found = soundtracks_in(&music);
        let names: Vec<_> = found.iter().map(|s| (s.name.as_str(), s.track_count)).collect();
        assert_eq!(
            names,
            vec![("Flat OST", 2), ("Split OST — FLAC", 1), ("Split OST — MP3", 1)],
            "no PDF-only folder, no m4a-only folder (rodio cannot play it)"
        );
    }

    #[test]
    fn a_library_without_a_music_folder_gives_nothing() {
        let base = crate::testutil::temp_dir("music-steam-none");
        assert!(soundtracks_in(&base.join("steamapps").join("music")).is_empty());
    }
}
