//! Sous-éléments rattachés (§12bis.2) : skins et sons. Routés à l'import vers
//! un stockage **séparé** dans la bibliothèque (`<lib>/skins/<parent>/<skin>` et
//! `<lib>/sounds/<parent>/<nom>`), tracés dans l'overlay `sub_mods`, sans jamais
//! polluer la bibliothèque principale (§12bis.3).
//!
//! Asymétrie (§12bis.2) :
//! - **Skin** : pas d'activation filesystem. Pour qu'AC le charge, il est
//!   **projeté** par junction dans le dossier `skins/` de la voiture cible
//!   (`<parent skins>/<skin>` → stockage séparé). Tous les skins présents sont
//!   disponibles.
//! - **Son** : exclusif (un seul actif). La bascule réelle des fichiers `sfx/`
//!   est un lot suivant — ici on **stocke et enregistre** le son (inactif).

use std::path::{Path, PathBuf};

use chrono::Local;
use rusqlite::Connection;
use serde::Serialize;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::modscan::{FoundSub, SubKind};
use crate::{activation, archive, overlay};

#[derive(Debug, Clone, Serialize)]
pub struct SubImported {
    /// "SKIN" | "SOUND"
    pub sub_type: String,
    pub parent_id: String,
    pub name: String,
    /// Skin projeté (visible par AC) ; faux si le parent est inconnu/conflit.
    pub projected: bool,
    pub warning: Option<String>,
}

/// Importe les sous-éléments détectés (§12bis.2). `copy` préserve la source.
pub fn import_subs(
    conn: &Connection,
    cfg: &AppConfig,
    library: &Path,
    source_name: &str,
    subs: &[FoundSub],
    copy: bool,
) -> Vec<SubImported> {
    let mut out = Vec::new();
    for sub in subs {
        match sub.kind {
            SubKind::Skin => import_skin_pack(conn, cfg, library, source_name, sub, copy, &mut out),
            SubKind::Sound => import_sound(conn, library, source_name, sub, copy, &mut out),
        }
    }
    out
}

fn copy_or_move(src: &Path, dst: &Path, copy: bool) -> Result<(), String> {
    if copy {
        archive::copy_dir(src, dst).map_err(|e| e.to_string())
    } else {
        archive::move_dir(src, dst).map_err(|e| e.to_string())
    }
}

fn import_skin_pack(
    conn: &Connection,
    cfg: &AppConfig,
    library: &Path,
    source_name: &str,
    sub: &FoundSub,
    copy: bool,
    out: &mut Vec<SubImported>,
) {
    let parent = &sub.parent_id;
    let Ok(entries) = std::fs::read_dir(sub.dir.join("skins")) else {
        return;
    };
    for e in entries.flatten() {
        let skin_src = e.path();
        if !skin_src.is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();

        // Idempotence : ne ré-importe pas un skin déjà connu pour ce parent.
        if overlay::sub_exists(conn, "SKIN", parent, &name).unwrap_or(false) {
            continue;
        }

        let dest = library.join("skins").join(parent).join(&name);
        if let Err(err) = copy_or_move(&skin_src, &dest, copy) {
            out.push(SubImported {
                sub_type: "SKIN".into(),
                parent_id: parent.clone(),
                name,
                projected: false,
                warning: Some(format!("stockage : {err}")),
            });
            continue;
        }

        let id = Uuid::new_v4().to_string();
        let _ = overlay::insert_sub_mod(
            conn,
            &id,
            "SKIN",
            parent,
            &name,
            &dest.to_string_lossy(),
            Some(source_name),
            &Local::now().to_rfc3339(),
        );

        // Projection : junction dans le skins/ de la voiture cible.
        let (projected, warning) = project_skin(conn, cfg, parent, &name, &dest);
        out.push(SubImported {
            sub_type: "SKIN".into(),
            parent_id: parent.clone(),
            name,
            projected,
            warning,
        });
    }
}

