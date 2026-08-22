//! Mods « autres » (§7.3) : tout mod importé qui n'est ni voiture, circuit,
//! skin, son, ni app (shaders, configs CSP, mods d'UI, weather patterns, mods
//! de physique globale…). Jamais perdu — stocké tel quel et listé.
//!
//! Activation par junction, comme les autres types, avec le même garde-fou
//! (jamais sur un vrai **dossier**) : une jonction n'est posée que là où AC n'a
//! encore rien à cet emplacement. Un fichier isolé dont le dossier parent
//! existe déjà réellement (ex. `content/gui/flags/`) est posé par lien fichier
//! (`activation::create_file_link`).
//!
//! **Un fichier déjà présent est remplacé, pas sauté** (§4.5.4) : l'original part
//! en sauvegarde et revient à la désactivation. C'est ce qui manquait aux mods
//! qui remplacent réellement du contenu — un mod façon CMRT visant
//! `content/gui/` s'installait à moitié, en silence. Comme partout ailleurs,
//! seul un exemplaire **plus récent** prend la place de ce qui tourne déjà.
//!
//! Ce n'est pas pour autant un moteur de superposition façon MO2 : deux mods
//! « autres » visant le même emplacement se départagent à la **priorité**
//! (marquée par l'utilisateur), pas par un ordre de chargement.

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
    /// Fichiers annexes redirigés vers le dossier ressources (§4.5.2).
    pub resources_extracted: usize,
    /// Composant **optionnel** (§4.6bis) : livré par l'auteur dans une archive
    /// à part **et** modifiant le jeu de base. Importé mais laissé inactif —
    /// c'est à l'utilisateur de dire s'il le veut.
    #[serde(default)]
    pub optional: bool,
    /// Combien de fichiers du jeu de base il remplacerait. C'est le chiffre qui
    /// rend la question répondable : « remplace 10 fichiers » se décide, « ce
    /// mod contient des trucs » non.
    #[serde(default)]
    pub game_files_replaced: usize,
}

/// Id à partir du nom d'archive/dossier importé (pas du dossier temp
/// d'extraction, dont le nom — un UUID — n'a aucun sens pour une archive).
///
/// Deux pièges, tous deux à l'origine d'une perte de données réelle :
///
/// 1. **Assainir avant de découper.** Le nom reçu pour un reste d'archive est
///    `<archive>__<chemin relatif>` et peut donc contenir des séparateurs
///    (`RSS….rar__content\driver`). Passé tel quel à `Path::file_stem()`, seul
///    le dernier segment survivait.
/// 2. **Ne retirer que les extensions d'archive.** `file_stem()` coupe au
///    dernier point : sur `RSS….rar__driver`, il voyait l'extension
///    `rar__driver` et renvoyait `RSS…`. Tous les restes d'une même archive
///    tombaient donc sur le même id ; le premier était importé, les suivants
///    rejetés par `other_exists` — et leurs fichiers, déjà déplacés dans le
///    dossier temporaire d'emballage, disparaissaient à son nettoyage. Pour
///    l'archive du Lanzo : `extension/`, `system/`, `content/texture` et le PDF
///    perdus, seul `content/driver` conservé.
fn other_id(source_name: &str) -> String {
    let sanitized: String = source_name
        .chars()
        .map(|c| if r#"\/:*?"<>|"#.contains(c) { '_' } else { c })
        .collect();
    let sanitized = sanitized.trim();
    if let Some((stem, ext)) = sanitized.rsplit_once('.') {
        if !stem.trim().is_empty()
            && crate::importer::NESTED_ARCHIVE_EXTS
                .iter()
                .any(|a| a.eq_ignore_ascii_case(ext))
        {
            return stem.trim().to_string();
        }
    }
    sanitized.to_string()
}

/// Importe un dossier non reconnu comme « autre mod » (§7.3) : stocké tel
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
    // Seul appelant en `BesideMod` (§4.5.2) : un « autre mod » est par
    // construction ce qui était livré à côté des mods reconnus — racine
    // d'archive ou reste ramassé par le balayage (§7.3). Un document isolé
    // y est bien une annexe, contrairement au même fichier trouvé dans un
    // dossier de mod.
    let res_dir = resources::resources_dir_for(library, "others", &[&id]);
    let resources_extracted =
        resources::file_mod(root, &dest, &res_dir, mode, !copy, resources::Source::BesideMod).ok()?;
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
        optional: false,
        game_files_replaced: 0,
    })
}

