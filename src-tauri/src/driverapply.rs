//! Poser le pilote choisi **dans le jeu**, et savoir le retirer
//! (`docs/csp-driver-research.md`).
//!
//! Deux fichiers, parce qu'Assetto Corsa range le pilote en deux endroits et
//! qu'aucun des deux ne peut porter l'autre :
//!
//! | Quoi | Où | Section |
//! | --- | --- | --- |
//! | le **corps** | `<voiture>/extension/ext_config.ini` | `[DRIVER3D_MODEL]` |
//! | la **tenue** | `<livrée>/skin.ini` | `[<mannequin>]` |
//!
//! Le corps passe par une surcharge CSP : CSP relit une section d'un ini de
//! `data.acd` depuis `ext_config.ini` quand on préfixe le nom du fichier —
//! `driver3d.ini` + `[MODEL]` → `[DRIVER3D_MODEL]`. **`data.acd` n'est pas
//! touché, donc le checksum en ligne tient.** La tenue, elle, n'a pas
//! d'équivalent : cherché dans le binaire de CSP et dans les 195 configs par
//! voiture de l'installation de référence, il n'existe aucune route pour elle.
//!
//! Les deux se lient par le **nom de la section de tenue, qui est celui du
//! mannequin** : remplacer le corps rend la section de la livrée inopérante, et
//! la tenue doit être réécrite sous le nouveau nom.
//!
//! # Trois règles que ce module ne transgresse pas
//!
//! **1. Sauvegarder avant d'écrire, jamais l'inverse** (règle d'or n°5), via
//! [`crate::gamebackup`] — c'est lui qui tient le registre et qui rattrape une
//! app tuée en cours de route. Une seule chose lui manquait ici : il ne sait
//! pas sauvegarder un fichier **absent**, or beaucoup de voitures n'ont pas
//! d'`ext_config.ini` du tout. Ce cas-là ne demande pas de sauvegarde — « il
//! n'y avait rien » se reconstitue parfaitement — mais il demande de savoir,
//! au retour, que le fichier est le nôtre et qu'il doit disparaître. D'où
//! l'en-tête [`MARKER`] : un fichier que nous avons créé le dit, en toutes
//! lettres, à la machine comme à l'humain qui l'ouvre.
//!
//! **2. Fusionner, jamais écraser.** Un `ext_config.ini` de mod fait
//! couramment plusieurs centaines de lignes (735 sur une NSX de l'installation
//! de référence) et un `skin.ini` porte d'autres sections, `[CREW]` en tête.
//! On remplace **une** section et on laisse le reste intact — sans quoi la
//! voiture perdrait ses phares et ses instruments pendant toute la durée de
//! notre pilote, et la sauvegarde ne la lui rendrait qu'au retour.
//!
//! **3. Effacer puis écrire, jamais écrire par-dessus.** `content/cars/<id>`
//! est déployé par hardlink fichier par fichier : le fichier du jeu et celui
//! de la bibliothèque **sont le même inode**. Écrire dedans modifierait la
//! copie de bibliothèque. Retirer le fichier avant de le récrire casse le lien
//! et laisse la bibliothèque intacte. (En mode junction, le dossier du jeu
//! *est* celui de la bibliothèque et rien ne peut les séparer : c'est alors la
//! sauvegarde qui protège, et elle seule.)

use std::path::Path;

use rusqlite::Connection;

use crate::config::AppConfig;

/// En-tête posé en tête de tout fichier **créé** par Pit Box.
///
/// Sa présence est ce qui autorise à supprimer le fichier au retour en
/// arrière : sans elle, on ne fait que retirer notre section et on laisse le
/// reste — un fichier qu'on n'a pas créé ne se supprime pas.
const MARKER: &str =
    "; Fichier créé par Pit Box (pilote choisi). Retiré quand le pilote revient à celui de la voiture.";

/// Section CSP qui remplace le mannequin, dans `ext_config.ini`.
const BODY_SECTION: &str = "DRIVER3D_MODEL";

/// Ce qu'on est allé poser, pour le dire à l'utilisateur.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    /// Fichiers réellement écrits. Vide quand tout était déjà en place — le cas
    /// courant dès la deuxième session avec la même voiture.
    pub written: Vec<String>,
}

