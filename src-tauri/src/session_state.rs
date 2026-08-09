//! Persistance du duo de session (voiture/circuit choisis, §8.6).
//!
//! Fichier dédié plutôt que le `localStorage` du webview : ses écritures ne
//! sont pas garanties synchrones sur disque côté WebView2 (le moteur
//! Chromium sous-jacent bufferise et flush périodiquement) — fermer l'app
//! peu après un clic peut perdre la sélection la plus récente. Bug réel
//! constaté : le circuit ne survivait quasiment jamais à un redémarrage
//! (choisi juste avant de fermer), contrairement à la voiture (choisie plus
//! tôt dans la session, le temps d'être vidangée sur disque). `std::fs::write`
//! est synchrone : la commande ne rend la main au frontend qu'une fois
//! réellement écrit.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPick {
    pub id: String,
    pub name: String,
    pub meta: String,
    pub preview: Option<String>,
    pub layout: Option<String>,
    pub skin: Option<String>,
    pub outline: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SessionPicks {
    pub car: Option<SessionPick>,
    pub track: Option<SessionPick>,
}

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("session.json"))
}

/// Vide (`SessionPicks::default()`) si le fichier n'existe pas encore ou est
/// illisible — premier démarrage, ou fichier corrompu : jamais bloquant.
pub fn load(app: &AppHandle) -> SessionPicks {
    let Some(path) = file(app) else {
        return SessionPicks::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => SessionPicks::default(),
    }
}

pub fn save(app: &AppHandle, picks: &SessionPicks) -> Result<(), String> {
    let path = file(app).ok_or("dossier de config indisponible")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(picks).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("écriture session.json échouée : {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let dir = crate::testutil::temp_dir("session-state");
        let path = dir.join("session.json");
        let picks = SessionPicks {
            car: Some(SessionPick {
                id: "car_a".into(),
                name: "Car A".into(),
                meta: "brand".into(),
                preview: Some(r"C:\preview.png".into()),
                layout: None,
                skin: Some("red".into()),
                outline: None,
            }),
            track: None,
        };
        let json = serde_json::to_string_pretty(&picks).unwrap();
        std::fs::write(&path, &json).unwrap();
        let back: SessionPicks = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back.car.unwrap().id, "car_a");
        assert!(back.track.is_none());
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let picks: SessionPicks = serde_json::from_str("{}").unwrap();
        assert!(picks.car.is_none());
        assert!(picks.track.is_none());
    }
}
