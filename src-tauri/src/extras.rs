//! Ajouts au jeu d'un mod (§4.5.3) : ce qu'une archive livre **à côté** du
//! dossier du mod mais qui lui appartient — configs CSP
//! (`extension/config/cars/rss/<id>/…`), shaders (`system/shaders/…`),
//! textures d'équipe (`content/texture/…`), modèle de pilote
//! (`content/driver/…`). AC les lit hors de `content/<type>/<id>`, ils ne
//! peuvent donc pas voyager dans le dossier du mod.
//!
//! **Stockés bruts, avec leur chemin relatif à la racine d'AC**, dans un arbre
//! dédié (`<lib>/extras/<type>/<id>/…`) — jamais dans la version, qui est
//! déployée telle quelle dans `content/`. Deux propriétés en découlent :
//!
//! - **L'import ne jette rien.** Ce qui n'est pas classé est conservé tel quel,
//!   donc l'*interprétation* (où poser, qui arbitre un fichier partagé) reste
//!   recalculable depuis la bibliothèque à tout moment. Aucune règle des
//!   versions précédentes à mémoriser, aucune archive à conserver : c'est
//!   l'entrée qui est préservée, pas la décision.
//! - **L'ajout vit et meurt avec son mod.** Posé à l'activation, retiré à
//!   la désactivation, supprimé avec lui — c'est ce que le passage par « autre
//!   mod » ne donnait pas : les fichiers d'une voiture désinstallée restaient
//!   dans AC, rattachés à une entrée anonyme que plus rien ne reliait au mod.
//!
//! Au **niveau du mod**, pas de la version (comme `resources/`, §4.5.2) : les
//! configs CSP d'une mise à jour remplacent celles de la précédente, ce qui est
//! le comportement voulu, et les couches (§4.3) partagent le même arbre.
//!
//! Pose **fichier par fichier** (hardlink), jamais par jonction de dossier :
//! plusieurs mods visent les mêmes arbres (`extension/textures/common/rss/…`
//! est livré à l'identique par chaque voiture RSS), et une jonction de dossier
//! en donnerait la propriété exclusive au premier arrivé.
//!
//! **Fichiers partagés** : chaque mod *réclame* les chemins d'AC dont il a
//! besoin (`extra_links`), et deux règles suffisent.
//!
//! - *Compteur de références* — un fichier n'est retiré d'AC que lorsque plus
//!   aucun mod ne le réclame. Désactiver une voiture RSS n'emporte pas les
//!   textures communes dont douze autres dépendent, et il n'y a plus de course
//!   à la propriété : le premier arrivé ne gagne rien.
//! - *Arbitrage par date* — l'exemplaire à la **date de modification la plus
//!   récente** gagne, un mod plus récent corrigeant en général des bugs de
//!   celui d'avant. La date traverse la chaîne intacte : 7-Zip restitue celle
//!   stockée dans l'archive, `std::fs::copy` la conserve sous Windows, et un
//!   hardlink partage l'entrée MFT. À égalité (archives repackées par un tiers,
//!   qui perdent les dates), c'est le dernier mod installé.
//!
//! Un fichier que **personne ne réclame** — contenu Kunos, ou mod installé hors
//! de l'app — relève du même arbitrage : un exemplaire plus récent le remplace,
//! mais seulement après que l'original a été mis à l'abri (`gamebackup`, §4.5.4),
//! et il revient dès que plus aucun mod ne réclame le chemin. Un exemplaire
//! plus ancien ou de même date ne prend jamais la place de ce qui tourne déjà.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use walkdir::WalkDir;

use crate::config::AppConfig;
use crate::modscan::ModKind;
use crate::overlay;

/// Arbre des ajouts au jeu d'un mod : `<lib>/extras/<type>/<id>`.
pub fn dir(library: &Path, kind: ModKind, id: &str) -> PathBuf {
    library.join("extras").join(kind.content_folder()).join(id)
}

