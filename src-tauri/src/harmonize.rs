//! Orchestration de l'harmonisation (§5/§5) : applique l'ontologie sur un
//! mod et persiste le résultat dans l'overlay. Utilisé à l'import et lors d'une
//! réapplication globale après édition des règles.

use rusqlite::Connection;

use crate::config::AppConfig;
use crate::modscan::ModKind;
use crate::overlay::{self, ModRow};
use crate::rules::{self, Harmonized, Rules};
use crate::uijson;

fn is_empty(c: Option<&str>) -> bool {
    c.is_none_or(|s| s.trim().is_empty())
}

/// Calcule l'harmonisation à partir des valeurs brutes du mod.
pub fn compute(
    rules: &Rules,
    kind: ModKind,
    raw_tags: &[String],
    name: &str,
    class: &str,
    native_country: Option<&str>,
) -> Harmonized {
    match kind {
        ModKind::Car => rules::apply_car(rules, raw_tags, name, class, is_empty(native_country)),
        ModKind::Track => rules::apply_track(rules, raw_tags),
    }
}

/// Persiste l'harmonisation.
///
/// **Le seul endroit où le pays final est décidé**, et c'est ce qui rend la
/// normalisation fiable : les deux sources se rejoignent ici — le champ natif
/// du `ui_*.json` s'il est renseigné, sinon celui qu'une règle a déduit d'un
/// tag (`extraction_country`). Normaliser plus haut, à la lecture du `ui_json`,
/// aurait laissé passer le second sans y toucher.
pub fn store(
    conn: &Connection,
    id: &str,
    h: &Harmonized,
    native_country: Option<&str>,
    rules: &Rules,
) -> rusqlite::Result<()> {
    let country = final_country(rules, h, native_country);
    overlay::update_harmonization(
        conn,
        id,
        h.brand.as_deref(),
        h.car_class.as_deref(),
        h.category.as_deref(),
        &h.categories,
        country.as_deref(),
        &h.tags_from_rule,
        h.drivetrain.as_deref(),
        h.engine_pos.as_deref(),
        h.aspiration.as_deref(),
        h.engine_config.as_deref(),
        h.gearbox.as_deref(),
    )
}

/// Le pays retenu : le natif s'il est renseigné, sinon celui qu'un tag a donné,
/// puis **normalisé dans les deux cas** (`rules::canonical_country`). Extrait de
/// `store` pour être testable sans base.
fn final_country(rules: &Rules, h: &Harmonized, native_country: Option<&str>) -> Option<String> {
    native_country
        .filter(|c| !c.trim().is_empty())
        .map(|s| s.to_string())
        .or_else(|| h.country.clone())
        .and_then(|c| rules::canonical_country(&c, &rules.country_aliases))
}

/// Réapplique l'ontologie à tous les mods (après édition des règles).
/// Renvoie le nombre de mods retraités.
pub fn harmonize_all(conn: &Connection, cfg: &AppConfig, rules: &Rules) -> rusqlite::Result<usize> {
    let mods = overlay::list_mods(conn)?;
    let mut n = 0;
    for m in &mods {
        if reharmonize_one(conn, cfg, rules, m).is_ok() {
            n += 1;
        }
    }
    Ok(n)
}

fn reharmonize_one(conn: &Connection, cfg: &AppConfig, rules: &Rules, m: &ModRow) -> rusqlite::Result<()> {
    let Some(h) = recompute_for(conn, cfg, rules, m) else {
        return Ok(());
    };
    let native_country = native_country(conn, cfg, m);
    store(conn, &m.id_interne, &h, native_country.as_deref(), rules)
}

/// Recalcule l'harmonisation d'un mod en relisant sa version active (lecture seule).
fn recompute_for(conn: &Connection, cfg: &AppConfig, rules: &Rules, m: &ModRow) -> Option<Harmonized> {
    let kind = if m.kind == "Track" {
        ModKind::Track
    } else {
        ModKind::Car
    };
    let vid = m.active_version_id.as_ref()?;
    let stored = overlay::get_version_path(conn, vid).ok().flatten()?;
    let lib = crate::libpath::resolve(cfg.library_path.as_deref(), &stored)?;
    let ui = match kind {
        ModKind::Car => uijson::read_car(&lib),
        ModKind::Track => uijson::read_track(&lib),
    }
    .unwrap_or_default();
    let class = ui.class.clone().unwrap_or_default();
    let name = ui.name.clone().unwrap_or_else(|| m.id_interne.clone());
    Some(compute(rules, kind, &ui.tags, &name, &class, ui.country.as_deref()))
}

