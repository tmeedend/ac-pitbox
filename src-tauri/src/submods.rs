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
    // Skin de circuit (TRACK_SKIN) ou de voiture (SKIN) ? Stockage et type adaptés.
    let track = is_track_skin(conn, parent, &sub.dir);
    let sub_type = if track { "TRACK_SKIN" } else { "SKIN" };
    let store_root = if track { "track_skins" } else { "skins" };

    // `sub.dir` contient directement les dossiers de skins (les deux formes
    // d'arborescence sont déjà résolues par modscan).
    let Ok(entries) = std::fs::read_dir(&sub.dir) else {
        return;
    };
    for e in entries.flatten() {
        let skin_src = e.path();
        if !skin_src.is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();

        // Idempotence : ne ré-importe pas un skin déjà connu pour ce parent.
        if overlay::sub_exists(conn, sub_type, parent, &name).unwrap_or(false) {
            continue;
        }

        let dest = library.join(store_root).join(parent).join(&name);
        if let Err(err) = copy_or_move(&skin_src, &dest, copy) {
            out.push(SubImported {
                sub_type: sub_type.into(),
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
            sub_type,
            parent,
            &name,
            &dest.to_string_lossy(),
            Some(source_name),
            &Local::now().to_rfc3339(),
        );

        // Projection : junction dans le skins/ de l'entité cible (voiture ou circuit).
        let (projected, warning) = project_skin(conn, cfg, parent, &name, &dest);
        out.push(SubImported {
            sub_type: sub_type.into(),
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

fn parent_skins_dir(conn: &Connection, cfg: &AppConfig, parent_id: &str) -> Option<PathBuf> {
    parent_subdir(conn, cfg, parent_id, "skins")
}

/// Dossier `<sub>/` de l'entité cible (voiture ou circuit) : version active en
/// bibliothèque si c'est un mod géré, sinon `content/<type>s/<id>/<sub>` (base
/// Kunos). Le type est déduit de l'overlay (Car → cars, Track → tracks).
fn parent_subdir(conn: &Connection, cfg: &AppConfig, parent_id: &str, sub: &str) -> Option<PathBuf> {
    let m = overlay::get_mod(conn, parent_id).ok().flatten();
    if let Some(m) = &m {
        if !m.is_stock {
            if let Some(vid) = &m.active_version_id {
                if let Ok(Some(p)) = overlay::get_version_path(conn, vid) {
                    return Some(Path::new(&p).join(sub));
                }
            }
        }
    }
    let folder = if m.as_ref().map(|m| m.kind.as_str()) == Some("Track") { "tracks" } else { "cars" };
    cfg.ac_install_path
        .as_ref()
        .map(|ac| ac.join("content").join(folder).join(parent_id).join(sub))
}

/// Détermine si un pack de skins cible un **circuit** (TRACK_SKIN) : parent connu
/// comme circuit dans l'overlay, ou chemin sous un dossier `tracks/`.
fn is_track_skin(conn: &Connection, parent_id: &str, src: &Path) -> bool {
    if let Ok(Some(m)) = overlay::get_mod(conn, parent_id) {
        return m.kind == "Track";
    }
    src.components().any(|c| c.as_os_str().to_string_lossy().eq_ignore_ascii_case("tracks"))
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
    out.push(SubImported {
        sub_type: "SOUND".into(),
        parent_id: parent.clone(),
        name,
        projected: false,
        warning: None,
    });
}

// --- Bascule exclusive du son (§12bis.2) ------------------------------------

/// Active un mod de son : remplace réellement le `sfx/` de la voiture par les
/// fichiers du mod (bascule exclusive). Le son d'origine est **sauvegardé une
/// fois** pour pouvoir y revenir — jamais détruit irréversiblement (§12bis.2).
pub fn activate_sound(conn: &Connection, cfg: &AppConfig, sub_id: &str) -> Result<(), String> {
    let sub = overlay::get_sub_mod(conn, sub_id).map_err(|e| e.to_string())?.ok_or("son introuvable")?;
    if sub.sub_type != "SOUND" {
        return Err("ce sous-élément n'est pas un mod de son".into());
    }
    let sfx = parent_subdir(conn, cfg, &sub.parent_id, "sfx").ok_or("voiture cible inconnue")?;
    let backup = sound_backup_dir(cfg, &sub.parent_id)?;

    // Sauvegarde du son d'origine, une seule fois (préserve le vrai original).
    if !backup.exists() {
        std::fs::create_dir_all(&backup).map_err(|e| e.to_string())?;
        if sfx.is_dir() {
            archive::copy_dir(&sfx, &backup).map_err(|e| format!("sauvegarde du son d'origine : {e}"))?;
        }
    }

    replace_dir_contents(Path::new(&sub.library_path), &sfx)?;
    overlay::set_active_sound(conn, &sub.parent_id, Some(sub_id)).map_err(|e| e.to_string())?;
    Ok(())
}

/// Restaure le son d'origine d'une voiture (désactive le mod de son actif).
pub fn restore_sound(conn: &Connection, cfg: &AppConfig, parent_id: &str) -> Result<(), String> {
    let backup = sound_backup_dir(cfg, parent_id)?;
    if backup.is_dir() {
        let sfx = parent_subdir(conn, cfg, parent_id, "sfx").ok_or("voiture cible inconnue")?;
        replace_dir_contents(&backup, &sfx)?;
    }
    overlay::set_active_sound(conn, parent_id, None).map_err(|e| e.to_string())?;
    Ok(())
}

/// Supprime proprement un sous-élément (§12bis.3) : retire la junction de
/// projection (skin) ou restaure le son d'origine (son actif), efface les
/// fichiers stockés, puis la ligne overlay. Garde-fou junction respecté.
pub fn remove_sub(conn: &Connection, cfg: &AppConfig, sub_id: &str) -> Result<(), String> {
    let sub = overlay::get_sub_mod(conn, sub_id).map_err(|e| e.to_string())?.ok_or("sous-élément introuvable")?;
    match sub.sub_type.as_str() {
        "SKIN" | "TRACK_SKIN" => {
            // Retire la junction de projection dans le skins/ de l'entité cible.
            if let Some(skins_dir) = parent_subdir(conn, cfg, &sub.parent_id, "skins") {
                let link = skins_dir.join(&sub.name);
                if activation::is_junction(&link) {
                    let _ = activation::remove_junction(&link);
                }
            }
        }
        "SOUND" => {
            // Si actif, on rétablit d'abord le son d'origine.
            if sub.is_active {
                restore_sound(conn, cfg, &sub.parent_id)?;
            }
        }
        _ => {}
    }
    // Fichiers stockés à part.
    let _ = std::fs::remove_dir_all(Path::new(&sub.library_path));
    overlay::delete_sub_mod(conn, sub_id).map_err(|e| e.to_string())
}

/// `<lib>/sounds/<parent>/__original__` : sauvegarde du son d'origine.
fn sound_backup_dir(cfg: &AppConfig, parent_id: &str) -> Result<PathBuf, String> {
    let lib = cfg.library_path.as_ref().ok_or("bibliothèque non configurée")?;
    Ok(lib.join("sounds").join(parent_id).join("__original__"))
}

/// Remplace le contenu de `dst` par celui de `src`. `dst` est toujours un vrai
/// dossier (sous-dossier `sfx/` de la voiture), jamais une junction.
fn replace_dir_contents(src: &Path, dst: &Path) -> Result<(), String> {
    if dst.exists() {
        std::fs::remove_dir_all(dst).map_err(|e| format!("nettoyage de {}: {e}", dst.display()))?;
    }
    archive::copy_dir(src, dst).map_err(|e| format!("copie du son : {e}"))
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

        // Suppression propre : fichiers stockés + ligne overlay effacés.
        let sub_id = overlay::list_subs_for_parent(&conn, "ferrari_488").unwrap()[0].id.clone();
        remove_sub(&conn, &cfg, &sub_id).unwrap();
        assert!(!library.join("skins").join("ferrari_488").join("af_corse_51").exists());
        assert!(overlay::list_subs_for_parent(&conn, "ferrari_488").unwrap().is_empty());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn track_skin_routed_by_parent_kind() {
        let base = std::env::temp_dir().join(format!("pitbox-tsk-{}", Uuid::new_v4()));
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig::default();
        let now = Local::now().to_rfc3339();

        // Le circuit « spa » est connu comme Track dans l'overlay.
        overlay::upsert_mod(&conn, "spa", "Track", None, Some("Spa"), "h", None, &now).unwrap();

        // Pack de skins pour spa : spa/skins/<skin>.
        let pack = base.join("src").join("spa");
        let skin = pack.join("skins").join("night");
        std::fs::create_dir_all(&skin).unwrap();
        std::fs::write(skin.join("ui_track_skin.json"), b"{}").unwrap();

        let subs = modscan::scan_subs(&base.join("src"));
        assert_eq!(subs.len(), 1);
        import_subs(&conn, &cfg, &library, "spa_skins.7z", &subs, true);

        // Classé TRACK_SKIN, stocké sous track_skins/.
        let stored = overlay::list_subs_for_parent(&conn, "spa").unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].sub_type, "TRACK_SKIN");
        assert!(library.join("track_skins").join("spa").join("night").is_dir());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn skin_pack_multi_car_shape() {
        // Forme `skins/<voiture>/<skin>` : un pack couvrant plusieurs voitures.
        let base = std::env::temp_dir().join(format!("pitbox-subB-{}", Uuid::new_v4()));
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig::default();

        let src = base.join("src");
        for (car, skin) in [("ferrari_488", "af_corse_51"), ("lambo_huracan", "team_a")] {
            let d = src.join("skins").join(car).join(skin);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("preview.jpg"), b"IMG").unwrap();
        }

        let subs = modscan::scan_subs(&src);
        assert_eq!(subs.len(), 2, "deux voitures cibles");
        let mut parents: Vec<String> = subs.iter().map(|s| s.parent_id.clone()).collect();
        parents.sort();
        assert_eq!(parents, vec!["ferrari_488", "lambo_huracan"]);

        let res = import_subs(&conn, &cfg, &library, "pack.7z", &subs, true);
        assert_eq!(res.len(), 2);
        assert!(library.join("skins").join("ferrari_488").join("af_corse_51").join("preview.jpg").is_file());
        assert!(library.join("skins").join("lambo_huracan").join("team_a").join("preview.jpg").is_file());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn sound_swap_and_restore() {
        let base = std::env::temp_dir().join(format!("pitbox-snd-{}", Uuid::new_v4()));
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig { library_path: Some(library.clone()), ..Default::default() };
        let now = Local::now().to_rfc3339();

        // Voiture (mod) avec son d'origine dans sfx/.
        let carv = library.join("cars").join("snd_car").join("v1");
        let sfx = carv.join("sfx");
        std::fs::create_dir_all(&sfx).unwrap();
        std::fs::write(sfx.join("GUIDs.txt"), b"ORIG").unwrap();
        std::fs::write(sfx.join("car.bank"), b"ORIGBANK").unwrap();
        overlay::upsert_mod(&conn, "snd_car", "Car", Some("B"), Some("Snd"), "h", None, &now).unwrap();
        overlay::insert_version(&conn, "v1", "snd_car", Some("1.0"), None, &now, &carv.to_string_lossy(), None, "sig", &[], &[], &[], &[]).unwrap();
        overlay::set_active_version(&conn, "snd_car", "v1").unwrap();

        // Mod de son stocké à part.
        let snd = library.join("sounds").join("snd_car").join("v8");
        std::fs::create_dir_all(&snd).unwrap();
        std::fs::write(snd.join("GUIDs.txt"), b"MOD").unwrap();
        std::fs::write(snd.join("car.bank"), b"MODBANK").unwrap();
        overlay::insert_sub_mod(&conn, "s1", "SOUND", "snd_car", "v8", &snd.to_string_lossy(), None, &now).unwrap();

        // Activation : sfx remplacé, original sauvegardé, sub actif.
        activate_sound(&conn, &cfg, "s1").unwrap();
        assert_eq!(std::fs::read_to_string(sfx.join("GUIDs.txt")).unwrap(), "MOD");
        assert_eq!(std::fs::read_to_string(library.join("sounds").join("snd_car").join("__original__").join("GUIDs.txt")).unwrap(), "ORIG");
        assert!(overlay::get_sub_mod(&conn, "s1").unwrap().unwrap().is_active);

        // Restauration : son d'origine revenu, sub inactif.
        restore_sound(&conn, &cfg, "snd_car").unwrap();
        assert_eq!(std::fs::read_to_string(sfx.join("GUIDs.txt")).unwrap(), "ORIG");
        assert!(!overlay::get_sub_mod(&conn, "s1").unwrap().unwrap().is_active);

        let _ = std::fs::remove_dir_all(&base);
    }
}
