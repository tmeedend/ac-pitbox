//! Génération d'un preset Quick Drive Content Manager (§8.3) à partir d'un
//! `RaceSetup` — remplace le `race.ini`/`PreparedConfig` (`race/config`), qui
//! ne déclenche jamais le téléchargement CSP automatique (VAO/config) : ce
//! dernier n'est vérifié que dans `GameWrapper.StartAsync_Ui()`, à partir de
//! `StartProperties.BasicProperties`, jamais peuplé par le chemin
//! `PreparedConfig`. `race/quick` (Quick Drive) passe par
//! `QuickDrive.ViewModel.Go()`, qui peuple `BasicProperties` correctement —
//! le même chemin qu'un lancement depuis l'UI de CM. Voir
//! `docs/L4-cm-launch-research.md` pour le détail de cette recherche.
//!
//! Schéma reconstitué **empiriquement** depuis 4 presets réels sauvegardés
//! par l'utilisateur via l'UI de CM (`pitbox-practice.cmpreset`,
//! `pitbox-hotlap.cmpreset`, `pitbox-weekend.cmpreset`, et le grid manuel de
//! `pitbox.cmpreset`), pas depuis la source AcTools (trop de pièges de
//! sérialisation Newtonsoft.Json à deviner sans référence réelle). Champs
//! marqués « best effort » ci-dessous : pas vus dans un preset de référence,
//! valeur choisie par analogie — à vérifier en usage réel.

use serde_json::{json, Value};

use crate::launch::{Opponent, RaceSetup, SessionType};

/// `TrackId` façon CM : `<piste>/<layout>` si un layout est choisi (même
/// convention que `race/csp` côté CM : `trackId.Split('/')`), sinon la piste
/// seule (mono-layout).
fn track_id(s: &RaceSetup) -> String {
    match s.track_layout.as_deref() {
        Some(layout) if !layout.is_empty() => format!("{}/{}", s.track_id, layout),
        _ => s.track_id.clone(),
    }
}

/// Assists (§8.6) : dégâts/carburant/pneus/aides, valables quel que soit le
/// type de session. `Damage` en pourcentage direct (0-100, comme notre champ) ;
/// `TyreWear`/`FuelConsumption` en multiplicateur de taux (1.0 = 100%, notre
/// échelle 0-200 divisée par 100 — vérifié sur `AssistsData` d'un preset réel
/// : `"Damage":100.0,...,"TyreWear":1.0,"FuelConsumption":1.0`).
/// `Abs`/`TractionControl` : entiers 0/1 dans les presets réels (pas des
/// booléens) — mappés depuis nos réglages "auto" (best effort : la
/// signification exacte d'un éventuel niveau 2 n'a pas été vue).
fn build_assists(s: &RaceSetup) -> Value {
    json!({
        "IdealLine": s.ideal_line,
        "AutoBlip": false,
        "StabilityControl": 0.0,
        "AutoBrake": false,
        "AutoShifter": false,
        "SlipSteam": 1.0,
        "AutoClutch": false,
        "Abs": if s.abs_auto { 1 } else { 0 },
        "TractionControl": if s.traction_control_auto { 1 } else { 0 },
        "VisualDamage": true,
        "Damage": s.damage as f64,
        "TyreWear": s.tyre_wear as f64 / 100.0,
        "FuelConsumption": s.fuel_rate as f64 / 100.0,
        "TyreBlankets": s.tyre_blankets,
    })
}

/// Grille d'adversaires explicite (§8.6, mode course) : `ModeId:"manual"`
/// avec des tableaux parallèles `CarIds`/`SkinIds`/`AiLevels` — un index par
/// adversaire, valeur confirmée en lisant `RaceGridViewModel.cs`
/// (AcManager.Controls) : c'est exactement ce que sérialise le mode grille
/// manuel de CM, pas seulement un tirage aléatoire dans un vivier.
/// `ShuffleCandidates:false` pour que CM n'aille pas re-mélanger notre ordre.
fn build_grid(opponents: &[Opponent]) -> Value {
    let car_ids: Vec<&str> = opponents.iter().map(|o| o.car_id.as_str()).collect();
    let skin_ids: Vec<Value> = opponents
        .iter()
        .map(|o| o.car_skin.as_deref().map(Value::from).unwrap_or(Value::Null))
        .collect();
    let ai_levels: Vec<f64> = opponents.iter().map(|o| o.ai_level as f64).collect();
    json!({
        "ModeId": "manual",
        "FilterValue": "",
        "CarIds": car_ids,
        "SkinIds": skin_ids,
        "AiLevels": ai_levels,
        "ShuffleCandidates": false,
        "VarietyLimitation": 0,
        "OpponentsNumber": opponents.len(),
        "StartingPosition": opponents.len() + 1,
        "AiLevel": 95.0,
        "AiLevelMin": 85.0,
        "AiLevelArrangeRandom": 0.0,
        "AiLevelArrangeReverse": false,
        "AiLevelArrangePowerRatio": false,
        "AiAggression": 0.0,
        "AiAggressionMin": 0.0,
        "AiAggressionArrangeRandom": 0.0,
        "AiAggressionArrangeReverse": false,
    })
}

