//! Export and import of the user's decisions on the rules (REGLES§9).
//!
//! **The export holds the overlays only** - list rules (`rules-overlay.json`)
//! and taxonomy tables (`taxonomy.json`) - never the catalogue: exporting it
//! would freeze whoever imports it on an old version, and fill the file with
//! data every user already has. It names the catalogue it was made under.
//!
//! **The import merges, it never replaces** (`pitbox_catalog::merge`): what
//! the file brings is added, and where both sides decided the same entry
//! differently the importer's decision stays. So there is nothing to ask
//! before, and nothing lost after; the report counts what was added, what was
//! kept, and what names rules this catalogue does not have.
//!
//! **The category decisions of the user come with it** (REGLES§3 level 4, §9):
//! a car forced into a category, the categories added to or taken off a track,
//! by mod id. They merge like the rest - a mod the importer already decided
//! keeps his decision - and one about a mod this library lacks is kept for the
//! day it is imported. The other per-mod corrections (tags typed on a car, a
//! name taken back) are not in it.

use std::path::Path;

use pitbox_catalog::merge::{merge_rules, merge_taxonomy, MergeStats, RulesCatalog};
use pitbox_catalog::rules::RulesOverlay;
use pitbox_catalog::taxonomy::TaxonomyOverlay;
use serde::{Deserialize, Serialize};

use crate::overlay::CategoryDecision;
use crate::rules::Rules;

/// Version of the export format, and the key that tells an export from any
/// other JSON file.
pub const FORMAT: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesExport {
    pub pitbox_rules_export: u32,
    pub app_version: String,
    /// The catalogue the decisions were made on (REGLES§9).
    pub catalog_version: String,
    #[serde(default)]
    pub rules: RulesOverlay,
    #[serde(default)]
    pub taxonomy: TaxonomyOverlay,
    /// The user's category decisions on single mods (REGLES§3 level 4).
    /// Absent from an export made before them.
    #[serde(default)]
    pub category_decisions: Vec<CategoryDecision>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportReport {
    #[serde(flatten)]
    pub stats: MergeStats,
    /// The catalogue the file was made on - said when it is not this one.
    pub catalog_version: String,
    /// The category decisions the file carries, for the caller to merge into
    /// the base (this module only reads files). Not sent to the screen.
    #[serde(skip)]
    pub decisions: Vec<CategoryDecision>,
    /// Of those, the mods that had none and now do.
    pub decisions_added: usize,
    /// Of those, the mods that already had one, whose decision stays.
    pub decisions_kept: usize,
}

fn overlays(dir: &Path, catalog: &Rules) -> (RulesOverlay, TaxonomyOverlay) {
    let legacy = crate::rules::read_legacy(dir);
    let rules = crate::rule_overlay::load_or_migrate(&dir.join("rules-overlay.json"), legacy.as_ref());
    let tax = crate::taxonomy::load_or_migrate(&dir.join("taxonomy.json"), &legacy.unwrap_or_default());
    (crate::rule_overlay::normalized(rules, catalog), tax)
}

/// The overlays alone - what a survey carries. The category decisions live in
/// the base: [`export_with_decisions`] adds them.
pub fn export(dir: &Path, catalog: &Rules) -> RulesExport {
    export_with_decisions(dir, catalog, Vec::new())
}

pub fn export_with_decisions(dir: &Path, catalog: &Rules, decisions: Vec<CategoryDecision>) -> RulesExport {
    let (rules, taxonomy) = overlays(dir, catalog);
    RulesExport {
        pitbox_rules_export: FORMAT,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        catalog_version: crate::catalog_update::version_in_force(dir),
        // A preference of this machine, not a way of classifying.
        rules: RulesOverlay {
            catalog_off: false,
            ..rules
        },
        taxonomy,
        category_decisions: decisions,
    }
}

pub fn write_export(dir: &Path, catalog: &Rules, path: &Path, decisions: Vec<CategoryDecision>) -> Result<(), String> {
    let json =
        serde_json::to_string_pretty(&export_with_decisions(dir, catalog, decisions)).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| format!("{}: {e}", path.display()))
}