/// Range un reste dans les ajouts au jeu du mod, à `rel` (son chemin relatif à la
/// racine de l'archive, donc à la racine d'AC). Fusionne avec l'existant : une
/// mise à jour du mod remplace ses propres fichiers, sans effacer les autres.
pub fn store(sat_dir: &Path, rel: &Path, src: &Path, copy: bool) -> Result<(), String> {
    let dest = sat_dir.join(rel);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("ajout au jeu : {e}"))?;
    }
    if src.is_dir() {
        if copy {
            crate::archive::copy_dir(src, &dest)
        } else {
            crate::archive::move_dir(src, &dest)
        }
        .map_err(|e| format!("ajout au jeu : {e}"))
    } else {
        if !copy && std::fs::rename(src, &dest).is_ok() {
            return Ok(());
        }
        std::fs::copy(src, &dest)
            .map(|_| ())
            .map_err(|e| format!("ajout au jeu : {e}"))
    }
}

/// Supprime l'arbre des ajouts au jeu d'un mod (suppression du mod).
pub fn remove_tree(library: &Path, kind: ModKind, id: &str) {
    let d = dir(library, kind, id);
    if d.exists() {
        if let Err(e) = std::fs::remove_dir_all(&d) {
            log::warn!("remove extras tree {}: {e}", d.display());
        }
    }
}

/// Exemplaire d'un fichier partagé proposé par un mod.
struct Claim {
    mod_id: String,
    src: PathBuf,
    /// Date de modification du fichier **dans l'archive de l'auteur** : 7-Zip
    /// restitue la date stockée, `std::fs::copy` la conserve sous Windows, et
    /// un hardlink partage l'entrée MFT. Elle traverse donc toute la chaîne
    /// intacte, et distingue deux versions d'un même fichier RSS.
    mtime: std::time::SystemTime,
    /// Départage deux exemplaires de même date : le dernier mod installé gagne.
    claimed_at: String,
}

/// Qui, parmi les mods qui réclament ce fichier, fournit l'exemplaire à poser.
/// **La date de modification la plus récente gagne** — un mod plus récent
/// corrige en général des bugs de celui d'avant. À égalité (archives repackées
/// par un tiers, qui perdent les dates), c'est le dernier mod installé.
fn best_claim(conn: &Connection, cfg: &AppConfig, ac_path: &Path) -> Option<Claim> {
    let (library, ac) = (cfg.library_path.as_ref()?, cfg.ac_install_path.as_ref()?);
    let rel = ac_path.strip_prefix(ac).ok()?;
    let rows = overlay::extra_claimants(conn, &ac_path.to_string_lossy())
        .inspect_err(|e| log::warn!("extra_claimants {}: {e}", ac_path.display()))
        .ok()?;
    rows.into_iter()
        .filter_map(|(mod_id, kind, claimed_at)| {
            let kind = if kind.eq_ignore_ascii_case("Track") {
                ModKind::Track
            } else {
                ModKind::Car
            };
            let src = dir(library, kind, &mod_id).join(rel);
            let mtime = std::fs::metadata(&src).and_then(|m| m.modified()).ok()?;
            Some(Claim {
                mod_id,
                src,
                mtime,
                claimed_at,
            })
        })
        .max_by(|a, b| a.mtime.cmp(&b.mtime).then_with(|| a.claimed_at.cmp(&b.claimed_at)))
}

/// Aligne le fichier posé dans AC sur l'exemplaire qui doit gagner. Sans
/// réclamant, le fichier est retiré : plus aucun mod n'en dépend.
///
/// Le fournisseur courant est lu en base, jamais déduit de la taille et de la
/// date du fichier posé : c'est précisément dans le cas qu'on veut arbitrer —
/// deux exemplaires de même date — que cette déduction se trompe.
fn sync(conn: &Connection, cfg: &AppConfig, ac_path: &Path) {
    let key = ac_path.to_string_lossy().into_owned();
    let Some(best) = best_claim(conn, cfg, ac_path) else {
        // Plus aucun réclamant. Si ce chemin était un fichier du jeu qu'un mod
        // avait remplacé, l'original revient (§4.5.4) ; sinon le fichier part.
        if crate::gamebackup::is_replaced(conn, ac_path) {
            crate::gamebackup::restore(conn, ac_path);
        } else if ac_path.is_file() {
            if let Err(e) = std::fs::remove_file(ac_path) {
                log::warn!("extras remove {}: {e}", ac_path.display());
            }
        }
        return;
    };
    let current = overlay::extra_provider(conn, &key).unwrap_or(None);
    if current.as_deref() == Some(best.mod_id.as_str()) && ac_path.is_file() {
        return;
    }
    if ac_path.exists() {
        if let Err(e) = std::fs::remove_file(ac_path) {
            log::warn!("extras replace {}: {e}", ac_path.display());
            return;
        }
    }
    match crate::deploy::link_or_copy(&best.src, ac_path) {
        Ok(()) => {
            if let Err(e) = overlay::set_extra_provider(conn, &key, &best.mod_id) {
                log::warn!("set_extra_provider {}: {e}", ac_path.display());
            }
        }
        Err(e) => log::warn!("extras replace {} <- {}: {e}", ac_path.display(), best.mod_id),
    }
}

