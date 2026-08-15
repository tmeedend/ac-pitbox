//! Satellites d'un mod (§4.6ter) : ce qu'une archive livre **à côté** du
//! dossier du mod mais qui lui appartient — configs CSP
//! (`extension/config/cars/rss/<id>/…`), shaders (`system/shaders/…`),
//! textures d'équipe (`content/texture/…`), modèle de pilote
//! (`content/driver/…`). AC les lit hors de `content/<type>/<id>`, ils ne
//! peuvent donc pas voyager dans le dossier du mod.
//!
//! **Stockés bruts, avec leur chemin relatif à la racine d'AC**, dans un arbre
//! dédié (`<lib>/satellites/<type>/<id>/…`) — jamais dans la version, qui est
//! déployée telle quelle dans `content/`. Deux propriétés en découlent :
//!
//! - **L'import ne jette rien.** Ce qui n'est pas classé est conservé tel quel,
//!   donc l'*interprétation* (où poser, qui arbitre un fichier partagé) reste
//!   recalculable depuis la bibliothèque à tout moment. Aucune règle des
//!   versions précédentes à mémoriser, aucune archive à conserver : c'est
//!   l'entrée qui est préservée, pas la décision.
//! - **Le satellite vit et meurt avec son mod.** Posé à l'activation, retiré à
//!   la désactivation, supprimé avec lui — c'est ce que le passage par « autre
//!   mod » ne donnait pas : les fichiers d'une voiture désinstallée restaient
//!   dans AC, rattachés à une entrée anonyme que plus rien ne reliait au mod.
//!
//! Au **niveau du mod**, pas de la version (comme `resources/`, §4.6) : les
//! configs CSP d'une mise à jour remplacent celles de la précédente, ce qui est
//! le comportement voulu, et les couches (§4.3) partagent le même arbre.
//!
//! Pose **fichier par fichier** (hardlink), jamais par jonction de dossier :
//! plusieurs mods visent les mêmes arbres (`extension/textures/common/rss/…`
//! est livré à l'identique par chaque voiture RSS), et une jonction de dossier
//! en donnerait la propriété exclusive au premier arrivé. Arbitrage actuel :
//! **rien n'est jamais écrasé** — un fichier déjà présent (Kunos ou posé par un
//! autre mod) est laissé intact et signalé.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use walkdir::WalkDir;

use crate::config::AppConfig;
use crate::modscan::ModKind;
use crate::overlay;

/// Arbre des satellites d'un mod : `<lib>/satellites/<type>/<id>`.
pub fn dir(library: &Path, kind: ModKind, id: &str) -> PathBuf {
    library.join("satellites").join(kind.content_folder()).join(id)
}

/// Range un reste sous le satellite du mod, à `rel` (son chemin relatif à la
/// racine de l'archive, donc à la racine d'AC). Fusionne avec l'existant : une
/// mise à jour du mod remplace ses propres fichiers, sans effacer les autres.
pub fn store(sat_dir: &Path, rel: &Path, src: &Path, copy: bool) -> Result<(), String> {
    let dest = sat_dir.join(rel);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("satellite : {e}"))?;
    }
    if src.is_dir() {
        if copy {
            crate::archive::copy_dir(src, &dest)
        } else {
            crate::archive::move_dir(src, &dest)
        }
        .map_err(|e| format!("satellite : {e}"))
    } else {
        if !copy && std::fs::rename(src, &dest).is_ok() {
            return Ok(());
        }
        std::fs::copy(src, &dest)
            .map(|_| ())
            .map_err(|e| format!("satellite : {e}"))
    }
}

/// Supprime l'arbre des satellites d'un mod (suppression du mod).
pub fn remove_tree(library: &Path, kind: ModKind, id: &str) {
    let d = dir(library, kind, id);
    if d.exists() {
        if let Err(e) = std::fs::remove_dir_all(&d) {
            log::warn!("remove satellite tree {}: {e}", d.display());
        }
    }
}

