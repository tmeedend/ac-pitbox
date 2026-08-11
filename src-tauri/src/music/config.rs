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
    /// Décochée par défaut : les deux ambiances jouent le pack embarqué
    /// (§16.1, `embedded_menu_dir`/`embedded_grid_dir`), aucun dossier à
    /// choisir. Cochée : `menu_folder`/`grid_folder` ci-dessous prennent le
    /// relais (repli sur `default_menu_dir`/`default_grid_dir`, vides, tant
    /// qu'aucun des deux n'est renseigné).
    pub use_custom_folders: bool,
    pub menu_folder: Option<PathBuf>,
    pub grid_folder: Option<PathBuf>,
    pub shuffle: bool,
    /// Volume global, 0.0–1.0.
    pub volume: f32,
    pub crossfade_ms: u32,
    pub fade_out_ms: u32,
    pub fade_in_ms: u32,
}

impl Default for MusicConfig {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            enabled: true,
            use_custom_folders: false,
            menu_folder: None,
            grid_folder: None,
            shuffle: true,
            volume: 0.45,
            crossfade_ms: 2500,
            fade_out_ms: 1500,
            fade_in_ms: 2000,
        }
    }
}

impl MusicConfig {
    pub fn effective_menu_folder(&self, app: &AppHandle) -> PathBuf {
        if self.use_custom_folders {
            self.menu_folder.clone().unwrap_or_else(|| default_menu_dir(app))
        } else {
            embedded_menu_dir(app)
        }
    }

    pub fn effective_grid_folder(&self, app: &AppHandle) -> PathBuf {
        if self.use_custom_folders {
            self.grid_folder.clone().unwrap_or_else(|| default_grid_dir(app))
        } else {
            embedded_grid_dir(app)
        }
    }
}

/// Dossier Musique de l'app (§3.1), sous le dossier de config — jamais le
/// dossier Musique de Windows, qui n'est qu'un point de départ pour le
/// sélecteur de dossier côté frontend (`open()` avec `defaultPath`).
fn music_root(app: &AppHandle) -> PathBuf {
    app.path().app_config_dir().unwrap_or_default().join("Music")
}

/// Repli des dossiers personnalisés (§3.1) tant qu'aucun n'a été choisi via
/// Parcourir — vides, juste un point de départ cohérent pour le sélecteur.
/// Distinct du pack embarqué ci-dessous : ce dossier-ci appartient à
/// l'utilisateur, jamais réécrit par l'app.
pub fn default_menu_dir(app: &AppHandle) -> PathBuf {
    music_root(app).join("menu")
}

pub fn default_grid_dir(app: &AppHandle) -> PathBuf {
    music_root(app).join("grid")
}

/// Pack par défaut embarqué dans le binaire (§16.1) : deux pistes sous
/// licence Pixabay Content License (usage libre, y compris redistribution,
/// voir `assets/music/CREDITS.md`), déposées via `include_bytes!` plutôt
/// qu'un dossier de ressources Tauri séparé — deux fichiers, pas besoin d'un
/// mécanisme de plus à maintenir.
const DEFAULT_MENU_TRACK: &[u8] = include_bytes!("../../assets/music/menu-ambience.mp3");
const DEFAULT_MENU_TRACK_NAME: &str = "menu-ambience.mp3";
const DEFAULT_GRID_TRACK: &[u8] = include_bytes!("../../assets/music/grid-ambience.mp3");
const DEFAULT_GRID_TRACK_NAME: &str = "grid-ambience.mp3";

/// Dossier entièrement piloté par l'app (jamais un endroit où l'utilisateur
/// est censé déposer ses propres fichiers) : contient la piste embarquée
/// correspondante, réécrite à chaque démarrage pour rester synchronisée avec
/// le binaire.
fn embedded_menu_dir(app: &AppHandle) -> PathBuf {
    music_root(app).join("embedded").join("menu")
}

fn embedded_grid_dir(app: &AppHandle) -> PathBuf {
    music_root(app).join("embedded").join("grid")
}

/// Crée les dossiers par défaut (§3.1, repli des dossiers personnalisés) et
/// dépose/rafraîchit le pack embarqué (§16.1) — best-effort, un échec
/// n'empêche pas l'app de démarrer.
pub fn ensure_default_dirs(app: &AppHandle) {
    for dir in [default_menu_dir(app), default_grid_dir(app)] {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::warn!("music: impossible de créer {} : {e}", dir.display());
        }
    }
    if let Err(e) = write_embedded_track(&embedded_menu_dir(app), DEFAULT_MENU_TRACK_NAME, DEFAULT_MENU_TRACK) {
        log::warn!("music: pack embarqué (menu) : {e}");
    }
    if let Err(e) = write_embedded_track(&embedded_grid_dir(app), DEFAULT_GRID_TRACK_NAME, DEFAULT_GRID_TRACK) {
        log::warn!("music: pack embarqué (grid) : {e}");
    }
}

