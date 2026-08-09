//! Mods « autres » (§6.1bis) : tout mod importé qui n'est ni voiture, circuit,
//! skin, son, ni app (shaders, configs CSP, mods d'UI, weather patterns, mods
//! de physique globale…). Jamais perdu — stocké tel quel et listé.
//!
//! Activation par junction, comme les autres types, avec le même garde-fou
//! (jamais sur un vrai dossier) : une jonction n'est posée que là où AC n'a
//! encore rien à cet emplacement. Un fichier isolé dont le dossier parent
//! existe déjà réellement (ex. `content/gui/flags/`) est posé par lien
//! fichier (`activation::create_file_link`) **si et seulement si** le fichier
//! lui-même n'existe pas encore — pure addition, rien écrasé. Ce n'est PAS un
//! moteur de superposition complet façon MO2 : un fichier qui existe déjà à
//! l'identique n'est jamais remplacé (limite assumée — mods qui remplacent du
//! contenu stock, hors périmètre pour l'instant). Deux mods « autres » qui
//! visent le même emplacement : la **priorité** (marquée par l'utilisateur)
//! tranche.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use chrono::Local;
use rusqlite::Connection;
use serde::Serialize;
use walkdir::WalkDir;

use crate::activation;
use crate::config::AppConfig;
use crate::overlay::{self, OtherModRow};
use crate::resources::{self, ExtractionMode};

#[derive(Debug, Clone, Serialize)]
pub struct OtherImported {
    pub id: String,
    /// Fichiers annexes redirigés vers le dossier ressources (§4.6).
    pub resources_extracted: usize,
}