/// Pose les satellites du mod dans AC et mémorise exactement ce qui a été posé
/// — c'est cette liste, et elle seule, qui sera retirée à la désactivation.
/// Best-effort : un fichier qui ne peut pas être posé est signalé, jamais forcé.
pub fn deploy(conn: &Connection, cfg: &AppConfig, kind: ModKind, mod_id: &str) -> Result<usize, String> {
    let (Some(library), Some(ac)) = (cfg.library_path.as_ref(), cfg.ac_install_path.as_ref()) else {
        return Ok(0);
    };
    let sat = dir(library, kind, mod_id);
    if !sat.is_dir() {
        return Ok(0);
    }

    let mut files: Vec<String> = Vec::new();
    let mut created_dirs: BTreeSet<PathBuf> = BTreeSet::new();
    for entry in WalkDir::new(&sat).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let src = entry.path();
        let Ok(rel) = src.strip_prefix(&sat) else { continue };
        let target = ac.join(rel);
        // Jamais d'écrasement (règle d'or n°5, §7.3) : un fichier déjà là —
        // contenu Kunos ou satellite d'un autre mod livrant le même fichier
        // partagé — est laissé intact. Le mod fonctionne quand même dans la
        // quasi-totalité des cas (fichiers partagés identiques d'une voiture à
        // l'autre) ; l'arbitrage fin est le sujet du refcount (§4.6ter).
        if target.exists() {
            continue;
        }
        // Dossiers qu'il faut créer pour poser ce fichier — mémorisés avant de
        // les créer, sinon on ne saurait plus, au retrait, lesquels étaient
        // déjà là. « Dossier vide » ne suffit pas comme critère : un dossier
        // d'AC préexistant peut le devenir.
        let mut cur = target.parent();
        while let Some(d) = cur {
            if d == ac || !d.starts_with(ac) || d.exists() {
                break;
            }
            created_dirs.insert(d.to_path_buf());
            cur = d.parent();
        }
        match crate::deploy::link_or_copy(src, &target) {
            Ok(()) => files.push(target.to_string_lossy().into_owned()),
            Err(e) => log::warn!("satellite deploy {} -> {}: {e}", mod_id, target.display()),
        }
    }

    let placed = files.len();
    let mut entries: Vec<(String, bool)> = files.into_iter().map(|f| (f, false)).collect();
    entries.extend(
        created_dirs
            .into_iter()
            .map(|d| (d.to_string_lossy().into_owned(), true)),
    );
    overlay::set_satellite_links(conn, mod_id, &entries).map_err(|e| e.to_string())?;
    Ok(placed)
}

