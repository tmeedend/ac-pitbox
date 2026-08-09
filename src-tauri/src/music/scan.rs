//! Scan de dossiers musicaux (§3.4, périmètre réduit) : compte les pistes
//! pour l'affichage dans l'onglet Musique, liste les fichiers pour le moteur
//! de lecture. Pas de cache d'index ni de normalisation RMS — voir `mod.rs`
//! pour la justification du report.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Extensions listées par la spec (§3.2). `rodio` (features par défaut) ne
/// décode que mp3/flac/ogg/wav ; un `.m4a` est compté ici pour l'information
/// affichée à l'utilisateur mais échouera à l'ouverture en lecture — géré
/// comme n'importe quel fichier corrompu (§9, piste suivante + log).
pub const AUDIO_EXTENSIONS: [&str; 5] = ["mp3", "flac", "ogg", "wav", "m4a"];

fn is_audio_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| AUDIO_EXTENSIONS.iter().any(|a| a.eq_ignore_ascii_case(ext)))
}

#[derive(Debug, Clone, Serialize)]
pub struct FolderInfo {
    pub track_count: usize,
}

/// Nombre de pistes lisibles d'un dossier — pour l'affichage "N pistes
/// détectées" (§6). Dossier absent ou illisible = 0, jamais une erreur (repli
/// silencieux, §9).
pub fn scan_folder(dir: &Path) -> FolderInfo {
    let track_count = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| is_audio_file(&e.path()))
                .count()
        })
        .unwrap_or(0);
    FolderInfo { track_count }
}

/// Liste triée des pistes d'un dossier, pour le moteur de lecture. Tri par
/// nom de fichier : ordre stable et déterministe avant tirage aléatoire.
pub fn list_tracks(dir: &Path) -> Vec<PathBuf> {
    let mut tracks: Vec<PathBuf> = std::fs::read_dir(dir)
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

        let info = scan_folder(&dir);
        assert_eq!(
            info.track_count, 2,
            "seuls mp3/flac sont comptés, extension insensible à la casse"
        );
    }

    #[test]
    fn missing_folder_reports_zero_not_an_error() {
        let info = scan_folder(Path::new(r"Z:\does-not-exist-pitbox"));
        assert_eq!(info.track_count, 0);
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
}