/// Pose les ajouts au jeu du mod dans AC et mémorise exactement ce qu'il réclame
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

    let mut files: Vec<PathBuf> = Vec::new();
    let mut created_dirs: BTreeSet<PathBuf> = BTreeSet::new();
    for entry in WalkDir::new(&sat).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let src = entry.path();
        let Ok(rel) = src.strip_prefix(&sat) else { continue };
        let target = ac.join(rel);

        // Fichier déjà présent : trois cas, et un seul est un refus.
        if target.exists() {
            let claimed = overlay::extra_claimants(conn, &target.to_string_lossy())
                .map(|c| !c.is_empty())
                .unwrap_or(false);
            // 1. Réclamé par un autre mod : fichier partagé, on s'y ajoute et
            //    l'arbitrage par date (`sync`) tranche.
            // 2. Déjà remplacé par nous : l'original est à l'abri, même chose.
            // 3. Fichier du jeu intact : le **même arbitrage par date**
            //    s'applique. Un exemplaire plus récent le remplace, après
            //    sauvegarde (§4.5.4) ; un exemplaire plus ancien ou de même date
            //    ne prend pas la place de ce qui tourne déjà. Sans cette
            //    comparaison, le dernier mod installé écraserait une font mise
            //    à jour par un autre outil, ce que rien ne justifie.
            if !claimed && !crate::gamebackup::is_replaced(conn, &target) {
                if !crate::gamebackup::is_newer(src, &target) {
                    log::warn!(
                        "extras {mod_id}: {} exists and is not older, left alone",
                        target.display()
                    );
                    continue;
                }
                // `protect` refuse s'il n'a pas pu sécuriser l'original — et
                // alors on ne touche à rien.
                if !crate::gamebackup::protect(conn, cfg, &target) {
                    log::warn!(
                        "extras {mod_id}: {} could not be backed up, left alone",
                        target.display()
                    );
                    continue;
                }
            }
            files.push(target);
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
            Ok(()) => files.push(target),
            Err(e) => log::warn!("extras deploy {} -> {}: {e}", mod_id, target.display()),
        }
    }

    let placed = files.len();
    let mut entries: Vec<(String, bool)> = files
        .iter()
        .map(|f| (f.to_string_lossy().into_owned(), false))
        .collect();
    entries.extend(
        created_dirs
            .into_iter()
            .map(|d| (d.to_string_lossy().into_owned(), true)),
    );
    // Enregistré **avant** l'arbitrage : `sync` lit les réclamations en base,
    // ce mod doit donc déjà y figurer pour pouvoir gagner.
    overlay::set_extra_links(conn, mod_id, kind.content_folder(), &entries).map_err(|e| e.to_string())?;
    for f in &files {
        sync(conn, cfg, f);
    }
    Ok(placed)
}