/// Met la voiture d'accord avec le pilote choisi — **une seule entrée pour les
/// deux sens**.
///
/// `chosen` à `None`, ou vide, veut dire « cette voiture n'a rien de
/// particulier » : ce qu'on avait posé est alors retiré. C'est ce qui fait
/// qu'enlever un choix dans l'écran Pilote se propage tout seul au lancement
/// suivant, sans commande dédiée ni ménage à part.
///
/// Rien n'est écrit quand le fichier dit déjà ce qu'il faut — le cas courant
/// dès la deuxième session avec la même voiture, et la raison pour laquelle
/// poser au lancement ne martyrise pas le dossier de la voiture.
pub fn sync(
    conn: &Connection,
    cfg: &AppConfig,
    car_dir: &Path,
    car_id: &str,
    skin_dir: Option<&Path>,
    chosen: Option<&crate::driver::OutfitOverride>,
) -> Applied {
    let wanted = chosen.filter(|o| {
        [&o.model, &o.suit, &o.gloves, &o.helmet]
            .into_iter()
            .any(|v| v.as_deref().is_some_and(|v| !v.is_empty()))
    });
    let Some(outfit) = wanted else {
        revert(conn, cfg, car_dir, car_id, skin_dir);
        return Applied::default();
    };
    apply(conn, cfg, car_dir, car_id, skin_dir, outfit)
}

/// Pose le corps et la tenue choisis pour cette voiture.
fn apply(
    conn: &Connection,
    cfg: &AppConfig,
    car_dir: &Path,
    car_id: &str,
    skin_dir: Option<&Path>,
    outfit: &crate::driver::OutfitOverride,
) -> Applied {
    let mut applied = Applied::default();
    let body = outfit.model.as_deref().filter(|m| !m.is_empty());

    // Le mannequin sous lequel la tenue doit être écrite : celui qu'on impose,
    // ou celui que la voiture déclare. C'est le nom de la section, donc s'y
    // tromper revient à écrire une tenue que rien ne lira.
    let declared = crate::driver::outfit_of(car_dir, car_id, None).map(|o| o.model);
    let model = body.or(declared.as_deref());

    if let Some(model) = body {
        let path = car_dir.join("extension").join("ext_config.ini");
        let section = format!("[{BODY_SECTION}]\nNAME={model}");
        if write_section(conn, cfg, &path, BODY_SECTION, &section) {
            applied.written.push(path.to_string_lossy().into_owned());
        }
    }

    if let (Some(skin), Some(model)) = (skin_dir, model) {
        let pieces: Vec<String> = [
            ("SUIT", &outfit.suit),
            ("GLOVES", &outfit.gloves),
            ("HELMET", &outfit.helmet),
        ]
        .into_iter()
        .filter_map(|(key, value)| {
            let value = value.as_deref().filter(|v| !v.is_empty())?;
            // `skin.ini` écrit ses chemins à la mode Windows, précédés
            // d'un séparateur : `SUIT=\plain\red`. On rend la même forme
            // que les auteurs de livrées, pas la nôtre.
            Some(format!("{key}=\\{}", value.replace('/', "\\")))
        })
        .collect();
        if !pieces.is_empty() {
            let path = skin.join("skin.ini");
            let section = format!("[{model}]\n{}", pieces.join("\n"));
            if write_section(conn, cfg, &path, model, &section) {
                applied.written.push(path.to_string_lossy().into_owned());
            }
        }
    }

    applied
}

/// Retire ce qu'on avait posé pour cette voiture, et rend le fichier tel qu'il
/// était.
///
/// Sans effet sur ce qu'on n'a pas posé : un `ext_config.ini` qu'aucune
/// sauvegarde ne réclame et qui ne porte pas notre en-tête n'est pas à nous.
fn revert(conn: &Connection, cfg: &AppConfig, car_dir: &Path, car_id: &str, skin_dir: Option<&Path>) {
    restore_file(
        conn,
        cfg,
        &car_dir.join("extension").join("ext_config.ini"),
        BODY_SECTION,
    );
    if let Some(skin) = skin_dir {
        // La section de tenue porte le nom du mannequin **d'origine** : c'est
        // sous celui-là qu'on la retire, puisque le corps est déjà rendu.
        //
        // Limite connue, et étroite : si on avait écrit la tenue sous un corps
        // substitué, cette ligne ne la retrouve pas. Les deux chemins normaux
        // la couvrent pourtant — un `skin.ini` qui existait est restauré en
        // entier par sa sauvegarde, un `skin.ini` qu'on a créé est supprimé —
        // et il ne reste que le cas où quelqu'un a réécrit le fichier entre
        // temps, en effaçant à la fois la trace de la sauvegarde et notre
        // en-tête. Le jour où ça se voit, c'est un registre de ce qu'on a
        // écrit qu'il faudra, pas une rustine ici.
        if let Some(model) = crate::driver::outfit_of(car_dir, car_id, None).map(|o| o.model) {
            restore_file(conn, cfg, &skin.join("skin.ini"), &model);
        }
    }
}

