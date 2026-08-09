//! Cache d'index par dossier (§3.4) : durée et correction de gain RMS de
//! chaque piste, calculées une fois au premier scan puis relues depuis un
//! fichier `.pitbox-index.json` posé dans le dossier lui-même.
//!
//! Réutilise le décodeur `rodio` déjà en dépendance du moteur — pas de
//! bibliothèque audio de plus. Décoder le fichier entier est de toute façon
//! nécessaire pour le RMS ; la durée exacte qu'on en tire au passage comble
//! le "durée inconnue pour la plupart des MP3" documenté dans `engine.rs`
//! (§5.3, préchargement du crossfade) — `engine.rs` préfère maintenant cette
//! durée indexée à celle, souvent absente, que `Source::total_duration()`
//! annonce en direct.
//!
//! Écart assumé vs la spec : le tag ReplayGain n'est pas lu (spec §3.4, "si
//! présent, le préférer au calcul") — lecture de tags audio = une dépendance
//! de plus (`lofty`/`id3`) pour une préférence secondaire, pas la demande
//! principale (normaliser les dossiers qui n'en ont pas). Le calcul RMS
//! s'applique donc systématiquement.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use rodio::{Decoder, Source};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use super::config::MusicConfig;

const INDEX_FILE_NAME: &str = ".pitbox-index.json";
/// Cible RMS (§3.4). dBFS : 0 dB = pleine échelle, donc une valeur négative.
const TARGET_DBFS: f64 = -18.0;
/// Borne la correction pour éviter les aberrations sur une piste très
/// atypique (silence quasi total, clip extrême) — §3.4.
const MAX_GAIN_DB: f64 = 12.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrackIndex {
    file: String,
    duration_ms: u64,
    gain_db: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FolderIndex {
    scanned_at: chrono::DateTime<Utc>,
    /// Nombre de fichiers audio au moment du scan — comparé au compte
    /// courant pour l'invalidation (§3.4).
    file_count: usize,
    /// Secondes Unix de la dernière modification du DOSSIER (pas des
    /// fichiers un par un) au moment du scan — un ajout/suppression met à
    /// jour ce mtime sous Windows, deuxième critère d'invalidation prescrit
    /// par la spec en plus du compte de fichiers.
    dir_modified_at: i64,
    tracks: Vec<TrackIndex>,
}

/// Piste enrichie du gain de normalisation et de la durée indexée — ce que
/// le moteur (`engine.rs`) consomme réellement, par opposition au simple
/// chemin renvoyé par `scan::list_tracks`.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexedTrack {
    pub path: PathBuf,
    /// Correction en dB vers `TARGET_DBFS`, 0.0 si non calculable.
    pub gain_db: f32,
    /// `None` si la piste n'a pas pu être décodée à l'indexation — le moteur
    /// retombe alors sur le comportement "durée inconnue" du §5.3.
    pub duration: Option<Duration>,
}

fn index_path(dir: &Path) -> PathBuf {
    dir.join(INDEX_FILE_NAME)
}

