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
}

#[derive(Deserialize)]
struct TrackEntry {
    release: String,
}

#[derive(Deserialize)]
struct Table {
    cars: HashMap<String, CarEntry>,
    tracks: HashMap<String, TrackEntry>,
}

static TABLE: LazyLock<Table> =
    LazyLock::new(|| serde_json::from_str(RAW).expect("docs/kunos_content_dates.json invalide"));

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