// --- Écriture -----------------------------------------------------------------

/// Écrit une section dans un ini, en gardant tout le reste. Renvoie `true` si
/// le fichier a réellement changé.
fn write_section(conn: &Connection, cfg: &AppConfig, path: &Path, name: &str, section: &str) -> bool {
    let existing = std::fs::read_to_string(path).ok();
    let merged = match &existing {
        Some(text) => merge_section(text, name, section),
        None => format!("{MARKER}\n\n{section}\n"),
    };
    if existing.as_deref() == Some(merged.as_str()) {
        return false; // déjà en place : on ne touche à rien
    }

    // Sauvegarde avant écriture (règle d'or n°5). Un fichier absent n'a rien à
    // sauvegarder, et son retour en arrière est sa suppression.
    if existing.is_some() && !crate::gamebackup::protect(conn, cfg, path) {
        log::warn!("driver: {} non sauvegardé, rien n'est écrit", path.display());
        return false;
    }

    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            log::warn!("driver: {} — {e}", parent.display());
            return false;
        }
    }
    // Effacer d'abord : le fichier est un hardlink vers la bibliothèque, et
    // écrire par-dessus modifierait la copie de bibliothèque avec lui.
    if path.exists() {
        if let Err(e) = std::fs::remove_file(path) {
            log::warn!("driver: {} non remplaçable — {e}", path.display());
            return false;
        }
    }
    match std::fs::write(path, merged) {
        Ok(()) => true,
        Err(e) => {
            log::warn!("driver: {} non écrit — {e}", path.display());
            false
        }
    }
}

/// Remplace la section `name` par `section`, ou l'ajoute en fin de fichier.
///
/// Un ini AC n'a pas de grammaire compliquée : une section court de son
/// en-tête crochets jusqu'au prochain, et tout le reste se recopie tel quel —
/// commentaires et espacement compris, parce que le fichier appartient à
/// l'auteur du mod et qu'on n'y passe pas la serpillière.
fn merge_section(text: &str, name: &str, section: &str) -> String {
    let header = format!("[{name}]");
    let mut out = String::with_capacity(text.len() + section.len() + 2);
    let mut skipping = false;
    let mut replaced = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if skipping {
                skipping = false;
            }
            // Comparaison insensible à la casse : `[driver_80]` et
            // `[DRIVER_80]` désignent le même mannequin, et les deux existent.
            if trimmed.eq_ignore_ascii_case(&header) {
                skipping = true;
                replaced = true;
                out.push_str(section);
                out.push('\n');
                continue;
            }
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }

    if !replaced {
        if !out.ends_with("\n\n") && !out.is_empty() {
            out.push('\n');
        }
        out.push_str(section);
        out.push('\n');
    }
    out
}

// --- Retour en arrière ---------------------------------------------------------

/// Rend un fichier à ce qu'il était : sa sauvegarde s'il en a une, sinon la
/// suppression s'il est de nous, sinon le retrait de notre seule section.
fn restore_file(conn: &Connection, cfg: &AppConfig, path: &Path, section: &str) {
    if crate::gamebackup::is_replaced(conn, path) {
        crate::gamebackup::restore(conn, path);
        return;
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    if !text.contains(MARKER) {
        // Pas de sauvegarde et pas notre en-tête : le fichier existait avant
        // nous sans qu'on l'ait touché, ou quelqu'un l'a réécrit depuis. On
        // retire notre section et rien d'autre.
        let stripped = strip_section(&text, section);
        if stripped != text {
            let _ = std::fs::remove_file(path);
            let _ = std::fs::write(path, stripped);
        }
        return;
    }
    // Fichier créé par nous : il repart, et son dossier avec lui s'il devient
    // vide — `extension/` n'existait pas non plus avant.
    let _ = std::fs::remove_file(path);
    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir(parent);
    }
    let _ = cfg;
}

