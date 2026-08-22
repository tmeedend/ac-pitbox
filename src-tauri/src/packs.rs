//! Le pack comme entité à part entière (§4.4).
//!
//! Un pack n'existe en base que sous la forme d'une colonne `source_pack` sur
//! chaque mod : le nom de l'archive qui les a livrés ensemble. Il possède
//! pourtant des choses que ses membres n'ont pas, et que rien ne montrait :
//! des **ajouts au jeu** (`extras/packs/<nom>`) et des **ressources**
//! (`resources/packs/<nom>`) — tout ce qui entourait les mods sans appartenir
//! à aucun en particulier.
//!
//! Cas réel qui a motivé cette fiche : un pack de 94 voitures livrant
//! `content/{driver,fonts,texture}`, soit 82 fichiers posés dans le jeu et
//! visibles **nulle part** dans l'app — `list_mod_extras` ne regarde que
//! `extras/cars/<id>`. Les fichiers étaient bien là, bien déployés, bien
//! retirés avec le dernier membre du pack ; simplement invisibles.
//!
//! Ce module n'est que de l'assemblage : les briques (cartes, ajouts au jeu,
//! ressources) existent déjà et servent les mêmes écrans ailleurs.

use rusqlite::Connection;
use serde::Serialize;

use crate::config::AppConfig;
use crate::extras::{self, OwnerKind};
use crate::library::{self, ModCard};

#[derive(Debug, Clone, Serialize)]
pub struct PackDetail {
    /// Nom du pack = nom de l'archive/dossier source (§4.4).
    pub name: String,
    /// Les mods livrés ensemble, dans l'ordre d'affichage de la bibliothèque.
    pub members: Vec<ModCard>,
    /// Fichiers posés dans le jeu par le pack lui-même — ceux que rien ne
    /// rattachait à un mod en particulier.
    pub extras: Vec<extras::ExtraFile>,
    /// Somme des tailles sur disque des membres, octets (§9.4). `0` tant
    /// qu'aucune n'a été calculée.
    pub members_bytes: i64,
    /// Taille des ajouts au jeu du pack, octets.
    pub extras_bytes: i64,
    /// Import le plus récent parmi les membres — la date à laquelle ce pack
    /// est entré en bibliothèque.
    pub imported_at: Option<String>,
}

/// Assemble la fiche d'un pack. Un pack sans aucun membre n'existe plus :
/// c'est une erreur, pas une fiche vide — le dernier membre supprimé emporte
/// les fichiers du pack avec lui (`maintenance::delete_broken`).
pub fn detail(conn: &Connection, cfg: &AppConfig, pack: &str) -> Result<PackDetail, String> {
    let members: Vec<ModCard> = library::list_cards(conn, cfg)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|c| c.base.source_pack.as_deref() == Some(pack))
        .collect();
    if members.is_empty() {
        return Err(crate::errors::PACK_NOT_FOUND.to_string());
    }

    let extras = extras::list(conn, cfg, OwnerKind::Pack, pack);
    Ok(PackDetail {
        name: pack.to_string(),
        members_bytes: members.iter().filter_map(|c| c.base.size_bytes).sum(),
        extras_bytes: extras.iter().map(|f| f.size_bytes as i64).sum(),
        imported_at: members.iter().filter_map(|c| c.base.updated_at.clone()).max(),
        extras,
        members,
    })
}

/// Dossier ressources du pack (§4.5.2) — les notices et documents livrés à
/// côté des mods. Déjà listés sur la fiche de chaque membre (marqués
/// « du pack ») ; ici, ils sont chez eux.
pub fn resources_dir(cfg: &AppConfig, pack: &str) -> Result<std::path::PathBuf, String> {
    let library = cfg
        .library_path
        .as_deref()
        .ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    Ok(crate::resources::resources_dir_for(library, "packs", &[pack]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overlay;
    use std::path::Path;

    fn seed(base: &Path) -> (Connection, AppConfig) {
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        for id in ["car_a", "car_b"] {
            overlay::upsert_mod(&conn, id, "Car", Some("B"), Some(id), "h", None, &now).unwrap();
            overlay::set_source(&conn, id, Some("Pack.7z"), None).unwrap();
        }
        // Un mod hors pack : il ne doit jamais apparaître dans la fiche.
        overlay::upsert_mod(&conn, "lonely", "Car", Some("B"), Some("Lonely"), "h", None, &now).unwrap();
        let cfg = AppConfig {
            library_path: Some(base.join("lib")),
            ac_install_path: Some(base.join("ac")),
            ..AppConfig::default()
        };
        (conn, cfg)
    }

    #[test]
    fn a_pack_sheet_lists_its_members_and_only_them() {
        let base = crate::testutil::temp_dir("pack-detail");
        let (conn, cfg) = seed(&base);
        let d = detail(&conn, &cfg, "Pack.7z").unwrap();
        let mut ids: Vec<&str> = d.members.iter().map(|m| m.base.id_interne.as_str()).collect();
        ids.sort();
        assert_eq!(ids, vec!["car_a", "car_b"], "les deux membres, pas le mod isolé");
    }

    #[test]
    fn a_pack_without_members_is_an_error_not_an_empty_sheet() {
        // Le dernier membre supprimé emporte les fichiers du pack
        // (`maintenance::delete_broken`) : une fiche vide raconterait qu'il
        // reste quelque chose alors qu'il n'y a plus rien.
        let base = crate::testutil::temp_dir("pack-gone");
        let (conn, cfg) = seed(&base);
        assert_eq!(
            detail(&conn, &cfg, "Autre.7z").err().as_deref(),
            Some(crate::errors::PACK_NOT_FOUND),
            "un pack inconnu est refusé par sa clé i18n"
        );
    }
}
