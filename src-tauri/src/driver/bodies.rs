//! The mannequins installed in `content/driver`, the era each one belongs to,
//! and how to tell a mannequin from any other `.kn5` (PILOTE§9).

use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;

/// Époque d'un mannequin, lue sur le nom de la texture de casque qu'il
/// échantillonne, et clé i18n du libellé que l'écran en affiche.
///
/// **Table maintenue en code, indexée sur le préfixe** (PILOTE§6.3) : c'est une
/// convention de nommage Kunos, pas une donnée du format, et un mannequin de
/// mod qui nomme ses images autrement tombe simplement en `None` — sans
/// erreur, et l'écran le dit en toutes lettres plutôt que de proposer un choix
/// sans effet (PILOTE§11.1). Mesuré sur les 52 mannequins de l'installation de
/// référence : les quatre préfixes ci-dessous couvrent tous ceux dont un
/// casque du jeu peut changer l'apparence, les autres (`RSS_Helmet`,
/// `HELMET_HR2`, `helmet_2019`, `2016_Suit_DIFFc` de `yk2_kana`) portent leur
/// casque avec eux.
pub(super) const ERAS: [(&str, &str); 4] = [
    ("helmet_2012", "modern"),
    ("helmet_1985", "1980s"),
    ("helmet_1975", "1970s"),
    ("helmet_1969", "1960s"),
];

pub(super) fn era_of(diffuse: &BTreeSet<String>) -> Option<&'static str> {
    ERAS.iter()
        .find(|(prefix, _)| diffuse.iter().any(|name| name.starts_with(prefix)))
        .map(|(_, era)| *era)
}

/// Un mannequin installé, tel qu'il s'offre au choix (PILOTE§9.1).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyOption {
    /// Nom de fichier sans extension, tel que `driver3d.ini` l'écrirait :
    /// `driver_60`, `gt-m_pro`. Ne se traduit pas.
    pub id: String,
    /// Clé de la table [`ERAS`], ou `None` — un mannequin sur lequel aucun
    /// casque du jeu ne se pose.
    pub era: Option<&'static str>,
}

/// Ce que l'écran Pilote reçoit : les mannequins utilisables, **et le nombre
/// de ceux qui ont été écartés**.
///
/// Le décompte n'est pas décoratif. Un `.kn5` posé dans `content/driver` mais
/// dépourvu de squelette n'apparaît nulle part — ni ici, ni dans l'inventaire
/// des compléments, qui cesse de lister les mannequins déployés puisque c'est
/// ici qu'ils vivent (refonte PILOTE§5). Sans ce chiffre, un mannequin importé
/// pourrait disparaître des deux écrans sans un mot.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BodyList {
    pub bodies: Vec<BodyOption>,
    /// `.kn5` présents mais inutilisables (illisibles ou sans squelette).
    pub discarded: usize,
}

