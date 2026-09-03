//! Commandes des réglages d'interface (§6.2/§8.6) — voir `ui_prefs.rs`.
//!
//! **`async` + `spawn_blocking`, comme tout ce qui touche au disque** (§6.3bis).
//! Elles étaient synchrones, donc exécutées sur le thread principal, et c'est
//! la piste la plus solide pour le blocage jamais élucidé que documente
//! `invokeSafe.ts` (« écran bibliothèque bloqué au chargement, cause exacte non
//! identifiée »). Le symptôme le plus coûteux n'était pas le blocage mais sa
//! rustine : un repli silencieux au bout de cinq secondes, qui transformait une
//! commande lente en **réglage perdu sans un mot** — exactement ce que la règle
//! d'or n°6 cherche à empêcher. Constaté sur cette machine : `ui_prefs.json`
//! inchangé pendant six heures d'utilisation, tenues et corps de pilote adoptés
//! puis disparus au redémarrage.
//!
//! L'écriture reste **synchrone à l'intérieur** de la tâche bloquante
//! (`std::fs::write`) : la commande ne rend la main que quand c'est réellement
//! sur disque, ce qui est tout l'intérêt du fichier par rapport à
//! `localStorage`.

use super::prelude::*;

#[tauri::command]
pub async fn get_ui_prefs(app: AppHandle) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || crate::ui_prefs::load(&app))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_ui_prefs(app: AppHandle, prefs: serde_json::Value) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::ui_prefs::save(&app, &prefs))
        .await
        .map_err(|e| e.to_string())?
}