/// Id à partir du nom d'archive/dossier importé (pas du dossier temp
/// d'extraction, dont le nom — un UUID — n'a aucun sens pour une archive).
fn other_id(source_name: &str) -> String {
    let stem = Path::new(source_name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| source_name.to_string());
    stem.chars()
        .map(|c| if r#"\/:*?"<>|"#.contains(c) { '_' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Importe un dossier non reconnu comme « autre mod » (§6.1bis) : stocké tel
/// quel dans la bibliothèque, jamais perdu. Idempotent (ignore si déjà connu).
pub fn import_other(
    conn: &Connection,
    library: &Path,
    source_name: &str,
    root: &Path,
    copy: bool,
    mode: ExtractionMode,
) -> Option<OtherImported> {
    let id = other_id(source_name);
    if overlay::other_exists(conn, &id).unwrap_or(false) {
        return None;
    }
    let dest = library.join("others").join(&id);
    // Fichiers annexes (§4.6) redirigés à part : structure inconnue par nature,
    // donc jamais d'image présumée annexe même à la racine (allow_root_images=false).
    let res_dir = resources::resources_dir_for(library, "others", &[&id]);
    let resources_extracted = resources::file_mod(root, &dest, &res_dir, mode, !copy, false).ok()?;
    overlay::insert_other_mod(
        conn,
        &id,
        &crate::libpath::to_relative(Some(library), &dest),
        Some(source_name),
        &Local::now().to_rfc3339(),
    )
    .ok()?;
    Some(OtherImported {
        id,
        resources_extracted,
    })
}

/// Chemins relatifs de tous les fichiers stockés d'un mod « autre ».
fn relative_files(dir: &Path) -> HashSet<PathBuf> {
    WalkDir::new(dir)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.path().strip_prefix(dir).ok().map(|p| p.to_path_buf()))
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct ConflictInfo {
    pub other_id: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct OtherModCard {
    #[serde(flatten)]
    pub row: OtherModRow,
    /// Autres mods « autres » partageant au moins un chemin de fichier (§6.1bis).
    pub conflicts: Vec<ConflictInfo>,
}

/// Liste les mods « autres » avec les conflits de fichiers détectés entre eux.
pub fn list_others(conn: &Connection, cfg: &AppConfig) -> rusqlite::Result<Vec<OtherModCard>> {
    let rows = overlay::list_other_mods(conn)?;
    let files: Vec<(String, HashSet<PathBuf>)> = rows
        .iter()
        .map(|r| {
            let files = crate::libpath::resolve(cfg.library_path.as_deref(), &r.library_path)
                .map(|dir| relative_files(&dir))
                .unwrap_or_default();
            (r.id.clone(), files)
        })
        .collect();

    Ok(rows
        .into_iter()
        .map(|row| {
            let mine = files.iter().find(|(id, _)| *id == row.id).map(|(_, f)| f);
            let conflicts = mine
                .map(|mine| {
                    files
                        .iter()
                        .filter(|(id, _)| *id != row.id)
                        .filter_map(|(id, f)| {
                            let n = mine.intersection(f).count();
                            (n > 0).then(|| ConflictInfo {
                                other_id: id.clone(),
                                count: n,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            OtherModCard { row, conflicts }
        })
        .collect())
}

pub fn set_priority(conn: &Connection, id: &str, priority: bool) -> Result<(), String> {
    overlay::set_other_priority(conn, id, priority).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivateOtherResult {
    pub junctions: usize,
    pub warnings: Vec<String>,
}

/// Pose une jonction à `current`'s enfants, en descendant tant que
/// l'emplacement AC correspondant existe déjà (vrai dossier). S'arrête et
/// jonctionne dès qu'un emplacement est libre. Un fichier isolé dont le
/// dossier parent existe déjà réellement est posé par lien fichier — mais
/// seulement s'il n'écrase rien (§6.1bis) ; sinon (fichier cible déjà
/// présent) laissé de côté sans le toucher, silencieusement.
#[allow(clippy::too_many_arguments)]
fn place(
    current: &Path,
    root: &Path,
    ac: &Path,
    mine_id: &str,
    mine_priority: bool,
    others: &[OtherModRow],
    junctions: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    let Ok(entries) = std::fs::read_dir(current) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        let Ok(rel) = p.strip_prefix(root) else { continue };
        let target = ac.join(rel);
        if !p.is_dir() {
            // Fichier isolé : posé par lien fichier si son dossier parent
            // existe déjà (vrai dossier ou tout juste jonctionné) ET que rien
            // ne s'y trouve encore à cet emplacement précis — pure addition,
            // jamais un écrasement (mods qui remplacent du contenu existant :
            // hors périmètre pour l'instant).
            if !target.parent().is_some_and(|p| p.exists()) {
                log::warn!(
                    "place {mine_id} {}: parent dir missing at {}",
                    rel.display(),
                    target.display()
                );
                warnings.push(format!("{} : dossier parent introuvable", rel.display()));
            } else if !target.exists() {
                match activation::create_file_link(&target, &p) {
                    Ok(()) => junctions.push(target.to_string_lossy().into_owned()),
                    Err(err) => warnings.push(format!("{} : {err}", rel.display())),
                }
            }
            continue;
        }
        if !target.exists() {
            match activation::create_junction(&target, &p) {
                Ok(()) => junctions.push(target.to_string_lossy().into_owned()),
                Err(err) => warnings.push(format!("{} : {err}", rel.display())),
            }
        } else if activation::is_junction(&target) {
            // Emplacement déjà pris par un autre mod « autre » actif : la
            // priorité tranche (le mod prioritaire gagne, §6.1bis).
            let holder = others
                .iter()
                .find(|o| o.id != mine_id && o.is_active && o.junctions.iter().any(|j| Path::new(j) == target));
            let take = holder.map(|h| mine_priority || !h.is_priority).unwrap_or(true);
            if take {
                let _ = activation::remove_junction(&target);
                match activation::create_junction(&target, &p) {
                    Ok(()) => junctions.push(target.to_string_lossy().into_owned()),
                    Err(err) => warnings.push(format!("{} : {err}", rel.display())),
                }
            } else {
                warnings.push(format!("{} : emplacement pris par un mod prioritaire", rel.display()));
            }
        } else {
            // Vrai dossier existant, jamais touché (garde-fou) : on essaie de
            // se glisser plus profond, dans un sous-dossier qui n'existe pas.
            place(&p, root, ac, mine_id, mine_priority, others, junctions, warnings);
        }
    }
}

/// Active un mod « autre » par junction (§6.1bis). Best-effort et partiel par
/// nature : ce qui ne peut pas être posé sans toucher un vrai dossier ou un
/// fichier isolé existant est simplement signalé, pas forcé.
pub fn activate_other(conn: &Connection, cfg: &AppConfig, id: &str) -> Result<ActivateOtherResult, String> {
    let m = overlay::get_other_mod(conn, id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::MOD_UNKNOWN)?;
    let ac = cfg.ac_install_path.as_ref().ok_or(crate::errors::AC_NOT_CONFIGURED)?;
    let others = overlay::list_other_mods(conn).map_err(|e| e.to_string())?;

    let mut junctions = Vec::new();
    let mut warnings = Vec::new();
    let src = crate::libpath::resolve(cfg.library_path.as_deref(), &m.library_path)
        .ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    place(
        &src,
        &src,
        ac,
        id,
        m.is_priority,
        &others,
        &mut junctions,
        &mut warnings,
    );

    overlay::set_other_active(conn, id, true, &junctions).map_err(|e| e.to_string())?;
    Ok(ActivateOtherResult {
        junctions: junctions.len(),
        warnings,
    })
}

/// Désactive un mod « autre » : retire exactement les jonctions posées à sa
/// dernière activation (garde-fou déjà dans `remove_junction`).
pub fn deactivate_other(conn: &Connection, id: &str) -> Result<(), String> {
    let m = overlay::get_other_mod(conn, id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::MOD_UNKNOWN)?;
    for j in &m.junctions {
        let _ = activation::remove_junction(Path::new(j));
    }
    overlay::set_other_active(conn, id, false, &[]).map_err(|e| e.to_string())
}

/// Supprime un mod « autre » : désactive (retire ses jonctions) puis efface
/// ses fichiers stockés et son entrée overlay.
pub fn delete_other(conn: &Connection, cfg: &AppConfig, id: &str) -> Result<(), String> {
    let m = overlay::get_other_mod(conn, id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::MOD_UNKNOWN)?;
    let _ = deactivate_other(conn, id);
    if let Some(dir) = crate::libpath::resolve(cfg.library_path.as_deref(), &m.library_path) {
        let _ = std::fs::remove_dir_all(dir);
    }
    overlay::delete_other_mod(conn, id).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tree(root: &Path, files: &[&str]) {
        for f in files {
            let p = root.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, b"x").unwrap();
        }
    }

    #[test]
    fn import_and_activate_new_gap() {
        let base = crate::testutil::temp_dir("other");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(&ac).unwrap();
        // AC déjà installé, avec sa propre arborescence extension/config/tracks/
        // (comme une vraie install CSP) — seul le sous-dossier "newtrack" manque.
        std::fs::create_dir_all(ac.join("extension").join("config").join("tracks")).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };

        // Mod « autre » : un nouveau sous-dossier de config CSP inexistant côté AC.
        let src = base.join("src").join("MyShaderMod");
        make_tree(&src, &["extension/config/tracks/newtrack/track.ini"]);

        let imported = import_other(&conn, &library, "MyShaderMod.zip", &src, true, ExtractionMode::InfoOnly).unwrap();
        assert_eq!(imported.id, "MyShaderMod");
        assert!(overlay::other_exists(&conn, "MyShaderMod").unwrap());

        let res = activate_other(&conn, &cfg, "MyShaderMod").unwrap();
        assert_eq!(
            res.junctions, 1,
            "un seul dossier à jonctionner (le plus haut gap libre)"
        );
        assert!(res.warnings.is_empty());
        assert!(activation::is_junction(
            &ac.join("extension").join("config").join("tracks").join("newtrack")
        ));
        assert!(
            ac.join("extension/config/tracks/newtrack/track.ini").is_file(),
            "contenu visible via la jonction"
        );

        deactivate_other(&conn, "MyShaderMod").unwrap();
        assert!(!ac
            .join("extension")
            .join("config")
            .join("tracks")
            .join("newtrack")
            .exists());
    }

    #[test]
    fn isolated_file_added_to_existing_real_folder_via_file_link() {
        // Cas réel (mods style CMRT) : le mod n'ajoute qu'un fichier dans un
        // dossier qui existe déjà côté AC (ex. content/gui/flags/) — jamais
        // un "gap" (le dossier existe), donc avant ce correctif silencieusement
        // ignoré (warning, jamais posé) même sans rien écraser.
        let base = crate::testutil::temp_dir("other-file-link");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        // Dossier AC déjà réel, avec un fichier stock existant à côté.
        std::fs::create_dir_all(ac.join("content").join("gui").join("flags")).unwrap();
        std::fs::write(
            ac.join("content").join("gui").join("flags").join("checkered.png"),
            b"STOCK",
        )
        .unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };

        // Mod « autre » : un seul nouveau fichier, pure addition, rien d'écrasé.
        let src = base.join("src").join("CMRT_Flags");
        make_tree(&src, &["content/gui/flags/new_flag.png"]);

        import_other(&conn, &library, "CMRT_Flags.zip", &src, true, ExtractionMode::InfoOnly).unwrap();
        let res = activate_other(&conn, &cfg, "CMRT_Flags").unwrap();
        assert_eq!(res.junctions, 1, "le fichier isolé est posé, pas seulement signalé");
        assert!(res.warnings.is_empty(), "pas de warning : pure addition");

        let target = ac.join("content").join("gui").join("flags").join("new_flag.png");
        assert!(activation::is_junction(&target), "posé par lien fichier, pas copié");
        assert_eq!(std::fs::read(&target).unwrap(), b"x", "contenu visible via le lien");

        // Le fichier stock existant, lui, n'a jamais été touché.
        assert_eq!(
            std::fs::read(ac.join("content").join("gui").join("flags").join("checkered.png")).unwrap(),
            b"STOCK"
        );

        deactivate_other(&conn, "CMRT_Flags").unwrap();
        assert!(!target.exists(), "lien retiré à la désactivation");
        assert!(
            ac.join("content")
                .join("gui")
                .join("flags")
                .join("checkered.png")
                .is_file(),
            "fichier stock toujours intact après désactivation"
        );
    }

    #[test]
    fn priority_wins_shared_gap() {
        let base = crate::testutil::temp_dir("other-prio");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(&ac).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };

        let src_a = base.join("src").join("ModA");
        make_tree(&src_a, &["extension/config/tracks/shared/track.ini"]);
        std::fs::write(src_a.join("extension/config/tracks/shared/track.ini"), b"A").unwrap();
        let src_b = base.join("src").join("ModB");
        make_tree(&src_b, &["extension/config/tracks/shared/track.ini"]);
        std::fs::write(src_b.join("extension/config/tracks/shared/track.ini"), b"B").unwrap();

        import_other(&conn, &library, "ModA.zip", &src_a, true, ExtractionMode::InfoOnly).unwrap();
        import_other(&conn, &library, "ModB.zip", &src_b, true, ExtractionMode::InfoOnly).unwrap();
        overlay::set_other_priority(&conn, "ModB", true).unwrap();

        activate_other(&conn, &cfg, "ModA").unwrap();
        let target = ac.join("extension").join("config").join("tracks").join("shared");
        assert_eq!(std::fs::read_to_string(target.join("track.ini")).unwrap(), "A");

        // ModB (prioritaire) prend le dessus sur l'emplacement disputé.
        let res_b = activate_other(&conn, &cfg, "ModB").unwrap();
        assert_eq!(res_b.junctions, 1);
        assert_eq!(std::fs::read_to_string(target.join("track.ini")).unwrap(), "B");

        // ModA (non prioritaire) ne peut pas reprendre l'emplacement à ModB.
        let res_a2 = activate_other(&conn, &cfg, "ModA").unwrap();
        assert_eq!(res_a2.junctions, 0);
        assert!(!res_a2.warnings.is_empty());
        assert_eq!(
            std::fs::read_to_string(target.join("track.ini")).unwrap(),
            "B",
            "ModB garde la main"
        );
    }

    #[test]
    fn conflict_detection_between_others() {
        let base = crate::testutil::temp_dir("other-conflict");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();

        let src_a = base.join("src").join("ModA");
        make_tree(&src_a, &["extension/config/x.ini", "extension/config/only_a.ini"]);
        let src_b = base.join("src").join("ModB");
        make_tree(&src_b, &["extension/config/x.ini"]);

        import_other(&conn, &library, "ModA.zip", &src_a, true, ExtractionMode::InfoOnly).unwrap();
        import_other(&conn, &library, "ModB.zip", &src_b, true, ExtractionMode::InfoOnly).unwrap();

        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let cards = list_others(&conn, &cfg).unwrap();
        let a = cards.iter().find(|c| c.row.id == "ModA").unwrap();
        assert_eq!(a.conflicts.len(), 1);
        assert_eq!(a.conflicts[0].other_id, "ModB");
        assert_eq!(a.conflicts[0].count, 1, "un seul fichier commun (x.ini)");
    }
}
