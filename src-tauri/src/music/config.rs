//! Configuration du module musique (§2 de la spec) : fichier séparé de
//! `config.json`, comme prescrit — l'ambiance musicale est un réglage à part
//! entière, pas un champ de plus dans `Prefs`.
//!
//! Écart assumé vs la spec : les dossiers sont stockés en chemins absolus
//! (`Option<PathBuf>`, `None` = dossier par défaut), pas en variables
//! d'environnement non résolues. La portabilité entre machines visée par la
//! spec avait du sens pour un `%APPDATA%\<AppName>` C# ; ici `app_config_dir()`
//! est déjà par-utilisateur et tous les autres chemins de `AppConfig`
//! (`config.rs`) suivent la même convention — mieux vaut rester cohérent avec
//! le reste du projet qu'avec une spec écrite pour une autre stack.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// Version du format de `music.json`. Sert de point de bascule pour une
/// future migration — aucune migration nécessaire tant qu'on reste en v1.
pub const CURRENT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MusicConfig {
    pub version: u32,
    /// Coupe-circuit global : si faux, entrer en Big Picture ne joue rien
    /// (§6, case à cocher "Activer la musique dans le mode Big Picture").
    pub enabled: bool,
    /// `None` = dossier par défaut (`default_menu_dir`), créé au premier
    /// démarrage.
    pub menu_folder: Option<PathBuf>,
    pub grid_folder: Option<PathBuf>,
    pub shuffle: bool,
    /// Volume global, 0.0–1.0.
    pub volume: f32,
    pub crossfade_ms: u32,
    pub fade_out_ms: u32,
    pub fade_in_ms: u32,
    /// "stop" | "duck" (§2).
    pub session_behavior: String,
    /// Volume cible en mode duck, 0.0–1.0 (valeur absolue, pas un facteur du
    /// volume principal — cf. l'exemple de la spec : 0.45 en temps normal,
    /// 0.12 en duck).
    pub session_duck_volume: f32,
}

impl Default for MusicConfig {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            enabled: true,
            menu_folder: None,
            grid_folder: None,
            shuffle: true,
            volume: 0.45,
            crossfade_ms: 2500,
            fade_out_ms: 1500,
            fade_in_ms: 2000,
            session_behavior: "stop".into(),
            session_duck_volume: 0.12,
        }
    }
}

impl MusicConfig {
    pub fn effective_menu_folder(&self, app: &AppHandle) -> PathBuf {
        self.menu_folder.clone().unwrap_or_else(|| default_menu_dir(app))
    }

    pub fn effective_grid_folder(&self, app: &AppHandle) -> PathBuf {
        self.grid_folder.clone().unwrap_or_else(|| default_grid_dir(app))
    }
}

/// Dossier Musique de l'app (§3.1), sous le dossier de config — jamais le
/// dossier Musique de Windows, qui n'est qu'un point de départ pour le
/// sélecteur de dossier côté frontend (`open()` avec `defaultPath`).
fn music_root(app: &AppHandle) -> PathBuf {
    app.path().app_config_dir().unwrap_or_default().join("Music")
}

pub fn default_menu_dir(app: &AppHandle) -> PathBuf {
    music_root(app).join("menu")
}

pub fn default_grid_dir(app: &AppHandle) -> PathBuf {
    music_root(app).join("grid")
}

/// Crée les dossiers par défaut s'ils n'existent pas encore (§3.1). Vides
/// pour l'instant (pas de pack CC0 embarqué, voir `mod.rs`) : best-effort,
/// un échec n'empêche pas l'app de démarrer.
pub fn ensure_default_dirs(app: &AppHandle) {
    for dir in [default_menu_dir(app), default_grid_dir(app)] {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::warn!("music: impossible de créer {} : {e}", dir.display());
        }
    }
}

fn config_file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("music.json"))
}

pub fn load(app: &AppHandle) -> MusicConfig {
    let Some(path) = config_file(app) else {
        return MusicConfig::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => MusicConfig::default(),
    }
}

pub fn save(app: &AppHandle, cfg: &MusicConfig) -> Result<(), String> {
    let path = config_file(app).ok_or("dossier de config indisponible")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("écriture music.json échouée : {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_matches_spec_values() {
        let cfg = MusicConfig::default();
        assert!(cfg.enabled);
        assert!(cfg.shuffle);
        assert_eq!(cfg.session_behavior, "stop");
        assert!((cfg.volume - 0.45).abs() < f32::EPSILON);
        assert_eq!(cfg.crossfade_ms, 2500);
    }

    #[test]
    fn round_trips_through_json() {
        let cfg = MusicConfig {
            menu_folder: Some(PathBuf::from(r"D:\Music\menu")),
            session_behavior: "duck".into(),
            ..MusicConfig::default()
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: MusicConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, back, "round-trip must preserve every field");
    }

    #[test]
    fn missing_fields_fall_back_to_defaults_serde_default() {
        // `#[serde(default)]` (§2, "règles de persistance") : un music.json
        // partiel (ancienne version, édition manuelle) ne doit pas planter,
        // les champs absents reprennent leur valeur par défaut.
        let partial: MusicConfig = serde_json::from_str(r#"{"enabled": false}"#).unwrap();
        assert!(!partial.enabled);
        assert!(partial.shuffle, "champ absent -> valeur par défaut");
        assert_eq!(partial.crossfade_ms, 2500);
    }
}
