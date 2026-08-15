//! Remplacement d'un fichier du jeu, avec sauvegarde et restauration (§4.9).
//!
//! Certains mods ne se contentent pas d'**ajouter** des fichiers : ils en
//! **remplacent** — shader `system/shaders/…` modifié, config CSP qui écrase
//! la stock, HUD façon CMRT qui remplace des images de `content/gui/`. Jusqu'ici
//! l'app refusait, et en silence : la pose sautait le fichier sans laisser de
//! trace, le mod s'installait à moitié et rien n'en informait l'utilisateur.
//!
//! La règle d'or n°5 (« aucun fichier du jeu altéré durablement ») n'interdit
//! pas de toucher un fichier : elle exige qu'il soit **sauvegardé et restauré**,
//! et qu'un filet de sécurité rattrape les fermetures anormales. C'est ce que
//! fait ce module, en généralisant au fichier isolé la discipline déjà éprouvée
//! sur les dossiers par `compose::recompose_stock` (§4.4) :
//!
//! 1. sauvegarde **avant** toute écriture, jamais l'inverse ;
//! 2. vérification que la sauvegarde est lisible avant de toucher au jeu ;
//! 3. la **première** sauvegarde fait foi — un second mod qui remplace le même
//!    fichier ne sauvegarde pas la version du premier par-dessus l'originale ;
//! 4. restauration dès que plus aucun mod ne réclame le chemin ;
//! 5. au démarrage, restauration de toute sauvegarde que plus rien ne réclame
//!    (`restore_orphans`) — le filet pour une app tuée en cours de pose.
//!
//! L'original vit dans `<lib>/game_backup/<chemin relatif à AC>`, et la base
//! (`game_backups`) fait le lien. Perdre la base ne perd donc pas l'original :
//! le chemin de la sauvegarde dit à lui seul où le fichier doit revenir.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::config::AppConfig;

/// Emplacement de la sauvegarde d'un fichier d'AC : `<lib>/game_backup/<rel>`.
fn backup_path(library: &Path, ac: &Path, ac_path: &Path) -> Option<PathBuf> {
    let rel = ac_path.strip_prefix(ac).ok()?;
    Some(library.join("game_backup").join(rel))
}

/// Met de côté l'original de `ac_path` avant qu'un mod ne le remplace.
/// Renvoie `true` si le fichier peut être remplacé — soit qu'on vienne de le
/// sauvegarder, soit qu'une sauvegarde existe déjà (cas d'un second mod qui
/// vise le même chemin : l'originale est déjà à l'abri, on n'y touche plus).
/// `false` = on n'a pas pu sécuriser l'original, donc on ne remplace pas.
pub fn protect(conn: &Connection, cfg: &AppConfig, ac_path: &Path) -> bool {
    let (Some(library), Some(ac)) = (cfg.library_path.as_ref(), cfg.ac_install_path.as_ref()) else {
        return false;
    };
    let Some(dest) = backup_path(library, ac, ac_path) else {
        log::warn!("game backup {}: outside AC, skipped", ac_path.display());
        return false;
    };
    let key = ac_path.to_string_lossy().into_owned();

    // Sauvegarde déjà connue : c'est elle l'originale, on ne l'écrase jamais.
    if dest.is_file() {
        if let Err(e) = crate::overlay::add_game_backup(conn, &key, &dest.to_string_lossy()) {
            log::warn!("add_game_backup {}: {e}", ac_path.display());
        }
        return true;
    }

    if let Some(parent) = dest.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            log::warn!("game backup {}: {e}", ac_path.display());
            return false;
        }
    }
    // Copie, pas déplacement : à cet instant le jeu doit rester utilisable même
    // si l'étape suivante échoue.
    if let Err(e) = std::fs::copy(ac_path, &dest) {
        log::warn!("game backup {} -> {}: {e}", ac_path.display(), dest.display());
        return false;
    }
    // Sauvegarde vérifiée avant de rendre la main : même précaution que
    // `compose::recompose_stock`, qui refuse d'agir sur une sauvegarde vide.
    let ok = std::fs::metadata(&dest)
        .ok()
        .zip(std::fs::metadata(ac_path).ok())
        .is_some_and(|(b, o)| b.len() == o.len());
    if !ok {
        log::warn!(
            "game backup {}: unreadable or truncated, not replacing",
            ac_path.display()
        );
        let _ = std::fs::remove_file(&dest);
        return false;
    }
    if let Err(e) = crate::overlay::add_game_backup(conn, &key, &dest.to_string_lossy()) {
        log::warn!("add_game_backup {}: {e}", ac_path.display());
        let _ = std::fs::remove_file(&dest);
        return false;
    }
    true
}

/// Vrai si ce chemin d'AC est un fichier du jeu que nous avons remplacé.
pub fn is_replaced(conn: &Connection, ac_path: &Path) -> bool {
    crate::overlay::game_backup_of(conn, &ac_path.to_string_lossy())
        .unwrap_or(None)
        .is_some()
}