/// Retire exactement ce qui a été posé à la dernière activation : les fichiers,
/// puis les dossiers créés pour l'occasion, du plus profond au plus superficiel.
/// `remove_dir` échoue sur un dossier non vide — c'est le second garde-fou : un
/// dossier encore utilisé par un autre mod n'est jamais emporté.
pub fn undeploy(conn: &Connection, cfg: &AppConfig, mod_id: &str) -> Result<(), String> {
    let links = overlay::get_satellite_links(conn, mod_id).map_err(|e| e.to_string())?;
    // Garde-fou : on n'efface jamais hors du dossier d'AC, même si la base dit
    // le contraire (bibliothèque déplacée, chemin d'AC changé depuis la pose).
    let ac = cfg.ac_install_path.as_ref();
    let inside_ac = |p: &Path| ac.is_some_and(|ac| p.starts_with(ac));

    for (p, _) in links.iter().filter(|(_, is_dir)| !is_dir) {
        let p = Path::new(p);
        if !inside_ac(p) {
            log::warn!("satellite undeploy {}: outside AC, skipped", p.display());
            continue;
        }
        if p.is_file() {
            if let Err(e) = std::fs::remove_file(p) {
                log::warn!("satellite undeploy {}: {e}", p.display());
            }
        }
    }
    let mut dirs: Vec<&Path> = links
        .iter()
        .filter(|(_, is_dir)| *is_dir)
        .map(|(p, _)| Path::new(p.as_str()))
        .filter(|p| inside_ac(p))
        .collect();
    // Du plus profond au plus superficiel, sinon un parent encore peuplé de ses
    // propres sous-dossiers ne pourrait jamais être retiré.
    dirs.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for d in dirs {
        let _ = std::fs::remove_dir(d);
    }
    overlay::set_satellite_links(conn, mod_id, &[]).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(p: &Path, body: &[u8]) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    fn cfg_for(base: &Path) -> AppConfig {
        AppConfig {
            library_path: Some(base.join("library")),
            ac_install_path: Some(base.join("ac")),
            ..Default::default()
        }
    }

    #[test]
    fn satellite_deployed_on_activate_and_fully_removed_on_deactivate() {
        // §4.6ter : le satellite vit et meurt avec son mod. C'est ce que le
        // passage par « autre mod » ne donnait pas — les fichiers d'une voiture
        // désinstallée restaient dans AC, rattachés à une entrée anonyme.
        let base = crate::testutil::temp_dir("sat");
        let cfg = cfg_for(&base);
        let library = cfg.library_path.clone().unwrap();
        let ac = cfg.ac_install_path.clone().unwrap();
        std::fs::create_dir_all(ac.join("content")).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();

        let sat = dir(&library, ModKind::Car, "rss_car");
        write(&sat.join("extension").join("config").join("cars").join("x.ini"), b"cfg");
        write(&sat.join("content").join("driver").join("pro.kn5"), b"model");

        let n = deploy(&conn, &cfg, ModKind::Car, "rss_car").unwrap();
        assert_eq!(n, 2, "les deux satellites sont posés");
        assert!(ac.join("extension").join("config").join("cars").join("x.ini").is_file());
        assert!(ac.join("content").join("driver").join("pro.kn5").is_file());

        undeploy(&conn, &cfg, "rss_car").unwrap();
        assert!(!ac.join("extension").join("config").join("cars").join("x.ini").exists());
        assert!(!ac.join("content").join("driver").join("pro.kn5").exists());
        assert!(
            !ac.join("extension").exists(),
            "les dossiers créés pour l'occasion sont élagués"
        );
        assert!(
            ac.join("content").is_dir(),
            "un dossier AC préexistant n'est jamais emporté"
        );
        assert!(
            sat.join("content").join("driver").join("pro.kn5").is_file(),
            "la bibliothèque garde le satellite : réactivable sans réimport"
        );
    }

    #[test]
    fn satellite_never_overwrites_an_existing_file() {
        // Règle d'or n°5 : aucun fichier du jeu altéré. Un fichier déjà présent
        // — contenu Kunos, ou satellite d'un autre mod livrant le même fichier
        // partagé — est laissé intact, et n'entre pas dans la liste des liens
        // posés (donc la désactivation ne peut pas l'emporter).
        let base = crate::testutil::temp_dir("sat-noover");
        let cfg = cfg_for(&base);
        let library = cfg.library_path.clone().unwrap();
        let ac = cfg.ac_install_path.clone().unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();

        let kunos = ac.join("system").join("shaders").join("stock.fxo");
        write(&kunos, b"KUNOS");
        let sat = dir(&library, ModKind::Car, "rss_car");
        write(&sat.join("system").join("shaders").join("stock.fxo"), b"MOD");
        write(&sat.join("system").join("shaders").join("new.fxo"), b"MOD");

        let n = deploy(&conn, &cfg, ModKind::Car, "rss_car").unwrap();
        assert_eq!(n, 1, "seul le fichier nouveau est posé");
        assert_eq!(std::fs::read(&kunos).unwrap(), b"KUNOS", "fichier du jeu intact");

        undeploy(&conn, &cfg, "rss_car").unwrap();
        assert!(
            kunos.is_file(),
            "la désactivation n'emporte pas ce qu'elle n'a pas posé"
        );
        assert_eq!(std::fs::read(&kunos).unwrap(), b"KUNOS");
    }

    #[test]
    fn store_merges_into_the_existing_satellite_tree() {
        // Une mise à jour du mod remplace ses propres fichiers sans effacer les
        // autres : l'arbre est au niveau du mod, partagé par les versions.
        let base = crate::testutil::temp_dir("sat-store");
        let sat = base.join("sat");
        write(&sat.join("system").join("old.fxo"), b"old");

        let src = base.join("src").join("extension");
        write(&src.join("config").join("a.ini"), b"a");
        store(&sat, Path::new("extension"), &src, true).unwrap();

        assert!(sat.join("extension").join("config").join("a.ini").is_file());
        assert!(sat.join("system").join("old.fxo").is_file(), "l'existant est conservé");
        assert!(src.join("config").join("a.ini").is_file(), "copie : source intacte");
    }
}
