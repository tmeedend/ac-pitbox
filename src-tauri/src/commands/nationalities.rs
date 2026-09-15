//! Liste des nationalités offertes par le jeu — voir `nationalities.rs`.

use super::prelude::*;

/// Jamais une erreur : sans installation Assetto Corsa lisible, la liste est
/// vide et la cellule du plateau retombe sur la saisie libre.
#[tauri::command]
pub fn nationalities(app: AppHandle) -> Vec<crate::nationalities::Nationality> {
    let cfg = crate::config::load(&app);
    match cfg.ac_install_path.as_deref() {
        Some(root) => crate::nationalities::nationalities(root),
        None => Vec::new(),
    }
}