/// Projette un skin stocké séparément dans le `skins/` de la voiture cible via
/// junction, pour qu'AC le charge (§12bis.2). Best-effort.
fn project_skin(
    conn: &Connection,
    cfg: &AppConfig,
    parent_id: &str,
    skin_name: &str,
    store: &Path,
) -> (bool, Option<String>) {
    let Some(skins_dir) = parent_skins_dir(conn, cfg, parent_id) else {
        return (false, Some("voiture cible inconnue : skin non projeté".into()));
    };
    if let Err(e) = std::fs::create_dir_all(&skins_dir) {
        return (false, Some(format!("création skins/ : {e}")));
    }
    let link = skins_dir.join(skin_name);
    if link.exists() {
        // Déjà présent (vrai dossier ou junction) : on ne touche à rien.
        return (false, Some(format!("« {skin_name} » déjà présent dans skins/ — non projeté")));
    }
    match activation::create_junction(&link, store) {
        Ok(()) => (true, None),
        Err(e) => (false, Some(format!("projection : {e}"))),
    }
}

/// Dossier `skins/` de la voiture cible : version active en bibliothèque si
/// c'est un mod géré, sinon `content/cars/<id>/skins` (voiture de base Kunos).
fn parent_skins_dir(conn: &Connection, cfg: &AppConfig, parent_id: &str) -> Option<PathBuf> {
    if let Ok(Some(m)) = overlay::get_mod(conn, parent_id) {
        if !m.is_stock {
            if let Some(vid) = m.active_version_id {
                if let Ok(Some(p)) = overlay::get_version_path(conn, &vid) {
                    return Some(Path::new(&p).join("skins"));
                }
            }
        }
    }
    cfg.ac_install_path
        .as_ref()
        .map(|ac| ac.join("content").join("cars").join(parent_id).join("skins"))
}

fn import_sound(
    conn: &Connection,
    library: &Path,
    source_name: &str,
    sub: &FoundSub,
    copy: bool,
    out: &mut Vec<SubImported>,
) {
    let parent = &sub.parent_id;
    let name = sub
        .dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| source_name.to_string());

    if overlay::sub_exists(conn, "SOUND", parent, &name).unwrap_or(false) {
        return;
    }

    let dest = library.join("sounds").join(parent).join(&name);
    if let Err(err) = copy_or_move(&sub.dir, &dest, copy) {
        out.push(SubImported {
            sub_type: "SOUND".into(),
            parent_id: parent.clone(),
            name,
            projected: false,
            warning: Some(format!("stockage : {err}")),
        });
        return;
    }

    let id = Uuid::new_v4().to_string();
    let _ = overlay::insert_sub_mod(
        conn,
        &id,
        "SOUND",
        parent,
        &name,
        &dest.to_string_lossy(),
        Some(source_name),
        &Local::now().to_rfc3339(),
    );
    // Bascule exclusive des fichiers sfx/ = lot suivant (§12bis.2). Stocké inactif.
    out.push(SubImported {
        sub_type: "SOUND".into(),
        parent_id: parent.clone(),
        name,
        projected: false,
        warning: Some("son stocké (bascule à venir)".into()),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modscan;

    #[test]
    fn skin_pack_routed_and_stored() {
        let base = std::env::temp_dir().join(format!("pitbox-sub-{}", Uuid::new_v4()));
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig::default();

        // Pack de skins : <carId>/skins/<skin>/preview.jpg (pas de ui/ → sous-élément).
        let pack = base.join("src").join("ferrari_488");
        let skin = pack.join("skins").join("af_corse_51");
        std::fs::create_dir_all(&skin).unwrap();
        std::fs::write(skin.join("preview.jpg"), b"IMG").unwrap();

        // Détection : un sous-élément SKIN, parent = ferrari_488.
        let subs = modscan::scan_subs(&base.join("src"));
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].parent_id, "ferrari_488");
        // Pas confondu avec une voiture.
        assert!(modscan::scan(&base.join("src")).is_empty());

        // Import (copie) : stocké à part + enregistré dans sub_mods.
        let res = import_subs(&conn, &cfg, &library, "ferrari_skins.7z", &subs, true);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].sub_type, "SKIN");
        assert!(library.join("skins").join("ferrari_488").join("af_corse_51").join("preview.jpg").is_file());
        let stored = overlay::list_subs_for_parent(&conn, "ferrari_488").unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].name, "af_corse_51");

        // Idempotence : ré-import → pas de doublon.
        let res2 = import_subs(&conn, &cfg, &library, "ferrari_skins.7z", &modscan::scan_subs(&base.join("src")), true);
        assert!(res2.is_empty());
        assert_eq!(overlay::list_subs_for_parent(&conn, "ferrari_488").unwrap().len(), 1);

        let _ = std::fs::remove_dir_all(&base);
    }
}
