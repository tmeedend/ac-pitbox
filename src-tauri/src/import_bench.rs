//! Persisted import speed benchmark (§4.2bis).
//!
//! Import duration is dominated by bytes moved, but the seconds-per-byte
//! depends entirely on the machine: an NVMe drive and a USB2 disk are two
//! orders of magnitude apart. Rather than shipping a constant that is wrong
//! for everyone, the app measures its own throughput and keeps it across
//! restarts, so the very first import of a session already has a usable
//! estimate.
//!
//! Stored as a small JSON file written with a synchronous `std::fs::write`,
//! never in `localStorage` nor in the overlay database — same rule and same
//! reasons as `session_state.rs`.
//!
//! **Accuracy is not the point.** The benchmark only sets the *relative*
//! weight of the items in a batch; the absolute scale is recalibrated live
//! from the elapsed time of the batch in progress (see
//! `import_progress::BatchState::eta`). A benchmark off by a factor of two
//! therefore produces a wrong ETA for a few seconds, then converges.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// Which cost a measurement belongs to. Extraction and filing are learnt
/// separately because they overlap when the batch is pipelined (§4.2bis) —
/// a single combined rate could not weight the two streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bucket {
    /// 7-Zip decompression, per byte of *archive*.
    ArchiveExtract,
    /// Everything after extraction, also per byte of archive: the expansion
    /// ratio is baked into the learnt rate, which avoids walking the work
    /// directory just to size it.
    ArchiveFile,
    /// Folder import preserving the source (physical copy).
    FolderCopy,
    /// Folder import consuming the source — a same-volume `rename` is nearly
    /// free, but the overlay writes, hashing and activation are not.
    FolderMove,
}

/// One accumulator. Amortised rather than averaged: each new measurement
/// decays the previous total, which weights a 2 GB mod more than a 3 MB skin
/// without any explicit weighting, and lets old measurements fade out when
/// the user moves their library to another drive.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(default)]
struct Accumulator {
    bytes: f64,
    secs: f64,
    samples: u32,
}

/// Decay applied to the running totals before each new measurement.
const DECAY: f64 = 0.85;

/// Below this, a measurement says more about scheduling noise than about
/// throughput — recording it would poison the rate.
const MIN_SAMPLE_BYTES: f64 = 512.0 * 1024.0;
const MIN_SAMPLE_SECS: f64 = 0.05;

/// Fixed per-item cost, independent of size: overlay writes, `ui_*.json`
/// parsing, activation. Without it a batch of two hundred 2 MB skins would be
/// estimated at nearly zero.
pub const ITEM_OVERHEAD_SECS: f64 = 0.6;

/// Starting rates, in seconds per byte, used until the machine has measured
/// itself. Deliberately rough — see the module docs on why that is enough.
const DEFAULT_ARCHIVE_EXTRACT: f64 = 1.0 / 60_000_000.0;
const DEFAULT_ARCHIVE_FILE: f64 = 1.6 / 120_000_000.0;
const DEFAULT_FOLDER_COPY: f64 = 1.0 / 150_000_000.0;
const DEFAULT_FOLDER_MOVE: f64 = 1.0 / 400_000_000.0;

/// `#[serde(default)]` on every field: a bucket added in a later version reads
/// back as "never measured" from an existing file, no migration to write —
/// same convention as `config::Prefs`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Bench {
    archive_extract: Accumulator,
    archive_file: Accumulator,
    folder_copy: Accumulator,
    folder_move: Accumulator,
}

impl Bench {
    fn accumulator(&self, bucket: Bucket) -> (&Accumulator, f64) {
        match bucket {
            Bucket::ArchiveExtract => (&self.archive_extract, DEFAULT_ARCHIVE_EXTRACT),
            Bucket::ArchiveFile => (&self.archive_file, DEFAULT_ARCHIVE_FILE),
            Bucket::FolderCopy => (&self.folder_copy, DEFAULT_FOLDER_COPY),
            Bucket::FolderMove => (&self.folder_move, DEFAULT_FOLDER_MOVE),
        }
    }

    fn accumulator_mut(&mut self, bucket: Bucket) -> &mut Accumulator {
        match bucket {
            Bucket::ArchiveExtract => &mut self.archive_extract,
            Bucket::ArchiveFile => &mut self.archive_file,
            Bucket::FolderCopy => &mut self.folder_copy,
            Bucket::FolderMove => &mut self.folder_move,
        }
    }

