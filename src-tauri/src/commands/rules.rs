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
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::harmonize::harmonize_all(&conn, &rules).map_err(|e| e.to_string())
}

/// Aperçu d'impact : nombre de mods affectés par un jeu de règles candidat,
/// sans rien enregistrer (§5.4).
#[tauri::command]
pub fn rules_impact(db: State<Db>, rules: Rules) -> Result<usize, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::harmonize::count_affected(&conn, &rules).map_err(|e| e.to_string())
}

/// Réapplique les règles enregistrées à toute la bibliothèque.
#[tauri::command]
pub fn reapply_rules(app: AppHandle, db: State<Db>) -> Result<usize, String> {
    let rules = crate::rules::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::harmonize::harmonize_all(&conn, &rules).map_err(|e| e.to_string())
}
