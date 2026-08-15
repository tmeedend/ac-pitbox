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
//! en donnerait la propriété exclusive au premier arrivé.
//!
//! **Fichiers partagés** : chaque mod *réclame* les chemins d'AC dont il a
//! besoin (`satellite_links`), et deux règles suffisent.
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
//! Un fichier que **personne ne réclame** est hors jeu : contenu Kunos, ou mod
//! installé hors de l'app. Jamais touché (règle d'or n°5), et surtout jamais
//! enregistré — sinon une désactivation pourrait l'emporter.

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
    let rows = overlay::satellite_claimants(conn, &ac_path.to_string_lossy())
        .inspect_err(|e| log::warn!("satellite_claimants {}: {e}", ac_path.display()))
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
        if ac_path.is_file() {
            if let Err(e) = std::fs::remove_file(ac_path) {
                log::warn!("satellite remove {}: {e}", ac_path.display());
            }
        }
        return;
    };
    let current = overlay::satellite_provider(conn, &key).unwrap_or(None);
    if current.as_deref() == Some(best.mod_id.as_str()) && ac_path.is_file() {
        return;
    }
    if ac_path.exists() {
        if let Err(e) = std::fs::remove_file(ac_path) {
            log::warn!("satellite replace {}: {e}", ac_path.display());
            return;
        }
    }
    match crate::deploy::link_or_copy(&best.src, ac_path) {
        Ok(()) => {
            if let Err(e) = overlay::set_satellite_provider(conn, &key, &best.mod_id) {
                log::warn!("set_satellite_provider {}: {e}", ac_path.display());
            }
        }
        Err(e) => log::warn!("satellite replace {} <- {}: {e}", ac_path.display(), best.mod_id),
    }
}

/// Pose les satellites du mod dans AC et mémorise exactement ce qu'il réclame
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

        // Fichier déjà là que **personne ne réclame** : contenu du jeu, ou mod
        // installé hors de l'app. Jamais touché (règle d'or n°5), et surtout
        // jamais enregistré — sinon une désactivation pourrait l'emporter.
        // Réclamé par un autre mod, en revanche, c'est un fichier partagé : on
        // s'y ajoute et l'arbitrage (`sync`) tranche.
        if target.exists() {
            let claimed = overlay::satellite_claimants(conn, &target.to_string_lossy())
                .map(|c| !c.is_empty())
                .unwrap_or(false);
            if !claimed {
                log::warn!("satellite {mod_id}: {} already exists, left alone", target.display());
                continue;
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
            Err(e) => log::warn!("satellite deploy {} -> {}: {e}", mod_id, target.display()),
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
    overlay::set_satellite_links(conn, mod_id, kind.content_folder(), &entries).map_err(|e| e.to_string())?;
    for f in &files {
        sync(conn, cfg, f);
    }
    Ok(placed)
}

/// Retire la réclamation du mod sur ses satellites, puis réaligne chaque
/// fichier : encore réclamé par un autre mod, il **reste** (et repasse à
/// l'exemplaire du meilleur réclamant restant) ; plus réclamé du tout, il est
/// retiré. C'est le compteur de références des fichiers partagés (§4.6ter) —
/// désactiver une voiture RSS n'emporte pas les textures communes dont douze
/// autres dépendent. Puis les dossiers créés pour l'occasion sont élagués, du
/// plus profond au plus superficiel ; `remove_dir` échoue sur un dossier non
/// vide, second garde-fou.
pub fn undeploy(conn: &Connection, cfg: &AppConfig, mod_id: &str) -> Result<(), String> {
    let links = overlay::get_satellite_links(conn, mod_id).map_err(|e| e.to_string())?;
    // Garde-fou : on n'efface jamais hors du dossier d'AC, même si la base dit
    // le contraire (bibliothèque déplacée, chemin d'AC changé depuis la pose).
    let ac = cfg.ac_install_path.as_ref();
    let inside_ac = |p: &Path| ac.is_some_and(|ac| p.starts_with(ac));

    // La réclamation part d'abord : `sync` compte ce qui reste en base, ce mod
    // ne doit plus y figurer.
    overlay::set_satellite_links(conn, mod_id, "", &[]).map_err(|e| e.to_string())?;
    for (p, _) in links.iter().filter(|(_, is_dir)| !is_dir) {
        let p = Path::new(p);
        if !inside_ac(p) {
            log::warn!("satellite undeploy {}: outside AC, skipped", p.display());
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
        // §4.6ter — compteur de références. Douze voitures RSS livrent le même
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
