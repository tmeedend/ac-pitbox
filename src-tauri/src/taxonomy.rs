//! The taxonomy tables of the application: catalogue + overlay (REGLES§2).
//!
//! The types and the merge live in the `pitbox-catalog` crate, shared with
//! `rules-tool` (the developer's tool that promotes decisions into the
//! catalogue). What stays here is what only the application does: the
//! embedded catalogue, the files of the user, and the one-time migration out of
//! `tag-rules.json`.
//!
//! The catalogue is its own file, `rules/taxonomy-catalog.json`, and not a
//! section of `default-tag-rules.json`: `rules-tool promote` rewrites it whole
//! from Rust structs, which keeps it byte-stable from one promotion to the
//! next, where rewriting the rules file would have re-sorted all its keys.

use std::path::Path;

pub use pitbox_catalog::taxonomy::{FamilyOverlay, MapOverlay, TaxonomyOverlay, TaxonomyTables, FORMAT};

use crate::rules::Rules;

pub const CATALOG: &str = include_str!("../rules/taxonomy-catalog.json");

/// The taxonomy catalogue in force - the embedded one, or the previous one
/// when the user went back to it (`catalog_update`).
pub fn catalog() -> TaxonomyTables {
    tables_of(&crate::rules::default_rules())
}

/// The three taxonomy tables of a set of rules.
pub fn tables_of(r: &Rules) -> TaxonomyTables {
    TaxonomyTables {
        families: r.car.category_families.clone(),
        country_aliases: r.country_aliases.map.clone(),
        country_tags: r.car.extraction_country.map.clone(),
        brand_aliases: r.brand_aliases.clone(),
    }
}

/// The three tables as every pre-layer version copied them into
/// `tag-rules.json` (REGLES§13.1) — **frozen**, never updated with the
/// catalogue.
///
/// The migration diffs a user's file against THIS, not against the current
/// catalogue: a user skipping versions would otherwise arrive with an old
/// copy, compared to a newer catalogue, and every difference - improvements
/// he never saw - would be recorded as HIS decision and frozen for good.
const PRE_LAYER: &str = include_str!("../rules/manifests/pre-layer-tables.json");

pub fn pre_layer() -> Rules {
    serde_json::from_str(PRE_LAYER).expect("the pre-layer manifest must be valid")
}

/// The overlay reproducing the tables a pre-layer `tag-rules.json` stored
/// whole — **so that the library is classified exactly as before**
/// (REGLES§13.5).
///
/// `baseline` is the frozen `pre_layer()` manifest, measured on the release
/// history: the country tags are identical in every published version
/// (v0.1.0 to v0.7.0), and the aliases and families were never published;
/// they only exist in files written by this branch, from these very tables.
/// So the copy a user's file holds came from the manifest, and every
/// difference is his. A section missing or empty in the file was refilled
/// from the catalogue on each load: no decision, no overlay.
pub fn migrate(file: &Rules, baseline: &Rules) -> TaxonomyOverlay {
    let mut o = TaxonomyOverlay {
        format: FORMAT,
        ..Default::default()
    };
    if !file.car.category_families.is_empty() {
        o.families = FamilyOverlay::diff(&file.car.category_families, &baseline.car.category_families);
    }
    if !file.country_aliases.map.is_empty() {
        o.country_aliases = MapOverlay::diff(&file.country_aliases.map, &baseline.country_aliases.map);
    }
    if !file.car.extraction_country.map.is_empty() {
        o.country_tags = MapOverlay::diff(&file.car.extraction_country.map, &baseline.car.extraction_country.map);
    }
    o.ignored_countries = file.country_aliases.ignored.clone();
    o
}

/// Writes the effective tables into `rules`: catalogue, then overlay.
pub fn apply(rules: &mut Rules, catalog: &TaxonomyTables, o: &TaxonomyOverlay) {
    let t = catalog.apply(o);
    rules.car.category_families = t.families;
    rules.country_aliases.map = t.country_aliases;
    rules.country_aliases.ignored = o.ignored_countries.clone();
    rules.car.extraction_country.map = t.country_tags;
    rules.brand_aliases = t.brand_aliases;
}

