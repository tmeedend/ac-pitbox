//! Nettoyage (§9.3) : détection assistée des **mods cassés** (fichiers de
//! bibliothèque manquants/invalides) et des **junctions orphelines** (pointant
//! vers une version supprimée). Porté de l'esprit de `clean.py`, mais non
//! destructif sans confirmation et respectant le garde-fou junction.

use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;

use crate::config::AppConfig;
use crate::modscan::ModKind;
use crate::{activation, overlay};

fn kind_of(s: &str) -> ModKind {
    if s == "Track" {
        ModKind::Track
    } else {
        ModKind::Car
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BrokenMod {
    pub id: String,
    pub kind: String,
    pub name: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrphanJunction {
    pub kind: String,
    pub id: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MaintenanceReport {
    pub broken: Vec<BrokenMod>,
    pub orphans: Vec<OrphanJunction>,
}

/// Analyse la bibliothèque + `content/` sans rien supprimer (§9.3).
pub fn scan(conn: &Connection, cfg: &AppConfig) -> Result<MaintenanceReport, String> {
    let mut broken = Vec::new();
    let mut orphans = Vec::new();

    // --- Mods cassés : fichiers de la version active manquants/invalides ---
    for m in overlay::list_mods(conn).map_err(|e| e.to_string())? {
        let kind = kind_of(&m.kind);
        let path = m
            .active_version_id
            .as_ref()
            .and_then(|vid| overlay::get_version_path(conn, vid).ok().flatten());
        let reason = match &path {
            None => Some("aucune version active".to_string()),
            Some(p) => {
                let dir = Path::new(p);
                if !dir.is_dir() {
                    Some("fichiers de la bibliothèque introuvables".to_string())
                } else if matches!(kind, ModKind::Car) && !dir.join("ui").join("ui_car.json").is_file() {
                    Some("ui/ui_car.json manquant".to_string())
                } else if matches!(kind, ModKind::Track) && !dir.join("ui").is_dir() {
                    Some("dossier ui/ manquant".to_string())
                } else {
                    None
                }
            }
        };
        if let Some(reason) = reason {
            broken.push(BrokenMod { id: m.id_interne, kind: m.kind, name: m.display_name, reason });
        }
    }

    // --- Junctions orphelines : reparse présent mais cible illisible (supprimée) ---
    if let Some(ac) = &cfg.ac_install_path {
        for kind in [ModKind::Car, ModKind::Track] {
            let dir = ac.join("content").join(kind.content_folder());
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for e in entries.flatten() {
                    let p = e.path();
                    // Junction dont on ne peut plus lister le contenu = cible disparue.
                    if activation::is_junction(&p) && std::fs::read_dir(&p).is_err() {
                        orphans.push(OrphanJunction {
                            kind: format!("{kind:?}"),
                            id: e.file_name().to_string_lossy().into_owned(),
                            path: p.to_string_lossy().into_owned(),
                        });
                    }
                }
            }
        }
    }

    Ok(MaintenanceReport { broken, orphans })
}

/// Supprime un mod cassé : fichiers de bibliothèque (toutes versions) + junction
/// éventuelle (garde-fou) + overlay.
pub fn delete_broken(conn: &Connection, cfg: &AppConfig, id: &str) -> Result<(), String> {
    let m = overlay::get_mod(conn, id).map_err(|e| e.to_string())?.ok_or("mod introuvable")?;
    let kind = kind_of(&m.kind);

    // Fichiers : versions individuelles + dossier parent du mod dans la bibliothèque.
    for v in overlay::get_versions(conn, id).map_err(|e| e.to_string())? {
        let _ = std::fs::remove_dir_all(Path::new(&v.library_path));
    }
    if let Some(lib) = &cfg.library_path {
        let _ = std::fs::remove_dir_all(lib.join(kind.content_folder()).join(id));
    }

    // Junction éventuelle dans content/ (uniquement si c'est bien une junction).
    if let Some(ac) = &cfg.ac_install_path {
        let link = ac.join("content").join(kind.content_folder()).join(id);
        if activation::is_junction(&link) {
            let _ = std::fs::remove_dir(&link);
        }
    }

    overlay::delete_mod(conn, id).map_err(|e| e.to_string())?;
    Ok(())
}

/// Désinstalle **tout un pack** (§4.7) : supprime chaque mod partageant ce
/// `source_pack` (fichiers + junction + overlay). Renvoie le nombre supprimé.
pub fn delete_pack(conn: &Connection, cfg: &AppConfig, pack: &str) -> Result<usize, String> {
    let ids = overlay::list_pack_ids(conn, pack).map_err(|e| e.to_string())?;
    let mut n = 0;
    for id in &ids {
        // `delete_broken` réalise la suppression complète d'un mod (cf. §9.3).
        delete_broken(conn, cfg, id)?;
        n += 1;
    }
    Ok(n)
}

/// Retire une junction orpheline. Garde-fou : refuse si ce n'est pas une junction.
pub fn remove_orphan(cfg: &AppConfig, kind: &str, id: &str) -> Result<(), String> {
    let ac = cfg.ac_install_path.as_ref().ok_or("dossier AC non configuré")?;
    let link = ac.join("content").join(kind_of(kind).content_folder()).join(id);
    activation::remove_junction(&link)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_broken_mod_missing_files() {
        let base = std::env::temp_dir().join(format!("pitbox-maint-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig::default();
        let now = chrono::Local::now().to_rfc3339();

        // Mod dont la version pointe vers un dossier inexistant.
        overlay::upsert_mod(&conn, "ghost", "Car", Some("B"), Some("Ghost"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn, "v1", "ghost", Some("1.0"), None, &now,
            &base.join("nope").to_string_lossy(), None, "sig", &[], &[], &[], &[],
        )
        .unwrap();
        overlay::set_active_version(&conn, "ghost", "v1").unwrap();

        let report = scan(&conn, &cfg).unwrap();
        assert_eq!(report.broken.len(), 1);
        assert_eq!(report.broken[0].id, "ghost");

        delete_broken(&conn, &cfg, "ghost").unwrap();
        assert!(overlay::get_mod(&conn, "ghost").unwrap().is_none());

        let _ = std::fs::remove_dir_all(&base);
    }
}