/// `ModeData` (§8.6) pour `QuickDrive_Practice.xaml` — schéma confirmé sur
/// `pitbox-practice.cmpreset` : `StartType`/`Penalties`/`PlayerBallast`/
/// `PlayerRestrictor`. Pas de grille (session solo).
fn mode_data_practice(s: &RaceSetup) -> String {
    json!({
        "StartType": if s.start_from_pit { "PIT" } else { "TRACK" },
        "Penalties": s.penalties,
        "PlayerBallast": 0,
        "PlayerRestrictor": 0,
    })
    .to_string()
}

/// `ModeData` pour `QuickDrive_Hotlap.xaml` — schéma confirmé sur
/// `pitbox-hotlap.cmpreset` : `GhostCar` correspond exactement à notre champ.
fn mode_data_hotlap(s: &RaceSetup) -> String {
    json!({
        "GhostCar": s.ghost_car,
        "DoNotRecordGhostCar": false,
        "GhostCarAdvantage": 0.0,
        "Penalties": s.penalties,
        "PlayerBallast": 0,
        "PlayerRestrictor": 0,
    })
    .to_string()
}

/// `ModeData` pour `QuickDrive_Weekend.xaml` — schéma confirmé sur
/// `pitbox-weekend.cmpreset`. `PracticeLength` est optionnel (`null` = phase
/// absente, la course rapide de CM le supporte : curseur `[0, 90]`).
/// `QualificationLength` ne l'est pas : la qualification est **toujours**
/// présente en mode Weekend côté CM (curseur `[5, 90]`, jamais désactivable)
/// — pas besoin du mode `QuickDrive_Race.xaml` séparé : une « course » chez
/// nous est un Weekend sans pratique, avec la qualification la plus courte
/// possible.
fn mode_data_weekend(s: &RaceSetup) -> String {
    json!({
        "PracticeLength": if s.practice_enabled { Some(s.practice_minutes) } else { None },
        // Toujours une durée concrète : CM ne sait pas désactiver la
        // qualification en mode Weekend (voir doc de `build_preset`).
        "QualificationLength": s.qualify_minutes,
        "Penalties": s.penalties,
        "JumpStartPenalty": s.jump_start_penalty,
        "LapsNumber": s.laps,
        "RaceGridSerialized": build_grid(&s.opponents).to_string(),
        "Version": 2,
    })
    .to_string()
}