/// Reads `taxonomy.json`, or builds it from the tables of the rules file the
/// first time (then writes it, so the migration happens once).
///
/// **A file that exists but does not parse is never overwritten**: an empty
/// overlay is used for this session and the failure is logged. Rewriting it
/// would erase the user's decisions for the sake of a clean start.
pub fn load_or_migrate(path: &Path, file: &Rules) -> TaxonomyOverlay {
    match std::fs::read_to_string(path) {
        Ok(s) => match serde_json::from_str::<TaxonomyOverlay>(&s) {
            Ok(o) => o,
            Err(e) => {
                log::warn!(
                    "{} unreadable, taxonomy overlay ignored this session: {e}",
                    path.display()
                );
                TaxonomyOverlay::default()
            }
        },
        Err(_) => {
            let o = migrate(file, &pre_layer());
            if let Err(e) = save(path, &o) {
                log::warn!("taxonomy overlay migration not written, will run again: {e}");
            }
            o
        }
    }
}

pub fn save(path: &Path, o: &TaxonomyOverlay) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(o).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::default_rules;
    use pitbox_catalog::taxonomy::CategoryFamily;

    fn sorted(mut f: Vec<CategoryFamily>) -> Vec<(String, Vec<String>)> {
        for x in &mut f {
            x.tags.sort();
        }
        f.into_iter().map(|x| (x.id, x.tags)).collect()
    }

    /// REGLES§13.5: migrating must not reclassify anything. The effective
    /// tables rebuilt from catalogue + migrated overlay are the tables the file
    /// held.
    #[test]
    fn migrating_a_whole_table_reproduces_it_exactly() {
        let baseline = pre_layer();
        let mut file = baseline.clone();
        file.car.category_families.retain(|f| f.id != "drift");
        file.car.category_families[0].tags.push("sport".into());
        file.car.category_families[1].name = Some("Sport".into());
        file.car.category_families.push(CategoryFamily {
            id: "endurance".into(),
            name: Some("Endurance".into()),
            icon: None,
            tags: vec!["wec".into()],
        });
        file.country_aliases.map.remove("usa");
        file.country_aliases.map.insert("nippon".into(), "Japan".into());
        file.car.extraction_country.map.insert("nihon".into(), "Japan".into());

        let o = migrate(&file, &baseline);
        let mut rebuilt = file.clone();
        apply(&mut rebuilt, &tables_of(&baseline), &o);

        let want = |f: &Rules| {
            (
                sorted(f.car.category_families.clone()),
                f.country_aliases.map.clone(),
                f.car.extraction_country.map.clone(),
            )
        };
        assert_eq!(want(&rebuilt), want(&file), "diff nul: same tables before and after");
        assert!(
            o.country_aliases.removed.contains("usa"),
            "a removed shipped alias is a tombstone"
        );
    }

    /// A section a pre-layer file did not have was refilled from the catalogue:
    /// not a decision, so nothing to record - and the next catalogue reaches it.
    #[test]
    fn a_missing_section_migrates_to_no_overlay() {
        let baseline = pre_layer();
        let mut file = baseline.clone();
        file.car.category_families.clear();
        file.country_aliases.map.clear();
        file.car.extraction_country.map.clear();
        assert!(!migrate(&file, &baseline).has_decisions());
    }

    /// REGLES§13.1: a user who skipped versions must not see catalogue
    /// improvements he never received recorded as his own decisions.
    #[test]
    fn migration_compares_to_the_frozen_manifest_not_the_current_catalogue() {
        let old_copy = pre_layer();
        let mut newer = tables_of(&pre_layer());
        newer.country_tags.insert("nihon".into(), "Japan".into());
        let o = migrate(&old_copy, &pre_layer());
        assert!(!o.has_decisions(), "an untouched copy is no decision");
        let mut rules = old_copy.clone();
        apply(&mut rules, &newer, &o);
        assert_eq!(
            rules.car.extraction_country.map.get("nihon").map(String::as_str),
            Some("Japan"),
            "the improvement arrives"
        );
    }

    /// The catalogue moved out of `default-tag-rules.json` into its own file:
    /// the rules the app builds must still carry it whole.
    #[test]
    fn the_default_rules_carry_the_catalogue() {
        assert_eq!(tables_of(&default_rules()), catalog());
        assert!(!catalog().families.is_empty() && !catalog().country_tags.is_empty());
    }

    /// A corrupt file is never overwritten by a migration.
    #[test]
    fn an_unreadable_overlay_is_left_alone() {
        let dir = crate::testutil::temp_dir("taxonomy");
        let path = dir.join("taxonomy.json");
        std::fs::write(&path, "{ not json").unwrap();
        let o = load_or_migrate(&path, &default_rules());
        assert_eq!(o, TaxonomyOverlay::default());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{ not json",
            "the user's file is untouched"
        );
    }
}
