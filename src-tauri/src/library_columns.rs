//! Persistance des colonnes de la bibliothèque (visibilité + ordre, §6.2) —
//! fichier dédié (`library_columns.json`), écriture synchrone. Même raison
//! que `session_state.rs`/`saved_sessions.rs` : `localStorage` n'est pas
//! garanti synchrone sur disque côté WebView2. Structure opaque côté Rust :
//! le schéma (une entrée par type de mod, `{visible, order}`) appartient au
//! frontend (`columns.ts`).

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("library_columns.json"))
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

pub fn save(app: &AppHandle, prefs: &serde_json::Value) -> Result<(), String> {
    let path = file(app).ok_or("dossier de config indisponible")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(prefs).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("écriture library_columns.json échouée : {e}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn round_trips_through_json() {
        let dir = crate::testutil::temp_dir("library-columns");
        let path = dir.join("library_columns.json");
        let prefs = serde_json::json!({
            "cars": { "visible": ["name", "brand"], "order": ["name", "brand", "year"] },
        });
        let json = serde_json::to_string_pretty(&prefs).unwrap();
        std::fs::write(&path, &json).unwrap();
        let back: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back["cars"]["order"][2], "year");
    }

    #[test]
    fn malformed_json_falls_back_to_empty_object() {
        let value: serde_json::Value = serde_json::from_str("{not json").unwrap_or_else(|_| serde_json::json!({}));
        assert!(value.as_object().unwrap().is_empty());
    }
}