/// Merges an export into the overlays of `dir` and writes them. The files as
/// they were are kept beside them (`*.before-import.json`): an import is one
/// click, and the way back should not depend on the startup backup's timing.
/// The decisions a file carries: a rules export, or a library survey
/// (`survey.rs`), whose `decisions` section IS an export - so one file is
/// enough, for the developer bringing back his other machine's survey as for
/// a contributor who only sends his.
fn read_decisions(text: &str) -> Result<RulesExport, serde_json::Error> {
    let v: serde_json::Value = serde_json::from_str(text)?;
    match v.get("decisions") {
        Some(d) if v.get("pitbox_survey").is_some() => serde_json::from_value(d.clone()),
        _ => serde_json::from_value(v),
    }
}

pub fn import(dir: &Path, catalog: &Rules, path: &Path) -> Result<ImportReport, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file = read_decisions(&text).map_err(|e| {
        log::warn!("{} is neither a rules export nor a survey: {e}", path.display());
        crate::errors::NOT_A_RULES_EXPORT.to_string()
    })?;
    if file.pitbox_rules_export > FORMAT {
        log::warn!(
            "rules export format {} is newer than this version reads",
            file.pitbox_rules_export
        );
        return Err(crate::errors::RULES_EXPORT_TOO_NEW.to_string());
    }

    let (mut rules, mut tax) = overlays(dir, catalog);
    let mut stats = MergeStats::default();
    let (c, s) = (&catalog.car, &catalog.car.extraction_specs);
    let lists = RulesCatalog {
        brand_fix: &c.brand_fix,
        name_to_tag: &c.name_to_tag,
        class_fix: &c.class_fix,
        car_tag_merge: &c.tag_merge,
        drivetrain: &s.drivetrain,
        aspiration: &s.aspiration,
        engine_config: &s.engine_config,
        engine_pos: &s.engine_pos,
        gearbox: &s.gearbox,
        track_tag_merge: &catalog.track.tag_merge,
    };
    merge_rules(&mut rules, &file.rules, &lists, &mut stats);
    merge_taxonomy(&mut tax, &file.taxonomy, &mut stats);

    for name in ["rules-overlay", "taxonomy"] {
        let from = dir.join(format!("{name}.json"));
        if from.is_file() {
            let to = dir.join(format!("{name}.before-import.json"));
            std::fs::copy(&from, &to).map_err(|e| format!("{}: {e}", to.display()))?;
        }
    }
    crate::rule_overlay::save(
        &dir.join("rules-overlay.json"),
        &crate::rule_overlay::normalized(rules, catalog),
    )?;
    let tax = tax.normalized(&crate::taxonomy::tables_of(catalog));
    crate::taxonomy::save(&dir.join("taxonomy.json"), &tax)?;
    Ok(ImportReport {
        stats,
        catalog_version: file.catalog_version,
        decisions: file.category_decisions,
        decisions_added: 0,
        decisions_kept: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{default_rules, BrandFix};

    /// REGLES§9: what one user exports, another imports - his own rules, his
    /// switched-off shipped rules - and importing twice adds nothing.
    #[test]
    fn an_export_travels_and_importing_it_twice_adds_nothing() {
        let base = crate::testutil::temp_dir("rules-share");
        let (a, b) = (base.join("a"), base.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let catalog = default_rules();

        let mut o = RulesOverlay::default();
        o.brand_fix.own.push(BrandFix {
            id: None,
            name_contains: "lanzo".into(),
            set_brand: "RSS".into(),
        });
        o.brand_fix.disabled.insert("pitbox.brand.bayro".into());
        o.catalog_off = true;
        crate::rule_overlay::save(&a.join("rules-overlay.json"), &o).unwrap();

        let file = base.join("mine.json");
        write_export(&a, &catalog, &file, Vec::new()).unwrap();
        let exported: RulesExport = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert!(!exported.rules.catalog_off, "a preference of the machine, not exported");

        let first = import(&b, &catalog, &file).unwrap();
        assert_eq!(first.stats.added, 2, "{first:?}");
        let got = crate::rule_overlay::load_or_migrate(&b.join("rules-overlay.json"), None);
        assert_eq!(got.brand_fix.own[0].set_brand, "RSS");
        assert!(got.brand_fix.disabled.contains("pitbox.brand.bayro"));

        let second = import(&b, &catalog, &file).unwrap();
        assert_eq!((second.stats.added, second.stats.already), (0, 2), "{second:?}");
        assert!(
            b.join("rules-overlay.before-import.json").is_file(),
            "the way back is kept"
        );
    }

    /// A survey carries the same decisions as an export: importing it takes
    /// them, the rest of the survey is not read.
    #[test]
    fn a_survey_imports_its_decisions() {
        let base = crate::testutil::temp_dir("rules-share-survey");
        let (a, b) = (base.join("a"), base.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let catalog = default_rules();
        let mut o = RulesOverlay::default();
        o.brand_fix.own.push(BrandFix {
            id: None,
            name_contains: "lanzo".into(),
            set_brand: "Lamborghini".into(),
        });
        crate::rule_overlay::save(&a.join("rules-overlay.json"), &o).unwrap();
        let survey = serde_json::json!({
            "pitbox_survey": 1,
            "summary": {},
            "cars": [],
            "decisions": export(&a, &catalog),
        });
        let file = base.join("survey.json");
        std::fs::write(&file, survey.to_string()).unwrap();

        let r = import(&b, &catalog, &file).unwrap();
        assert_eq!(r.stats.added, 1, "{r:?}");
        let got = crate::rule_overlay::load_or_migrate(&b.join("rules-overlay.json"), None);
        assert_eq!(got.brand_fix.own[0].set_brand, "Lamborghini");
    }

    /// REGLES§9: the category decisions of a library leave with the export and
    /// come in by mod id; where the importer already decided a mod, his
    /// decision stays - importing is never a replacement.
    #[test]
    fn category_decisions_travel_and_the_importers_own_stay() {
        let base = crate::testutil::temp_dir("rules-share-decisions");
        let (a, b) = (base.join("a"), base.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let catalog = default_rules();
        let car = CategoryDecision {
            forced: Some("#hot_rods".into()),
            ..CategoryDecision::new("car")
        };
        let track = CategoryDecision {
            added: vec!["#rally".into()],
            removed: vec!["#drift".into()],
            ..CategoryDecision::new("track")
        };
        let file = base.join("mine.json");
        write_export(&a, &catalog, &file, vec![car, track.clone()]).unwrap();

        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let mine = CategoryDecision {
            forced: Some("#mine".into()),
            ..CategoryDecision::new("car")
        };
        crate::overlay::set_category_decision(&conn, &mine).unwrap();

        let report = import(&b, &catalog, &file).unwrap();
        assert_eq!(report.decisions.len(), 2, "the file carries them");
        let (added, kept) = crate::categories::merge_decisions(&conn, &catalog, &report.decisions).unwrap();
        assert_eq!((added, kept), (1, 1), "one new, one already decided here");
        assert_eq!(crate::overlay::get_category_decision(&conn, "car").unwrap(), Some(mine));
        assert_eq!(
            crate::overlay::get_category_decision(&conn, "track").unwrap(),
            Some(track)
        );
        let (again, kept) = crate::categories::merge_decisions(&conn, &catalog, &report.decisions).unwrap();
        assert_eq!((again, kept), (0, 2), "importing twice adds nothing");
    }

    /// An export made before the decisions existed has no such section, and
    /// imports as it always did.
    #[test]
    fn an_export_made_before_category_decisions_still_imports() {
        let base = crate::testutil::temp_dir("rules-share-old");
        let file = base.join("old.json");
        std::fs::write(
            &file,
            r#"{"pitbox_rules_export":1,"app_version":"0.9.0","catalog_version":"0.9.0"}"#,
        )
        .unwrap();
        let report = import(&base, &default_rules(), &file).unwrap();
        assert!(report.decisions.is_empty());
    }

    #[test]
    fn any_other_json_is_refused_by_name() {
        let base = crate::testutil::temp_dir("rules-share-refused");
        let file = base.join("other.json");
        std::fs::write(&file, r#"{"car":{}}"#).unwrap();
        assert_eq!(
            import(&base, &default_rules(), &file).unwrap_err(),
            crate::errors::NOT_A_RULES_EXPORT
        );
    }
}