/// Le même fichier sans la section nommée.
fn strip_section(text: &str, name: &str) -> String {
    let header = format!("[{name}]");
    let mut out = String::with_capacity(text.len());
    let mut skipping = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            skipping = trimmed.eq_ignore_ascii_case(&header);
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXISTING: &str = "\
[DATA]
DISABLE_LIGHTSINI = 1

[ANALOG_INDICATOR_...]
BIND_TO=RPM
OBJECT_NAME=ARROW_RPM
";

    /// Règle n°2 : on remplace une section, on ne réécrit pas le fichier. Un
    /// `ext_config.ini` de mod fait des centaines de lignes qui font vivre les
    /// phares et les instruments.
    #[test]
    fn merging_keeps_everything_else() {
        let merged = merge_section(EXISTING, BODY_SECTION, "[DRIVER3D_MODEL]\nNAME=driver_501");
        assert!(merged.contains("[DATA]"), "la section d'origine survit");
        assert!(merged.contains("DISABLE_LIGHTSINI = 1"), "ses clés aussi");
        assert!(merged.contains("BIND_TO=RPM"), "et celles d'après");
        assert!(merged.contains("NAME=driver_501"), "la nôtre est là");
    }

    /// Une deuxième pose ne duplique pas la section : elle la remplace.
    #[test]
    fn applying_twice_replaces_rather_than_appends() {
        let once = merge_section(EXISTING, BODY_SECTION, "[DRIVER3D_MODEL]\nNAME=driver_501");
        let twice = merge_section(&once, BODY_SECTION, "[DRIVER3D_MODEL]\nNAME=driver_60");
        assert_eq!(twice.matches("[DRIVER3D_MODEL]").count(), 1, "une seule section");
        assert!(twice.contains("NAME=driver_60"), "la nouvelle valeur");
        assert!(!twice.contains("NAME=driver_501"), "et pas l'ancienne");
        assert!(twice.contains("[DATA]"), "le reste du fichier n'a pas bougé");
    }

    /// La casse d'un nom de mannequin varie d'un `skin.ini` à l'autre
    /// (`[driver_80]` et `[DRIVER_80]` existent tous les deux) : viser la
    /// mauvaise casse écrirait une seconde section que rien ne lirait.
    #[test]
    fn section_names_match_whatever_their_case() {
        let text = "[Driver_80]\nSUIT=\\plain\\red\n";
        let merged = merge_section(text, "driver_80", "[driver_80]\nHELMET=\\a\\b");
        assert_eq!(merged.matches('[').count(), 1, "une seule section, pas deux");
        assert!(!merged.contains("SUIT"), "l'ancienne est bien partie");
    }

    /// Retirer notre section laisse le fichier utilisable.
    #[test]
    fn stripping_leaves_the_rest_alone() {
        let merged = merge_section(EXISTING, BODY_SECTION, "[DRIVER3D_MODEL]\nNAME=driver_501");
        let stripped = strip_section(&merged, BODY_SECTION);
        assert!(!stripped.contains("DRIVER3D_MODEL"), "notre section est partie");
        assert!(stripped.contains("[DATA]"), "le reste est intact");
        assert!(stripped.contains("BIND_TO=RPM"), "jusqu'au bout");
    }

    /// Un fichier créé de toutes pièces porte son en-tête : c'est lui, et lui
    /// seul, qui autorisera à le supprimer au retour en arrière.
    #[test]
    fn a_file_we_create_says_so() {
        let tmp = crate::testutil::temp_dir("driver_apply_new");
        let path = tmp.join("extension").join("ext_config.ini");
        std::fs::create_dir_all(path.parent().unwrap()).expect("dossier");
        std::fs::write(&path, format!("{MARKER}\n\n[DRIVER3D_MODEL]\nNAME=driver_501\n")).expect("écriture");
        let text = std::fs::read_to_string(&path).expect("relecture");
        assert!(text.contains(MARKER), "l'en-tête est là");
        assert!(text.contains("NAME=driver_501"), "et la section aussi");
    }
}