/// Construit le preset Quick Drive complet (§8.3) — remplace le couple
/// `build_race_ini` / `PreparedConfig`. Renvoie le JSON sérialisé, prêt à
/// écrire dans un fichier temporaire et passer via `race/quick?presetFile=…`.
///
/// **Limites connues (best effort, pas vues dans un preset de référence)** :
/// - **Skin du joueur** : le schéma Quick Drive ne porte **aucun** champ skin
///   pour la voiture du joueur (confirmé en lisant `QuickDrive.xaml.cs`, et
///   mesuré : deux `.cmpreset` sauvegardés par CM avec deux skins différents
///   sont identiques octet pour octet) — le skin vient uniquement d'un
///   paramètre `carSkinId` passé à `RunAsync` par le code C#, jamais du JSON,
///   et `race/quick` (invocation URI) ne le transmet pas. `s.car_skin` n'est
///   donc pas envoyé **par ce preset** : il est réinjecté après coup dans le
///   `race.ini` écrit par CM, voir `raceini.rs` (§9.2).
/// - **Évolution du grip / état de piste** : pas de champ dédié trouvé —
///   les 4 presets de référence utilisent tous le même `TrackPropertiesData`
///   ("Optimum"/sec). `s.grip` n'est **pas encore appliqué** — toujours piste
///   optimale sèche pour l'instant.
/// - **Durée en Practice** : le `ModeData` de `QuickDrive_Practice.xaml`
///   (confirmé sur `pitbox-practice.cmpreset`, et sur la classe C# CM
///   elle-même) n'a aucun champ de durée — contrairement à l'ancien race.ini
///   (`DURATION_MINUTES`), une session Practice via Quick Drive est
///   illimitée par design (on roule tant qu'on veut, sortie manuelle). Pas
///   de champ correspondant dans `RaceSetup` : rien à envoyer.
/// - **Qualification jamais désactivable** : `QuickDrive_Weekend.xaml.cs`
///   (CM) borne sa durée à `[5, 90]` minutes et son `Save()` n'écrit jamais
///   de durée nulle — aucun état « off » n'existe côté CM pour le mode
///   Weekend. `s.qualify_minutes` est donc toujours envoyée (jamais `null`),
///   avec la même borne mini de 5 min côté UI plutôt qu'une case à cocher
///   qui ne pourrait jamais réellement désactiver la phase.
pub fn build_preset(s: &RaceSetup) -> Result<String, String> {
    let (mode_path, mode_data) = match s.session_type {
        SessionType::Practice => ("/Pages/Drive/QuickDrive_Practice.xaml", mode_data_practice(s)),
        SessionType::Hotlap => ("/Pages/Drive/QuickDrive_Hotlap.xaml", mode_data_hotlap(s)),
        SessionType::Race => ("/Pages/Drive/QuickDrive_Weekend.xaml", mode_data_weekend(s)),
    };

    let weather_id = if s.weather.is_empty() {
        "3_clear"
    } else {
        s.weather.as_str()
    };
    let wind_speed = s.wind_speed_kmh.unwrap_or(0) as f64;

    let preset = json!({
        "Mode": mode_path,
        "ModeData": mode_data,
        "CarId": s.car_id,
        "TrackId": track_id(s),
        "WeatherId": weather_id,
        "RealConditions": false,
        "Temperature": s.ambient_c.unwrap_or(22) as f64,
        "Time": (s.time_hours * 3600.0) as i64,
        "TimeMultipler": 1,
        "udt": s.season_date.is_some(),
        "dtv": s.season_date.as_ref().map(|d| format!("{d}T00:00:00")),
        "tpc": true,
        "TrackPropertiesData": json!({
            "s": 1.0, "t": 1.0, "r": 0.0, "g": 1, "d": "Perfect track for hotlapping.", "w": false,
        }).to_string(),
        "asc": true,
        "AssistsData": build_assists(s).to_string(),
        "ico": false,
        "wsf": wind_speed,
        "wst": wind_speed,
        "wd": s.wind_direction_deg.unwrap_or(0) as f64,
        "rws": false,
        "rwd": false,
        "rcTimezones": true,
        "rcManTime": false,
        "rcManWind": false,
        "rcLw": false,
        "rte": false,
        "rti": false,
        "crt": s.road_c.is_some(),
    });

    serde_json::to_string(&preset).map_err(|e| format!("sérialisation du preset Quick Drive : {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::RaceSetup;

    fn base_setup(session_type: SessionType) -> RaceSetup {
        RaceSetup {
            car_id: "ks_praga_r1".into(),
            car_skin: None,
            track_id: "spa".into(),
            track_layout: None,
            session_type,
            opponents: Vec::new(),
            ai_level_min: 92,
            ai_level_max: 98,
            laps: 5,
            weather: "sol_01_clear".into(),
            time_hours: 13.0,
            ambient_c: Some(24),
            road_c: None,
            wind_speed_kmh: Some(6),
            wind_direction_deg: Some(250),
            year_min: 1950,
            year_max: 2026,
            season: None,
            season_date: None,
            penalties: false,
            jump_start_penalty: 0,
            grip: 96,
            practice_enabled: false,
            practice_minutes: 20,
            qualify_minutes: 10,
            ghost_car: false,
            start_from_pit: true,
            damage: 50,
            fuel_rate: 100,
            tyre_wear: 100,
            tyre_blankets: false,
            abs_auto: true,
            traction_control_auto: true,
            ideal_line: false,
        }
    }

    #[test]
    fn practice_preset_has_correct_mode_and_no_grid() {
        let s = base_setup(SessionType::Practice);
        let json = build_preset(&s).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["Mode"], "/Pages/Drive/QuickDrive_Practice.xaml");
        assert_eq!(v["CarId"], "ks_praga_r1");
        assert_eq!(v["TrackId"], "spa");
        let mode_data: Value = serde_json::from_str(v["ModeData"].as_str().unwrap()).unwrap();
        assert_eq!(mode_data["StartType"], "PIT");
    }

    #[test]
    fn practice_start_from_track_when_not_from_pit() {
        let mut s = base_setup(SessionType::Practice);
        s.start_from_pit = false;
        let json = build_preset(&s).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        let mode_data: Value = serde_json::from_str(v["ModeData"].as_str().unwrap()).unwrap();
        assert_eq!(mode_data["StartType"], "TRACK");
    }

    #[test]
    fn hotlap_preset_carries_ghost_car() {
        let mut s = base_setup(SessionType::Hotlap);
        s.ghost_car = true;
        let json = build_preset(&s).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["Mode"], "/Pages/Drive/QuickDrive_Hotlap.xaml");
        let mode_data: Value = serde_json::from_str(v["ModeData"].as_str().unwrap()).unwrap();
        assert_eq!(mode_data["GhostCar"], true);
    }

    #[test]
    fn race_preset_uses_weekend_mode_with_explicit_grid() {
        let mut s = base_setup(SessionType::Race);
        s.laps = 10;
        s.practice_enabled = true;
        s.practice_minutes = 30;
        s.qualify_minutes = 20;
        s.opponents = vec![
            Opponent {
                car_id: "ks_ferrari_488_gt3".into(),
                ai_level: 92,
                car_skin: Some("red".into()),
            },
            Opponent {
                car_id: "ks_porsche_991_gt3_r".into(),
                ai_level: 87,
                car_skin: None,
            },
        ];
        let json = build_preset(&s).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["Mode"], "/Pages/Drive/QuickDrive_Weekend.xaml");

        let mode_data: Value = serde_json::from_str(v["ModeData"].as_str().unwrap()).unwrap();
        assert_eq!(mode_data["LapsNumber"], 10);
        assert_eq!(mode_data["PracticeLength"], 30);
        assert_eq!(mode_data["QualificationLength"], 20);

        let grid: Value = serde_json::from_str(mode_data["RaceGridSerialized"].as_str().unwrap()).unwrap();
        assert_eq!(grid["ModeId"], "manual");
        assert_eq!(grid["OpponentsNumber"], 2);
        assert_eq!(grid["CarIds"][0], "ks_ferrari_488_gt3");
        assert_eq!(grid["CarIds"][1], "ks_porsche_991_gt3_r");
        assert_eq!(grid["SkinIds"][0], "red");
        assert!(grid["SkinIds"][1].is_null());
        assert_eq!(grid["AiLevels"][0], 92.0);
    }

    #[test]
    fn race_without_practice_omits_practice_length_but_keeps_qualification() {
        let s = base_setup(SessionType::Race);
        let json = build_preset(&s).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        let mode_data: Value = serde_json::from_str(v["ModeData"].as_str().unwrap()).unwrap();
        assert!(
            mode_data["PracticeLength"].is_null(),
            "pas d'essais libres = phase absente"
        );
        assert_eq!(
            mode_data["QualificationLength"], 10,
            "la qualification n'a pas d'état désactivé côté CM (Weekend) : toujours une durée concrète"
        );
    }

    #[test]
    fn track_layout_appended_with_slash() {
        let mut s = base_setup(SessionType::Practice);
        s.track_layout = Some("gp".into());
        let json = build_preset(&s).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["TrackId"], "spa/gp");
    }

    #[test]
    fn assists_scale_matches_reference_presets() {
        let mut s = base_setup(SessionType::Practice);
        s.damage = 100;
        s.tyre_wear = 100;
        s.fuel_rate = 200;
        s.abs_auto = false;
        let json = build_preset(&s).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        let assists: Value = serde_json::from_str(v["AssistsData"].as_str().unwrap()).unwrap();
        assert_eq!(assists["Damage"], 100.0);
        assert_eq!(assists["TyreWear"], 1.0);
        assert_eq!(assists["FuelConsumption"], 2.0);
        assert_eq!(assists["Abs"], 0);
    }

    #[test]
    fn tyre_blankets_flows_into_assists_data() {
        let mut s = base_setup(SessionType::Practice);
        s.tyre_blankets = true;
        let json = build_preset(&s).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        let assists: Value = serde_json::from_str(v["AssistsData"].as_str().unwrap()).unwrap();
        assert_eq!(assists["TyreBlankets"], true);
    }

    #[test]
    fn season_date_sets_udt_and_dtv() {
        let mut s = base_setup(SessionType::Practice);
        s.season_date = Some("2026-07-15".into());
        let json = build_preset(&s).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["udt"], true);
        assert_eq!(v["dtv"], "2026-07-15T00:00:00");
    }
}
