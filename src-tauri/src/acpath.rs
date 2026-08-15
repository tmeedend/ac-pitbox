//! Ce qu'est — et surtout ce que n'est pas — un chemin relatif à la racine
//! d'Assetto Corsa (§4.5.3).
//!
//! Le balayage des restes (§7.3) pose une hypothèse simple : le chemin d'un
//! reste **relatif à la racine de l'archive** est son chemin **relatif à la
//! racine d'AC**. Elle est vraie quand l'auteur a livré `content/driver/…` ou
//! `extension/config/…`, et fausse dans deux cas qu'on rencontre en vrai :
//!
//! - l'auteur livre un dossier de jeu **à nu**, sans son préfixe : un
//!   `driver/` posé à côté du dossier de la voiture, alors qu'AC le lit dans
//!   `content/driver/`. C'est ce que [`normalize_leftover`] rattrape ;
//! - l'auteur emballe sa livraison dans un dossier à lui — `Ferrari F2002
//!   V1.4/`, `Track Installation/`, `Optional - No ambient sounds/`. Là il n'y
//!   a rien à rattraper : ce n'est pas un chemin de jeu, et le poser revient à
//!   déverser un dossier d'archive à la racine de l'install. C'est ce que
//!   [`is_ac_relative`] refuse.
//!
//! Le refus ne jette rien (§4.5.3, « l'import ne jette rien ») : le reste est
//! rangé en bibliothèque et listé dans « Ajouts au jeu » comme les autres,
//! simplement jamais posé dans le jeu. L'interprétation reste donc
//! recalculable — si une règle sait un jour quoi en faire, la matière est là.

use std::path::{Path, PathBuf};

use walkdir::WalkDir;

/// Dossiers qu'Assetto Corsa et CSP lisent à la racine de l'install. Tout ce
/// qui commence ailleurs n'est pas un chemin de jeu.
///
/// Liste volontairement **permissive** : son travail est d'écarter les
/// dossiers d'emballage (« Track Installation »), pas d'arbitrer finement ce
/// qu'un mod a le droit de viser. Un faux positif ici ne fait que rendre à
/// l'app le comportement qu'elle avait avant ce garde-fou ; un faux négatif
/// empêcherait un mod légitime de s'installer.
const AC_ROOT_DIRS: &[&str] = &[
    "content",
    "system",
    "extension",
    "apps",
    "cfg",
    "launcher",
    "sdk",
    "server",
    "plugins",
    "mods",
];

/// Premier segment d'un chemin relatif, en minuscules.
fn first_segment(rel: &Path) -> Option<String> {
    rel.components()
        .next()
        .and_then(|c| c.as_os_str().to_str())
        .map(|s| s.to_ascii_lowercase())
}

/// Vrai si `rel` **mène** dans le jeu : son premier segment est un dossier
/// qu'AC lit. Sert à la descente d'un arbre de dossiers, où `content` seul est
/// un début de chemin parfaitement valide alors que ce n'en est pas encore un.
pub fn leads_into_game(rel: &Path) -> bool {
    first_segment(rel).is_some_and(|s| AC_ROOT_DIRS.contains(&s.as_str()))
}

/// Vrai si `rel` peut être le chemin d'un **fichier** relatif à la racine
/// d'AC : il mène dans le jeu, et il y descend d'au moins un cran.
///
/// Un chemin à un seul segment (un fichier isolé à la racine de l'archive) est
/// donc refusé : AC ne lit pas de fichier à la racine de son install, et
/// l'exception qui viendrait à l'esprit — le `dwrite.dll` d'une install CSP —
/// est précisément ce qu'un gestionnaire de mods ne doit pas poser tout seul.
pub fn is_ac_relative(rel: &Path) -> bool {
    rel.components().count() >= 2 && leads_into_game(rel)
}

/// Le dossier contient un `.kn5` à n'importe quelle profondeur.
///
/// AC range les modèles de pilote à plat (`content/driver/driver_501.kn5`)
/// mais aussi en sous-dossiers (`content/driver/driver_skins/…`) : chercher en
/// profondeur couvre les deux formes, et un dossier `driver` qui ne contient
/// aucun modèle n'a de toute façon aucune raison d'être traité comme tel.
fn contains_kn5(dir: &Path) -> bool {
    dir.is_dir()
        && WalkDir::new(dir)
            .into_iter()
            .flatten()
            .any(|e| e.file_type().is_file() && e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("kn5")))
}

