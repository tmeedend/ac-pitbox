//! Persistance des sessions de lancement sauvegardées par l'utilisateur
//! (§8.4bis) — fichier dédié (`saved_sessions.json`), écriture synchrone.
//! Même bug que le duo de session et les presets (§8.6, voir
//! `session_state.rs`) : `localStorage` n'est pas garanti synchrone sur
//! disque côté WebView2, ce qui perdait une sauvegarde nommée à la fermeture
//! de l'app plutôt qu'au clic sur Sauvegarder/Retirer. Structure opaque côté
//! Rust : le schéma (`SavedSession`, clé `<type>::<nom>`) appartient au
//! frontend (`savedSessions.ts`).

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("saved_sessions.json"))
}

/// Objet vide si le fichier n'existe pas encore ou est illisible — premier
/// démarrage, ou fichier corrompu : jamais bloquant.
pub fn load(app: &AppHandle) -> serde_json::Value {
    let Some(path) = file(app) else {
        return serde_json::json!({});
    };
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|_| serde_json::json!({})),
        Err(_) => serde_json::json!({}),
    }
}

pub fn save(app: &AppHandle, all: &serde_json::Value) -> Result<(), String> {
    let path = file(app).ok_or("dossier de config indisponible")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(all).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("écriture saved_sessions.json échouée : {e}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn round_trips_through_json() {
        let dir = crate::testutil::temp_dir("saved-sessions");
        let path = dir.join("saved_sessions.json");
        let all = serde_json::json!({
            "race::My Grid": { "name": "My Grid", "savedAt": "2026-01-01T00:00:00.000Z" },
        });
        let json = serde_json::to_string_pretty(&all).unwrap();
        std::fs::write(&path, &json).unwrap();
        let back: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back["race::My Grid"]["name"], "My Grid");
    }

    #[test]
    fn malformed_json_falls_back_to_empty_object() {
        // Même repli que `load()` (fichier corrompu, ou d'un futur format
        // incompatible) : jamais bloquant, jamais de panique.
        let value: serde_json::Value = serde_json::from_str("{not json").unwrap_or_else(|_| serde_json::json!({}));
        assert!(value.as_object().unwrap().is_empty());
    }
}
