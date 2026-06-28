//! Lancement de session (L4) — §8.3 résolu et validé : on construit un `race.ini`
//! et on le passe à Content Manager via `acmanager://race/config?configFile=…`.
//! CM l'utilise tel quel (`PreparedConfig`) sans l'écraser, et gère Steam.
//! Voir `docs/L4-cm-launch-research.md`.

use std::fmt::Write as _;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use rusqlite::Connection;
use serde::Deserialize;
use uuid::Uuid;

use crate::activation;
use crate::config::AppConfig;
use crate::modscan::ModKind;
use crate::overlay;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SessionType {
    Practice,
    Hotlap,
    Race,
}

impl SessionType {
    /// (TYPE numérique AC, NAME, SPAWN_SET).
    fn ac(self) -> (u8, &'static str, &'static str) {
        match self {
            SessionType::Practice => (1, "Practice", "PIT"),
            SessionType::Hotlap => (4, "Hotlap", "HOTLAP_START"),
            SessionType::Race => (3, "Race", "START"),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RaceSetup {
    pub car_id: String,
    pub car_skin: Option<String>,
    pub track_id: String,
    pub track_layout: Option<String>,
    pub session_type: SessionType,
    /// Adversaires IA (mode course uniquement).
    #[serde(default)]
    pub ai_count: u32,
    #[serde(default = "default_ai_level")]
    pub ai_level: u32,
    #[serde(default)]
    pub laps: u32,
    #[serde(default = "default_duration")]
    pub duration_minutes: u32,
    /// Nom du dossier météo (ex. "3_clear").
    #[serde(default)]
    pub weather: String,
    /// Heure du jour (0-24).
    #[serde(default = "default_time")]
    pub time_hours: f32,
    /// Températures implicites (air/piste) calculées côté météo (§8.5).
    #[serde(default)]
    pub ambient_c: Option<i32>,
    #[serde(default)]
    pub road_c: Option<i32>,
    // --- Options de course (§8.6, bloc repliable) ---
    #[serde(default)]
    pub penalties: bool,
    /// Faux départ : 0 = aucune, 1 = téléport, 2 = drive-through.
    #[serde(default)]
    pub jump_start_penalty: u32,
    /// Évolution du grip : DYNAMIC_TRACK SESSION_START (86 vert … 100 optimal).
    #[serde(default = "default_grip")]
    pub grip: u32,
    /// Qualification avant la course (mode course uniquement).
    #[serde(default)]
    pub qualifying: bool,
    #[serde(default = "default_qualify_minutes")]
    pub qualify_minutes: u32,
    // --- Réglages dépendants du type (§8.4) ---
    /// Ghost car (Hotlap uniquement) → [GHOST_CAR] du race.ini.
    #[serde(default)]
    pub ghost_car: bool,
    /// Dégâts/usure/carburant (Course) → assists.ini (pas race.ini). En %.
    #[serde(default = "default_damage")]
    pub damage: u32,
    #[serde(default = "default_rate")]
    pub fuel_rate: u32,
    #[serde(default = "default_rate")]
    pub tyre_wear: u32,
}

fn default_ai_level() -> u32 {
    96
}
fn default_duration() -> u32 {
    15
}
fn default_time() -> f32 {
    13.0
}
fn default_grip() -> u32 {
    96
}
fn default_qualify_minutes() -> u32 {
    10
}
fn default_damage() -> u32 {
    50
}
fn default_rate() -> u32 {
    100
}

/// Angle solaire approximatif depuis l'heure (CM affine via ses helpers lighting).
fn sun_angle(time_hours: f32) -> f32 {
    (16.0 * (time_hours - 13.0)).clamp(-80.0, 80.0)
}

/// Construit le contenu d'un `race.ini` (structure dérivée d'un vrai fichier CM).
pub fn build_race_ini(s: &RaceSetup) -> String {
    let (session_type, session_name, spawn_set) = s.session_type.ac();
    let cars_total = if s.session_type == SessionType::Race {
        1 + s.ai_count
    } else {
        1
    };
    let weather = if s.weather.is_empty() { "3_clear" } else { &s.weather };

    let mut ini = String::new();
    let _ = write!(
        ini,
        "[HEADER]\nVERSION=2\n__CM_FEATURE_SET=2\n\n\
         [RACE]\nMODEL={car}\nSKIN={skin}\nTRACK={track}\nCONFIG_TRACK={layout}\n\
         AI_LEVEL={ai_level}\nCARS={cars}\nDRIFT_MODE=0\nRACE_LAPS={laps}\n\
         FIXED_SETUP=0\nPENALTIES={pen}\nJUMP_START_PENALTY={jsp}\n\n\
         [OPTIONS]\nUSE_MPH=0\n\n\
         [CAR_0]\nSETUP=\nSKIN={skin}\nMODEL=-\nMODEL_CONFIG=\nBALLAST=0\nRESTRICTOR=0\n\
         DRIVER_NAME=Player\nNATIONALITY=FRA\nNATION_CODE=FRA\n\n",
        car = s.car_id,
        skin = s.car_skin.clone().unwrap_or_default(),
        track = s.track_id,
        layout = s.track_layout.clone().unwrap_or_default(),
        ai_level = s.ai_level,
        cars = cars_total,
        laps = if s.session_type == SessionType::Race { s.laps } else { 0 },
        pen = if s.penalties { 1 } else { 0 },
        jsp = s.jump_start_penalty,
    );

    // Grille IA (course) : course one-make minimale (même modèle).
    if s.session_type == SessionType::Race {
        for i in 1..=s.ai_count {
            let _ = write!(
                ini,
                "[CAR_{i}]\nSETUP=\nSKIN=\nMODEL={car}\nMODEL_CONFIG=\nBALLAST=0\nRESTRICTOR=0\n\
                 DRIVER_NAME=AI {i}\nNATIONALITY=FRA\nNATION_CODE=FRA\nAI_LEVEL={ai}\n\n",
                car = s.car_id,
                ai = s.ai_level,
            );
        }
    }

    // Sessions : qualif optionnelle avant la course, sinon session unique.
    if s.session_type == SessionType::Race && s.qualifying {
        let _ = write!(
            ini,
            "[SESSION_0]\nNAME=Qualify\nTYPE=2\nDURATION_MINUTES={q}\nSPAWN_SET=PIT\n\n\
             [SESSION_1]\nNAME=Race\nTYPE=3\nDURATION_MINUTES=0\nSPAWN_SET=START\n\n",
            q = s.qualify_minutes,
        );
    } else {
        let dur = if s.session_type == SessionType::Hotlap { 0 } else { s.duration_minutes };
        let _ = write!(
            ini,
            "[SESSION_0]\nNAME={session_name}\nTYPE={session_type}\nDURATION_MINUTES={dur}\nSPAWN_SET={spawn_set}\n\n",
        );
    }

    let _ = write!(
        ini,
        "[LIGHTING]\nSUN_ANGLE={sun:.2}\nTIME_MULT=1.0\nCLOUD_SPEED=0.2\n\n\
         [WEATHER]\nNAME={weather}\n\n\
         [TEMPERATURE]\nAMBIENT={ambient}\nROAD={road}\n\n\
         [WIND]\nSPEED_KMH_MIN=0\nSPEED_KMH_MAX=0\nDIRECTION_DEG=0\n\n\
         [DYNAMIC_TRACK]\nSESSION_START={grip}\nRANDOMNESS=2\nLAP_GAIN=30\nSESSION_TRANSFER=80\n\n\
         [GROOVE]\nVIRTUAL_LAPS=10\nMAX_LAPS=1\nSTARTING_LAPS=1\n\n\
         [LAP_INVALIDATOR]\nALLOWED_TYRES_OUT=-1\n\n\
         [GHOST_CAR]\nRECORDING={g}\nPLAYING={g}\nLOAD={g}\nENABLED={g}\nSECONDS_ADVANTAGE=0\nFILE=\n",
        sun = sun_angle(s.time_hours),
        weather = weather,
        ambient = s.ambient_c.unwrap_or(26),
        road = s.road_c.unwrap_or(30),
        grip = s.grip,
        // Ghost car pertinent uniquement en Hotlap.
        g = if s.ghost_car && s.session_type == SessionType::Hotlap { 1 } else { 0 },
    );

    ini
}

/// Applique dégâts / carburant / usure (en %) dans `assists.ini` (Documents AC),
/// en préservant les autres réglages (ABS, TC…). Best-effort : ces réglages
/// vivent dans assists.ini et non dans race.ini (cf. Game.Properties.cs de CM).
fn apply_gameplay(damage: u32, fuel_rate: u32, tyre_wear: u32) {
    let Ok(profile) = std::env::var("USERPROFILE") else {
        return;
    };
    let path = std::path::Path::new(&profile)
        .join("Documents")
        .join("Assetto Corsa")
        .join("cfg")
        .join("assists.ini");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let mut out = String::new();
    for line in text.lines() {
        let t = line.trim_start();
        if t.starts_with("DAMAGE=") {
            out.push_str(&format!("DAMAGE={damage}"));
        } else if t.starts_with("FUEL_RATE=") {
            out.push_str(&format!("FUEL_RATE={fuel_rate}"));
        } else if t.starts_with("TYRE_WEAR=") {
            out.push_str(&format!("TYRE_WEAR={tyre_wear}"));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    let _ = std::fs::write(&path, out);
}

/// Garantit qu'un contenu est disponible dans `content/` : présent (vrai dossier
/// ou junction) → OK ; sinon présent dans la bibliothèque → on l'active ; sinon
/// erreur.
fn ensure_available(conn: &Connection, cfg: &AppConfig, kind: ModKind, id: &str) -> Result<(), String> {
    let ac = cfg.ac_install_path.as_ref().ok_or("dossier AC non configuré")?;
    let content = ac.join("content").join(kind.content_folder()).join(id);
    if content.exists() {
        return Ok(());
    }
    if overlay::mod_exists(conn, id).map_err(|e| e.to_string())? {
        return activation::activate(conn, cfg, id, None);
    }
    Err(format!("« {id} » n'est pas installé dans Assetto Corsa."))
}

/// Lance la session : active le contenu au besoin, écrit le race.ini, invoque CM.
pub fn launch(conn: &Connection, cfg: &AppConfig, setup: &RaceSetup) -> Result<(), String> {
    let cm = cfg
        .content_manager_exe
        .as_ref()
        .ok_or("Content Manager non configuré")?;

    ensure_available(conn, cfg, ModKind::Car, &setup.car_id)?;
    ensure_available(conn, cfg, ModKind::Track, &setup.track_id)?;

    // Dégâts/usure/carburant (Course) vivent dans assists.ini.
    if setup.session_type == SessionType::Race {
        apply_gameplay(setup.damage, setup.fuel_rate, setup.tyre_wear);
    }

    let ini = build_race_ini(setup);
    let path = std::env::temp_dir().join(format!("pitbox-race-{}.ini", Uuid::new_v4()));
    std::fs::write(&path, ini).map_err(|e| format!("écriture race.ini : {e}"))?;

    let uri = format!("acmanager://race/config?configFile={}", path.display());
    let mut cmd = Command::new(cm);
    cmd.arg(&uri);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.spawn().map_err(|e| format!("lancement de Content Manager : {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> RaceSetup {
        RaceSetup {
            car_id: "ks_toyota_gt86".into(),
            car_skin: None,
            track_id: "magione".into(),
            track_layout: None,
            session_type: SessionType::Practice,
            ai_count: 0,
            ai_level: 96,
            laps: 0,
            duration_minutes: 15,
            weather: "3_clear".into(),
            time_hours: 13.0,
            ambient_c: None,
            road_c: None,
            penalties: false,
            jump_start_penalty: 0,
            grip: 96,
            qualifying: false,
            qualify_minutes: 10,
            ghost_car: false,
            damage: 50,
            fuel_rate: 100,
            tyre_wear: 100,
        }
    }

    #[test]
    fn practice_ini_structure() {
        let ini = build_race_ini(&base());
        assert!(ini.contains("[HEADER]\nVERSION=2\n__CM_FEATURE_SET=2"));
        assert!(ini.contains("MODEL=ks_toyota_gt86"));
        assert!(ini.contains("TRACK=magione"));
        assert!(ini.contains("CARS=1"));
        assert!(ini.contains("[SESSION_0]\nNAME=Practice\nTYPE=1"));
        assert!(ini.contains("SPAWN_SET=PIT"));
        assert!(ini.contains("NAME=3_clear"));
        assert!(!ini.contains("[CAR_1]")); // pas de grille IA hors course
    }

    #[test]
    fn race_ini_has_ai_grid() {
        let mut s = base();
        s.session_type = SessionType::Race;
        s.ai_count = 3;
        s.laps = 5;
        let ini = build_race_ini(&s);
        assert!(ini.contains("TYPE=3")); // Race
        assert!(ini.contains("CARS=4")); // 1 joueur + 3 IA
        assert!(ini.contains("RACE_LAPS=5"));
        assert!(ini.contains("[CAR_1]"));
        assert!(ini.contains("[CAR_3]"));
        assert!(!ini.contains("[CAR_4]"));
        assert!(ini.contains("DRIVER_NAME=AI 3"));
    }

    #[test]
    fn race_with_qualifying_has_two_sessions() {
        let mut s = base();
        s.session_type = SessionType::Race;
        s.qualifying = true;
        s.qualify_minutes = 12;
        s.grip = 88;
        let ini = build_race_ini(&s);
        assert!(ini.contains("[SESSION_0]\nNAME=Qualify\nTYPE=2\nDURATION_MINUTES=12"));
        assert!(ini.contains("[SESSION_1]\nNAME=Race\nTYPE=3"));
        assert!(ini.contains("SESSION_START=88"));
    }

    #[test]
    fn hotlap_is_single_car_no_duration() {
        let mut s = base();
        s.session_type = SessionType::Hotlap;
        let ini = build_race_ini(&s);
        assert!(ini.contains("TYPE=4"));
        assert!(ini.contains("SPAWN_SET=HOTLAP_START"));
        assert!(ini.contains("DURATION_MINUTES=0"));
    }

    #[test]
    fn ghost_car_only_when_hotlap() {
        // Ghost activé en Hotlap → ENABLED=1.
        let mut s = base();
        s.session_type = SessionType::Hotlap;
        s.ghost_car = true;
        assert!(build_race_ini(&s).contains("[GHOST_CAR]\nRECORDING=1\nPLAYING=1\nLOAD=1\nENABLED=1"));

        // Ghost activé mais en Practice → ignoré (ENABLED=0).
        let mut p = base();
        p.session_type = SessionType::Practice;
        p.ghost_car = true;
        assert!(build_race_ini(&p).contains("[GHOST_CAR]\nRECORDING=0\nPLAYING=0\nLOAD=0\nENABLED=0"));
    }
}
