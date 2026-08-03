//! Activation / désactivation dans `content/` (§2/§7).
//!
//! **Mécanisme actuel : hardlinks par fichier** (`deploy.rs`), comme Vortex —
//! zéro duplication, zéro reparse point, pas de droits admin. Toute nouvelle
//! activation utilise ce mécanisme.
//!
//! **Compat ascendante** : les mods déjà actifs sous l'ancien mécanisme
//! (symlink `mklink /D`, ci-dessous `create_junction`/`is_junction`/
//! `remove_junction` malgré leur nom — legacy) continuent de fonctionner tels
//! quels indéfiniment, inoffensif. Ils sont migrés vers les hardlinks
//! **transparemment à leur prochaine (ré)activation**, jamais par migration
//! forcée : `is_mod_active`/`activate`/`deactivate` reconnaissent les deux
//! formes, mais seule `deploy::deploy_tree`/`compose_tree` est utilisée pour
//! écrire du nouveau contenu.
//!
//! Garde-fou absolu : on ne supprime JAMAIS dans `content/` un dossier qui
//! n'est ni une junction/symlink créée par l'app, ni un déploiement hardlinks
//! marqué (`deploy::is_deployed`) — protection du contenu installé hors de
//! l'app (Kunos, autre outil).

use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use rusqlite::Connection;

use crate::config::AppConfig;
use crate::deploy;
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

pub(crate) fn content_link(cfg: &AppConfig, kind: ModKind, id: &str) -> Option<PathBuf> {
    cfg.ac_install_path
        .as_ref()
        .map(|ac| ac.join("content").join(kind.content_folder()).join(id))
}

/// Vrai si le mod est actif : déploiement hardlinks marqué (mécanisme
/// courant) OU symlink géré par l'app (ancien mécanisme, toujours reconnu).
pub fn is_mod_active(cfg: &AppConfig, kind: ModKind, id: &str) -> bool {
    content_link(cfg, kind, id).is_some_and(|l| is_junction(&l) || deploy::is_deployed(&l))
}

/// Crée une junction `link` → `target` via `mklink /J` (sans fenêtre console).
pub fn create_junction(link: &Path, target: &Path) -> Result<(), String> {
    let mut cmd = Command::new("cmd");
    cmd.arg("/C");
    // raw_arg : on maîtrise nous-mêmes les guillemets (chemins à espaces).
    #[cfg(windows)]
    {
        cmd.raw_arg(format!(
            "mklink /D \"{}\" \"{}\"",
            link.display(),
            target.display()
        ));
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = (link, target);
        return Err(crate::errors::JUNCTIONS_WINDOWS_ONLY.into());
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
        return Err(crate::errors::NOT_A_JUNCTION.into());
    }
    std::fs::remove_dir(link).map_err(|e| format!("suppression de la junction : {e}"))
}

/// Le garde-fou absolu du module : un vrai dossier dans `content/` n'est jamais
/// touché. Message destiné à l'utilisateur, donc clé i18n (cf. `errors.rs`).
const GUARD_MSG: &str = crate::errors::REAL_FOLDER_GUARD;

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
        .ok_or(crate::errors::MOD_NOT_FOUND)?;
    if m.is_stock {
        return Err(crate::errors::STOCK_NOT_ACTIVATABLE.into());
    }
    let kind = kind_of(&m.kind);

    let vid = version_id
        .map(str::to_string)
        .or(m.active_version_id)
        .ok_or(crate::errors::NO_VERSION_TO_ACTIVATE)?;
    let target = overlay::get_version_path(conn, &vid)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::VERSION_NOT_FOUND)?;
    let link = content_link(cfg, kind, mod_id).ok_or(crate::errors::AC_NOT_CONFIGURED)?;

    // Garde-fou + nettoyage d'un déploiement existant, quelle que soit sa
    // forme (symlink hérité ou hardlinks). Toute réactivation redéploie en
    // hardlinks, migrant transparemment les mods encore en symlink.
    // Une erreur de `symlink_metadata` = le lien n'existe pas : rien à nettoyer.
    if let Ok(meta) = std::fs::symlink_metadata(&link) {
        if meta.file_type().is_symlink() {
            remove_junction(&link)?;
        } else if deploy::is_deployed(&link) {
            deploy::remove_deployment(&link)?;
        } else {
            return Err(GUARD_MSG.into());
        }
    }

    deploy::deploy_tree(Path::new(&target), &link, mod_id, kind)?;
    overlay::set_active_version(conn, mod_id, &vid).map_err(|e| e.to_string())?;
    // Compose par-dessus la base si le mod a des couches actives (§4.4) ;
    // sans couche, recompose ré-affirme simplement la junction vers la version.
    crate::compose::recompose(conn, cfg, mod_id)?;
    Ok(())
}