/// Rattrape un dossier de jeu livré **à nu** à la racine de l'archive.
///
/// Une seule règle pour l'instant, celle qu'on a vue en vrai : un dossier
/// `driver/` contenant un modèle `.kn5` est le `content/driver/` d'AC. Cas
/// réel : la Ferrari 599 GTO livre `driver/driver_501.kn5` à côté du dossier
/// de la voiture, et sans ce préfixe le pilote atterrissait dans
/// `<AC>\driver\` — un dossier que le jeu ne lit pas, donc un pilote
/// silencieusement absent, et un dossier de plus à la racine de l'install.
///
/// Renvoie `None` quand la règle ne s'applique pas : l'appelant garde son
/// chemin d'origine. Volontairement **une** règle et non une table de tous les
/// sous-dossiers de `content/` : `weather/` et `sfx/` existent aussi sous
/// `extension/`, on ne peut pas trancher sans regarder le contenu, et deviner
/// mal ici pose des fichiers au mauvais endroit dans le jeu.
pub fn normalize_leftover(rel: &Path, src: &Path) -> Option<PathBuf> {
    if first_segment(rel)? != "driver" || !contains_kn5(src) {
        return None;
    }
    Some(Path::new("content").join(rel))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"x").unwrap();
    }

    #[test]
    fn bare_driver_folder_with_a_model_becomes_content_driver() {
        // Cas réel (Ferrari 599 GTO) : `driver/driver_501.kn5` livré à côté du
        // dossier de la voiture. Sans le préfixe, le pilote est posé dans
        // `<AC>\driver\`, que le jeu ne lit pas.
        let base = crate::testutil::temp_dir("acpath-driver");
        let src = base.join("driver");
        write(&src.join("driver_501.kn5"));

        assert_eq!(
            normalize_leftover(Path::new("driver"), &src),
            Some(PathBuf::from("content").join("driver")),
            "préfixé par content/"
        );

        // Modèle en profondeur (skins de pilote) : même traitement.
        let deep = base.join("Driver");
        write(&deep.join("driver_skins").join("red").join("body.kn5"));
        assert_eq!(
            normalize_leftover(Path::new("Driver"), &deep),
            Some(PathBuf::from("content").join("Driver")),
            "casse ignorée, profondeur ignorée"
        );
    }

    #[test]
    fn driver_folder_without_a_model_is_left_alone() {
        // Un dossier nommé `driver` sans le moindre modèle n'a aucune raison
        // d'être pris pour le `content/driver` d'AC — le `.kn5` est ce qui
        // distingue le cas réel d'une homonymie.
        let base = crate::testutil::temp_dir("acpath-nodriver");
        let src = base.join("driver");
        write(&src.join("readme.txt"));
        assert_eq!(normalize_leftover(Path::new("driver"), &src), None);

        // Déjà correctement préfixé : rien à faire non plus.
        let ok = base.join("content").join("driver");
        write(&ok.join("driver_501.kn5"));
        assert_eq!(
            normalize_leftover(Path::new("content").join("driver").as_path(), &ok),
            None,
            "un chemin déjà bon n'est jamais re-préfixé"
        );
    }

    #[test]
    fn wrapper_folders_are_not_ac_paths() {
        // Cas réels relevés en bibliothèque : des dossiers d'emballage de
        // l'auteur, posés tels quels à la racine de l'install parce que leur
        // chemin d'archive était pris pour un chemin de jeu.
        for junk in [
            "Ferrari F2002 V1.4/READ ME.txt",
            "Track Installation/track.7z",
            "Optional - No ambient sounds/content/tracks/spa/spa.kn5",
            "readme.txt",
            "dwrite.dll",
        ] {
            assert!(
                !is_ac_relative(Path::new(junk)),
                "{junk} n'est pas un chemin relatif à la racine d'AC"
            );
        }

        for real in [
            "content/driver/driver_501.kn5",
            "content/gui/flags/checkered.png",
            "extension/config/cars/rss/rss_gtm_lanzo_v8/car.ini",
            "system/shaders/gl/ks_base.fx",
            "apps/python/helper/helper.py",
        ] {
            assert!(is_ac_relative(Path::new(real)), "{real} est un vrai chemin de jeu");
        }
    }

    #[test]
    fn a_lone_game_root_folder_still_leads_into_the_game() {
        // `others::place` descend dossier par dossier : au premier cran il ne
        // voit que `content`, qui n'est pas encore un chemin de fichier
        // valide mais doit évidemment être suivi. Confondre les deux tests
        // bloquerait la pose de tout « autre mod » légitime.
        assert!(leads_into_game(Path::new("content")), "début de chemin suivi");
        assert!(!is_ac_relative(Path::new("content")), "mais pas un chemin de fichier");
        assert!(
            !leads_into_game(Path::new("Track Installation")),
            "emballage écarté dès la racine"
        );
    }
}
