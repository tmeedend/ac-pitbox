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

/// Pack de skins (§12bis.2) : dossier `<carId>` contenant un sous-dossier
/// `skins/` peuplé, mais **sans** `ui/ui_car.json` (sinon c'est une vraie
/// voiture). Le nom du dossier est la voiture cible.
pub fn is_skin_pack(dir: &Path) -> bool {
    if is_car(dir) {
        return false;
    }
    let skins = dir.join("skins");
    skins.is_dir()
        && std::fs::read_dir(&skins)
            .map(|mut e| e.any(|x| x.map(|x| x.path().is_dir()).unwrap_or(false)))
            .unwrap_or(false)
}

/// Type d'un sous-élément rattaché détecté à l'import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubKind {
    Skin,
    Sound,
}

#[derive(Debug, Clone)]
pub struct FoundSub {
    pub kind: SubKind,
    /// Voiture cible (nom du dossier) à laquelle rattacher le sous-élément.
    pub parent_id: String,
    /// Dossier source (contient `skins/` pour un skin, `GUIDs.txt`+`.bank` pour un son).
    pub dir: PathBuf,
}

/// Descend récursivement à partir de `root` et collecte les voitures/circuits.
/// Un dossier reconnu comme mod n'est pas exploré plus profond.
pub fn scan(root: &Path) -> Vec<FoundMod> {
    let mut found = Vec::new();
    descend(root, &mut found);
    found
}

/// Descend et collecte les **sous-éléments** (packs de skins, mods de son) qui
/// ne sont pas des mods de premier niveau (§12bis.2). Disjoint de `scan` : une
/// vraie voiture/circuit (avec `ui/`) est ignorée ici.
pub fn scan_subs(root: &Path) -> Vec<FoundSub> {
    let mut found = Vec::new();
    descend_subs(root, &mut found);
    found
}

fn descend_subs(dir: &Path, out: &mut Vec<FoundSub>) {
    // Vrai mod de premier niveau : géré par `scan`, pas ici.
    if is_car(dir) || is_track(dir) {
        return;
    }
    let dir_name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if is_skin_pack(dir) {
        out.push(FoundSub { kind: SubKind::Skin, parent_id: dir_name, dir: dir.to_path_buf() });
        return;
    }
    if is_car_sound(dir) {
        out.push(FoundSub { kind: SubKind::Sound, parent_id: dir_name, dir: dir.to_path_buf() });
        return;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                descend_subs(&p, out);
            }
        }
    }
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
