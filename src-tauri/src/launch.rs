//! Lancement de session (L4/§8.3) : on construit un **preset Quick Drive**
//! (`quickdrive.rs`) et on le passe à Content Manager via
//! `acmanager://race/quick?presetFile=…`. Remplace l'ancien mécanisme
//! `race/config`/`PreparedConfig` (race.ini) : lui seul déclenche le chemin
//! `QuickDrive.ViewModel.Go()` côté CM, qui peuple correctement
//! `StartProperties.BasicProperties` — condition pour que le téléchargement
//! CSP automatique (VAO/config manquants) se déclenche. Voir
//! `docs/L4-cm-launch-research.md`.

use std::path::Path;
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

/// Un adversaire du plateau (mode course, §8.6) : voiture + son propre niveau
/// IA (réparti dans la fourchette min-max choisie, pas une valeur unique).
#[derive(Debug, Clone, Deserialize)]
pub struct Opponent {
    pub car_id: String,
    pub ai_level: u32,
    /// Skin de l'adversaire (§8.6 : plateau réglable finement depuis la popup
    /// de sélection). `None` → le jeu applique son skin par défaut.
    #[serde(default)]
    pub car_skin: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RaceSetup {
    pub car_id: String,
    /// Skin du joueur : pas de champ dédié dans le schéma Quick Drive
    /// (`race/quick`), CM retombe sur le dernier skin utilisé pour cette
    /// voiture. Conservé pour l'aller-retour front, non lu ici.
    #[allow(dead_code)]
    pub car_skin: Option<String>,
    pub track_id: String,
    pub track_layout: Option<String>,
    pub session_type: SessionType,
    /// Plateau d'adversaires (mode course uniquement) — généré par type de
    /// plateau côté front (même voiture/catégorie/ère/libre), puis ajustable.
    #[serde(default)]
    pub opponents: Vec<Opponent>,
    /// Fourchette de niveau IA (comme CM) : le plateau est réparti dedans,
    /// pas une valeur unique — plateau plus vivant. `ai_level_min` n'est pas
    /// lu ici : la répartition se fait côté front, chaque `Opponent` porte
    /// déjà son propre niveau ; le champ ne fait que l'aller-retour.
    #[serde(default = "default_ai_level_min")]
    #[allow(dead_code)]
    pub ai_level_min: u32,
    #[serde(default = "default_ai_level_max")]
    #[allow(dead_code)]
    pub ai_level_max: u32,
    #[serde(default)]
    pub laps: u32,
    /// Durée (Practice) : le schéma Quick Drive `QuickDrive_Practice.xaml`
    /// n'a pas de champ durée dans son `ModeData` — session à durée libre.
    /// Conservé pour compat aller-retour front, non lu ici.
    #[serde(default = "default_duration")]
    #[allow(dead_code)]
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
    /// Vent implicite (idem température) : déduit de la météo + heure.
    #[serde(default)]
    pub wind_speed_kmh: Option<u32>,
    #[serde(default)]
    pub wind_direction_deg: Option<u32>,
    /// Fourchette d'année du vivier d'adversaires (§8.6, remplace « même ère »)
    /// — filtrage fait côté front, transportée ici pour cohérence de la
    /// sérialisation (comme ai_level_min/max : toujours une valeur concrète).
    #[serde(default = "default_year_min")]
    #[allow(dead_code)]
    pub year_min: i32,
    #[serde(default = "default_year_max")]
    #[allow(dead_code)]
    pub year_max: i32,
    /// Saison optionnelle (§8.6bis) : "spring"|"summer"|"autumn"|"winter",
    /// juste persistée — c'est `season_date` qui est réellement écrite.
    #[serde(default)]
    #[allow(dead_code)]
    pub season: Option<String>,
    /// Date ISO (YYYY-MM-DD) associée à la saison choisie, calculée côté front.
    /// Écrite dans `[LIGHTING] __CM_DATE` (timestamp Unix), la clé que lit
    /// CSP pour la date de simulation (donc la saison) — validée sur une
    /// capture de race.ini produit par Content Manager (§8.6bis).
    #[serde(default)]
    pub season_date: Option<String>,
    // --- Options de course (§8.6, toutes visibles, pas de bloc repliable) ---
    #[serde(default)]
    pub penalties: bool,
    /// Faux départ : 0 = aucune, 1 = téléport, 2 = drive-through.
    #[serde(default)]
    pub jump_start_penalty: u32,
    /// Évolution du grip : DYNAMIC_TRACK SESSION_START (86 vert … 100 optimal).
    /// Pas de champ correspondant trouvé dans `TrackPropertiesData` (Quick
    /// Drive) — toujours « Optimum »/sec pour l'instant. Conservé pour
    /// l'aller-retour front, non lu ici.
    #[serde(default = "default_grip")]
    #[allow(dead_code)]
    pub grip: u32,
    /// Essais libres avant la course (mode course uniquement, weekend Quick
    /// Drive) : phase optionnelle indépendante de la qualification — les deux
    /// peuvent être activées ensemble ou séparément.
    #[serde(default)]
    pub practice_enabled: bool,
    #[serde(default = "default_practice_minutes")]
    pub practice_minutes: u32,
    /// Qualification avant la course (mode course uniquement).
    #[serde(default)]
    pub qualifying: bool,
    #[serde(default = "default_qualify_minutes")]
    pub qualify_minutes: u32,
    // --- Réglages dépendants du type (§8.4) ---
    /// Ghost car (Hotlap uniquement) → [GHOST_CAR] du race.ini.
    #[serde(default)]
    pub ghost_car: bool,
    /// Départ en Practice (mode Practice uniquement) → `StartType` du
    /// `ModeData` `QuickDrive_Practice.xaml`. `true` = "PIT" (valeur
    /// confirmée sur `pitbox-practice.cmpreset`) ; `false` = "TRACK", **non
    /// vérifiée sur un preset réel** — à confirmer si le départ sur piste ne
    /// se comporte pas comme attendu en jeu.
    #[serde(default = "default_true")]
    pub start_from_pit: bool,
    /// Simulation — dégâts/usure/carburant → assists.ini. Actifs quel que
    /// soit le type de session (§8.6, pas réservés à la Course). En %.
    #[serde(default = "default_damage")]
    pub damage: u32,
    #[serde(default = "default_rate")]
    pub fuel_rate: u32,
    #[serde(default = "default_rate")]
    pub tyre_wear: u32,
    // --- Aides à la conduite (Course uniquement, §8.6) ---
    #[serde(default = "default_true")]
    pub abs_auto: bool,
    #[serde(default = "default_true")]
    pub traction_control_auto: bool,
    #[serde(default)]
    pub ideal_line: bool,
}

fn default_ai_level_min() -> u32 {
    92
}
fn default_ai_level_max() -> u32 {
    98
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
fn default_practice_minutes() -> u32 {
    20
}
fn default_year_min() -> i32 {
    1950
}
fn default_year_max() -> i32 {
    2026
}
fn default_damage() -> u32 {
    50
}
fn default_rate() -> u32 {
    100
}
fn default_true() -> bool {
    true
}

/// Applique simulation (dégâts/carburant/usure) + aides à la conduite dans
/// `assists.ini` (Documents AC), en préservant les autres réglages —
/// best-effort, clés vérifiées sur un fichier réel (§8.6 : simulation active
/// quel que soit le type de session ; aides = Course uniquement, appelées
/// avec les valeurs par défaut sinon).
#[allow(clippy::too_many_arguments)]
fn apply_assists(damage: u32, fuel_rate: u32, tyre_wear: u32, abs: bool, traction_control: bool, ideal_line: bool) {
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
    let b = |v: bool| if v { 1 } else { 0 };
    let mut out = String::new();
    for line in text.lines() {
        let t = line.trim_start();
        if t.starts_with("DAMAGE=") {
            out.push_str(&format!("DAMAGE={damage}"));
        } else if t.starts_with("FUEL_RATE=") {
            out.push_str(&format!("FUEL_RATE={fuel_rate}"));
        } else if t.starts_with("TYRE_WEAR=") {
            out.push_str(&format!("TYRE_WEAR={tyre_wear}"));
        } else if t.starts_with("ABS=") {
            out.push_str(&format!("ABS={}", b(abs)));
        } else if t.starts_with("TRACTION_CONTROL=") {
            out.push_str(&format!("TRACTION_CONTROL={}", b(traction_control)));
        } else if t.starts_with("IDEAL_LINE=") {
            out.push_str(&format!("IDEAL_LINE={}", b(ideal_line)));
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
    let ac = cfg.ac_install_path.as_ref().ok_or(crate::errors::AC_NOT_CONFIGURED)?;
    let content = ac.join("content").join(kind.content_folder()).join(id);
    if content.exists() {
        return Ok(());
    }
    if overlay::mod_exists(conn, id).map_err(|e| e.to_string())? {
        return activation::activate(conn, cfg, id, None);
    }
    Err(format!("« {id} » n'est pas installé dans Assetto Corsa."))
}

/// Ouvre Content Manager sans argument (§12bis.5) : pratique pour parcourir
/// son propre menu (réglages CM, contenu…) sans passer par une session Pit Box.
pub fn open_content_manager(cfg: &AppConfig) -> Result<(), String> {
    let cm = cfg
        .content_manager_exe
        .as_ref()
        .ok_or(crate::errors::CM_NOT_CONFIGURED)?;
    let mut cmd = Command::new(cm);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.spawn().map_err(|e| format!("lancement de Content Manager : {e}"))?;
    Ok(())
}

/// Lance un replay dans Content Manager (§6.1, onglet Médias). Même mécanisme
/// que l'association de fichier Windows pour `.acreplay` (double-clic = CM
/// démarre la lecture) — on passe simplement le chemin en argument à
/// l'exécutable directement plutôt que de compter sur l'association système,
/// cohérent avec `launch`/`open_content_manager` qui invoquent déjà CM ainsi.
pub fn launch_replay(cfg: &AppConfig, replay_path: &Path) -> Result<(), String> {
    let cm = cfg
        .content_manager_exe
        .as_ref()
        .ok_or(crate::errors::CM_NOT_CONFIGURED)?;
    let mut cmd = Command::new(cm);
    cmd.arg(replay_path);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.spawn().map_err(|e| format!("lancement du replay : {e}"))?;
    Ok(())
}

/// Lance la session : active le contenu au besoin, écrit le race.ini, invoque CM.
pub fn launch(conn: &Connection, cfg: &AppConfig, setup: &RaceSetup) -> Result<(), String> {
    let cm = cfg
        .content_manager_exe
        .as_ref()
        .ok_or(crate::errors::CM_NOT_CONFIGURED)?;

    ensure_available(conn, cfg, ModKind::Car, &setup.car_id)?;
    ensure_available(conn, cfg, ModKind::Track, &setup.track_id)?;
    // Adversaires : best-effort — un adversaire manquant ne doit pas bloquer
    // toute la session, seulement être absent du plateau final.
    if setup.session_type == SessionType::Race {
        for opp in &setup.opponents {
            let _ = ensure_available(conn, cfg, ModKind::Car, &opp.car_id);
        }
    }

    // Simulation (dégâts/usure/carburant) + aides : actives quel que soit le
    // type de session (§8.6) ; vivent dans assists.ini, pas race.ini.
    apply_assists(
        setup.damage,
        setup.fuel_rate,
        setup.tyre_wear,
        setup.abs_auto,
        setup.traction_control_auto,
        setup.ideal_line,
    );

    // Preset Quick Drive (§8.3) plutôt qu'un race.ini/PreparedConfig : seul ce
    // chemin peuple StartProperties.BasicProperties côté CM, condition pour
    // que le téléchargement CSP automatique (VAO/config manquants) se
    // déclenche — voir quickdrive.rs et docs/L4-cm-launch-research.md.
    let preset = crate::quickdrive::build_preset(setup)?;
    let path = std::env::temp_dir().join(format!("pitbox-quickdrive-{}.json", Uuid::new_v4()));
    std::fs::write(&path, preset).map_err(|e| format!("écriture du preset Quick Drive : {e}"))?;

    let uri = format!("acmanager://race/quick?presetFile={}", path.display());
    let mut cmd = Command::new(cm);
    cmd.arg(&uri);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.spawn().map_err(|e| format!("lancement de Content Manager : {e}"))?;

    // Marqueur « déjà essayé » définitif (§6.5) : posé au lancement, fiabilise
    // les faux zéros de CM. Non bloquant si l'écriture échoue.
    let now = chrono::Local::now().to_rfc3339();
    let _ = crate::overlay::mark_launched(conn, &setup.car_id, &now);
    let _ = crate::overlay::mark_launched(conn, &setup.track_id, &now);
    Ok(())
}
