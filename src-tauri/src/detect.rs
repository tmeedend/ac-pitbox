//! Détection automatique des chemins au premier démarrage (§12).
//!
//! Heuristiques Windows : bibliothèques Steam (AppID Assetto Corsa = 244210),
//! emplacements habituels de Content Manager et de 7-Zip.

use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct DetectedPaths {
    pub ac_install_path: Option<PathBuf>,
    pub content_manager_exe: Option<PathBuf>,
    pub sevenzip_exe: Option<PathBuf>,
    /// Suggestion de bibliothèque (§12), pas une détection à proprement
    /// parler — rien n'existe encore à ce chemin la première fois. Dans le
    /// dossier utilisateur plutôt que Documents/Bureau/Images : ces derniers
    /// sont ceux que Windows redirige vers OneDrive par défaut (« Sauvegarde
    /// des dossiers connus »), ce qui tenterait de synchroniser une
    /// bibliothèque de plusieurs centaines de Go vers le cloud.
    pub library_path: Option<PathBuf>,
}

/// Extrait la dernière valeur entre guillemets d'une ligne de fichier `.vdf`
/// (ex. `"path"  "D:\\SteamLibrary"` -> `D:\SteamLibrary`).
fn last_quoted(line: &str) -> Option<String> {
    let quoted: Vec<&str> = line
        .split('"')
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, s)| s)
        .collect();
    quoted.last().map(|s| s.replace("\\\\", "\\"))
}

/// Racines de bibliothèques Steam : dossiers Steam usuels + entrées de
/// `libraryfolders.vdf`.
fn steam_libraries() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let candidates = [
        PathBuf::from(r"C:\Program Files (x86)\Steam"),
        PathBuf::from(r"C:\Program Files\Steam"),
    ];
    for steam in candidates {
        if !steam.is_dir() {
            continue;
        }
        roots.push(steam.clone());
        let vdf = steam.join("steamapps").join("libraryfolders.vdf");
        if let Ok(text) = std::fs::read_to_string(&vdf) {
            for line in text.lines() {
                let l = line.trim();
                if l.starts_with("\"path\"") {
                    if let Some(p) = last_quoted(l) {
                        roots.push(PathBuf::from(p));
                    }
                }
            }
        }
    }
    roots
}

/// Cherche une install Assetto Corsa contenant un dossier `content/`.
pub fn find_ac() -> Option<PathBuf> {
    for root in steam_libraries() {
        let ac = root.join("steamapps").join("common").join("assettocorsa");
        if ac.join("content").is_dir() {
            return Some(ac);
        }
    }
    None
}

/// The executable of a Windows shell command such as
/// `"D:\Games\Content Manager.exe" "%1"`: the quoted first token, or, unquoted,
/// everything up to the first `.exe` — a path with spaces is often left
/// unquoted in handlers written by hand.
fn exe_from_command(command: &str) -> Option<PathBuf> {
    let command = command.trim();
    if let Some(rest) = command.strip_prefix('"') {
        return rest.split('"').next().filter(|s| !s.is_empty()).map(PathBuf::from);
    }
    let end = command.to_ascii_lowercase().find(".exe")? + ".exe".len();
    Some(PathBuf::from(&command[..end]))
}

/// Where the `acmanager://` protocol opens: Content Manager registers itself
/// as its handler when it runs. It is the place Windows itself uses for CM's
/// links, wherever CM was unpacked — measured on a real install in
/// `D:\Games`, which neither of the two usual folders finds.
#[cfg(windows)]
fn cm_from_protocol() -> Option<PathBuf> {
    use winreg::enums::HKEY_CLASSES_ROOT;
    use winreg::RegKey;

    let key = RegKey::predef(HKEY_CLASSES_ROOT)
        .open_subkey(r"acmanager\shell\open\command")
        .ok()?;
    let command: String = key.get_value("").ok()?;
    exe_from_command(&command)
}

#[cfg(not(windows))]
fn cm_from_protocol() -> Option<PathBuf> {
    None
}

/// Content Manager se trouve le plus souvent dans le dossier AC, sinon dans
/// `%LOCALAPPDATA%\AcTools Content Manager` — et ailleurs, n'importe où : CM
/// se décompresse où l'on veut. En dernier recours, l'association du protocole
/// `acmanager://` dit où il est.
pub fn find_cm(ac: Option<&Path>) -> Option<PathBuf> {
    if let Some(ac) = ac {
        for name in ["Content Manager.exe", "Content Manager Safe.exe"] {
            let p = ac.join(name);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let p = PathBuf::from(local)
            .join("AcTools Content Manager")
            .join("Content Manager.exe");
        if p.is_file() {
            return Some(p);
        }
    }
    cm_from_protocol().filter(|p| p.is_file())
}

/// 7-Zip standalone, sinon le `7z.exe` que Content Manager embarque pour son
/// propre usage (plugin `7Zip`, présent dès que CM a extrait une archive au
/// moins une fois) — beaucoup d'utilisateurs CM n'ont jamais installé 7-Zip
/// à part, ce chemin couvre ce cas sans dépendance supplémentaire à poser.
pub fn find_7zip() -> Option<PathBuf> {
    for p in [r"C:\Program Files\7-Zip\7z.exe", r"C:\Program Files (x86)\7-Zip\7z.exe"] {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let p = PathBuf::from(local)
            .join("AcTools Content Manager")
            .join("Plugins")
            .join("7Zip")
            .join("7z.exe");
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

pub fn autodetect() -> DetectedPaths {
    let ac = find_ac();
    let content_manager_exe = find_cm(ac.as_deref());
    let sevenzip_exe = find_7zip();
    let library_path = dirs::home_dir().map(|h| h.join("PitBox Library"));
    DetectedPaths {
        ac_install_path: ac,
        content_manager_exe,
        sevenzip_exe,
        library_path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rule: the executable of the `acmanager://` handler is read from its
    /// command line — the shape Content Manager writes (measured on a real
    /// install), and the unquoted shape a handler written by hand may have.
    #[test]
    fn the_handler_command_gives_the_executable() {
        assert_eq!(
            exe_from_command(r#""D:\Games\Content Manager.exe" "%1""#),
            Some(PathBuf::from(r"D:\Games\Content Manager.exe")),
            "quoted, as CM registers it"
        );
        assert_eq!(
            exe_from_command(r"C:\AC Tools\Content Manager.EXE %1"),
            Some(PathBuf::from(r"C:\AC Tools\Content Manager.EXE")),
            "unquoted, spaces in the path"
        );
        assert_eq!(exe_from_command(r#""""#), None, "empty");
        assert_eq!(exe_from_command("rundll32 something"), None, "no executable named");
    }
}