fn native_country(conn: &Connection, cfg: &AppConfig, m: &ModRow) -> Option<String> {
    let vid = m.active_version_id.as_ref()?;
    let stored = overlay::get_version_path(conn, vid).ok().flatten()?;
    let lib = crate::libpath::resolve(cfg.library_path.as_deref(), &stored)?;
    uijson::read_car(&lib).and_then(|ui| ui.country)
}

/// Aperçu d'impact (§5) : nombre de mods dont l'harmonisation changerait
/// avec le jeu de règles candidat (comparé aux tags règle / catégorie / classe
/// actuellement stockés). Ne modifie rien.
pub fn count_affected(conn: &Connection, cfg: &AppConfig, rules: &Rules) -> rusqlite::Result<usize> {
    use std::collections::BTreeSet;
    let mods = overlay::list_mods(conn)?;
    let mut n = 0;
    for m in &mods {
        let Some(h) = recompute_for(conn, cfg, rules, m) else {
            continue;
        };
        let cand: BTreeSet<&String> = h.tags_from_rule.iter().collect();
        let cur: BTreeSet<&String> = m.tags_from_rule.iter().collect();
        if cand != cur || h.category != m.category || h.categories != m.categories || h.car_class != m.car_class {
            n += 1;
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A country reaches the overlay by one of TWO paths, and they must not
    /// disagree: the `country` field of the mod's `ui_*.json`, or, when that
    /// field is empty, a tag the rules recognise (`extraction_country`). This
    /// is why the normalisation lives in `store` and nowhere else — done at
    /// `ui_json` read time, it would have left the second path untouched, and
    /// a car tagged `usa` would have been filed apart from a car declaring
    /// `U.S.A.` while both mean the same country.
    #[test]
    fn both_paths_to_a_country_land_on_the_same_spelling() {
        let rules = rules::default_rules();

        // 1. Déclaré dans le fichier, orthographe non canonique.
        let declared = compute(&rules, ModKind::Car, &[], "Any Car", "street", Some("U.S.A."));
        let from_file = super::final_country(&rules, &declared, Some("U.S.A."));

        // 2. Champ natif vide : c'est le tag qui parle.
        let tagged = compute(&rules, ModKind::Car, &["usa".into()], "Any Car", "street", None);
        let from_tag = super::final_country(&rules, &tagged, None);

        assert_eq!(from_file.as_deref(), Some("United States"), "declared in the file");
        assert_eq!(from_tag.as_deref(), Some("United States"), "deduced from a tag");
        assert_eq!(from_file, from_tag, "the two paths file the car under one country");
    }

    /// A declared country always wins over a tag: the author took the trouble
    /// to write it. The alias table normalises it, it never overrides it.
    #[test]
    fn a_declared_country_is_normalised_not_replaced_by_a_tag() {
        let rules = rules::default_rules();
        let h = compute(
            &rules,
            ModKind::Car,
            &["germany".into()],
            "Any Car",
            "street",
            Some("U.S.A."),
        );
        assert_eq!(
            super::final_country(&rules, &h, Some("U.S.A.")).as_deref(),
            Some("United States"),
            "the file said the United States, the tag does not get to say Germany"
        );
    }

    /// A track declares a country like a car does, and writes it just as
    /// freely — hence a table shared by both families rather than one filed
    /// under `car`.
    #[test]
    fn a_track_country_goes_through_the_same_table() {
        let rules = rules::default_rules();
        let h = compute(&rules, ModKind::Track, &[], "Any Track", "", Some("Great Britain"));
        assert_eq!(
            super::final_country(&rules, &h, Some("Great Britain")).as_deref(),
            Some("United Kingdom")
        );
    }
}
