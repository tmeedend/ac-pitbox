//! Music sources (§16.3): a source is a **folder** (a playlist) or a single
//! **file** (played in a loop). This module lists what a source plays and
//! counts it for the pickers; the gain and duration of each track are the
//! business of `index.rs`.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Extensions listées par la spec (MUSIQUE§3.2). `rodio` (features par défaut) ne
/// décode que mp3/flac/ogg/wav ; un `.m4a` est compté ici pour l'information
/// affichée à l'utilisateur mais échouera à l'ouverture en lecture — géré
/// comme n'importe quel fichier corrompu (MUSIQUE§9, piste suivante + log).
pub const AUDIO_EXTENSIONS: [&str; 5] = ["mp3", "flac", "ogg", "wav", "m4a"];

pub fn is_audio_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| AUDIO_EXTENSIONS.iter().any(|a| a.eq_ignore_ascii_case(ext)))
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceInfo {
    pub track_count: usize,
    /// The source is one file, looped — not a folder.
    pub is_file: bool,
    /// The source no longer exists (moved, deleted, a drive unplugged).
    pub missing: bool,
}

/// What a source holds, for the "N tracks" of a picker (MUSIQUE§6). A missing
/// or unreadable source counts 0, never an error (silent fallback, MUSIQUE§9).
pub fn scan_source(source: &Path) -> SourceInfo {
    let is_file = source.is_file();
    SourceInfo {
        track_count: list_tracks(source).len(),
        is_file,
        missing: !is_file && !source.is_dir(),
    }
}

/// The tracks a source plays, sorted by file name: a stable order before the
/// shuffle. A file source is its own one-track playlist — which is exactly a
/// loop, since a playlist that runs out starts again.
pub fn list_tracks(source: &Path) -> Vec<PathBuf> {
    if source.is_file() {
        return if is_audio_file(source) {
            vec![source.to_path_buf()]
        } else {
            Vec::new()
        };
    }
    let mut tracks: Vec<PathBuf> = std::fs::read_dir(source)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| is_audio_file(p))
                .collect()
        })
        .unwrap_or_default();
    tracks.sort();
    tracks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_only_known_audio_extensions() {
        let dir = crate::testutil::temp_dir("music-scan");
        std::fs::write(dir.join("track01.mp3"), b"x").unwrap();
        std::fs::write(dir.join("track02.FLAC"), b"x").unwrap();
        std::fs::write(dir.join("cover.jpg"), b"x").unwrap();
        std::fs::write(dir.join("readme.txt"), b"x").unwrap();

        let info = scan_source(&dir);
        assert_eq!(
            info.track_count, 2,
            "seuls mp3/flac sont comptés, extension insensible à la casse"
        );
        assert!(!info.is_file && !info.missing);
    }

    #[test]
    fn missing_source_reports_zero_not_an_error() {
        let info = scan_source(Path::new(r"Z:\does-not-exist-pitbox"));
        assert_eq!(info.track_count, 0);
        assert!(info.missing, "said missing, for the orphan mark of the pickers");
    }

    #[test]
    fn list_tracks_is_sorted_and_filtered() {
        let dir = crate::testutil::temp_dir("music-list");
        std::fs::write(dir.join("b.ogg"), b"x").unwrap();
        std::fs::write(dir.join("a.wav"), b"x").unwrap();
        std::fs::write(dir.join("skip.png"), b"x").unwrap();

        let tracks = list_tracks(&dir);
        let names: Vec<_> = tracks
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, vec!["a.wav", "b.ogg"]);
    }

    // Rule (§16.3): a file is a source of its own, one track looped; a file
    // that is not audio plays nothing.
    #[test]
    fn a_file_source_is_a_playlist_of_itself() {
        let dir = crate::testutil::temp_dir("music-file-source");
        let track = dir.join("theme.ogg");
        std::fs::write(&track, b"x").unwrap();
        std::fs::write(dir.join("cover.jpg"), b"x").unwrap();

        assert_eq!(list_tracks(&track), vec![track.clone()]);
        let info = scan_source(&track);
        assert!(info.is_file && info.track_count == 1);
        assert!(
            list_tracks(&dir.join("cover.jpg")).is_empty(),
            "not audio: nothing to play"
        );
    }
}
