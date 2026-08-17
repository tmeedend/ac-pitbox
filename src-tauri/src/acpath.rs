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

/// Zones d'AC qu'un **outil externe tient à jour tout seul** : le téléchargeur
/// de configs de Content Manager, alimenté par le dépôt `acc-extension-config`
/// (un serveur le tire toutes les 5 minutes et le convertit en un format que CM
/// récupère automatiquement).
///
/// Un mod *peut* y déposer un fichier — certaines archives de circuit livrent
/// leur config CSP dans `extension/config/tracks/loaded/` — mais il n'y est pas
/// chez lui, et ce n'est pas la bonne pratique : `loaded/` est le **dernier**
/// des trois emplacements que CSP consulte (après `content/tracks/<id>/
/// extension/ext_config.ini`, qui est la place prévue pour un auteur, puis
/// `extension/config/tracks/<id>.ini`), et c'est précisément celui que la
/// synchro écrase. On pose quand même — l'app n'arbitre pas les choix de
/// l'auteur — mais on le **dit** sur la fiche : un fichier que CM remplacera
/// sans prévenir ne doit pas avoir l'air d'un ajout stable.
const EXTERNALLY_MANAGED: &[&[&str]] = &[
    &["extension", "config", "tracks", "loaded"],
    &["extension", "config", "cars", "loaded"],
    // Les vao-patches se téléchargent aussi par lots depuis CM (constaté sur
    // une install réelle : une vingtaine de `ks_*.vao-patch` à la seconde près).
    &["extension", "vao-patches"],
    &["extension", "vao-patches-cars"],
];

/// Vrai si `rel` tombe dans une zone qu'un outil externe synchronise
/// ([`EXTERNALLY_MANAGED`]).
pub fn is_externally_managed(rel: &Path) -> bool {
    let segs: Vec<String> = rel
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .map(|s| s.to_ascii_lowercase())
        .collect();
    EXTERNALLY_MANAGED
        .iter()
        // `>` et non `>=` : le préfixe seul est le dossier, pas un fichier
        // dedans — c'est le contenu qui est géré, pas le dossier lui-même.
        .any(|p| segs.len() > p.len() && segs.iter().zip(p.iter()).all(|(a, b)| a == b))
}

