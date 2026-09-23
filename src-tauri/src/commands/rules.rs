//! Commandes du moteur de tags (§5) : lecture, édition, aperçu d'impact et
//! réapplication à toute la bibliothèque.

use super::prelude::*;

#[tauri::command]
pub fn get_rules(app: AppHandle) -> Rules {
    crate::rules::load(&app)
}

/// Enregistre les règles et réapplique l'ontologie à toute la bibliothèque.
/// Renvoie le nombre de mods retraités.
#[tauri::command]
pub fn save_rules(app: AppHandle, db: State<Db>, rules: Rules) -> Result<usize, String> {
    crate::rules::save(&app, &rules)?;
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::harmonize::harmonize_all(&conn, &cfg, &rules).map_err(|e| e.to_string())
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

#[derive(serde::Serialize)]
pub struct TaxonomyTables {
    families: Vec<CategoryFamily>,
    country_aliases: std::collections::BTreeMap<String, String>,
    country_tags: std::collections::BTreeMap<String, String>,
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
        catalog: tables(&crate::rules::default_rules()),
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

/// Aperçu d'impact : nombre de mods affectés par un jeu de règles candidat,
/// sans rien enregistrer (§5).
#[tauri::command]
pub fn rules_impact(app: AppHandle, db: State<Db>, rules: Rules) -> Result<usize, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::harmonize::count_affected(&conn, &cfg, &rules).map_err(|e| e.to_string())
}

/// Réapplique les règles enregistrées à toute la bibliothèque.
#[tauri::command]
pub fn reapply_rules(app: AppHandle, db: State<Db>) -> Result<usize, String> {
    let rules = crate::rules::load(&app);
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::harmonize::harmonize_all(&conn, &cfg, &rules).map_err(|e| e.to_string())
}
