//! Commands of the tag engine (§5): the rules in two layers (REGLES§2) and the
//! Rules screen that edits their overlay (REGLES§8), the taxonomy tabs, the
//! catalogue update report, and re-applying everything to the library.

use super::prelude::*;

#[tauri::command]
pub fn get_rules(app: AppHandle) -> Rules {
    crate::rules::load(&app)
}

fn rules_view(app: &AppHandle, o: &RulesOverlay, effects: &crate::harmonize::Effects) -> Result<RulesView, String> {
    let dir = crate::rules::config_dir(app)?;
    let version = crate::catalog_update::version_in_force(&dir);
    Ok(crate::rule_overlay::view(
        o,
        &crate::rules::default_rules(),
        effects,
        version,
    ))
}

/// The Rules screen: every section in execution order, each rule with the
/// number of mods it acts on - measured, so the whole library is read.
#[tauri::command]
pub fn get_rules_view(app: AppHandle, db: State<Db>) -> Result<RulesView, String> {
    let o = crate::rules::load_rules_overlay(&app)?;
    let rules = crate::rules::load(&app);
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let effects = crate::harmonize::effects(&conn, &cfg, &rules).map_err(|e| e.to_string())?;
    rules_view(&app, &o, &effects)
}

/// Writes the screen's decisions and re-applies them to the whole library;
/// the counters come from that same pass (REGLES§8.3, "recalculated after
/// every change").
#[tauri::command]
pub fn save_rules_overlay(app: AppHandle, db: State<Db>, overlay: RulesOverlay) -> Result<RulesView, String> {
    let o = crate::rules::save_rules_overlay(&app, overlay)?;
    // Harmonise with the rules as they now APPLY, reloaded from what was
    // written - the order the next start will use.
    let rules = crate::rules::load(&app);
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let (_, effects) = crate::harmonize::harmonize_all_counting(&conn, &cfg, &rules).map_err(|e| e.to_string())?;
    rules_view(&app, &o, &effects)
}

/// The taxonomy tables in their two layers (REGLES§2): what the catalogue
/// ships, what the user decided, and what applies. The Categories and
/// Countries tabs need all three - the catalogue to say "modified" and to
/// restore, the overlay to edit, the effective tables to display.
#[derive(serde::Serialize)]
pub struct TaxonomyView {
    catalog: TaxonomyTables,
    overlay: TaxonomyOverlay,
    effective: TaxonomyTables,
}

fn tables(r: &Rules) -> TaxonomyTables {
    TaxonomyTables {
        families: r.car.category_families.clone(),
        country_aliases: r.country_aliases.map.clone(),
        country_tags: r.car.extraction_country.map.clone(),
    }
}

fn view(rules: &Rules, overlay: TaxonomyOverlay) -> TaxonomyView {
    TaxonomyView {
        catalog: crate::taxonomy::catalog(),
        overlay,
        effective: tables(rules),
    }
}

#[tauri::command]
pub fn get_taxonomy(app: AppHandle) -> TaxonomyView {
    view(&crate::rules::load(&app), crate::rules::load_taxonomy(&app))
}

/// Writes the family decisions. **No re-harmonisation**: a family is an index
/// over tags read by the library, not a value stored per mod.
#[tauri::command]
pub fn save_family_overlay(app: AppHandle, families: FamilyOverlay) -> Result<TaxonomyView, String> {
    let mut o = crate::rules::load_taxonomy(&app);
    o.families = families;
    let (rules, o) = crate::rules::save_taxonomy(&app, o)?;
    Ok(view(&rules, o))
}

/// Writes the country decisions and re-applies them: the country is decided
/// at write time (`harmonize::store`), so an alias or a country tag changes
/// what is stored for every mod it reaches.
#[tauri::command]
pub fn save_country_overlay(
    app: AppHandle,
    db: State<Db>,
    aliases: MapOverlay,
    tags: MapOverlay,
    ignored: Vec<String>,
) -> Result<TaxonomyView, String> {
    let mut o = crate::rules::load_taxonomy(&app);
    o.country_aliases = aliases;
    o.country_tags = tags;
    o.ignored_countries = ignored;
    let (rules, o) = crate::rules::save_taxonomy(&app, o)?;
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::harmonize::harmonize_all(&conn, &cfg, &rules).map_err(|e| e.to_string())?;
    Ok(view(&rules, o))
}

/// The last catalogue update as the Workshop and the notification show it
/// (REGLES§6.2): its report while not dismissed, and whether "go back" is
/// possible or in force.
#[derive(serde::Serialize)]
pub struct CatalogReportView {
    report: Option<crate::catalog_update::Report>,
    can_revert: bool,
    reverted: bool,
    previous_version: Option<String>,
    current_version: Option<String>,
}

#[tauri::command]
pub fn get_catalog_report(app: AppHandle) -> Result<CatalogReportView, String> {
    let dir = crate::rules::config_dir(&app)?;
    let s = crate::catalog_update::load_state(&dir);
    Ok(CatalogReportView {
        report: s.report.filter(|_| !s.report_dismissed),
        can_revert: s.previous.is_some(),
        reverted: s.reverted,
        previous_version: s.previous.map(|p| p.app_version),
        current_version: s.current.map(|c| c.app_version),
    })
}

/// "Go back to the previous catalogue", or return to the current one
/// (REGLES§6.4). Re-classifies the library; returns the mods processed.
#[tauri::command]
pub fn set_catalog_reverted(app: AppHandle, db: State<Db>, reverted: bool) -> Result<usize, String> {
    let dir = crate::rules::config_dir(&app)?;
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::catalog_update::set_reverted(&dir, &conn, &cfg, reverted)
}

#[tauri::command]
pub fn dismiss_catalog_report(app: AppHandle) -> Result<(), String> {
    let dir = crate::rules::config_dir(&app)?;
    crate::catalog_update::dismiss_report(&dir)
}

/// Réapplique les règles enregistrées à toute la bibliothèque.
#[tauri::command]
pub fn reapply_rules(app: AppHandle, db: State<Db>) -> Result<usize, String> {
    let rules = crate::rules::load(&app);
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::harmonize::harmonize_all(&conn, &cfg, &rules).map_err(|e| e.to_string())
}