/// Retire la réclamation du mod sur ses ajouts au jeu, puis réaligne chaque
/// fichier : encore réclamé par un autre mod, il **reste** (et repasse à
/// l'exemplaire du meilleur réclamant restant) ; plus réclamé du tout, il est
/// retiré. C'est le compteur de références des fichiers partagés (§4.5.4) —
/// désactiver une voiture RSS n'emporte pas les textures communes dont douze
/// autres dépendent. Puis les dossiers créés pour l'occasion sont élagués, du
/// plus profond au plus superficiel ; `remove_dir` échoue sur un dossier non
/// vide, second garde-fou.
pub fn undeploy(conn: &Connection, cfg: &AppConfig, mod_id: &str) -> Result<(), String> {
    let links = overlay::get_extra_links(conn, mod_id).map_err(|e| e.to_string())?;
    // Garde-fou : on n'efface jamais hors du dossier d'AC, même si la base dit
    // le contraire (bibliothèque déplacée, chemin d'AC changé depuis la pose).
    let ac = cfg.ac_install_path.as_ref();
    let inside_ac = |p: &Path| ac.is_some_and(|ac| p.starts_with(ac));

    // La réclamation part d'abord : `sync` compte ce qui reste en base, ce mod
    // ne doit plus y figurer.
    overlay::set_extra_links(conn, mod_id, "", &[]).map_err(|e| e.to_string())?;
    for (p, _) in links.iter().filter(|(_, is_dir)| !is_dir) {
        let p = Path::new(p);
        if !inside_ac(p) {
            log::warn!("extras undeploy {}: outside AC, skipped", p.display());
            continue;
        }
        sync(conn, cfg, p);
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
    Ok(())
}

/// Une entrée de l'onglet « Ajouts au jeu » de la fiche (§4.5.5).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExtraFile {
    /// Chemin relatif à la racine d'AC — c'est *l'*information utile : elle dit
    /// où le fichier atterrit dans le jeu (`extension/config/cars/…`).
    pub rel_path: String,
    pub size_bytes: u64,
    /// Actuellement posé dans AC par ce mod. Faux = un autre mod fournit le
    /// même fichier (partagé), ou le mod est inactif.
    pub deployed: bool,
    /// Mod qui fournit l'exemplaire posé, quand ce n'est pas celui-ci.
    pub provided_by: Option<String>,
    /// Ce chemin était un fichier du jeu : l'original est sauvegardé et sera
    /// restauré (§4.5.4). Signalé sur la fiche — une modification réversible mais
    /// invisible reste un piège.
    pub replaces_game_file: bool,
}

