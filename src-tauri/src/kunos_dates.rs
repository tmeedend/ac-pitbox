//! Table statique des dates du contenu officiel Kunos (voitures + circuits),
//! embarquée depuis `docs/kunos_content_dates.json` — seule source de vérité,
//! tenue à jour manuellement par l'utilisateur. Sert à compléter, à la
//! (ré)indexation du contenu de base (§12bis.1, `stock::index_stock_content`),
//! les champs que `ui_car.json`/`ui_track.json` ne fournissent pas :
//! - année du modèle (voitures) : lue dans `ui_car.json` si présente, sinon
//!   ici, sinon laissée vide ;
//! - date de publication (voitures + circuits) : toujours prise ici (`release`
//!   = date du pack), le contenu de base n'a pas d'estimation par fichiers
//!   comme les mods importés.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Deserialize;

use crate::modscan::ModKind;

const RAW: &str = include_str!("../../docs/kunos_content_dates.json");

#[derive(Deserialize)]
struct CarEntry {
    year: Option<i64>,
    release: String,
    pack: String,
}

#[derive(Deserialize)]
struct TrackEntry {
    release: String,
    pack: String,
}

#[derive(Deserialize)]
struct PackEntry {
    name: String,
}

#[derive(Deserialize)]
struct Table {
    packs: HashMap<String, PackEntry>,
    cars: HashMap<String, CarEntry>,
    tracks: HashMap<String, TrackEntry>,
}

static TABLE: LazyLock<Table> =
    LazyLock::new(|| serde_json::from_str(RAW).expect("docs/kunos_content_dates.json invalide"));

/// Clé du pack Kunos qui a introduit ce contenu, si référencé (`"base"` pour
/// le jeu de base, sinon l'un des DLC de `packs`).
fn pack_key(kind: ModKind, id: &str) -> Option<&'static str> {
    match kind {
        ModKind::Car => TABLE.cars.get(id).map(|c| c.pack.as_str()),
        ModKind::Track => TABLE.tracks.get(id).map(|t| t.pack.as_str()),
    }
}

/// Année du modèle réel d'une voiture Kunos, si référencée.
pub fn car_year(id: &str) -> Option<i64> {
    TABLE.cars.get(id).and_then(|c| c.year)
}

/// Date de sortie du pack ayant introduit ce contenu, si référencé.
pub fn release_date(kind: ModKind, id: &str) -> Option<String> {
    match kind {
        ModKind::Car => TABLE.cars.get(id).map(|c| c.release.clone()),
        ModKind::Track => TABLE.tracks.get(id).map(|t| t.release.clone()),
    }
}

/// Nom d'affichage du DLC qui a introduit ce contenu (§10bis, fiche détail —
/// bloc Source/Origine) — `None` pour le jeu de base (`pack == "base"`, le
/// frontend affiche alors son propre libellé traduit) ou un contenu non
/// référencé, jamais le nom brut de `"base"` (`"Assetto Corsa (base / 1.0)"`,
/// trop technique pour l'UI).
pub fn pack_name(kind: ModKind, id: &str) -> Option<String> {
    let key = pack_key(kind, id)?;
    if key == "base" {
        return None;
    }
    TABLE.packs.get(key).map(|p| p.name.clone())
}

/// True when this folder id belongs to official Kunos content — base game or
/// any DLC (§12bis.1). This is what tells apart, among the real folders sitting
/// in `content/`, the game's own content from a mod the user installed by hand
/// before Pit Box existed (`overlay::ModRow::is_unmanaged`).
///
/// The table is the criterion because it is the only signal that cannot lie:
/// `ui_car.json`'s `author` field is optional, and a mod derived from a Kunos
/// car keeps the original author — while no modder ever names a folder
/// `ks_porsche_911_gt3_rs`. Missing an id here therefore only ever downgrades
/// official content to "unmanaged mod", never the reverse: the safe direction,
/// since an unmanaged mod is protected from every write.
pub fn is_official(kind: ModKind, id: &str) -> bool {
    pack_key(kind, id).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Règle protégée : le jeu de base ne doit jamais afficher le nom
    /// technique brut de son pack (§10bis) — `None`, pas
    /// "Assetto Corsa (base / 1.0)".
    #[test]
    fn base_game_pack_name_is_none() {
        assert_eq!(pack_name(ModKind::Car, "abarth500"), None);
    }

    /// Règle protégée : un DLC référencé renvoie le nom d'affichage du pack,
    /// pas sa clé technique (§10bis).
    #[test]
    fn dlc_pack_name_resolves_display_name() {
        assert_eq!(
            pack_name(ModKind::Car, "ks_porsche_718_boxster_s"),
            Some("Porsche Pack Vol.2".to_string()),
        );
    }

    /// Règle protégée : contenu non référencé dans la table -> `None`, jamais
    /// une erreur (le frontend retombe sur « Jeu de base » par défaut).
    #[test]
    fn unknown_content_pack_name_is_none() {
        assert_eq!(pack_name(ModKind::Car, "not_a_real_car"), None);
    }

    /// Rule: official content is recognised for both kinds, base game and DLC
    /// alike (§12bis.1) — this is what keeps a real install from being flagged
    /// as a pile of unmanaged mods.
    #[test]
    fn official_content_is_recognised_for_base_and_dlc() {
        assert!(is_official(ModKind::Car, "abarth500"), "base game car");
        assert!(is_official(ModKind::Car, "ks_porsche_718_boxster_s"), "DLC car");
        assert!(is_official(ModKind::Track, "monza"), "base game track");
        assert!(is_official(ModKind::Track, "ks_nordschleife"), "DLC track");
    }

    /// Rule: anything the table does not know is a mod, not game content
    /// (§12bis.1). A track id is looked up in the track table only — an id
    /// shared with a car must not leak across kinds.
    #[test]
    fn unknown_id_is_not_official() {
        assert!(!is_official(ModKind::Car, "rss_gtm_lanzo_v8"), "mod car");
        assert!(!is_official(ModKind::Track, "shutoko_revival_project"), "mod track");
        assert!(
            !is_official(ModKind::Track, "abarth500"),
            "car id must not count as a track"
        );
    }
}