    /// Seconds per byte for this bucket, falling back to the shipped default
    /// while nothing has been measured.
    fn rate(&self, bucket: Bucket) -> f64 {
        let (acc, default) = self.accumulator(bucket);
        if acc.samples == 0 || acc.bytes <= 0.0 || acc.secs <= 0.0 {
            return default;
        }
        acc.secs / acc.bytes
    }

    /// Estimated seconds for `bytes` in this bucket, per-item overhead excluded
    /// (the caller adds it once per item, not once per phase).
    pub fn estimate(&self, bucket: Bucket, bytes: u64) -> f64 {
        bytes as f64 * self.rate(bucket)
    }

    /// Folds one real measurement in. Ignores samples too small to carry
    /// information.
    pub fn record(&mut self, bucket: Bucket, bytes: u64, secs: f64) {
        if (bytes as f64) < MIN_SAMPLE_BYTES || secs < MIN_SAMPLE_SECS || !secs.is_finite() {
            return;
        }
        let acc = self.accumulator_mut(bucket);
        acc.bytes = acc.bytes * DECAY + bytes as f64;
        acc.secs = acc.secs * DECAY + secs;
        acc.samples = acc.samples.saturating_add(1);
    }
}

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("import_bench.json"))
}

/// Defaults if the file does not exist yet or cannot be read — first run, or a
/// corrupted file: a benchmark is never worth blocking an import for.
pub fn load(app: &AppHandle) -> Bench {
    let Some(path) = file(app) else {
        return Bench::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => Bench::default(),
    }
}

/// Best-effort: a benchmark that fails to persist only costs a less accurate
/// estimate next time, so it must never surface as an import error — but it is
/// logged, since nothing else would show it on a packaged install (§9.4).
pub fn save(app: &AppHandle, bench: &Bench) {
    let Some(path) = file(app) else { return };
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            log::warn!("import_bench: create config dir: {e}");
            return;
        }
    }
    match serde_json::to_string_pretty(bench) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                log::warn!("import_bench: write {}: {e}", path.display());
            }
        }
        Err(e) => log::warn!("import_bench: serialize: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Règle : sans mesure, l'estimation tombe sur la valeur par défaut du
    /// seau — jamais zéro, sinon la barre globale part à 100 % au premier item.
    #[test]
    fn estimate_falls_back_to_default_rate() {
        let bench = Bench::default();
        let secs = bench.estimate(Bucket::ArchiveExtract, 60_000_000);
        assert!(
            secs > 0.5 && secs < 2.0,
            "environ une seconde pour 60 Mo, obtenu {secs}"
        );
    }

    /// Règle : une mesure réelle remplace la valeur par défaut, et un gros
    /// échantillon pèse plus qu'un petit (c'est l'amortissement qui pondère).
    #[test]
    fn record_shifts_rate_towards_measurement() {
        let mut bench = Bench::default();
        // 100 Mo en 10 s = 10 Mo/s, bien plus lent que le défaut.
        bench.record(Bucket::ArchiveExtract, 100_000_000, 10.0);
        let secs = bench.estimate(Bucket::ArchiveExtract, 100_000_000);
        assert!(
            (secs - 10.0).abs() < 0.1,
            "l'estimation suit la mesure unique, obtenu {secs}"
        );
    }

    /// Règle : un échantillon minuscule ne dit rien du débit — l'enregistrer
    /// ferait basculer le taux sur du bruit d'ordonnancement.
    #[test]
    fn record_ignores_samples_too_small_to_mean_anything() {
        let mut bench = Bench::default();
        let before = bench.estimate(Bucket::FolderCopy, 1_000_000_000);
        bench.record(Bucket::FolderCopy, 1_024, 3.0);
        let after = bench.estimate(Bucket::FolderCopy, 1_000_000_000);
        assert_eq!(before, after, "un échantillon de 1 Ko ne change rien");
    }

    /// Règle : l'amortissement fait oublier les anciennes mesures — sinon un
    /// changement de disque resterait invisible pendant des centaines d'imports.
    #[test]
    fn old_measurements_fade_out() {
        let mut bench = Bench::default();
        for _ in 0..40 {
            bench.record(Bucket::FolderMove, 100_000_000, 10.0);
        }
        for _ in 0..40 {
            bench.record(Bucket::FolderMove, 100_000_000, 1.0);
        }
        let secs = bench.estimate(Bucket::FolderMove, 100_000_000);
        assert!(secs < 1.5, "le nouveau débit a supplanté l'ancien, obtenu {secs}");
    }
}