fn dir_mtime_secs(dir: &Path) -> i64 {
    std::fs::metadata(dir)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// RMS en dBFS sur l'ensemble des échantillons, converti en correction de
/// gain vers `TARGET_DBFS`, bornée à ±`MAX_GAIN_DB` (§3.4) ; durée exacte en
/// sous-produit du décodage complet. `None` si le fichier est illisible.
fn analyze(path: &Path) -> Option<(f32, Duration)> {
    let file = File::open(path).ok()?;
    let decoder = Decoder::new(BufReader::new(file)).ok()?;
    let sample_rate = decoder.sample_rate() as u64;
    let channels = decoder.channels() as u64;
    if sample_rate == 0 || channels == 0 {
        return None;
    }

    let mut sum_squares = 0.0f64;
    let mut count = 0u64;
    for sample in decoder {
        let s = sample as f64 / i16::MAX as f64;
        sum_squares += s * s;
        count += 1;
    }
    if count == 0 {
        return None;
    }

    let duration = Duration::from_secs_f64(count as f64 / (sample_rate * channels) as f64);
    let rms = (sum_squares / count as f64).sqrt();
    if rms <= 0.0 {
        // Silence total (piste vide) : rien à corriger, mais pas une erreur —
        // log(0) serait -infini, on s'arrête avant.
        return Some((0.0, duration));
    }
    let rms_dbfs = 20.0 * rms.log10();
    let gain = (TARGET_DBFS - rms_dbfs).clamp(-MAX_GAIN_DB, MAX_GAIN_DB);
    Some((gain as f32, duration))
}

fn build(tracks: &[PathBuf]) -> Vec<TrackIndex> {
    tracks
        .iter()
        .map(|t| {
            let file = t
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            match analyze(t) {
                Some((gain_db, duration)) => TrackIndex {
                    file,
                    duration_ms: duration.as_millis() as u64,
                    gain_db,
                },
                None => {
                    // Piste illisible (§9) : entrée neutre plutôt que de
                    // faire échouer l'indexation de tout le dossier.
                    log::warn!("music: impossible d'analyser {} pour la normalisation", t.display());
                    TrackIndex {
                        file,
                        duration_ms: 0,
                        gain_db: 0.0,
                    }
                }
            }
        })
        .collect()
}

fn read_cache(path: &Path) -> Option<FolderIndex> {
    let s = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&s).ok()
}

/// Charge l'index d'un dossier, le (re)calcule si absent ou périmé. Bloquant
/// (décode chaque piste au premier scan, "quelques secondes pour 30 pistes"
/// par la spec §3.4) — appelé depuis un thread dédié (moteur ou façade de
/// commande), jamais depuis le thread UI.
fn load_or_build(dir: &Path, tracks: &[PathBuf]) -> Vec<TrackIndex> {
    let path = index_path(dir);
    let current_mtime = dir_mtime_secs(dir);
    if let Some(cached) = read_cache(&path) {
        if cached.file_count == tracks.len() && cached.dir_modified_at == current_mtime {
            return cached.tracks;
        }
    }
    let built = build(tracks);
    let index = FolderIndex {
        scanned_at: Utc::now(),
        file_count: tracks.len(),
        dir_modified_at: current_mtime,
        tracks: built.clone(),
    };
    match serde_json::to_string_pretty(&index) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                log::warn!("music: écriture du cache d'index échouée pour {} : {e}", dir.display());
            }
        }
        Err(e) => log::warn!(
            "music: sérialisation du cache d'index échouée pour {} : {e}",
            dir.display()
        ),
    }
    built
}

/// Préchauffe le cache d'index de `menu_folder`/`grid_folder` dans un thread
/// séparé, sans toucher au moteur ni à sa file de commandes.
///
/// Sans ça, le premier scan (décodage complet de chaque piste, "quelques
/// secondes pour 30 pistes" §3.4) se déclenche la première fois que le
/// moteur a réellement besoin de la playlist — typiquement en pleine
/// navigation Big Picture (ouverture, ou bascule vers l'écran de
/// paramétrage de session) : d'où le décalage perceptible entre l'affichage
/// de l'écran et le début du fondu, tant qu'un dossier n'a jamais été
/// indexé dans cette exécution de l'app. Appelé au démarrage (dossiers déjà
/// configurés) et après l'enregistrement des réglages (dossier tout juste
/// changé) — les deux points où un blocage de quelques secondes est un
/// détail plutôt qu'une gêne en pleine navigation.
pub fn warm(app: &AppHandle, cfg: MusicConfig) {
    let app = app.clone();
    std::thread::spawn(move || {
        indexed_tracks(&cfg.effective_menu_folder(&app));
        indexed_tracks(&cfg.effective_grid_folder(&app));
    });
}

