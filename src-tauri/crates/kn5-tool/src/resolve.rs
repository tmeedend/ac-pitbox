//! Picking the model file inside a car folder (spec §4.2).
//!
//! Lives here rather than in the `kn5` crate, which is pure parsing with no
//! filesystem access. Lot 4 will need the same logic inside the application;
//! it moves to a shared place then, once the Tauri side tells us what shape it
//! actually needs.

use std::path::{Path, PathBuf};

/// Where the chosen model came from — reported by the tool so a surprising
/// choice can be traced back to the rule that made it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelSource {
    /// `data/lods.ini` named it explicitly.
    LodsIni,
    /// Picked by the heuristic: largest remaining candidate.
    Heuristic,
}

pub struct ResolvedModel {
    pub path: PathBuf,
    pub source: ModelSource,
}

/// Suffixes of the lower LOD models, which are never the main one.
const LOD_SUFFIXES: [&str; 4] = ["_lodb", "_lodc", "_lodd", "_lode"];

/// Resolves the LOD A model of a car folder.
///
/// Returns `None` when the folder holds no usable model — a skin-only or
/// data-only folder, which is a normal thing to meet while scanning
/// `content/cars` and not an error.
pub fn resolve_model(car_dir: &Path) -> Option<ResolvedModel> {
    if let Some(path) = from_lods_ini(car_dir) {
        return Some(ResolvedModel {
            path,
            source: ModelSource::LodsIni,
        });
    }
    largest_candidate(car_dir).map(|path| ResolvedModel {
        path,
        source: ModelSource::Heuristic,
    })
}

/// Reads `[LOD_0] FILE=` from `data/lods.ini`, when the data folder is
/// unpacked. Packed cars keep it inside the encrypted `data.acd`, which we
/// deliberately do not decrypt (§4.2) — the heuristic covers those.
fn from_lods_ini(car_dir: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(car_dir.join("data").join("lods.ini")).ok()?;
    let mut in_lod0 = false;
    for line in text.lines() {
        let line = line.split(';').next().unwrap_or("").trim();
        if line.starts_with('[') {
            in_lod0 = line.eq_ignore_ascii_case("[LOD_0]");
            continue;
        }
        if !in_lod0 {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            if key.trim().eq_ignore_ascii_case("FILE") {
                let name = value.trim();
                if name.is_empty() {
                    return None;
                }
                let path = car_dir.join(name);
                // The ini can name a file that was never shipped; falling
                // through to the heuristic beats reporting a missing file.
                return path.is_file().then_some(path);
            }
        }
    }
    None
}

/// Every `*.kn5` at the root of the folder, minus the collider and the lower
/// LODs; largest one wins (§4.2).
fn largest_candidate(car_dir: &Path) -> Option<PathBuf> {
    let mut best: Option<(u64, PathBuf)> = None;
    for entry in std::fs::read_dir(car_dir).ok()?.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()).map(str::to_ascii_lowercase) else {
            continue;
        };
        let is_kn5 = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("kn5"));
        if !is_kn5 || stem == "collider" || LOD_SUFFIXES.iter().any(|s| stem.ends_with(s)) {
            continue;
        }
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        if best.as_ref().is_none_or(|(best_size, _)| size > *best_size) {
            best = Some((size, path));
        }
    }
    best.map(|(_, path)| path)
}