/// Réécrit systématiquement (jamais de garde "si nouveau") : ce dossier n'est
/// jamais celui que l'utilisateur personnalise (voir `use_custom_folders`),
/// donc rien à préserver — et ça permet à une mise à jour de l'app de
/// remplacer une piste embarquée par une autre sans étape de migration.
fn write_embedded_track(dir: &std::path::Path, filename: &str, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(filename), bytes)
}

fn config_file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("music.json"))
}

pub fn load(app: &AppHandle) -> MusicConfig {
    let Some(path) = config_file(app) else {
        return MusicConfig::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(s) => parse_with_migration(&s),
        Err(_) => MusicConfig::default(),
    }
}

/// Migration silencieuse (§16.1) : un `music.json` écrit avant l'ajout du
/// pack embarqué n'a pas le champ `use_custom_folders` — `#[serde(default)]`
/// seul le ferait retomber à `false` et basculerait sans prévenir sur le
/// pack embarqué qui vient d'apparaître, alors qu'un dossier avait déjà été
/// choisi explicitement (seule façon de personnaliser avant cette version).
/// Distingue donc "champ absent" de "champ présent à `false`" en repassant
/// par `serde_json::Value` avant la désérialisation typée.
fn parse_with_migration(raw: &str) -> MusicConfig {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return MusicConfig::default();
    };
    let had_use_custom_folders_field = value.get("use_custom_folders").is_some();
    let mut cfg: MusicConfig = serde_json::from_value(value).unwrap_or_default();
    if !had_use_custom_folders_field && (cfg.menu_folder.is_some() || cfg.grid_folder.is_some()) {
        cfg.use_custom_folders = true;
    }
    cfg
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
        assert!(!cfg.use_custom_folders, "pack embarqué utilisé par défaut");
        assert!((cfg.volume - 0.45).abs() < f32::EPSILON);
        assert_eq!(cfg.crossfade_ms, 2500);
    }

    #[test]
    fn round_trips_through_json() {
        let cfg = MusicConfig {
            use_custom_folders: true,
            menu_folder: Some(PathBuf::from(r"D:\Music\menu")),
            ..MusicConfig::default()
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: MusicConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, back, "round-trip must preserve every field");
    }

    #[test]
    fn embedded_track_is_always_kept_in_sync_with_the_binary() {
        let base = crate::testutil::temp_dir("music-embedded-track");
        let dir = base.join("embedded-menu");

        write_embedded_track(&dir, "track.mp3", b"v1").unwrap();
        assert_eq!(std::fs::read(dir.join("track.mp3")).unwrap(), b"v1");

        // Une mise à jour de l'app change les octets compilés : le prochain
        // démarrage doit toujours réécrire, jamais préserver l'ancien fichier.
        write_embedded_track(&dir, "track.mp3", b"v2-longer").unwrap();
        assert_eq!(std::fs::read(dir.join("track.mp3")).unwrap(), b"v2-longer");
    }

    #[test]
    fn pre_embedded_pack_config_with_custom_folder_keeps_using_it() {
        // music.json écrit avant l'ajout de `use_custom_folders` (§16.1) : un
        // dossier déjà choisi ne doit pas basculer silencieusement sur le
        // pack embarqué qui vient d'apparaître.
        let cfg = parse_with_migration(r#"{"menu_folder": "D:\\Music\\menu"}"#);
        assert!(
            cfg.use_custom_folders,
            "dossier déjà choisi -> reste en mode personnalisé"
        );
        assert_eq!(cfg.menu_folder, Some(PathBuf::from(r"D:\Music\menu")));
    }

    #[test]
    fn config_without_any_prior_custom_folder_defaults_to_embedded_pack() {
        let cfg = parse_with_migration("{}");
        assert!(!cfg.use_custom_folders);
    }

    #[test]
    fn config_already_in_new_format_is_respected_verbatim() {
        // Le champ est déjà présent (fichier déjà écrit par cette version) :
        // pas de migration, même si un dossier personnalisé traîne encore —
        // l'utilisateur a pu décocher la case volontairement.
        let cfg = parse_with_migration(r#"{"use_custom_folders": false, "menu_folder": "D:\\Music\\menu"}"#);
        assert!(
            !cfg.use_custom_folders,
            "champ déjà présent -> valeur respectée telle quelle"
        );
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
