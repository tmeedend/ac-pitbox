//! Commandes de l'import des presets Content Manager (§6) — voir `cmimport.rs`.

// Pas de `prelude` ici : le parcours ne touche ni à la config ni à la base,
// il ne lit que le dossier de Content Manager.

/// Parcourt les presets de CM. Jamais une erreur : CM non installé rend un
/// scan vide, ce qui est un non-résultat et pas une panne (§6.1).
#[tauri::command]
pub fn scan_cm_presets() -> crate::cmimport::CmScan {
    crate::cmimport::scan()
}