/// Liste ce qu'un mod installe hors de `content/<type>/<id>`, **lu en direct
/// sur disque** comme le bloc Ressources (§4.5.5) : un mod importé avant que
/// l'app ne suive ces fichiers n'a rien à réimporter pour que l'onglet se
/// remplisse. L'état de pose, lui, vient de la base.
pub fn list(conn: &Connection, cfg: &AppConfig, kind: ModKind, mod_id: &str) -> Vec<ExtraFile> {
    let (Some(library), Some(ac)) = (cfg.library_path.as_ref(), cfg.ac_install_path.as_ref()) else {
        return Vec::new();
    };
    let sat = dir(library, kind, mod_id);
    if !sat.is_dir() {
        return Vec::new();
    }
    let mut out: Vec<ExtraFile> = WalkDir::new(&sat)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let rel = e.path().strip_prefix(&sat).ok()?.to_path_buf();
            let target = ac.join(&rel);
            let provider = overlay::extra_provider(conn, &target.to_string_lossy()).unwrap_or(None);
            Some(ExtraFile {
                rel_path: rel.to_string_lossy().replace('\\', "/"),
                size_bytes: e.metadata().map(|m| m.len()).unwrap_or(0),
                deployed: provider.as_deref() == Some(mod_id),
                provided_by: provider.filter(|p| p != mod_id),
                replaces_game_file: crate::gamebackup::is_replaced(conn, &target),
            })
        })
        .collect();
    out.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    out
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

    /// Date de modification explicite — c'est le critère d'arbitrage, il faut
    /// pouvoir le poser plutôt que dépendre de l'ordre d'écriture des tests.
    fn set_mtime(p: &Path, secs_since_epoch: u64) {
        let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs_since_epoch);
        std::fs::File::options()
            .write(true)
            .open(p)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    #[test]
    fn shared_file_survives_until_the_last_mod_stops_claiming_it() {
        // §4.5.4 — compteur de références. Douze voitures RSS livrent le même
        // `extension/textures/common/rss/…` : en désactiver une ne doit pas
        // emporter le fichier dont les onze autres dépendent. Et l'arbitrage
        // par date décide de l'exemplaire posé, dans les deux sens : quand le
        // plus récent s'en va, on repasse à celui qui reste.
        let base = crate::testutil::temp_dir("sat-shared");
        let cfg = cfg_for(&base);
        let library = cfg.library_path.clone().unwrap();
        let ac = cfg.ac_install_path.clone().unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();

        let rel = Path::new("extension").join("textures").join("common").join("rss.dds");
        let old_car = dir(&library, ModKind::Car, "rss_old");
        let new_car = dir(&library, ModKind::Car, "rss_new");
        write(&old_car.join(&rel), b"ANCIENNE");
        write(&new_car.join(&rel), b"NOUVELLE");
        set_mtime(&old_car.join(&rel), 1_000_000);
        set_mtime(&new_car.join(&rel), 2_000_000);
        let target = ac.join(&rel);

        // La plus ancienne d'abord : elle pose le fichier.
        deploy(&conn, &cfg, ModKind::Car, "rss_old").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"ANCIENNE");

        // La plus récente ensuite : son exemplaire gagne, sans dépendre de
        // l'ordre d'installation.
        deploy(&conn, &cfg, ModKind::Car, "rss_new").unwrap();
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"NOUVELLE",
            "la date de modification la plus récente gagne"
        );

        // La plus récente s'en va : le fichier reste, à l'exemplaire restant.
        undeploy(&conn, &cfg, "rss_new").unwrap();
        assert!(target.is_file(), "encore réclamé par rss_old : jamais retiré");
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"ANCIENNE",
            "on repasse à l'exemplaire du réclamant restant"
        );

        // Plus personne : le fichier part, et les dossiers créés avec lui.
        undeploy(&conn, &cfg, "rss_old").unwrap();
        assert!(!target.exists(), "plus aucun réclamant : retiré");
        assert!(!ac.join("extension").exists(), "dossiers créés pour l'occasion élagués");
    }

    #[test]
    fn list_reports_where_each_file_lands_and_who_provides_it() {
        // §4.5.5, onglet « Ajouts au jeu » : la fiche doit dire *où* le mod
        // pose ses fichiers dans le jeu, et lesquels sont en fait fournis par
        // un autre mod (fichier partagé). Sans ça, un mod peut poser 69
        // fichiers hors de son dossier sans que rien ne le montre.
        let base = crate::testutil::temp_dir("sat-list");
        let cfg = cfg_for(&base);
        let library = cfg.library_path.clone().unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();

        let shared = Path::new("system").join("shaders").join("shared.fxo");
        let own = Path::new("extension").join("config").join("mine.ini");
        write(&dir(&library, ModKind::Car, "car_a").join(&shared), b"AAAA");
        write(&dir(&library, ModKind::Car, "car_a").join(&own), b"MINE");
        write(&dir(&library, ModKind::Car, "car_b").join(&shared), b"BBBB");
        // car_b, plus récent, gagnera le fichier partagé.
        set_mtime(&dir(&library, ModKind::Car, "car_a").join(&shared), 1_000_000);
        set_mtime(&dir(&library, ModKind::Car, "car_b").join(&shared), 2_000_000);

        deploy(&conn, &cfg, ModKind::Car, "car_a").unwrap();
        deploy(&conn, &cfg, ModKind::Car, "car_b").unwrap();

        let listed = list(&conn, &cfg, ModKind::Car, "car_a");
        assert_eq!(listed.len(), 2, "les deux fichiers de car_a sont listés");

        let mine = listed
            .iter()
            .find(|f| f.rel_path == "extension/config/mine.ini")
            .unwrap();
        assert!(mine.deployed, "fichier propre : posé par ce mod");
        assert!(mine.provided_by.is_none());
        assert_eq!(mine.size_bytes, 4);

        let sh = listed
            .iter()
            .find(|f| f.rel_path == "system/shaders/shared.fxo")
            .unwrap();
        assert!(!sh.deployed, "fichier partagé perdu à l'arbitrage");
        assert_eq!(
            sh.provided_by.as_deref(),
            Some("car_b"),
            "la fiche nomme le mod qui fournit l'exemplaire posé"
        );

        // Chemins relatifs à AC, séparateurs normalisés pour l'affichage.
        assert!(listed.iter().all(|f| !f.rel_path.contains('\\')));
    }

    #[test]
    fn equal_dates_are_settled_by_the_last_installed_mod() {
        // Archives repackées par un tiers : toutes les dates sont identiques,
        // le critère principal ne départage plus. Repli sur le dernier mod
        // installé — ici le dernier à avoir réclamé le fichier.
        let base = crate::testutil::temp_dir("sat-tie");
        let cfg = cfg_for(&base);
        let library = cfg.library_path.clone().unwrap();
        let ac = cfg.ac_install_path.clone().unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();

        let rel = Path::new("system").join("shaders").join("shared.fxo");
        for (id, body) in [("car_a", b"AAAA"), ("car_b", b"BBBB")] {
            let p = dir(&library, ModKind::Car, id).join(&rel);
            write(&p, body);
            set_mtime(&p, 1_500_000);
        }

        deploy(&conn, &cfg, ModKind::Car, "car_a").unwrap();
        assert_eq!(std::fs::read(ac.join(&rel)).unwrap(), b"AAAA");
        deploy(&conn, &cfg, ModKind::Car, "car_b").unwrap();
        assert_eq!(
            std::fs::read(ac.join(&rel)).unwrap(),
            b"BBBB",
            "à égalité de date, le dernier installé gagne"
        );
    }

    #[test]
    fn extras_deployed_on_activate_and_fully_removed_on_deactivate() {
        // §4.5.3 : l'ajout vit et meurt avec son mod. C'est ce que le
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
        assert_eq!(n, 2, "les deux ajouts sont posés");
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
            "la bibliothèque garde l'ajout : réactivable sans réimport"
        );
    }

    #[test]
    fn a_newer_mod_file_replaces_a_game_file_and_the_original_comes_back() {
        // §4.5.4 : la règle d'or n°5 n'interdit pas de toucher un fichier du jeu,
        // elle exige qu'il soit sauvegardé et restauré. Avant, la pose sautait
        // le fichier en silence et le mod s'installait à moitié — c'est ce qui
        // cassait les mods qui remplacent vraiment (HUD façon CMRT, shaders).
        let base = crate::testutil::temp_dir("sat-replace");
        let cfg = cfg_for(&base);
        let library = cfg.library_path.clone().unwrap();
        let ac = cfg.ac_install_path.clone().unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();

        let kunos = ac.join("system").join("shaders").join("stock.fxo");
        write(&kunos, b"KUNOS");
        set_mtime(&kunos, 1_000_000);
        let sat = dir(&library, ModKind::Car, "rss_car");
        write(&sat.join("system").join("shaders").join("stock.fxo"), b"MOD");
        write(&sat.join("system").join("shaders").join("new.fxo"), b"MOD");
        set_mtime(&sat.join("system").join("shaders").join("stock.fxo"), 2_000_000);

        let n = deploy(&conn, &cfg, ModKind::Car, "rss_car").unwrap();
        assert_eq!(n, 2, "le nouveau fichier ET le remplacement sont posés");
        assert_eq!(std::fs::read(&kunos).unwrap(), b"MOD", "le mod prend la place");
        assert!(
            crate::gamebackup::is_replaced(&conn, &kunos),
            "et le remplacement est tracé, pas silencieux"
        );

        undeploy(&conn, &cfg, "rss_car").unwrap();
        assert_eq!(
            std::fs::read(&kunos).unwrap(),
            b"KUNOS",
            "l'original du jeu revient à la désactivation"
        );
        assert!(!crate::gamebackup::is_replaced(&conn, &kunos));
        assert!(
            !ac.join("system").join("shaders").join("new.fxo").exists(),
            "l'ajout pur, lui, part"
        );
    }

    #[test]
    fn an_older_mod_file_never_displaces_what_already_runs() {
        // Même arbitrage par date que pour les fichiers partagés : un
        // exemplaire plus ancien (ou de même date) ne prend pas la place de ce
        // qui tourne déjà. Sans ça, le dernier mod installé écraserait une font
        // mise à jour par un autre outil, ce que rien ne justifie.
        let base = crate::testutil::temp_dir("sat-older");
        let cfg = cfg_for(&base);
        let library = cfg.library_path.clone().unwrap();
        let ac = cfg.ac_install_path.clone().unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();

        let existing = ac.join("content").join("fonts").join("shared.txt");
        write(&existing, b"RECENT");
        set_mtime(&existing, 2_000_000);
        let sat = dir(&library, ModKind::Car, "old_car");
        write(&sat.join("content").join("fonts").join("shared.txt"), b"ANCIEN");
        set_mtime(&sat.join("content").join("fonts").join("shared.txt"), 1_000_000);

        let n = deploy(&conn, &cfg, ModKind::Car, "old_car").unwrap();
        assert_eq!(n, 0, "rien posé : l'exemplaire du mod est plus ancien");
        assert_eq!(std::fs::read(&existing).unwrap(), b"RECENT", "intact");
        assert!(
            !crate::gamebackup::is_replaced(&conn, &existing),
            "aucune sauvegarde inutile"
        );

        undeploy(&conn, &cfg, "old_car").unwrap();
        assert_eq!(
            std::fs::read(&existing).unwrap(),
            b"RECENT",
            "la désactivation n'emporte pas ce qu'elle n'a pas posé"
        );
    }

    #[test]
    fn store_merges_into_the_existing_extras_tree() {
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