/// Remet l'original en place et oublie la sauvegarde. Appelé quand plus aucun
/// mod ne réclame le chemin. Sans sauvegarde connue : rien à faire, et surtout
/// pas d'invention — un fichier qu'on n'a pas remplacé ne nous appartient pas.
pub fn restore(conn: &Connection, ac_path: &Path) {
    let key = ac_path.to_string_lossy().into_owned();
    let Some(backup) = crate::overlay::game_backup_of(conn, &key).unwrap_or(None) else {
        return;
    };
    let backup = PathBuf::from(backup);
    if !backup.is_file() {
        // Sauvegarde disparue (bibliothèque nettoyée à la main) : on ne peut
        // plus restaurer, mais on cesse de prétendre le contraire.
        log::warn!(
            "game restore {}: backup missing at {}",
            ac_path.display(),
            backup.display()
        );
        let _ = crate::overlay::remove_game_backup(conn, &key);
        return;
    }
    if let Some(parent) = ac_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if ac_path.exists() {
        if let Err(e) = std::fs::remove_file(ac_path) {
            log::warn!("game restore {}: {e}", ac_path.display());
            return;
        }
    }
    match std::fs::copy(&backup, ac_path) {
        Ok(_) => {
            let _ = std::fs::remove_file(&backup);
            if let Err(e) = crate::overlay::remove_game_backup(conn, &key) {
                log::warn!("remove_game_backup {}: {e}", ac_path.display());
            }
        }
        // La sauvegarde reste en base : le filet de démarrage réessaiera.
        Err(e) => log::warn!("game restore {} <- {}: {e}", ac_path.display(), backup.display()),
    }
}

/// Filet de sécurité au démarrage (règle d'or n°5) : restaure tout fichier du
/// jeu sauvegardé que plus aucun mod ne réclame. Rattrape une app tuée entre la
/// sauvegarde et la pose, ou entre le retrait et la restauration — les deux
/// fenêtres où le jeu resterait avec un fichier modifié sans que rien ne le
/// dise.
pub fn restore_orphans(conn: &Connection) {
    let rows = match crate::overlay::list_game_backups(conn) {
        Ok(r) => r,
        Err(e) => {
            log::warn!("list_game_backups: {e}");
            return;
        }
    };
    for (ac_path, _) in rows {
        let claimed = crate::overlay::extra_claimants(conn, &ac_path)
            .map(|c| !c.is_empty())
            .unwrap_or(true); // en cas de doute, on ne restaure pas
        if !claimed {
            log::warn!("game restore (startup): {ac_path} no longer claimed");
            restore(conn, Path::new(&ac_path));
        }
    }
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
    fn the_first_backup_is_the_one_that_is_kept() {
        // Règle d'or n°5. Deux mods remplacent le même fichier du jeu : le
        // second ne doit pas sauvegarder la version du premier par-dessus
        // l'originale, sinon la restauration rendrait un fichier de mod au lieu
        // du fichier Kunos — et l'original serait perdu pour de bon.
        let base = crate::testutil::temp_dir("gb-first");
        let cfg = cfg_for(&base);
        let ac = cfg.ac_install_path.clone().unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();

        let target = ac.join("system").join("shaders").join("stock.fxo");
        write(&target, b"KUNOS");

        assert!(protect(&conn, &cfg, &target), "original mis à l'abri");
        std::fs::write(&target, b"MOD-A").unwrap();

        assert!(protect(&conn, &cfg, &target), "second mod : déjà protégé");
        std::fs::write(&target, b"MOD-B").unwrap();

        restore(&conn, &target);
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"KUNOS",
            "c'est l'original Kunos qui revient, pas la version du premier mod"
        );
        assert!(
            !is_replaced(&conn, &target),
            "la sauvegarde est oubliée après restauration"
        );
    }

    #[test]
    fn startup_restores_a_backup_no_mod_claims_anymore() {
        // Filet de sécurité (règle d'or n°5) : app tuée entre la sauvegarde et
        // la pose, ou entre le retrait et la restauration. Au démarrage, un
        // fichier du jeu modifié que plus personne ne réclame doit redevenir
        // celui du jeu.
        let base = crate::testutil::temp_dir("gb-startup");
        let cfg = cfg_for(&base);
        let ac = cfg.ac_install_path.clone().unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();

        let target = ac.join("content").join("gui").join("logo.png");
        write(&target, b"KUNOS-LOGO");
        assert!(protect(&conn, &cfg, &target));
        std::fs::write(&target, b"MOD-LOGO").unwrap();

        // Aucune réclamation en base : le mod a disparu sans repasser par ici.
        restore_orphans(&conn);
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"KUNOS-LOGO",
            "le jeu retrouve son fichier au démarrage suivant"
        );
    }

    #[test]
    fn a_file_we_never_replaced_is_never_invented() {
        // `restore` sans sauvegarde connue ne doit rien faire : un fichier
        // qu'on n'a pas remplacé ne nous appartient pas.
        let base = crate::testutil::temp_dir("gb-noop");
        let cfg = cfg_for(&base);
        let ac = cfg.ac_install_path.clone().unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();

        let target = ac.join("system").join("foreign.fxo");
        write(&target, b"NOT-OURS");
        restore(&conn, &target);
        assert_eq!(std::fs::read(&target).unwrap(), b"NOT-OURS", "intact");
        assert!(!is_replaced(&conn, &target));
    }
}