/// Racine réelle d'une livraison, en traversant l'**emballage** de l'auteur.
///
/// Un packageur enveloppe très souvent tout son contenu dans un dossier unique
/// portant le nom de l'archive (`NFS_TOURNAMENT_CLASS_A_2026-02-15/content/…`).
/// `modscan` sait déjà descendre cet emballage pour trouver les mods — d'où des
/// voitures correctement importées. Le balayage des restes (§7.3), lui,
/// calculait ses chemins depuis la racine d'extraction et **gardait le segment
/// d'emballage** : `content/texture` devenait `NFS_…/content/texture`, que
/// [`is_ac_relative`] refuse à juste titre. Le reste était donc rangé en
/// bibliothèque puis jamais posé dans le jeu.
///
/// Bug réel : trois packs (NFS Tournament A et B, A3DR Porsche 993) dont les
/// `content/texture` et `content/fonts` n'ont jamais atteint AC, alors que leurs
/// voitures, elles, étaient bien installées.
///
/// **Deux garde-fous**, et ils comptent autant que la règle :
///
/// - on ne descend que dans un dossier **seul à son niveau**. Un dossier parmi
///   plusieurs n'est pas un emballage mais un choix de l'auteur (`Optional - No
///   ambient sounds/` à côté de son alternative) : le traverser installerait
///   d'office une variante que l'utilisateur n'a pas choisie ;
/// - on ne traverse **jamais un dossier de jeu**. Un `content/` seul à la racine
///   *est* la racine ; le traverser ferait de `cars/` un chemin de premier
///   niveau, et le contenu partirait à `<AC>\cars\`.
pub fn effective_root(dir: &Path) -> PathBuf {
    let mut cur = dir.to_path_buf();
    loop {
        let Ok(mut entries) = std::fs::read_dir(&cur).map(|e| e.flatten()) else {
            return cur;
        };
        let (Some(only), None) = (entries.next(), entries.next()) else {
            return cur;
        };
        if !only.file_type().is_ok_and(|t| t.is_dir()) || leads_into_game(Path::new(&only.file_name())) {
            return cur;
        }
        cur = only.path();
    }
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
    fn a_lone_wrapper_folder_is_traversed_but_a_game_folder_never_is() {
        // Bug réel (NFS Tournament, A3DR Porsche) : l'archive emballe tout dans
        // un dossier à son nom. Les voitures étaient trouvées — `modscan`
        // descend — mais les restes gardaient le segment d'emballage et
        // n'arrivaient jamais dans le jeu.
        let base = crate::testutil::temp_dir("acpath-root");

        // Emballage à traverser : un seul dossier, qui n'est pas un dossier AC.
        let wrapped = base.join("wrapped");
        write(
            &wrapped
                .join("NFS_TOURNAMENT_A")
                .join("content")
                .join("texture")
                .join("t.dds"),
        );
        assert_eq!(
            effective_root(&wrapped),
            wrapped.join("NFS_TOURNAMENT_A"),
            "l'emballage est traversé"
        );

        // Emballages imbriqués : on descend tant que le dossier est seul.
        let twice = base.join("twice");
        write(
            &twice
                .join("Pack")
                .join("Pack v2")
                .join("content")
                .join("fonts")
                .join("f.png"),
        );
        assert_eq!(effective_root(&twice), twice.join("Pack").join("Pack v2"));

        // `content/` seul EST la racine : le traverser enverrait le contenu
        // dans `<AC>\cars\`.
        let game = base.join("game");
        write(&game.join("content").join("cars").join("x").join("y.ini"));
        assert_eq!(effective_root(&game), game, "un dossier de jeu n'est jamais traversé");

        // Plusieurs entrées : ce n'est pas un emballage, c'est un choix de
        // l'auteur. En traverser un installerait une variante non choisie.
        let choice = base.join("choice");
        write(
            &choice
                .join("Optional - No sounds")
                .join("content")
                .join("tracks")
                .join("spa")
                .join("a.kn5"),
        );
        write(
            &choice
                .join("Standard")
                .join("content")
                .join("tracks")
                .join("spa")
                .join("a.kn5"),
        );
        assert_eq!(
            effective_root(&choice),
            choice,
            "on ne choisit pas à la place de l'auteur"
        );
    }

    #[test]
    fn cm_managed_folders_are_recognised() {
        // §4.5.5 : ces chemins sont posés comme les autres, mais la fiche doit
        // dire qu'un outil externe les réécrira. `loaded/` est la cible de
        // synchro du dépôt CSP, pas un emplacement stable.
        for managed in [
            "extension/config/tracks/loaded/bahrain_international_circuit.ini",
            "extension/config/cars/loaded/ks_mazda_mx5.ini",
            "extension/vao-patches/spa.vao-patch",
            "Extension/Config/Tracks/Loaded/Spa.ini",
        ] {
            assert!(
                is_externally_managed(Path::new(managed)),
                "{managed} est en zone auto-gérée"
            );
        }

        for own in [
            // Voisin immédiat, mais hors de `loaded/` : c'est un emplacement
            // local que la synchro ne touche pas.
            "extension/config/tracks/spa.ini",
            "extension/config/cars/rss/rss_gtm_lanzo_v8/car.ini",
            "content/tracks/spa/extension/ext_config.ini",
            "system/shaders/gl/ks_base.fx",
        ] {
            assert!(!is_externally_managed(Path::new(own)), "{own} appartient au mod");
        }

        assert!(
            !is_externally_managed(Path::new("extension/vao-patches")),
            "le dossier lui-même n'est pas un fichier géré"
        );
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
