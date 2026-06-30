//! Activation / désactivation par **directory junctions** Windows (§7).
//! Zéro duplication, pas de droits admin (contrairement aux symlinks).
//!
//! Garde-fou absolu : on ne supprime JAMAIS dans `content/` un dossier qui
//! n'est pas une junction créée par l'app (protection du contenu installé hors
//! de l'app). Détection junction vs vrai dossier obligatoire avant suppression.

use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use chrono::Local;
use rusqlite::Connection;

use crate::config::AppConfig;
use crate::modscan::ModKind;
use crate::overlay;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn kind_of(s: &str) -> ModKind {
    if s == "Track" {
        ModKind::Track
    } else {
        ModKind::Car
    }
}

/// Vrai si le chemin existe et est un point de reparse (junction/symlink).
/// `symlink_metadata` ne suit pas le lien : un vrai dossier renvoie `false`.
pub fn is_junction(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

fn content_link(cfg: &AppConfig, kind: ModKind, id: &str) -> Option<PathBuf> {
    cfg.ac_install_path
        .as_ref()
        .map(|ac| ac.join("content").join(kind.content_folder()).join(id))
}

/// Vrai si le mod est actif (junction gérée présente dans content/).
pub fn is_mod_active(cfg: &AppConfig, kind: ModKind, id: &str) -> bool {
    content_link(cfg, kind, id).is_some_and(|l| is_junction(&l))
}

/// Crée une junction `link` → `target` via `mklink /J` (sans fenêtre console).
pub fn create_junction(link: &Path, target: &Path) -> Result<(), String> {
    let mut cmd = Command::new("cmd");
    cmd.arg("/C");
    // raw_arg : on maîtrise nous-mêmes les guillemets (chemins à espaces).
    #[cfg(windows)]
    {
        cmd.raw_arg(format!(
            "mklink /J \"{}\" \"{}\"",
            link.display(),
            target.display()
        ));
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = (link, target);
        return Err("junctions disponibles uniquement sous Windows".into());
    }

    let out = cmd
        .output()
        .map_err(|e| format!("impossible de créer la junction : {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "mklink a échoué : {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Supprime la junction `link` (sans toucher à la cible). Refuse si ce n'est
/// pas une junction (garde-fou).
pub fn remove_junction(link: &Path) -> Result<(), String> {
    if !is_junction(link) {
        return Err("refus : le chemin n'est pas une junction".into());
    }
    std::fs::remove_dir(link).map_err(|e| format!("suppression de la junction : {e}"))
}

const GUARD_MSG: &str =
    "Un vrai dossier (non-junction) existe déjà dans content/ — opération refusée pour protéger un contenu installé hors de l'app.";

/// Active un mod : crée la junction `content/<type>s/<id>` → version choisie.
/// Si `version_id` est fourni, il devient la version active.
pub fn activate(
    conn: &Connection,
    cfg: &AppConfig,
    mod_id: &str,
    version_id: Option<&str>,
) -> Result<(), String> {
    let m = overlay::get_mod(conn, mod_id)
        .map_err(|e| e.to_string())?
        .ok_or("mod introuvable")?;
    if m.is_stock {
        return Err("contenu de base Kunos : déjà présent, non activable (§12bis.1)".into());
    }
    let kind = kind_of(&m.kind);

    let vid = version_id
        .map(str::to_string)
        .or(m.active_version_id)
        .ok_or("aucune version à activer")?;
    let target = overlay::get_version_path(conn, &vid)
        .map_err(|e| e.to_string())?
        .ok_or("version introuvable")?;
    let link = content_link(cfg, kind, mod_id).ok_or("dossier AC non configuré")?;

    // Garde-fou + nettoyage d'une éventuelle junction existante.
    match std::fs::symlink_metadata(&link) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                remove_junction(&link)?;
            } else {
                return Err(GUARD_MSG.into());
            }
        }
        Err(_) => {} // n'existe pas : OK
    }

    create_junction(&link, Path::new(&target))?;
    overlay::set_active_version(conn, mod_id, &vid).map_err(|e| e.to_string())?;
    let label = version_label(conn, &vid);
    overlay::add_history(conn, mod_id, &Local::now().to_rfc3339(), "ACTIVATE", &label)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Désactive un mod : retire la junction (le contenu reste dans la bibliothèque).
pub fn deactivate(conn: &Connection, cfg: &AppConfig, mod_id: &str) -> Result<(), String> {
    let m = overlay::get_mod(conn, mod_id)
        .map_err(|e| e.to_string())?
        .ok_or("mod introuvable")?;
    let kind = kind_of(&m.kind);
    let link = content_link(cfg, kind, mod_id).ok_or("dossier AC non configuré")?;

    match std::fs::symlink_metadata(&link) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                remove_junction(&link)?;
                overlay::add_history(conn, mod_id, &Local::now().to_rfc3339(), "DEACTIVATE", "")
                    .map_err(|e| e.to_string())?;
            } else {
                return Err(GUARD_MSG.into());
            }
        }
        Err(_) => {} // déjà inactif
    }
    Ok(())
}

fn version_label(conn: &Connection, version_id: &str) -> String {
    conn.query_row(
        "SELECT COALESCE(version_label, '') FROM versions WHERE id = ?1",
        [version_id],
        |r| r.get::<_, String>(0),
    )
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn junction_create_remove_and_guard() {
        // Windows uniquement (mklink). Ignoré ailleurs.
        if !cfg!(windows) {
            return;
        }
        let base =
            std::env::temp_dir().join(format!("pitbox-junc-{}", uuid::Uuid::new_v4()));
        let target = base.join("target");
        let link = base.join("link");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("file.txt"), b"hello").unwrap();

        // Création + détection.
        create_junction(&link, &target).expect("junction créée");
        assert!(is_junction(&link), "doit être détectée comme junction");
        assert!(link.join("file.txt").is_file(), "contenu visible via la junction");

        // Suppression de la junction : la cible reste intacte.
        remove_junction(&link).expect("junction supprimée");
        assert!(!link.exists());
        assert!(target.join("file.txt").is_file(), "cible préservée");

        // Garde-fou : remove_junction refuse un vrai dossier.
        let real = base.join("real");
        std::fs::create_dir_all(&real).unwrap();
        assert!(remove_junction(&real).is_err(), "refus sur un vrai dossier");
        assert!(real.exists(), "vrai dossier non supprimé");

        let _ = std::fs::remove_dir_all(&base);
    }
}