/// Les mannequins qu'on peut proposer, triés par nom.
///
/// **Un corps qu'on ne peut pas prendre n'a pas à être montré** (PILOTE§9.3) : les
/// illisibles et ceux sans squelette sont écartés en silence. Le critère est
/// mesuré, pas supposé — un mannequin sans *skinned mesh* n'a pas de rig, donc
/// ni le `driver_base_pos.knh` de la voiture ni son `steer.ksanim` n'ont prise
/// sur lui, et il s'afficherait dans sa pose de repos au travers de
/// l'habitacle. Sur l'installation de référence ça écarte exactement sept
/// fichiers sur 52 : les six variantes de LOD B, qui sont des copies rigides
/// des mannequins qu'elles doublent, et une blague (`cheems.kn5`, un chien).
///
/// Le coût est celui d'un parcours complet du dossier — **0,3 s pour les 52
/// mannequins de l'installation de référence**, soit 800 Mo lus et parsés, ce
/// qui surprend jusqu'à ce qu'on se rappelle que le gros d'un KN5 est en
/// textures et qu'on ne les décode pas ici. Pas de cache disque, donc : ce
/// serait un fichier de plus à invalider pour économiser un tiers de seconde
/// sur un écran qu'on ouvre rarement.
pub fn bodies(ac_root: &Path) -> BodyList {
    let dir = ac_root.join("content").join("driver");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        log::warn!("driver: {} illisible", dir.display());
        return BodyList {
            bodies: Vec::new(),
            discarded: 0,
        };
    };
    let mut seen = 0usize;
    let mut out: Vec<BodyOption> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e.eq_ignore_ascii_case("kn5")))
        .inspect(|_| seen += 1)
        .filter_map(|path| {
            let id = path.file_stem()?.to_string_lossy().into_owned();
            let bytes = std::fs::read(&path)
                .inspect_err(|e| log::warn!("driver: {} illisible — {e}", path.display()))
                .ok()?;
            let model = kn5::parse(&bytes)
                .inspect_err(|e| log::debug!("driver: {id} écarté, illisible — {e}"))
                .ok()?;
            if !has_skeleton(&model) {
                log::debug!("driver: {id} écarté, aucun squelette");
                return None;
            }
            let diffuse: BTreeSet<String> = model
                .materials
                .iter()
                .filter_map(|m| m.texture_for("txDiffuse"))
                .map(str::to_lowercase)
                .collect();
            Some(BodyOption {
                era: era_of(&diffuse),
                id,
            })
        })
        .collect();
    out.sort_by_key(|body| body.id.to_lowercase());
    let discarded = seen.saturating_sub(out.len());
    BodyList { bodies: out, discarded }
}

/// Préfixe dont AC affuble chaque nœud d'un mannequin. C'est par lui qu'il
/// retrouve le rig, et aucun autre type de modèle ne le porte.
const DRIVER_NODE_PREFIX: &str = "DRIVER:";

/// Ce `.kn5` est-il un mannequin de pilote ?
///
/// **La question se pose vraiment** : un mod de pilote se distribue souvent
/// comme un `.kn5` nu, sans le moindre dossier pour dire où il va — quatre des
/// huit exemples réels examinés (`senna.kn5`, `tom.kn5`, `jp_police_man.kn5`,
/// `FemaleAsianDriver.kn5`). Sans réponse, ces fichiers atterrissent n'importe
/// où plutôt que dans `content/driver/`.
///
/// **Deux critères, tous deux nécessaires, mesurés et sans exception** :
///
/// | | meshes skinnés | nœuds `DRIVER:` | roues |
/// | --- | --- | --- | --- |
/// | six mods de pilote trouvés en ligne | 2 à 10 | 57 à 76 | 0 |
/// | `driver.kn5` de Kunos | 2 | 72 | 0 |
/// | **une voiture** (`thefuckingsabre.kn5`) | 0 | 0 | 2 |
/// | **un collider** | 0 | 0 | 0 |
///
/// Le squelette seul ne suffirait pas — rien n'interdit à une voiture d'animer
/// une pièce — et le préfixe seul non plus, un mod pouvant nommer ainsi un
/// accessoire. Les deux ensemble n'ont donné aucun faux positif.
///
/// **Coût** : le parsing complet du fichier, une quinzaine de millisecondes
/// pour un mannequin de quinze mégaoctets. À n'appeler que sur un `.kn5` dont
/// on ne connaît pas déjà la destination — sur les autres, le chemin répond
/// déjà, et gratuitement.
pub fn is_driver_model(path: &Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else { return false };
    let Ok(model) = kn5::parse(&bytes) else { return false };
    if !has_skeleton(&model) {
        return false;
    }
    let mut prefixed = false;
    model.visit_nodes(&mut |node| {
        if node.name.len() >= DRIVER_NODE_PREFIX.len()
            && node.name[..DRIVER_NODE_PREFIX.len()].eq_ignore_ascii_case(DRIVER_NODE_PREFIX)
        {
            prefixed = true;
        }
    });
    prefixed
}

