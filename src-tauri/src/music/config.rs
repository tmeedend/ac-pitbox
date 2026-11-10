//! Configuration du module musique (MUSIQUE§2 de la spec) : fichier séparé de
//! `config.json`, comme prescrit — l'ambiance musicale est un réglage à part
//! entière, pas un champ de plus dans `Prefs`.
//!
//! Écart assumé vs la spec : les sources sont stockées en chemins absolus
//! (`Option<PathBuf>`, `None` = dossier par défaut), pas en variables
//! d'environnement non résolues. La portabilité entre machines visée par la
//! spec avait du sens pour un `%APPDATA%\<AppName>` C# ; ici `app_config_dir()`
//! est déjà par-utilisateur et tous les autres chemins de `AppConfig`
//! (`config.rs`) suivent la même convention — mieux vaut rester cohérent avec
//! le reste du projet qu'avec une spec écrite pour une autre stack.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// Version of the `music.json` format.
///
/// - v1: `enabled` (bool) and `volume`.
/// - v2: `play_mode` replaces `enabled`, `music_volume` replaces `volume` (a
///   future `sfx_volume` will sit next to it), and the contextual music
///   (`assignments`, §16.4) appears. See `parse_with_migration`.
pub const CURRENT_VERSION: u32 = 2;

/// How many sources the "Recent" list of a picker remembers.
pub const RECENT_MAX: usize = 8;

/// When music plays at all (§16.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayMode {
    Off,
    /// Only while Big Picture is open — the behaviour of v1, and the default.
    #[default]
    BigPicture,
    /// Also in windowed mode, paused while the window is minimised.
    Always,
}

/// Music attached to a term or a screen (§16.4), by **canonical id**: a brand
/// name, a family id, a track category tag (`#drift`), a country (the game's
/// English name) and a screen key (`cars`, `online`…). A merge of name
/// variants keeps them, since the variants all resolve to the same id.
///
/// A term that no longer exists keeps its assignment: the Music settings show
/// it as an orphan, and only an explicit removal drops it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Assignments {
    pub brand: BTreeMap<String, PathBuf>,
    pub family: BTreeMap<String, PathBuf>,
    pub track_category: BTreeMap<String, PathBuf>,
    pub country: BTreeMap<String, PathBuf>,
    pub screen: BTreeMap<String, PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MusicConfig {
    pub version: u32,
    pub play_mode: PlayMode,
    /// Décochée par défaut : les deux ambiances jouent le pack embarqué
    /// (§16.1, `embedded_menu_dir`/`embedded_grid_dir`), aucune source à
    /// choisir, et les attributions sont ignorées (conservées, jamais lues).
    /// Cochée : `menu_folder`/`grid_folder` ci-dessous prennent le relais
    /// (repli sur `default_menu_dir`/`default_grid_dir`, vides, tant qu'aucun
    /// des deux n'est renseigné).
    pub use_custom_folders: bool,
    /// A source: a folder (a playlist) or, since v2, a single file (looped).
    /// The field names predate files and are part of the stored format.
    pub menu_folder: Option<PathBuf>,
    pub grid_folder: Option<PathBuf>,
    pub shuffle: bool,
    /// Music volume, 0.0–1.0.
    pub music_volume: f32,
    pub crossfade_ms: u32,
    pub fade_out_ms: u32,
    pub fade_in_ms: u32,
    /// Now-playing notification (MUSIQUE§5.5). Off by default: a card popping
    /// up at every track is something one opts into, not something to turn off.
    pub show_now_playing: bool,
    pub assignments: Assignments,
    /// Sources picked last, most recent first, for the "Recent" group of the
    /// pickers (§16.8). Capped at `RECENT_MAX`.
    pub recent_sources: Vec<PathBuf>,
}

impl Default for MusicConfig {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            play_mode: PlayMode::default(),
            use_custom_folders: false,
            menu_folder: None,
            grid_folder: None,
            shuffle: true,
            music_volume: 0.45,
            crossfade_ms: 2500,
            fade_out_ms: 1500,
            fade_in_ms: 2000,
            show_now_playing: false,
            assignments: Assignments::default(),
            recent_sources: Vec::new(),
        }
    }
}

/// The two default ambiences: MENU everywhere, GRID on the session settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Base {
    Menu,
    Grid,
}

impl MusicConfig {
    pub fn effective_base(&self, app: &AppHandle, base: Base) -> PathBuf {
        let root = music_root(app);
        self.base_source(&root, base)
    }

    /// The source a base ambience plays, given the music root of the app.
    fn base_source(&self, root: &Path, base: Base) -> PathBuf {
        if !self.use_custom_folders {
            return root.join("embedded").join(base_dir_name(base));
        }
        let chosen = match base {
            Base::Menu => &self.menu_folder,
            Base::Grid => &self.grid_folder,
        };
        chosen.clone().unwrap_or_else(|| root.join(base_dir_name(base)))
    }

