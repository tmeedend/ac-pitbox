//! Index cache per source (MUSIQUE§3.4, §16.3): duration and RMS gain
//! correction of each track, computed once at the first scan and read back
//! from a JSON file of **the app's own folder** (`app_config_dir/Music/index/`)
//! — one file per source path. Nothing is ever written in the user's music
//! folders: until v2 the cache was a `.pitbox-index.json` placed in the folder
//! itself, which wrote into the user's library and failed on a read-only one.
//! Those old files are left where they are, never read again.
//!
//! Réutilise le décodeur `rodio` déjà en dépendance du moteur — pas de
//! bibliothèque audio de plus. Décoder le fichier entier est de toute façon
//! nécessaire pour le RMS ; la durée exacte qu'on en tire au passage comble
//! le "durée inconnue pour la plupart des MP3" documenté dans `engine.rs`
//! (MUSIQUE§5.3, préchargement du crossfade).
//!
//! Écart assumé vs la spec : le tag ReplayGain n'est pas lu (MUSIQUE§3.4, "si
//! présent, le préférer au calcul"). Le calcul RMS s'applique donc
//! systématiquement.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use rodio::{Decoder, Source};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use super::config::{Base, MusicConfig};

/// Cible RMS (MUSIQUE§3.4). dBFS : 0 dB = pleine échelle, donc une valeur négative.
const TARGET_DBFS: f64 = -18.0;
/// Borne la correction pour éviter les aberrations sur une piste très
/// atypique (silence quasi total, clip extrême) — MUSIQUE§3.4.
const MAX_GAIN_DB: f64 = 12.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrackIndex {
    file: String,
    duration_ms: u64,
    gain_db: f32,
}

/// What the cache is checked against (§16.3). A folder: its number of audio
/// files and its modification date (an addition or a removal updates it under
/// Windows). A file: its modification date and its size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct Signature {
    count: usize,
    modified: i64,
    size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SourceIndex {
    /// The source this file indexes: the file name is a hash of it, and a
    /// collision must read as a stale cache, not as someone else's gains.
    source: PathBuf,
    scanned_at: chrono::DateTime<Utc>,
    signature: Signature,
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
    /// retombe alors sur le comportement "durée inconnue" du MUSIQUE§5.3.
    pub duration: Option<Duration>,
}

/// FNV-1a over the lowercased path: a stable name for the cache file of a
/// source. Not `DefaultHasher`, whose output may change between Rust releases
/// — every cache would then be rebuilt after an update for nothing.
fn cache_key(source: &Path) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in source.to_string_lossy().to_lowercase().bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn cache_file(cache_dir: &Path, source: &Path) -> PathBuf {
    cache_dir.join(format!("{}.json", cache_key(source)))
}