/// Combien de fichiers du **jeu de base** cet arbre remplacerait s'il était posé.
///
/// « Du jeu de base » = le chemin existe déjà dans AC, aucun mod ne le réclame
/// (§4.5.4) et ce n'est pas un exemplaire qu'on a soi-même posé. C'est la
/// mesure du **rayon d'action**, et c'est elle qui distingue un mod qui
/// s'installe chez lui d'un mod qui s'installe chez les autres : un fichier
/// ajouté ne coûte rien à personne, un fichier remplacé change le jeu pour
/// toutes les voitures et toutes les sessions.
///
/// Les chemins qui ne mènent nulle part dans le jeu (§4.5.3) ne comptent pas :
/// ils ne seront pas posés, ils ne remplacent donc rien. D'où la traversée de
/// l'emballage de l'auteur, comme partout ailleurs : mesuré depuis la racine
/// d'extraction, un composant livré dans un dossier à son nom ne présenterait
/// **aucun** chemin de jeu et serait donc compté à zéro — soit exactement
/// l'inverse de ce que ce décompte doit détecter.
pub fn game_files_replaced(conn: &Connection, cfg: &AppConfig, dir: &Path) -> usize {
    let Some(ac) = cfg.ac_install_path.as_ref() else {
        return 0;
    };
    let dir = &crate::acpath::effective_root(dir);
    WalkDir::new(dir)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            let Ok(rel) = e.path().strip_prefix(dir) else {
                return false;
            };
            if !crate::acpath::is_ac_relative(rel) {
                return false;
            }
            let target = ac.join(rel);
            if !target.exists() {
                return false;
            }
            // Réclamé par un mod, ou déjà remplacé par nous : l'original est
            // déjà sous notre garde, ce n'est plus « du jeu de base ».
            let claimed = overlay::extra_claimants(conn, &target.to_string_lossy())
                .map(|c| !c.is_empty())
                .unwrap_or(false);
            !claimed && !crate::gamebackup::is_replaced(conn, &target)
        })
        .count()
}

/// Chemins relatifs de tous les fichiers stockés d'un mod « autre », comptés
/// depuis la racine réelle de la livraison — emballage de l'auteur traversé
/// ([`crate::acpath::effective_root`]).
///
/// Traverser **ici**, et pas seulement à l'import, soigne les entrées déjà en
/// bibliothèque : celles importées avant le correctif ont l'emballage figé dans
/// leur arbre stocké, et rien ne les répare puisqu'un « autre mod » ne connaît
/// pas la mise à jour (§7.3) — réimporter une archive déjà connue est ignoré en
/// silence. Sans ça, il faudrait supprimer et réimporter chaque entrée, archive
/// source en main.
fn relative_files(dir: &Path) -> HashSet<PathBuf> {
    let root = crate::acpath::effective_root(dir);
    WalkDir::new(&root)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.path().strip_prefix(&root).ok().map(|p| p.to_path_buf()))
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
    /// Autres mods « autres » partageant au moins un chemin de fichier (§7.3).
    pub conflicts: Vec<ConflictInfo>,
    /// Fichiers visant une zone qu'un outil externe synchronise
    /// ([`crate::acpath::is_externally_managed`]) — `extension/config/*/loaded/`,
    /// vao-patches. Même signalement que dans « Ajouts au jeu » (§4.5.5) : les
    /// deux mécanismes de pose posent dans le même jeu, il n'y a pas de raison
    /// qu'un seul des deux prévienne. Un pack multi-mods qui livre des configs
    /// CSP que rien ne rattache atterrit précisément ici (§7.3).
    pub externally_managed: usize,
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
            let externally_managed = mine
                .map(|f| f.iter().filter(|p| crate::acpath::is_externally_managed(p)).count())
                .unwrap_or(0);
            OtherModCard {
                row,
                conflicts,
                externally_managed,
            }
        })
        .collect())
}