    /// What plays for a resolved context (§16.5): the assigned source when the
    /// user's own music is on and the front found one, the base ambience
    /// otherwise. **The switch is applied here, not only in the front**: with
    /// "Use my own music folders" unchecked, no assignment may ever reach the
    /// speakers, whatever a screen asked for.
    pub fn source_for(&self, root: &Path, assigned: Option<&Path>, base: Base) -> PathBuf {
        match assigned {
            Some(p) if self.use_custom_folders => p.to_path_buf(),
            _ => self.base_source(root, base),
        }
    }
}

fn base_dir_name(base: Base) -> &'static str {
    match base {
        Base::Menu => "menu",
        Base::Grid => "grid",
    }
}

/// Dossier Musique de l'app (MUSIQUE§3.1), sous le dossier de config — jamais le
/// dossier Musique de Windows, qui n'est qu'un point de départ pour le
/// sélecteur de dossier côté frontend (`open()` avec `defaultPath`).
pub fn music_root(app: &AppHandle) -> PathBuf {
    app.path().app_config_dir().unwrap_or_default().join("Music")
}

/// Where the index caches live (§16.3): in the app's own folder, never in the
/// user's music folders.
pub fn index_dir(app: &AppHandle) -> PathBuf {
    music_root(app).join("index")
}

/// Repli des dossiers personnalisés (MUSIQUE§3.1) tant qu'aucun n'a été choisi via
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

/// Crée les dossiers par défaut (MUSIQUE§3.1, repli des dossiers personnalisés) et
/// dépose/rafraîchit le pack embarqué (§16.1) — best-effort, un échec
/// n'empêche pas l'app de démarrer.
pub fn ensure_default_dirs(app: &AppHandle) {
    let root = music_root(app);
    for dir in [default_menu_dir(app), default_grid_dir(app), index_dir(app)] {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::warn!("music: impossible de créer {} : {e}", dir.display());
        }
    }
    let embedded = root.join("embedded");
    if let Err(e) = write_embedded_track(&embedded.join("menu"), DEFAULT_MENU_TRACK_NAME, DEFAULT_MENU_TRACK) {
        log::warn!("music: pack embarqué (menu) : {e}");
    }
    if let Err(e) = write_embedded_track(&embedded.join("grid"), DEFAULT_GRID_TRACK_NAME, DEFAULT_GRID_TRACK) {
        log::warn!("music: pack embarqué (grid) : {e}");
    }
}

/// Réécrit systématiquement (jamais de garde "si nouveau") : ce dossier n'est
/// jamais celui que l'utilisateur personnalise (voir `use_custom_folders`),
/// donc rien à préserver — et ça permet à une mise à jour de l'app de
/// remplacer une piste embarquée par une autre sans étape de migration.
fn write_embedded_track(dir: &Path, filename: &str, bytes: &[u8]) -> std::io::Result<()> {
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

/// Reads any `music.json` an earlier version wrote, through `serde_json::Value`
/// so that "field absent" and "field present" can be told apart before the
/// typed parse — `#[serde(default)]` alone would hide the difference.
///
/// Two migrations, applied in order:
///
/// 1. **Before `use_custom_folders`** (§16.1): a file with no such field but a
///    folder already chosen keeps playing that folder. Defaulting the field to
///    `false` would silently switch to the embedded pack that had just
///    appeared, while choosing a folder was then the only way to customise.
/// 2. **v1 → v2**: `enabled: true` becomes `play_mode: "big_picture"` (what it
///    meant: Big Picture was the only place music played), `false` becomes
///    `"off"`; `volume` becomes `music_volume`. The old keys are left out of
///    the rewritten file — the struct no longer has them.
fn parse_with_migration(raw: &str) -> MusicConfig {
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return MusicConfig::default();
    };
    let had_use_custom_folders_field = value.get("use_custom_folders").is_some();
    let version = value.get("version").and_then(|v| v.as_u64()).unwrap_or(1);
    if version < 2 {
        if let Some(obj) = value.as_object_mut() {
            if let Some(enabled) = obj.remove("enabled").and_then(|v| v.as_bool()) {
                let mode = if enabled { "big_picture" } else { "off" };
                obj.entry("play_mode").or_insert_with(|| mode.into());
            }
            if let Some(volume) = obj.remove("volume") {
                obj.entry("music_volume").or_insert(volume);
            }
        }
    }
    let mut cfg: MusicConfig = serde_json::from_value(value).unwrap_or_default();
    if !had_use_custom_folders_field && (cfg.menu_folder.is_some() || cfg.grid_folder.is_some()) {
        cfg.use_custom_folders = true;
    }
    cfg.version = CURRENT_VERSION;
    cfg
}

