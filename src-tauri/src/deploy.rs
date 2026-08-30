//! Déploiement dans `content/` par **hardlinks par fichier** (§2), comme
//! Vortex : une vraie arborescence de dossiers sous `content/<type>s/<id>`,
//! chaque fichier est un hardlink NTFS vers son original en bibliothèque —
//! zéro duplication (même enregistrement de fichier), zéro reparse point,
//! **pas de droits admin** (contrairement à `mklink /D`, qui exige le mode
//! développeur ou une élévation). Repli en **copie physique** fichier par
//! fichier si bibliothèque et jeu sont sur des disques différents (le
//! hardlink échoue alors nativement — `CreateHardLinkW` refuse de traverser
//! les volumes — on rattrape avec une copie, exactement comme le déplacement
//! adaptatif de l'import, §4.2).
//!
//! **Garde-fou** : contrairement à une junction/symlink (détectable par son
//! type de reparse point via `symlink_metadata`), une arborescence de
//! hardlinks est un **vrai dossier**, indiscernable d'un dossier Kunos ou de
//! tout autre contenu par les seuls attributs du système de fichiers. On pose
//! donc un marqueur (`MARKER_FILE`) à la racine du dossier déployé une fois le
//! déploiement terminé : sa présence est la preuve que *nous* avons créé ce
//! dossier — jamais de suppression dans `content/` sans lui (ni sans être une
//! junction/symlink, pour les mods encore sous l'ancien mécanisme).
//!
//! Les mods déjà actifs par symlink (`mklink /D`, ancien mécanisme) restent
//! tels quels indéfiniment — inoffensif — et ne sont migrés vers les
//! hardlinks qu'à leur prochaine (ré)activation (§7, `activation.rs`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::layers::HostKind;

/// Fichier caché posé à la racine de chaque dossier déployé par hardlinks —
/// le garde-fou de suppression en dépend, ne jamais le supprimer à la main.
pub const MARKER_FILE: &str = ".pitbox-deployed.json";

#[derive(Debug, Serialize, Deserialize)]
struct Marker {
    mod_id: String,
    kind: String,
    deployed_at: String,
}

/// Vrai si `path` est un dossier déployé par nous (marqueur présent). Ne suit
/// jamais un reparse point (un symlink n'a pas ce marqueur, même s'il pointe
/// vers un dossier qui en a un — non pertinent ici, on teste le chemin direct).
pub fn is_deployed(path: &Path) -> bool {
    path.join(MARKER_FILE).is_file()
}

/// Pose un hardlink `dst` → `src` ; repli en copie physique si le hardlink
/// échoue (disques différents, système de fichiers qui ne le supporte pas…).
pub(crate) fn link_or_copy(src: &Path, dst: &Path) -> Result<(), String> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if std::fs::hard_link(src, dst).is_ok() {
        return Ok(());
    }
    std::fs::copy(src, dst)
        .map(|_| ())
        .map_err(|e| format!("liaison/copie de {} : {e}", src.display()))
}

