//! Inspection d'un dossier de mod (lecture seule) : features CSP, skins, layouts.
//! Porté de la détection CSP de `tracks.py` (sections d'`ext_config.ini`).

use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use walkdir::WalkDir;

use crate::modscan::ModKind;

/// Détecte les features CSP en lisant le `ext_config.ini` embarqué du mod.
/// (À l'import, le mod n'est pas encore installé : on lit sa propre config.)
pub fn csp_features(mod_dir: &Path) -> Vec<String> {
    let mut feats = Vec::new();
    let candidates = [
        mod_dir.join("extension").join("ext_config.ini"),
        mod_dir.join("ext_config.ini"),
    ];
    for cfg in candidates {
        let Ok(text) = std::fs::read_to_string(&cfg) else {
            continue;
        };
        let upper = text.to_uppercase();
        if upper.contains("[GRASS_FX") {
            feats.push("grassfx".to_string());
        }
        if upper.contains("[RAIN_FX") {
            feats.push("rainfx".to_string());
        }
        if upper.contains("[LIGHT_SERIES_1") {
            feats.push("lightingfx".to_string());
        }
        if upper.contains("SEASON_WINTER") {
            feats.push("weatherfx".to_string());
        }
    }
    feats.sort();
    feats.dedup();
    feats
}

/// Skins d'une voiture : sous-dossiers de `skins/`.
pub fn car_skins(car_dir: &Path) -> Vec<String> {
    list_subdirs(&car_dir.join("skins"))
}

/// Layouts d'un circuit : sous-dossiers de `ui/` contenant un `ui_track.json`.
/// Renvoie un layout par défaut si le circuit est mono-layout.
pub fn track_layouts(track_dir: &Path) -> Vec<String> {
    let ui = track_dir.join("ui");
    let mut layouts = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&ui) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() && p.join("ui_track.json").is_file() {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    layouts.push(name.to_string());
                }
            }
        }
    }
    layouts.sort();
    if layouts.is_empty() && ui.join("ui_track.json").is_file() {
        layouts.push("(default)".to_string());
    }
    layouts
}

/// Date de publication estimée (§6.2) : date de modification la plus récente
/// parmi les fichiers du mod, lue **avant** rangement en bibliothèque. Pour une
/// archive, `dir` est le dossier d'extraction temporaire — 7-Zip restitue sur
/// les fichiers extraits les dates internes de l'archive, donc la valeur
/// obtenue reflète ces dates internes, pas l'instant de l'extraction.
pub fn estimate_published_at(dir: &Path) -> Option<String> {
    let newest = WalkDir::new(dir)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .max()?;
    let dt: DateTime<Local> = newest.into();
    Some(dt.to_rfc3339())
}

fn list_subdirs(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            if e.path().is_dir() {
                if let Some(name) = e.file_name().to_str() {
                    out.push(name.to_string());
                }
            }
        }
    }
    out.sort();
    out
}

/// Vignettes de preview AC : presque toujours `.jpg`, parfois `.png`.
const PREVIEW_NAMES: [&str; 2] = ["preview.jpg", "preview.png"];
const OUTLINE_NAMES: [&str; 4] = ["outline.png", "outline.jpg", "preview.png", "preview.jpg"];

fn first_existing(dir: &Path, names: &[&str]) -> Option<String> {
    for n in names {
        let p = dir.join(n);
        if p.is_file() {
            return Some(p.to_string_lossy().into_owned());
        }
    }
    None
}

/// Cherche une image de circuit (`names`) dans `ui/`, puis dans le premier
/// sous-dossier de layout qui en contient une.
fn track_find(mod_dir: &Path, names: &[&str]) -> Option<String> {
    let ui = mod_dir.join("ui");
    if let Some(p) = first_existing(&ui, names) {
        return Some(p);
    }
    std::fs::read_dir(&ui)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .find_map(|p| first_existing(&p, names))
}

/// Image illustratrice d'un circuit (`preview.png`) — le fond de la vignette.
pub fn track_preview(mod_dir: &Path) -> Option<String> {
    track_find(mod_dir, &["preview.png", "preview.jpg"])
}

/// Tracé d'un circuit (`outline.png`/`map.png`) — superposé à la photo (§6.1).
pub fn track_outline(mod_dir: &Path) -> Option<String> {
    track_find(mod_dir, &["outline.png", "outline.jpg", "map.png"])
}

/// Chemin absolu d'une vignette de preview pour la galerie (§6.1), si trouvée.
/// Voiture : preview du premier skin qui en a une. Circuit : outline/preview.
pub fn preview_path(kind: ModKind, mod_dir: &Path) -> Option<String> {
    match kind {
        ModKind::Car => {
            let mut skins: Vec<PathBuf> = std::fs::read_dir(mod_dir.join("skins"))
                .ok()?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            skins.sort();
            skins.iter().find_map(|s| first_existing(s, &PREVIEW_NAMES))
        }
        ModKind::Track => {
            let ui = mod_dir.join("ui");
            if let Some(p) = first_existing(&ui, &OUTLINE_NAMES) {
                return Some(p);
            }
            // layouts : premier sous-dossier ui/ avec un outline.
            let entries = std::fs::read_dir(&ui).ok()?;
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .find_map(|p| first_existing(&p, &OUTLINE_NAMES))
        }
    }
}