/// Un rig que la pose de la voiture et son animation de volant peuvent bouger.
fn has_skeleton(model: &kn5::Kn5Model) -> bool {
    let mut found = false;
    model.visit_nodes(&mut |node| {
        if matches!(node.kind, kn5::Kn5NodeKind::SkinnedMesh(_)) {
            found = true;
        }
    });
    found
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// Règle PILOTE§6.2 : l'époque se lit sur la texture de casque que le mannequin
    /// échantillonne, et rien d'autre — un mannequin de mod qui nomme ses
    /// images à lui n'a pas d'époque, il n'a pas non plus de casque à proposer.
    #[test]
    fn an_era_is_read_on_the_helmet_texture_the_mannequin_asks_for() {
        let era = |names: &[&str]| era_of(&names.iter().map(|n| n.to_string()).collect());
        assert_eq!(era(&["2016_suit_diff.dds", "helmet_2012.dds"]), Some("modern"));
        assert_eq!(era(&["helmet_1985.dds"]), Some("1980s"));
        assert_eq!(era(&["helmet_1975.dds"]), Some("1970s"));
        assert_eq!(era(&["helmet_1969.dds"]), Some("1960s"));
        assert_eq!(era(&["rss_helmet.dds", "2016_suit_diff.dds"]), None, "un casque de mod");
        assert_eq!(era(&[]), None, "et un mannequin sans texture du tout");
    }

    /// Ce que le détecteur dit des mods de pilote réels, et des contre-exemples.
    ///
    /// Le corpus est un dossier de mods téléchargés tels quels : `.kn5` nus,
    /// dossiers maison, archives imbriquées. Attendu : **tous** les mannequins
    /// reconnus, **aucune** voiture ni collider.
    ///
    /// ```text
    /// PITBOX_DRIVER_MODS="C:\...\exemples-drivers" cargo test --lib driver -- --ignored --nocapture what_the_detector
    /// ```
    #[test]
    #[ignore = "needs a folder of real driver mods; measurement, not a check"]
    fn what_the_detector_says_about_real_mods() {
        let Ok(root) = std::env::var("PITBOX_DRIVER_MODS") else {
            eprintln!("PITBOX_DRIVER_MODS unset, skipping");
            return;
        };
        let mut seen = 0;
        let mut drivers = 0;
        for entry in walkdir::WalkDir::new(root).into_iter().flatten() {
            let path = entry.path();
            if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("kn5")) {
                continue;
            }
            let verdict = is_driver_model(path);
            seen += 1;
            drivers += usize::from(verdict);
            eprintln!(
                "{:>8}  {}",
                if verdict { "PILOTE" } else { "—" },
                path.file_name().unwrap_or_default().to_string_lossy()
            );
        }
        eprintln!("\n=== {drivers} mannequins sur {seen} fichiers .kn5 ===");
    }

    /// Les corps que l'écran proposerait sur l'installation de référence, et
    /// ceux qu'il écarte — la mesure qui a fixé le critère de [`bodies`].
    ///
    /// ```text
    /// PITBOX_AC_ROOT="D:\...\assettocorsa" cargo test --lib driver -- --ignored --nocapture which_bodies
    /// ```
    #[test]
    #[ignore = "needs a real Assetto Corsa install; measurement, not a check"]
    fn which_bodies_can_be_offered() {
        let Ok(ac_root) = std::env::var("PITBOX_AC_ROOT") else {
            eprintln!("PITBOX_AC_ROOT unset, skipping");
            return;
        };
        let root = PathBuf::from(ac_root);
        let installed = std::fs::read_dir(root.join("content").join("driver"))
            .expect("read content/driver")
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("kn5")))
            .count();
        let offered = bodies(&root);
        let mut by_era: std::collections::BTreeMap<&str, Vec<&str>> = Default::default();
        for body in &offered.bodies {
            by_era.entry(body.era.unwrap_or("—")).or_default().push(&body.id);
        }
        for (era, ids) in &by_era {
            eprintln!("{era:>8} : {:3} — {}", ids.len(), ids.join(", "));
        }
        eprintln!(
            "
=== {} corps proposés sur {installed} installés ===",
            offered.bodies.len()
        );
    }
}
