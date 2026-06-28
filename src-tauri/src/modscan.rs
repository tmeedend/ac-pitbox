//! Détection de type et descente récursive — porté de `archives.py`
//! (`isCar` / `isTrack` / `isCarSound` / `recursiveMoveModsToValidModDir`).
//!
//! Gère les archives à racine décalée, les mods imbriqués et plusieurs mods
//! dans une même archive.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ModKind {
    Car,
    Track,
}

impl ModKind {
    /// Segment de dossier dans `content/` : "cars" | "tracks".
    pub fn content_folder(self) -> &'static str {
        match self {
            ModKind::Car => "cars",
            ModKind::Track => "tracks",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FoundMod {
    pub kind: ModKind,
    /// Dossier qui deviendra `content/<type>s/<id>` (contient `ui/`, etc.).
    pub dir: PathBuf,
}

/// `<dir>/ui/ui_car.json` présent.
pub fn is_car(dir: &Path) -> bool {
    dir.is_dir() && dir.join("ui").join("ui_car.json").is_file()
}

/// `ui_track.json` à la racine `ui/` ou dans un sous-dossier de layout.
pub fn is_track(dir: &Path) -> bool {
    let ui = dir.join("ui");
    if !ui.is_dir() {
        return false;
    }
    if ui.join("ui_track.json").is_file() {
        return true;
    }
    if let Ok(entries) = std::fs::read_dir(&ui) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() && p.join("ui_track.json").is_file() {
                return true;
            }
        }
    }
    false
}

/// Mod son : présence de `*.bank` + `GUIDs.txt`. Détecté mais hors périmètre
/// car/track de L1 (cf. §14.5) — on évite simplement d'y descendre.
pub fn is_car_sound(dir: &Path) -> bool {
    if !dir.is_dir() || !dir.join("GUIDs.txt").is_file() {
        return false;
    }
    std::fs::read_dir(dir)
        .map(|entries| {
            entries.flatten().any(|e| {
                e.path()
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("bank"))
            })
        })
        .unwrap_or(false)
}

/// Descend récursivement à partir de `root` et collecte les voitures/circuits.
/// Un dossier reconnu comme mod n'est pas exploré plus profond.
pub fn scan(root: &Path) -> Vec<FoundMod> {
    let mut found = Vec::new();
    descend(root, &mut found);
    found
}

fn descend(dir: &Path, out: &mut Vec<FoundMod>) {
    if is_track(dir) {
        out.push(FoundMod { kind: ModKind::Track, dir: dir.to_path_buf() });
        return;
    }
    if is_car(dir) {
        out.push(FoundMod { kind: ModKind::Car, dir: dir.to_path_buf() });
        return;
    }
    if is_car_sound(dir) {
        // Hors périmètre L1 : on ne descend pas dedans, on ne l'importe pas.
        return;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                descend(&p, out);
            }
        }
    }
}