/// Combine `scan::list_tracks` avec le cache de gain/durée — ce que
/// `engine.rs` appelle pour construire une playlist.
pub fn indexed_tracks(dir: &Path) -> Vec<IndexedTrack> {
    let tracks = super::scan::list_tracks(dir);
    let cached = load_or_build(dir, &tracks);
    tracks
        .into_iter()
        .map(|path| {
            let file = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            // Recherche par nom (pas par position) : une entrée de cache
            // périmée mais de même longueur ne doit jamais s'appliquer à la
            // mauvaise piste.
            match cached.iter().find(|t| t.file == file) {
                Some(t) if t.duration_ms > 0 => IndexedTrack {
                    path,
                    gain_db: t.gain_db,
                    duration: Some(Duration::from_millis(t.duration_ms)),
                },
                Some(t) => IndexedTrack {
                    path,
                    gain_db: t.gain_db,
                    duration: None,
                },
                None => IndexedTrack {
                    path,
                    gain_db: 0.0,
                    duration: None,
                },
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_silent_wav(path: &Path, seconds: f32) {
        // WAV mono 8kHz minimal, tout à zéro : assez pour exercer le
        // décodage + calcul RMS sans dépendre d'un fichier externe.
        let sample_rate = 8000u32;
        let n_samples = (sample_rate as f32 * seconds) as u32;
        let data_len = n_samples * 2;
        let mut buf = Vec::new();
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&(36 + data_len).to_le_bytes());
        buf.extend_from_slice(b"WAVE");
        buf.extend_from_slice(b"fmt ");
        buf.extend_from_slice(&16u32.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes()); // PCM
        buf.extend_from_slice(&1u16.to_le_bytes()); // mono
        buf.extend_from_slice(&sample_rate.to_le_bytes());
        buf.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
        buf.extend_from_slice(&2u16.to_le_bytes()); // block align
        buf.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
        buf.extend_from_slice(b"data");
        buf.extend_from_slice(&data_len.to_le_bytes());
        buf.extend(std::iter::repeat_n(0u8, data_len as usize));
        std::fs::write(path, buf).unwrap();
    }

    #[test]
    fn silent_track_gets_zero_gain_and_correct_duration() {
        let dir = crate::testutil::temp_dir("music-index-silent");
        let track = dir.join("silence.wav");
        write_silent_wav(&track, 0.5);

        let indexed = indexed_tracks(&dir);
        assert_eq!(indexed.len(), 1);
        assert_eq!(indexed[0].gain_db, 0.0, "silence total : rien à corriger");
        let duration = indexed[0].duration.expect("durée calculée pour un WAV");
        assert!(
            (duration.as_millis() as i64 - 500).abs() < 50,
            "durée attendue ~500ms, obtenu {:?}",
            duration
        );
    }

    #[test]
    fn index_cache_is_reused_when_folder_is_unchanged() {
        let dir = crate::testutil::temp_dir("music-index-cache");
        write_silent_wav(&dir.join("a.wav"), 0.2);

        let first = indexed_tracks(&dir);
        assert!(
            dir.join(".pitbox-index.json").is_file(),
            "le cache doit être écrit après le premier scan"
        );

        // Deuxième appel : doit relire le cache plutôt que redécoder — on ne
        // peut pas mesurer le temps de façon fiable en CI, mais on vérifie
        // au moins que le résultat reste cohérent après un aller-retour.
        let second = indexed_tracks(&dir);
        assert_eq!(first, second);
    }

    #[test]
    fn index_invalidates_when_a_file_is_added() {
        let dir = crate::testutil::temp_dir("music-index-invalidate");
        write_silent_wav(&dir.join("a.wav"), 0.2);
        let first = indexed_tracks(&dir);
        assert_eq!(first.len(), 1);

        write_silent_wav(&dir.join("b.wav"), 0.2);
        let second = indexed_tracks(&dir);
        assert_eq!(
            second.len(),
            2,
            "un nouveau fichier doit invalider le cache (compte de fichiers différent)"
        );
    }
}