/// Reproduit l'arborescence de `source` dans `dest` (dossiers réels + hardlink
/// par fichier). `dest` doit être vide/inexistant — le garde-fou (junction,
/// déploiement existant, vrai dossier étranger) est du ressort de l'appelant.
fn link_tree(source: &Path, dest: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for entry in WalkDir::new(source).into_iter().filter_map(|e| e.ok()) {
        let rel = entry.path().strip_prefix(source).unwrap();
        if rel.as_os_str().is_empty() {
            continue; // la racine elle-même
        }
        let target = dest.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        } else if entry.file_type().is_file() {
            link_or_copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Superpose `source` sur `dest` déjà peuplé (§4.3) : hardlink chaque fichier,
/// en remplaçant l'entrée existante le cas échéant (chemin déjà fourni par la
/// base ou une couche de priorité inférieure). Toujours un hardlink vers le
/// fichier réellement gagnant — jamais de copie de fusion, la bibliothèque a
/// déjà un fichier physique distinct par entité (mod/couche).
fn overlay_tree(source: &Path, dest: &Path) -> Result<(), String> {
    for entry in WalkDir::new(source).into_iter().filter_map(|e| e.ok()) {
        let rel = entry.path().strip_prefix(source).unwrap();
        if rel.as_os_str().is_empty() {
            continue;
        }
        let target = dest.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        } else if entry.file_type().is_file() {
            if target.exists() {
                std::fs::remove_file(&target).map_err(|e| e.to_string())?;
            }
            link_or_copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn write_marker(dest: &Path, mod_id: &str, kind: HostKind) -> Result<(), String> {
    let marker = Marker {
        mod_id: mod_id.to_string(),
        kind: kind.as_str().to_string(),
        deployed_at: chrono::Local::now().to_rfc3339(),
    };
    let json = serde_json::to_string_pretty(&marker).map_err(|e| e.to_string())?;
    std::fs::write(dest.join(MARKER_FILE), json).map_err(|e| e.to_string())
}

/// Déploie `source` (la version active en bibliothèque, ou le dossier de base
/// Kunos sauvegardé) directement dans `dest` (`content/<type>s/<id>`) par
/// hardlinks. `dest` ne doit pas exister — l'appelant (`activation.rs`,
/// `compose.rs`) a déjà retiré tout déploiement précédent (garde-fou compris).
pub fn deploy_tree(source: &Path, dest: &Path, mod_id: &str, kind: HostKind) -> Result<(), String> {
    link_tree(source, dest)?;
    write_marker(dest, mod_id, kind)
}

/// Déploie `base` puis superpose `layers` (dans l'ordre, la dernière gagne)
/// directement dans `dest`, en hardlinks (§4.3) — composition base + couches
/// sans dossier de composition intermédiaire : `dest` (`content/<type>s/<id>`)
/// EST le résultat composé, pas une projection d'une copie ailleurs.
pub fn compose_tree(base: &Path, layers: &[PathBuf], dest: &Path, mod_id: &str, kind: HostKind) -> Result<(), String> {
    link_tree(base, dest)?;
    for layer_dir in layers {
        overlay_tree(layer_dir, dest)?;
    }
    write_marker(dest, mod_id, kind)
}

/// Recompose `dest` comme l'union de `layers` (dans l'ordre, la dernière
/// gagne) — pour un usage **hors mods** (ex. `skins/default/` d'un circuit,
/// §8 : Content Manager y copie les fichiers des skins de circuit
/// actifs, vérifié empiriquement — décocher tous les skins vide entièrement
/// ce dossier côté CM, aucun « fond » à préserver). Pas de marqueur
/// `.pitbox-deployed.json` ici : ce n'est pas un déploiement de mod, juste
/// une composition de fichiers dans un sous-dossier. `dest` est entièrement
/// vidé puis reconstruit à chaque appel (pas de mise à jour incrémentale).
pub fn compose_layers_into(layers: &[PathBuf], dest: &Path) -> Result<(), String> {
    if dest.exists() {
        std::fs::remove_dir_all(dest).map_err(|e| format!("nettoyage de {} : {e}", dest.display()))?;
    }
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for layer_dir in layers {
        overlay_tree(layer_dir, dest)?;
    }
    Ok(())
}

/// Retire un déploiement hardlinks. Garde-fou : refuse si le marqueur est
/// absent (jamais un dossier qu'on n'a pas soi-même créé). Les fichiers de la
/// bibliothèque source ne sont jamais affectés — un hardlink est une entrée de
/// répertoire parmi d'autres vers les mêmes données ; en retirer une (ce
/// dossier) ne touche pas les autres (les fichiers de la bibliothèque).
pub fn remove_deployment(path: &Path) -> Result<(), String> {
    if !is_deployed(path) {
        return Err(crate::errors::NOT_DEPLOYED_BY_PITBOX.into());
    }
    std::fs::remove_dir_all(path).map_err(|e| format!("suppression du déploiement : {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> crate::testutil::TempDir {
        crate::testutil::temp_dir("deploy")
    }

    #[test]
    fn deploy_tree_creates_real_dir_with_marker_and_hardlinks() {
        let base = temp();
        let source = base.join("source");
        std::fs::create_dir_all(source.join("ui")).unwrap();
        std::fs::write(source.join("ui").join("ui_track.json"), "{}").unwrap();
        std::fs::write(source.join("model.kn5"), b"FAKE").unwrap();
        let dest = base.join("dest");

        deploy_tree(&source, &dest, "spa", HostKind::Track).unwrap();

        assert!(dest.join("ui").join("ui_track.json").is_file());
        assert!(dest.join("model.kn5").is_file());
        assert!(is_deployed(&dest), "marqueur posé");
        assert!(
            std::fs::symlink_metadata(&dest).unwrap().file_type().is_dir(),
            "vrai dossier, pas un reparse point"
        );

        // Vraie liaison physique (pas une copie) : modifier l'original en
        // bibliothèque doit se répercuter dans le dossier déployé.
        std::fs::write(source.join("model.kn5"), b"CHANGED").unwrap();
        assert_eq!(std::fs::read(dest.join("model.kn5")).unwrap(), b"CHANGED");
    }

    #[test]
    fn remove_deployment_refuses_without_marker() {
        let base = temp();
        let real = base.join("real");
        std::fs::create_dir_all(&real).unwrap();
        std::fs::write(real.join("kunos.txt"), "kunos").unwrap();

        assert!(remove_deployment(&real).is_err(), "refus : pas notre marqueur");
        assert!(real.exists(), "dossier étranger jamais supprimé");
    }

    #[test]
    fn remove_deployment_removes_dir_but_spares_library_original() {
        let base = temp();
        let source = base.join("source");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("f.txt"), "data").unwrap();
        let dest = base.join("dest");
        deploy_tree(&source, &dest, "car1", HostKind::Car).unwrap();

        remove_deployment(&dest).unwrap();

        assert!(!dest.exists(), "déploiement retiré");
        assert!(source.join("f.txt").is_file(), "original bibliothèque intact");
        assert_eq!(std::fs::read_to_string(source.join("f.txt")).unwrap(), "data");
    }

    #[test]
    fn compose_tree_last_layer_wins_without_intermediate_dir() {
        let base = temp();
        let src = base.join("base");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("conf.txt"), "base").unwrap();
        std::fs::write(src.join("only_base.txt"), "b").unwrap();

        let la = base.join("layer_a");
        std::fs::create_dir_all(&la).unwrap();
        std::fs::write(la.join("conf.txt"), "A").unwrap();

        let lb = base.join("layer_b");
        std::fs::create_dir_all(&lb).unwrap();
        std::fs::write(lb.join("conf.txt"), "B").unwrap();
        std::fs::write(lb.join("only_b.txt"), "new").unwrap();

        let dest = base.join("dest");
        compose_tree(&src, &[la.clone(), lb.clone()], &dest, "spa", HostKind::Track).unwrap();

        assert_eq!(
            std::fs::read_to_string(dest.join("conf.txt")).unwrap(),
            "B",
            "dernière couche gagne"
        );
        assert!(
            dest.join("only_base.txt").is_file(),
            "fichier de base non touché conservé"
        );
        assert!(dest.join("only_b.txt").is_file(), "ajout de couche présent");
        assert!(is_deployed(&dest));
    }

    #[test]
    fn link_or_copy_falls_back_to_copy_when_hardlink_fails() {
        // Force l'échec du hardlink (destination déjà existante — même code
        // d'erreur système que le cas disques différents empruntent le même
        // repli, non simulable sans deux volumes réels) puis vérifie le repli
        // en copie physique.
        let base = temp();
        std::fs::create_dir_all(&base).unwrap();
        let src = base.join("src.txt");
        std::fs::write(&src, "source").unwrap();
        let dst = base.join("dst.txt");
        std::fs::write(&dst, "already here").unwrap();

        link_or_copy(&src, &dst).unwrap();
        assert_eq!(
            std::fs::read_to_string(&dst).unwrap(),
            "source",
            "repli copie a bien écrasé/copié le contenu"
        );
    }
}