/// Dossier de bibliothèque d'un mod « autre », résolu depuis l'overlay — le
/// pendant de `library::folder_path` pour ce type d'entrée.
///
/// **Le dossier de contenu peut ne pas exister**, et l'entrée est valide quand
/// même : `import_other` range en `BesideMod` (§4.5.2), donc une livraison
/// réduite à un document part **entièrement** en ressources et ne crée jamais
/// `<lib>/others/<id>/`. Cas réel, l'archive `_RSS_Settings` : son
/// `READ ME.pdf` de racine, que rien ne rattachait à l'app livrée à côté,
/// devenait une entrée « autre mod » dont « ouvrir le dossier » répondait
/// « mod introuvable » — alors que le PDF était bel et bien en bibliothèque,
/// sous `resources/others/<id>/`.
///
/// On rend donc le dossier de contenu s'il existe, le dossier ressources
/// sinon. L'erreur reste réservée au cas où **aucun des deux** n'est là :
/// celui-là est une vraie perte.
pub fn folder_path(conn: &Connection, cfg: &AppConfig, id: &str) -> Result<PathBuf, String> {
    let m = overlay::get_other_mod(conn, id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::MOD_UNKNOWN)?;
    let library = cfg.library_path.as_deref();
    crate::libpath::resolve(library, &m.library_path)
        .filter(|d| d.is_dir())
        .or_else(|| library.map(|l| resources::resources_dir_for(l, "others", &[id])))
        .filter(|d| d.is_dir())
        .ok_or_else(|| crate::errors::MOD_NOT_FOUND.to_string())
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
/// dossier parent existe déjà réellement est posé par lien fichier ; s'il vise
/// un fichier déjà présent, il le **remplace après sauvegarde** (§4.5.4) — et
/// seulement si son exemplaire est plus récent.
#[allow(clippy::too_many_arguments)]
fn place(
    conn: &Connection,
    cfg: &AppConfig,
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
        // Même refus que pour les ajouts au jeu (§4.5.3) : un « autre mod »
        // dont le chemin n'est pas un chemin de jeu reste en bibliothèque
        // plutôt que d'être jonctionné à la racine de l'install.
        //
        // Le test diffère selon qu'on regarde un dossier ou un fichier. En
        // descendant, `content` seul est un début de chemin valide alors que
        // ce n'en est pas encore un ; un fichier, lui, doit désigner un chemin
        // complet — d'où `leads_into_game` d'un côté, `is_ac_relative` de
        // l'autre.
        let acceptable = if p.is_dir() {
            crate::acpath::leads_into_game(rel)
        } else {
            crate::acpath::is_ac_relative(rel)
        };
        if !acceptable {
            log::warn!("place {mine_id}: {} is not an AC path, skipped", rel.display());
            warnings.push(format!("{} : hors chemin de jeu, non posé", rel.display()));
            continue;
        }
        let target = ac.join(rel);
        if !p.is_dir() {
            // Fichier isolé : posé par lien fichier si son dossier parent
            // existe déjà (vrai dossier ou tout juste jonctionné). Un fichier
            // déjà présent à cet emplacement n'est plus sauté en silence — il
            // est remplacé après sauvegarde de l'original (§4.5.4), sous la même
            // condition que partout ailleurs : seul un exemplaire plus récent
            // prend la place de ce qui tourne déjà.
            if !target.parent().is_some_and(|p| p.exists()) {
                log::warn!(
                    "place {mine_id} {}: parent dir missing at {}",
                    rel.display(),
                    target.display()
                );
                warnings.push(format!("{} : dossier parent introuvable", rel.display()));
                continue;
            }
            if target.exists() {
                if !crate::gamebackup::is_newer(&p, &target) {
                    log::warn!(
                        "place {mine_id} {}: target exists and is not older, left alone",
                        rel.display()
                    );
                    continue;
                }
                // `protect` refuse s'il n'a pas pu sécuriser l'original : alors
                // on ne touche à rien, plutôt que d'altérer sans filet.
                if !crate::gamebackup::protect(conn, cfg, &target) {
                    warnings.push(format!("{} : sauvegarde impossible, non remplacé", rel.display()));
                    continue;
                }
                if let Err(e) = std::fs::remove_file(&target) {
                    log::warn!("place {mine_id} {}: {e}", rel.display());
                    continue;
                }
            }
            match activation::create_file_link(&target, &p) {
                Ok(()) => junctions.push(target.to_string_lossy().into_owned()),
                Err(err) => warnings.push(format!("{} : {err}", rel.display())),
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
            // priorité tranche (le mod prioritaire gagne, §7.3).
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
            place(
                conn,
                cfg,
                &p,
                root,
                ac,
                mine_id,
                mine_priority,
                others,
                junctions,
                warnings,
            );
        }
    }
}

/// Active un mod « autre » par junction (§7.3). Best-effort et partiel par
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
    // Emballage de l'auteur traversé, même raison que `relative_files` : les
    // entrées importées avant le correctif le portent encore dans leur arbre
    // stocké, et `place` refuserait tout (`NFS_…` n'est pas un chemin de jeu).
    let src = crate::libpath::resolve(cfg.library_path.as_deref(), &m.library_path)
        .map(|d| crate::acpath::effective_root(&d))
        .ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    place(
        conn,
        cfg,
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
        let path = Path::new(j);
        let _ = activation::remove_junction(path);
        // Si ce chemin était un fichier du jeu que ce mod avait remplacé,
        // l'original revient (§4.5.4). No-op sur une simple addition.
        crate::gamebackup::restore(conn, path);
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
    fn other_id_keeps_the_leftover_label_and_strips_only_archive_extensions() {
        // Bug réel (§7.3) : `other_id` découpait au dernier point via
        // `Path::file_stem`, donc `<archive>.rar__<label>` perdait son label —
        // tous les restes d'une archive partageaient un id et s'annulaient.
        assert_eq!(
            other_id("RSS_Lanzo.rar__content\\driver"),
            "RSS_Lanzo.rar__content_driver",
            "le label du reste survit, séparateurs assainis"
        );
        assert_ne!(
            other_id("RSS_Lanzo.rar__content\\driver"),
            other_id("RSS_Lanzo.rar__system"),
            "deux restes de la même archive ne partagent jamais un id"
        );
        assert_ne!(
            other_id("Pack.rar__driver"),
            other_id("Pack.rar__content\\driver"),
            "deux restes homonymes dans des dossiers différents restent distincts"
        );

        // Extension d'archive : retirée (nom d'archive nu passé par l'import).
        assert_eq!(other_id("MyShaderMod.zip"), "MyShaderMod");
        assert_eq!(other_id("Pack.7z"), "Pack");
        assert_eq!(other_id("Pack.RAR"), "Pack", "insensible à la casse");

        // Toute autre extension appartient au nom : un dossier `mod.v2` ne
        // devient pas `mod`, deux versions ne se confondent pas.
        assert_eq!(other_id("mod.v2"), "mod.v2");
        assert_eq!(other_id("Settings_24-10-25"), "Settings_24-10-25");
        assert_ne!(other_id("mod.v2"), other_id("mod.v3"));
    }

    #[test]
    fn an_other_mod_replaces_a_game_file_and_gives_it_back() {
        // §4.5.4 : un mod « autre » qui vise un fichier existant du jeu ne doit
        // plus être sauté en silence — c'est ce qui installait à moitié les
        // mods façon CMRT (`content/gui/…`) sans que rien ne le dise. L'original
        // part en sauvegarde et revient à la désactivation.
        let base = crate::testutil::temp_dir("other-replace");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        // `content/gui/` existe déjà côté AC, avec un fichier Kunos dedans.
        let kunos = ac.join("content").join("gui").join("logo.png");
        std::fs::create_dir_all(kunos.parent().unwrap()).unwrap();
        std::fs::write(&kunos, b"KUNOS-LOGO").unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };

        let src = base.join("src").join("HudMod");
        make_tree(&src, &["content/gui/logo.png"]);
        std::fs::write(src.join("content").join("gui").join("logo.png"), b"MOD-LOGO").unwrap();
        // L'exemplaire du mod doit être le plus récent pour prendre la place.
        let t = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
        std::fs::File::options()
            .write(true)
            .open(src.join("content").join("gui").join("logo.png"))
            .unwrap()
            .set_modified(t)
            .unwrap();

        import_other(&conn, &library, "HudMod.zip", &src, true, ExtractionMode::InfoOnly).unwrap();
        let res = activate_other(&conn, &cfg, "HudMod").unwrap();
        assert_eq!(res.junctions, 1, "le fichier est bien posé");
        assert_eq!(std::fs::read(&kunos).unwrap(), b"MOD-LOGO", "le mod prend la place");
        assert!(
            crate::gamebackup::is_replaced(&conn, &kunos),
            "et le remplacement est tracé, pas silencieux"
        );

        deactivate_other(&conn, "HudMod").unwrap();
        assert_eq!(
            std::fs::read(&kunos).unwrap(),
            b"KUNOS-LOGO",
            "l'original du jeu revient à la désactivation"
        );
        assert!(!crate::gamebackup::is_replaced(&conn, &kunos));
    }

    #[test]
    fn an_older_other_mod_file_leaves_the_game_file_alone() {
        // Même arbitrage par date que partout : un exemplaire plus ancien ne
        // déloge pas ce qui tourne déjà, et ne crée aucune sauvegarde inutile.
        let base = crate::testutil::temp_dir("other-older");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(&library).unwrap();
        let kunos = ac.join("content").join("gui").join("logo.png");
        std::fs::create_dir_all(kunos.parent().unwrap()).unwrap();
        std::fs::write(&kunos, b"RECENT").unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let cfg = AppConfig {
            ac_install_path: Some(ac.clone()),
            library_path: Some(library.clone()),
            ..Default::default()
        };

        let src = base.join("src").join("OldHud");
        make_tree(&src, &["content/gui/logo.png"]);
        let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        std::fs::File::options()
            .write(true)
            .open(src.join("content").join("gui").join("logo.png"))
            .unwrap()
            .set_modified(t)
            .unwrap();

        import_other(&conn, &library, "OldHud.zip", &src, true, ExtractionMode::InfoOnly).unwrap();
        let res = activate_other(&conn, &cfg, "OldHud").unwrap();
        assert_eq!(res.junctions, 0, "rien posé : l'exemplaire du mod est plus ancien");
        assert_eq!(std::fs::read(&kunos).unwrap(), b"RECENT", "fichier du jeu intact");
        assert!(
            !crate::gamebackup::is_replaced(&conn, &kunos),
            "aucune sauvegarde inutile"
        );
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

    #[test]
    fn others_report_files_landing_in_cm_managed_folders() {
        // §4.5.3/§7.3 : un pack multi-mods dont les configs CSP ne se rattachent
        // à aucune voiture atterrit en « autre mod ». Le signalement « zone
        // Content Manager » ne peut donc pas vivre seulement dans « Ajouts au
        // jeu » — les deux mécanismes posent dans le même jeu.
        let base = crate::testutil::temp_dir("other-managed");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();

        let src = base.join("src").join("Pack");
        make_tree(
            &src,
            &[
                "extension/config/tracks/loaded/spa.ini",
                "extension/vao-patches/spa.vao-patch",
                // Hors zone auto-gérée : appartient bien au mod.
                "content/fonts/pack.png",
            ],
        );
        import_other(&conn, &library, "Pack.zip", &src, true, ExtractionMode::InfoOnly).unwrap();

        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        let card = list_others(&conn, &cfg)
            .unwrap()
            .into_iter()
            .find(|c| c.row.id == "Pack")
            .unwrap();
        assert_eq!(
            card.externally_managed, 2,
            "la config et le vao-patch sont comptés, pas la font"
        );
    }

    #[test]
    fn an_entry_wrapped_by_its_author_still_reaches_the_game() {
        // Bug réel (NFS Tournament, A3DR Porsche) : l'archive emballe tout dans
        // un dossier à son nom. Les voitures étaient importées — `modscan`
        // descend l'emballage — mais `content/texture` et `content/fonts`
        // gardaient le segment et `place` les refusait, à juste titre. Les
        // fichiers restaient en bibliothèque sans jamais atteindre AC.
        //
        // Traversé à l'activation et pas seulement à l'import : une entrée déjà
        // en bibliothèque ne se répare pas autrement (§7.3, pas de mise à jour
        // d'un « autre mod » — un réimport est ignoré en silence).
        let base = crate::testutil::temp_dir("other-wrapped");
        let library = base.join("library");
        let ac = base.join("ac");
        std::fs::create_dir_all(ac.join("content").join("texture")).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();

        // Tel que l'ancien import l'a rangé : emballage inclus.
        let stored = library.join("others").join("NFS_Pack.7z__NFS_Pack_content_texture");
        make_tree(&stored, &["NFS_Pack/content/texture/crew_brand/logo.dds"]);
        overlay::insert_other_mod(
            &conn,
            "NFS_Pack.7z__NFS_Pack_content_texture",
            &crate::libpath::to_relative(Some(&library), &stored),
            Some("NFS_Pack.7z"),
            "2026-08-16T00:32:21+02:00",
        )
        .unwrap();

        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ac_install_path: Some(ac.clone()),
            ..Default::default()
        };
        activate_other(&conn, &cfg, "NFS_Pack.7z__NFS_Pack_content_texture").unwrap();

        assert!(
            ac.join("content").join("texture").join("crew_brand").exists(),
            "le dossier arrive à sa vraie place, sans le segment d'emballage"
        );
        assert!(
            !ac.join("NFS_Pack").exists(),
            "et rien ne se déverse à la racine de l'install"
        );

        // Les chemins comptés (conflits, zone CM) partent eux aussi de la vraie
        // racine, sinon ils ne ressembleraient à aucun chemin de jeu.
        let files = relative_files(&stored);
        assert!(
            files.contains(
                &PathBuf::from("content")
                    .join("texture")
                    .join("crew_brand")
                    .join("logo.dds")
            ),
            "chemin relatif compté depuis la racine réelle"
        );
    }

    #[test]
    fn folder_path_resolves_the_library_folder_of_an_other_mod() {
        // Le bouton « ouvrir le dossier » résout côté Rust, jamais depuis un
        // chemin reçu du front — c'est ce qui permet de garder fermé le scope
        // ACL du plugin `opener`.
        let base = crate::testutil::temp_dir("other-folder");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();

        let src = base.join("src").join("ShaderMod");
        make_tree(&src, &["system/shaders/x.fx"]);
        import_other(&conn, &library, "ShaderMod.zip", &src, true, ExtractionMode::InfoOnly).unwrap();

        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        assert_eq!(
            folder_path(&conn, &cfg, "ShaderMod").unwrap(),
            library.join("others").join("ShaderMod"),
            "le dossier de bibliothèque du mod"
        );
        assert!(
            folder_path(&conn, &cfg, "does_not_exist").is_err(),
            "un id inconnu ne renvoie pas un chemin inventé"
        );
    }

    #[test]
    fn folder_path_falls_back_to_resources_when_everything_went_there() {
        // Bug réel (`_RSS_Settings`) : une livraison réduite à un document part
        // entièrement en ressources (§4.5.2, rangement `BesideMod`), donc
        // `<lib>/others/<id>/` n'est jamais créé. « Ouvrir le dossier »
        // répondait « mod introuvable » alors que le PDF était en bibliothèque.
        let base = crate::testutil::temp_dir("other-folder-res");
        let library = base.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();

        let src = base.join("src").join("JustAPdf");
        make_tree(&src, &["READ ME.pdf"]);
        import_other(&conn, &library, "JustAPdf.rar", &src, true, ExtractionMode::InfoOnly).unwrap();

        let cfg = AppConfig {
            library_path: Some(library.clone()),
            ..Default::default()
        };
        assert!(
            !library.join("others").join("JustAPdf").is_dir(),
            "rien n'est allé dans le dossier de contenu : tout était une annexe"
        );
        assert_eq!(
            folder_path(&conn, &cfg, "JustAPdf").unwrap(),
            resources::resources_dir_for(&library, "others", &["JustAPdf"]),
            "on ouvre le dossier ressources, là où le document est réellement"
        );
    }
}
