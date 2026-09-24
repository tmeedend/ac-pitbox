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
//! kept, and what names rules this catalogue does not have. The per-mod
//! corrections (tags typed on a car, a name taken back) are not in it: they
//! belong to one library, not to a way of classifying.

use std::path::Path;

use pitbox_catalog::merge::{merge_rules, merge_taxonomy, MergeStats, RulesCatalog};
use pitbox_catalog::rules::RulesOverlay;
use pitbox_catalog::taxonomy::TaxonomyOverlay;
use serde::{Deserialize, Serialize};

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
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportReport {
    #[serde(flatten)]
    pub stats: MergeStats,
    /// The catalogue the file was made on - said when it is not this one.
    pub catalog_version: String,
}

fn overlays(dir: &Path, catalog: &Rules) -> (RulesOverlay, TaxonomyOverlay) {
    let legacy = crate::rules::read_legacy(dir);
    let rules = crate::rule_overlay::load_or_migrate(&dir.join("rules-overlay.json"), legacy.as_ref());
    let tax = crate::taxonomy::load_or_migrate(&dir.join("taxonomy.json"), &legacy.unwrap_or_default());
    (crate::rule_overlay::normalized(rules, catalog), tax)
}

pub fn export(dir: &Path, catalog: &Rules) -> RulesExport {
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
    }
}

pub fn write_export(dir: &Path, catalog: &Rules, path: &Path) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&export(dir, catalog)).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| format!("{}: {e}", path.display()))
}

/// Merges an export into the overlays of `dir` and writes them. The files as
/// they were are kept beside them (`*.before-import.json`): an import is one
/// click, and the way back should not depend on the startup backup's timing.
pub fn import(dir: &Path, catalog: &Rules, path: &Path) -> Result<ImportReport, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file: RulesExport = serde_json::from_str(&text).map_err(|e| {
        log::warn!("{} is not a rules export: {e}", path.display());
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
        write_export(&a, &catalog, &file).unwrap();
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