pub fn save(app: &AppHandle, cfg: &MusicConfig) -> Result<(), String> {
    let path = config_file(app).ok_or("dossier de config indisponible")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut cfg = cfg.clone();
    cfg.version = CURRENT_VERSION;
    cfg.recent_sources.truncate(RECENT_MAX);
    let json = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("écriture music.json échouée : {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_matches_spec_values() {
        let cfg = MusicConfig::default();
        assert_eq!(cfg.play_mode, PlayMode::BigPicture, "Big Picture only by default");
        assert!(cfg.shuffle);
        assert!(!cfg.use_custom_folders, "pack embarqué utilisé par défaut");
        assert!((cfg.music_volume - 0.45).abs() < f32::EPSILON);
        assert_eq!(cfg.crossfade_ms, 2500);
        assert!(!cfg.show_now_playing, "now-playing notification off by default");
        assert_eq!(cfg.assignments, Assignments::default());
    }

    #[test]
    fn round_trips_through_json() {
        let mut cfg = MusicConfig {
            use_custom_folders: true,
            menu_folder: Some(PathBuf::from(r"D:\Music\menu")),
            play_mode: PlayMode::Always,
            ..MusicConfig::default()
        };
        cfg.assignments
            .brand
            .insert("Porsche".into(), PathBuf::from(r"D:\Music\flat-six.mp3"));
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(
            json.contains(r#""play_mode":"always""#),
            "mode stored in snake case: {json}"
        );
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

    // Rule (§16.1, music.json v1 → v2): the old on/off switch becomes a play
    // mode — on meant "in Big Picture", the only place music used to play —
    // and the volume moves to `music_volume`. Read from a file exactly as v1
    // wrote it, every other field kept.
    #[test]
    fn a_v1_file_migrates_to_play_mode_and_music_volume() {
        let v1_on = r#"{
            "version": 1,
            "enabled": true,
            "use_custom_folders": true,
            "menu_folder": "D:\\Music\\menu",
            "grid_folder": null,
            "shuffle": false,
            "volume": 0.7,
            "crossfade_ms": 3000,
            "fade_out_ms": 1500,
            "fade_in_ms": 2000,
            "show_now_playing": true
        }"#;
        let cfg = parse_with_migration(v1_on);
        assert_eq!(cfg.play_mode, PlayMode::BigPicture, "enabled: true -> Big Picture only");
        assert!((cfg.music_volume - 0.7).abs() < 1e-6, "volume carried to music_volume");
        assert_eq!(cfg.version, CURRENT_VERSION, "version bumped");
        assert!(
            !cfg.shuffle && cfg.show_now_playing && cfg.crossfade_ms == 3000,
            "other fields kept"
        );
        assert_eq!(cfg.menu_folder, Some(PathBuf::from(r"D:\Music\menu")));
        assert_eq!(cfg.assignments, Assignments::default(), "no assignment invented");

        let v1_off = parse_with_migration(r#"{"version": 1, "enabled": false, "volume": 0.2}"#);
        assert_eq!(v1_off.play_mode, PlayMode::Off, "enabled: false -> off");
        assert!((v1_off.music_volume - 0.2).abs() < 1e-6);

        // A v1 file with no version at all (the field was defaulted).
        let unversioned = parse_with_migration(r#"{"enabled": false}"#);
        assert_eq!(unversioned.play_mode, PlayMode::Off, "a file without version is v1");

        let rewritten = serde_json::to_string(&cfg).unwrap();
        assert!(
            !rewritten.contains("\"enabled\"") && !rewritten.contains("\"volume\""),
            "old keys are not written back: {rewritten}"
        );
    }

    // A v2 file is read as is: `enabled`/`volume` keys left there by hand do
    // not override what v2 says.
    #[test]
    fn a_v2_file_is_not_migrated_again() {
        let cfg =
            parse_with_migration(r#"{"version": 2, "play_mode": "always", "enabled": false, "music_volume": 0.3}"#);
        assert_eq!(cfg.play_mode, PlayMode::Always);
        assert!((cfg.music_volume - 0.3).abs() < 1e-6);
    }

    #[test]
    fn missing_fields_fall_back_to_defaults_serde_default() {
        // `#[serde(default)]` (MUSIQUE§2, "règles de persistance") : un music.json
        // partiel (ancienne version, édition manuelle) ne doit pas planter,
        // les champs absents reprennent leur valeur par défaut.
        let partial: MusicConfig = serde_json::from_str(r#"{"play_mode": "off"}"#).unwrap();
        assert_eq!(partial.play_mode, PlayMode::Off);
        assert!(partial.shuffle, "champ absent -> valeur par défaut");
        assert_eq!(partial.crossfade_ms, 2500);
    }

    // Rule (§16.4): with "Use my own music folders" unchecked, assignments are
    // kept but never played — the base ambience plays, embedded pack included.
    #[test]
    fn assignments_only_play_with_the_users_own_music_on() {
        let root = Path::new(r"C:\cfg\Music");
        let assigned = Path::new(r"D:\Music\drift");
        let off = MusicConfig::default();
        assert_eq!(
            off.source_for(root, Some(assigned), Base::Grid),
            root.join("embedded").join("grid"),
            "switch off: the embedded GRID, whatever was assigned"
        );
        let on = MusicConfig {
            use_custom_folders: true,
            ..MusicConfig::default()
        };
        assert_eq!(on.source_for(root, Some(assigned), Base::Grid), assigned);
        assert_eq!(
            on.source_for(root, None, Base::Menu),
            root.join("menu"),
            "nothing assigned: the user's MENU, its default folder while unset"
        );
    }
}