fn secs(t: std::io::Result<std::time::SystemTime>) -> i64 {
    t.ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn signature(source: &Path, tracks: &[PathBuf]) -> Signature {
    let meta = std::fs::metadata(source);
    let modified = meta.as_ref().map(|m| secs(m.modified())).unwrap_or(0);
    let is_file = meta.as_ref().is_ok_and(|m| m.is_file());
    Signature {
        count: tracks.len(),
        modified,
        size: if is_file { meta.map(|m| m.len()).unwrap_or(0) } else { 0 },
    }
}

/// RMS en dBFS sur l'ensemble des échantillons, converti en correction de
/// gain vers `TARGET_DBFS`, bornée à ±`MAX_GAIN_DB` (MUSIQUE§3.4) ; durée exacte en
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

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn build(tracks: &[PathBuf]) -> Vec<TrackIndex> {
    tracks
        .iter()
        .map(|t| {
            let file = file_name(t);
            match analyze(t) {
                Some((gain_db, duration)) => TrackIndex {
                    file,
                    duration_ms: duration.as_millis() as u64,
                    gain_db,
                },
                None => {
                    // Piste illisible (MUSIQUE§9) : entrée neutre plutôt que de
                    // faire échouer l'indexation de toute la source.
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

fn read_cache(path: &Path) -> Option<SourceIndex> {
    let s = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&s).ok()
}

/// Charge l'index d'une source, le (re)calcule si absent ou périmé. Bloquant
/// (décode chaque piste au premier scan, "quelques secondes pour 30 pistes"
/// par la spec MUSIQUE§3.4) — appelé depuis un thread dédié (moteur ou façade de
/// commande), jamais depuis le thread UI.
fn load_or_build(cache_dir: &Path, source: &Path, tracks: &[PathBuf]) -> Vec<TrackIndex> {
    let path = cache_file(cache_dir, source);
    let current = signature(source, tracks);
    if let Some(cached) = read_cache(&path) {
        if cached.source == source && cached.signature == current {
            return cached.tracks;
        }
    }
    let built = build(tracks);
    let index = SourceIndex {
        source: source.to_path_buf(),
        scanned_at: Utc::now(),
        signature: current,
        tracks: built.clone(),
    };
    let written = std::fs::create_dir_all(cache_dir)
        .map_err(|e| e.to_string())
        .and_then(|()| serde_json::to_string_pretty(&index).map_err(|e| e.to_string()))
        .and_then(|json| std::fs::write(&path, json).map_err(|e| e.to_string()));
    if let Err(e) = written {
        log::warn!("music: cache d'index non écrit pour {} : {e}", source.display());
    }
    built
}

/// Préchauffe les caches d'index dans un thread séparé, sans toucher au moteur
/// ni à sa file de commandes : les deux ambiances de base, et — la musique de
/// l'utilisateur allumée — chaque source attribuée (§16.4).
///
/// Sans ça, le premier scan (décodage complet de chaque piste) se déclenche la
/// première fois que le moteur a réellement besoin de la playlist — en pleine
/// navigation, d'où un décalage perceptible entre l'écran et le fondu. Appelé
/// au démarrage et après l'enregistrement des réglages, les deux points où un
/// blocage de quelques secondes est un détail.
pub fn warm(app: &AppHandle, cfg: MusicConfig) {
    let cache_dir = super::config::index_dir(app);
    let mut sources = vec![cfg.effective_base(app, Base::Menu), cfg.effective_base(app, Base::Grid)];
    if cfg.use_custom_folders {
        let a = &cfg.assignments;
        for map in [&a.brand, &a.family, &a.track_category, &a.country, &a.screen] {
            sources.extend(map.values().cloned());
        }
    }
    sources.sort();
    sources.dedup();
    std::thread::spawn(move || {
        for source in sources {
            indexed_tracks(&cache_dir, &source);
        }
    });
}

/// Combine `scan::list_tracks` avec le cache de gain/durée — ce que
/// `engine.rs` appelle pour construire une playlist.
pub fn indexed_tracks(cache_dir: &Path, source: &Path) -> Vec<IndexedTrack> {
    let tracks = super::scan::list_tracks(source);
    if tracks.is_empty() {
        // Nothing to index: no cache file for a missing source.
        return Vec::new();
    }
    let cached = load_or_build(cache_dir, source, &tracks);
    tracks
        .into_iter()
        .map(|path| {
            let file = file_name(&path);
            // Recherche par nom (pas par position) : une entrée de cache
            // périmée mais de même longueur ne doit jamais s'appliquer à la
            // mauvaise piste.
            match cached.iter().find(|t| t.file == file) {
                Some(t) => IndexedTrack {
                    path,
                    gain_db: t.gain_db,
                    duration: (t.duration_ms > 0).then(|| Duration::from_millis(t.duration_ms)),
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
        let base = crate::testutil::temp_dir("music-index-silent");
        let (music, cache) = (base.join("music"), base.join("cache"));
        std::fs::create_dir_all(&music).unwrap();
        write_silent_wav(&music.join("silence.wav"), 0.5);

        let indexed = indexed_tracks(&cache, &music);
        assert_eq!(indexed.len(), 1);
        assert_eq!(indexed[0].gain_db, 0.0, "silence total : rien à corriger");
        let duration = indexed[0].duration.expect("durée calculée pour un WAV");
        assert!(
            (duration.as_millis() as i64 - 500).abs() < 50,
            "durée attendue ~500ms, obtenu {:?}",
            duration
        );
    }

    // Rule (§16.3): the cache lives in the app's own folder, one file per
    // source — never in the user's music folder.
    #[test]
    fn the_cache_is_written_in_the_app_folder_and_never_in_the_source() {
        let base = crate::testutil::temp_dir("music-index-cache");
        let (music, cache) = (base.join("music"), base.join("cache"));
        std::fs::create_dir_all(&music).unwrap();
        write_silent_wav(&music.join("a.wav"), 0.2);

        let first = indexed_tracks(&cache, &music);
        assert!(
            cache_file(&cache, &music).is_file(),
            "cache written under the cache dir"
        );
        let in_source: Vec<_> = std::fs::read_dir(&music)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(in_source, vec!["a.wav"], "nothing written next to the tracks");

        let second = indexed_tracks(&cache, &music);
        assert_eq!(first, second, "read back from the cache, same result");
    }

    #[test]
    fn index_invalidates_when_a_file_is_added() {
        let base = crate::testutil::temp_dir("music-index-invalidate");
        let (music, cache) = (base.join("music"), base.join("cache"));
        std::fs::create_dir_all(&music).unwrap();
        write_silent_wav(&music.join("a.wav"), 0.2);
        assert_eq!(indexed_tracks(&cache, &music).len(), 1);

        write_silent_wav(&music.join("b.wav"), 0.2);
        let second = indexed_tracks(&cache, &music);
        assert_eq!(
            second.len(),
            2,
            "un nouveau fichier doit invalider le cache (compte de fichiers différent)"
        );
        assert!(
            second.iter().all(|t| t.duration.is_some()),
            "the new track was indexed too"
        );
    }

    // Rule (§16.3): a file source has a cache of its own, checked against the
    // file's date and size — a file replaced by a longer one is re-indexed.
    #[test]
    fn a_file_source_is_indexed_and_invalidated_by_its_size() {
        let base = crate::testutil::temp_dir("music-index-file");
        let cache = base.join("cache");
        let track = base.join("loop.wav");
        write_silent_wav(&track, 0.2);

        let first = indexed_tracks(&cache, &track);
        assert_eq!(first.len(), 1);
        let short = first[0].duration.unwrap();

        write_silent_wav(&track, 0.6);
        let second = indexed_tracks(&cache, &track);
        assert!(
            second[0].duration.unwrap() > short,
            "bigger file: re-indexed, new duration {:?} > {:?}",
            second[0].duration,
            short
        );
    }

    #[test]
    fn two_sources_get_two_cache_files() {
        assert_ne!(
            cache_key(Path::new(r"D:\Music\menu")),
            cache_key(Path::new(r"D:\Music\grid"))
        );
        assert_eq!(
            cache_key(Path::new(r"D:\Music\Menu")),
            cache_key(Path::new(r"d:\music\menu")),
            "Windows paths are case-insensitive: one cache for both spellings"
        );
    }
}