/// Désactive un mod : retire la junction (le contenu reste dans la bibliothèque).
pub fn deactivate(conn: &Connection, cfg: &AppConfig, mod_id: &str) -> Result<(), String> {
    let m = overlay::get_mod(conn, mod_id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::MOD_NOT_FOUND)?;
    let kind = kind_of(&m.kind);
    let link = content_link(cfg, kind, mod_id).ok_or(crate::errors::AC_NOT_CONFIGURED)?;

    // Une erreur de `symlink_metadata` = rien sur le disque, donc déjà inactif.
    if let Ok(meta) = std::fs::symlink_metadata(&link) {
        if meta.file_type().is_symlink() {
            remove_junction(&link)?;
        } else if deploy::is_deployed(&link) {
            deploy::remove_deployment(&link)?;
        } else {
            return Err(GUARD_MSG.into());
        }
    }
    // Les couches (§4.4) restent enregistrées et seront réappliquées à la
    // prochaine activation — plus de dossier de composition intermédiaire à
    // nettoyer (le contenu composé, s'il y en avait, vivait directement dans
    // `link`, déjà retiré ci-dessus).
    Ok(())
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
            crate::testutil::temp_dir("junc");
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
    }

    #[test]
    fn activate_deactivate_leave_no_history() {
        // Activer/désactiver un mod ne doit plus polluer son historique.
        if !cfg!(windows) {
            return;
        }
        let base = crate::testutil::temp_dir("acthist");
        let ac = base.join("ac");
        let library = base.join("library");
        std::fs::create_dir_all(ac.join("content").join("cars")).unwrap();
        let carv = library.join("cars").join("test_car").join("v1");
        std::fs::create_dir_all(&carv).unwrap();
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_mod(&conn, "test_car", "Car", Some("B"), Some("Test"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn, "v1", "test_car", Some("1.0"), None, &now,
            &carv.to_string_lossy(), None, "sig", &[], &[], &[], &[], None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "test_car", "v1").unwrap();
        let cfg = AppConfig { ac_install_path: Some(ac), library_path: Some(library), ..Default::default() };

        activate(&conn, &cfg, "test_car", None).unwrap();
        deactivate(&conn, &cfg, "test_car").unwrap();

        assert!(overlay::get_history(&conn, "test_car").unwrap().is_empty());
    }

    /// Circuit de test (Spa) : `activate` doit déployer par hardlinks (§2), pas
    /// par symlink — sans droits admin (aucune élévation dans ce test, comme
    /// en usage réel : `CreateHardLinkW` n'en a jamais besoin, contrairement à
    /// `CreateSymbolicLink`). Vérifie aussi que `deactivate` retire proprement
    /// le déploiement sans toucher à la bibliothèque.
    #[test]
    fn activate_deploys_spa_via_hardlinks_not_symlink() {
        let base = crate::testutil::temp_dir("hardlink-spa");
        let ac = base.join("ac");
        let library = base.join("library");
        std::fs::create_dir_all(ac.join("content").join("tracks")).unwrap();
        let spav1 = library.join("tracks").join("spa").join("v1");
        std::fs::create_dir_all(spav1.join("ui")).unwrap();
        std::fs::write(spav1.join("ui").join("ui_track.json"), br#"{"name":"Spa"}"#).unwrap();
        std::fs::create_dir_all(spav1.join("ai")).unwrap();
        std::fs::write(spav1.join("ai").join("fast_lane.ai"), b"AI_DATA").unwrap();

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_mod(&conn, "spa", "Track", Some("B"), Some("Spa"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn, "v1", "spa", Some("1.0"), None, &now,
            &spav1.to_string_lossy(), None, "sig", &[], &[], &[], &[], None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "spa", "v1").unwrap();
        let cfg = AppConfig { ac_install_path: Some(ac.clone()), library_path: Some(library), ..Default::default() };

        activate(&conn, &cfg, "spa", None).unwrap();
        let link = ac.join("content").join("tracks").join("spa");

        assert!(!is_junction(&link), "plus de reparse point — vrai dossier");
        assert!(deploy::is_deployed(&link), "marqueur de déploiement hardlinks présent");
        assert!(link.join("ui").join("ui_track.json").is_file(), "toute l'arborescence est là (ex. ai/)");
        assert!(link.join("ai").join("fast_lane.ai").is_file());
        assert!(is_mod_active(&cfg, ModKind::Track, "spa"), "détecté actif via le nouveau mécanisme");

        deactivate(&conn, &cfg, "spa").unwrap();
        assert!(!link.exists(), "déploiement retiré");
        assert!(spav1.join("ai").join("fast_lane.ai").is_file(), "bibliothèque intacte après désactivation");
        assert!(!is_mod_active(&cfg, ModKind::Track, "spa"));
    }

    /// Un mod encore actif sous l'ancien mécanisme (symlink `mklink /D`) doit
    /// rester détecté actif tel quel (aucune migration forcée), et se migrer
    /// tout seul, silencieusement, à sa prochaine (ré)activation.
    #[test]
    fn legacy_symlinked_mod_still_detected_active_and_migrates_on_reactivate() {
        if !cfg!(windows) {
            return;
        }
        let base = crate::testutil::temp_dir("legacy-sym");
        let ac = base.join("ac");
        let library = base.join("library");
        std::fs::create_dir_all(ac.join("content").join("cars")).unwrap();
        let carv = library.join("cars").join("legacy_car").join("v1");
        std::fs::create_dir_all(&carv).unwrap();
        std::fs::write(carv.join("data.txt"), "legacy").unwrap();

        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        overlay::upsert_mod(&conn, "legacy_car", "Car", Some("B"), Some("Legacy"), "h", None, &now).unwrap();
        overlay::insert_version(
            &conn, "v1", "legacy_car", Some("1.0"), None, &now,
            &carv.to_string_lossy(), None, "sig", &[], &[], &[], &[], None,
        )
        .unwrap();
        overlay::set_active_version(&conn, "legacy_car", "v1").unwrap();
        let cfg = AppConfig { ac_install_path: Some(ac.clone()), library_path: Some(library), ..Default::default() };

        // Simule un mod activé sous l'ancien mécanisme, sans passer par `activate`.
        let link = ac.join("content").join("cars").join("legacy_car");
        create_junction(&link, &carv).unwrap();
        assert!(is_junction(&link));

        assert!(is_mod_active(&cfg, ModKind::Car, "legacy_car"), "symlink hérité toujours reconnu actif");

        // Réactiver (ex. changement de version, ou juste re-cliquer Activer)
        // migre silencieusement vers les hardlinks.
        activate(&conn, &cfg, "legacy_car", None).unwrap();
        assert!(!is_junction(&link), "migré : plus un symlink");
        assert!(deploy::is_deployed(&link), "migré : déploiement hardlinks");
        assert!(link.join("data.txt").is_file());
    }
}
