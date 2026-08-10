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

/// Réglages de l'écran de session (§8.4/§8.6) : dernière sélection (véhicule,
/// circuit, type, adversaires) et presets par type de session. Même bug que
/// le duo voiture/circuit ci-dessus (`localStorage` pas fiable côté WebView2
/// à la fermeture) — même remède, fichier dédié à côté de `session.json` pour
/// ne pas faire écrire `save_session_picks` et `save_launch_state` dans le
/// même fichier (chaque commande réécrit tout le fichier, l'une écraserait
/// l'autre). Structure opaque côté Rust : le schéma appartient au frontend
/// (`Launch.svelte`), Rust ne fait que le faire transiter sur disque.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LaunchState {
    pub selection: Option<serde_json::Value>,
    pub presets: Option<serde_json::Value>,
}

fn launch_state_file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("launch_state.json"))
}

/// Vide si le fichier n'existe pas encore ou est illisible — premier
/// démarrage, ou fichier corrompu : jamais bloquant.
pub fn load_launch_state(app: &AppHandle) -> LaunchState {
    let Some(path) = launch_state_file(app) else {
        return LaunchState::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => LaunchState::default(),
    }
}

pub fn save_launch_state(app: &AppHandle, state: &LaunchState) -> Result<(), String> {
    let path = launch_state_file(app).ok_or("dossier de config indisponible")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("écriture launch_state.json échouée : {e}"))
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

    #[test]
    fn launch_state_round_trips_through_json() {
        let dir = crate::testutil::temp_dir("launch-state");
        let path = dir.join("launch_state.json");
        let state = LaunchState {
            selection: Some(serde_json::json!({ "session_type": "race", "car_id": "car_a" })),
            presets: Some(serde_json::json!({ "race": { "laps": 5 } })),
        };
        let json = serde_json::to_string_pretty(&state).unwrap();
        std::fs::write(&path, &json).unwrap();
        let back: LaunchState = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back.selection.unwrap()["session_type"], "race", "selection preserved verbatim");
        assert_eq!(back.presets.unwrap()["race"]["laps"], 5, "presets preserved verbatim");
    }

    #[test]
    fn launch_state_missing_fields_fall_back_to_defaults() {
        let state: LaunchState = serde_json::from_str("{}").unwrap();
        assert!(state.selection.is_none());
        assert!(state.presets.is_none());
    }
}
