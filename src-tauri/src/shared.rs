//! Ressources partagées (§4.8) : fonts (`content/fonts`) et drivers 3D
//! (`content/driver`). Petits fichiers souvent partagés entre mods.
//!
//! Stratégie légère validée : installées **globalement** dans l'install AC, et
//! **non gérées en activation/désactivation** (les désactiver casserait les
//! autres mods qui les partagent ; un orphelin coûte quelques Ko). Le nettoyage
//! des orphelins est laissé à L5.
//!
//! Détection de collision **par contenu** lors de l'installation d'un fichier
//! déjà présent : identique (même empreinte) → silencieux ; différent → écrasé
//! par défaut et **signalé** (le vrai risque est un mélange incohérent de
//! versions, pas la présence d'un doublon identique).

use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

/// Dossier de ressources partagées repéré dans un arbre extrait.
#[derive(Debug, Clone)]
pub struct SharedDir {
    /// "fonts" | "driver"
    pub kind: &'static str,
    pub path: PathBuf,
}

/// Disposition d'une ressource partagée à l'installation.
#[derive(Debug, Clone, Serialize)]
pub struct SharedResult {
    /// "fonts" | "driver"
    pub kind: String,
    /// Nom du fichier/dossier installé.
    pub name: String,
    /// "installed" (nouveau) | "identical" (déjà là, même contenu) |
    /// "replaced" (déjà là, contenu différent → écrasé).
    pub disposition: String,
}

/// Repère les dossiers `content/fonts` et `content/driver` dans l'arbre extrait
/// (`root`). Le parent doit s'appeler `content` pour éviter les faux positifs
/// (une voiture nommée « driver », un dossier « fonts » d'un autre jeu…).
pub fn scan(root: &Path) -> Vec<SharedDir> {
    let mut out = Vec::new();
    for entry in WalkDir::new(root).into_iter().flatten() {
        if !entry.file_type().is_dir() {
            continue;
        }
        let kind = match entry.file_name().to_string_lossy().to_ascii_lowercase().as_str() {
            "fonts" => "fonts",
            "driver" => "driver",
            _ => continue,
        };
        let parent_is_content = entry
            .path()
            .parent()
            .and_then(|p| p.file_name())
            .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("content"));
        if parent_is_content {
            out.push(SharedDir { kind, path: entry.path().to_path_buf() });
        }
    }
    out
}

/// Installe globalement les ressources partagées trouvées sous `root` dans
/// `<ac>/content/{fonts,driver}` (§4.8). Toujours en **copie** (la source temp
/// peut être supprimée par ailleurs, et un import « copier » doit la préserver).
/// Renvoie la disposition de chaque entrée pour information.
pub fn install(ac_install: &Path, root: &Path) -> Vec<SharedResult> {
    let mut out = Vec::new();
    for sd in scan(root) {
        let dest_base = ac_install.join("content").join(sd.kind);
        let Ok(entries) = std::fs::read_dir(&sd.path) else {
            continue;
        };
        for e in entries.flatten() {
            let src = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            let dest = dest_base.join(&name);

            let disposition = if !dest.exists() {
                if copy_entry(&src, &dest).is_err() {
                    continue;
                }
                "installed"
            } else if hash_entry(&src) == hash_entry(&dest) {
                "identical" // même contenu → silencieux (§4.8)
            } else {
                // Contenu différent → écrasé par défaut (§4.8), et signalé.
                let _ = remove_entry(&dest);
                if copy_entry(&src, &dest).is_err() {
                    continue;
                }
                "replaced"
            };
            out.push(SharedResult {
                kind: sd.kind.to_string(),
                name,
                disposition: disposition.to_string(),
            });
        }
    }
    out
}

fn copy_entry(src: &Path, dst: &Path) -> std::io::Result<()> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if src.is_dir() {
        crate::archive::copy_dir(src, dst)
    } else {
        std::fs::copy(src, dst).map(|_| ())
    }
}

fn remove_entry(p: &Path) -> std::io::Result<()> {
    if p.is_dir() {
        std::fs::remove_dir_all(p)
    } else {
        std::fs::remove_file(p)
    }
}

/// Empreinte de contenu d'une entrée (fichier ou dossier). Fonts/drivers sont
/// petits : on hache les octets réels (« même empreinte » = vraiment identique),
/// pas seulement la taille.
fn hash_entry(p: &Path) -> String {
    let mut hasher = Sha256::new();
    if p.is_dir() {
        let mut files: Vec<(String, PathBuf)> = WalkDir::new(p)
            .into_iter()
            .flatten()
            .filter(|e| e.file_type().is_file())
            .map(|e| {
                let rel = e
                    .path()
                    .strip_prefix(p)
                    .unwrap_or(e.path())
                    .to_string_lossy()
                    .replace('\\', "/")
                    .to_lowercase();
                (rel, e.path().to_path_buf())
            })
            .collect();
        files.sort();
        for (rel, fp) in files {
            hasher.update(rel.as_bytes());
            hasher.update(b"\0");
            if let Ok(bytes) = std::fs::read(&fp) {
                hasher.update(&bytes);
            }
            hasher.update(b"\n");
        }
    } else if let Ok(bytes) = std::fs::read(p) {
        hasher.update(&bytes);
    }
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Crée un arbre <root>/content/fonts/<font> et content/driver/<drv>/model.kn5.
    fn make_pack(root: &Path, font_body: &str) {
        let content = root.join("content");
        let fonts = content.join("fonts");
        std::fs::create_dir_all(&fonts).unwrap();
        std::fs::write(fonts.join("myfont.txt"), font_body).unwrap();

        let drv = content.join("driver").join("driver_model");
        std::fs::create_dir_all(&drv).unwrap();
        std::fs::write(drv.join("model.kn5"), b"DRIVER_KN5").unwrap();
    }

    #[test]
    fn install_identical_then_replaced() {
        let base = std::env::temp_dir().join(format!("pitbox-shared-{}", uuid::Uuid::new_v4()));
        let ac = base.join("ac");
        std::fs::create_dir_all(ac.join("content")).unwrap();

        // 1er pack : tout est nouveau.
        let pack1 = base.join("pack1");
        make_pack(&pack1, "FONT_V1");
        let r1 = install(&ac, &pack1);
        assert_eq!(r1.len(), 2, "1 font + 1 driver");
        assert!(r1.iter().all(|x| x.disposition == "installed"));
        assert!(ac.join("content").join("fonts").join("myfont.txt").is_file());
        assert!(ac.join("content").join("driver").join("driver_model").join("model.kn5").is_file());

        // 2e pack identique : silencieux (identique).
        let pack2 = base.join("pack2");
        make_pack(&pack2, "FONT_V1");
        let r2 = install(&ac, &pack2);
        assert!(r2.iter().all(|x| x.disposition == "identical"), "{r2:?}");

        // 3e pack : font modifiée → écrasée (défaut), driver inchangé.
        let pack3 = base.join("pack3");
        make_pack(&pack3, "FONT_V2_DIFFERENT");
        let r3 = install(&ac, &pack3);
        let font = r3.iter().find(|x| x.kind == "fonts").unwrap();
        let drv = r3.iter().find(|x| x.kind == "driver").unwrap();
        assert_eq!(font.disposition, "replaced");
        assert_eq!(drv.disposition, "identical");
        let installed =
            std::fs::read_to_string(ac.join("content").join("fonts").join("myfont.txt")).unwrap();
        assert_eq!(installed, "FONT_V2_DIFFERENT", "la version différente a écrasé");

        let _ = std::fs::remove_dir_all(&base);
    }
}
