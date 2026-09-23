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

/// Writes the category family table (Categories tab, TAXO§6) without
/// re-harmonising anything — see `rules::save_category_families`.
#[tauri::command]
pub fn save_category_families(app: AppHandle, families: Vec<CategoryFamily>) -> Result<Vec<CategoryFamily>, String> {
    crate::rules::save_category_families(&app, families)
}

/// The shipped family table, for "restore" (TAXO§6.2).
#[tauri::command]
pub fn default_category_families() -> Vec<CategoryFamily> {
    crate::rules::default_rules().car.category_families
}

/// Writes the country aliases (Countries tab, TAXO§6) and re-applies them.
///
/// Unlike the families, this one DOES re-harmonise: the country is decided at
/// write time (`harmonize::store`), so an alias changes the value stored for
/// every mod that spells it. Returns the number of mods processed.
#[tauri::command]
pub fn save_country_aliases(app: AppHandle, db: State<Db>, aliases: CountryAliases) -> Result<usize, String> {
    let mut rules = crate::rules::load(&app);
    rules.country_aliases = crate::rules::normalize_country_aliases(aliases);
    crate::rules::save(&app, &rules)?;
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::harmonize::harmonize_all(&conn, &cfg, &rules).map_err(|e| e.to_string())
}

/// The shipped aliases, for "Restore".
#[tauri::command]
pub fn default_country_aliases() -> CountryAliases {
    crate::rules::default_rules().country_aliases
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
