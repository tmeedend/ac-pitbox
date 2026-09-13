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
//! Schéma reconstitué **empiriquement** depuis 5 presets réels sauvegardés
//! par l'utilisateur via l'UI de CM (`pitbox-practice.cmpreset`,
//! `pitbox-hotlap.cmpreset`, `pitbox-weekend.cmpreset`, le grid manuel de
//! `pitbox.cmpreset`, et `pitbox-trackday.cmpreset`), pas depuis la source
//! AcTools (trop de pièges de sérialisation Newtonsoft.Json à deviner sans
//! référence réelle). Champs marqués « best effort » ci-dessous : pas vus
//! dans un preset de référence, valeur choisie par analogie — à vérifier en
//! usage réel.

use serde_json::{json, Value};

use crate::launch::{Opponent, PracticeStart, RaceSetup, SessionType};

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
/// `Abs`/`TractionControl` : entiers (pas des booléens), aux trois valeurs que
/// le launcher d'AC déclare lui-même — `0` Off, `1` Factory, `2` On, voir
/// `AssistLevel`.
fn build_assists(s: &RaceSetup) -> Value {
    json!({
        "IdealLine": s.ideal_line,
        "AutoBlip": false,
        "StabilityControl": 0.0,
        "AutoBrake": false,
        "AutoShifter": false,
        "SlipSteam": 1.0,
        "AutoClutch": false,
        "Abs": s.abs.to_ac(),
        "TractionControl": s.traction_control.to_ac(),
        "VisualDamage": true,
        "Damage": s.damage as f64,
        "TyreWear": s.tyre_wear as f64 / 100.0,
        "FuelConsumption": s.fuel_rate as f64 / 100.0,
        "TyreBlankets": s.tyre_blankets,
    })
}

/// Les six états de piste **du jeu**, `cfg/templates/tracks.ini` recopié
/// verbatim : grip de départ, report d'une session à l'autre, aléa, et nombre
/// de tours pour gagner un point de grip.
///
/// Content Manager lit ce même fichier pour peupler sa liste (relevé dans
/// `launcher/themes/default/js/ui.js` : `getiniasjson/templates/tracks.ini`),
/// et affiche les trois premiers en pourcentages — ce qui est la preuve que
/// son preset les stocke divisés par cent, voir [`build_track_properties`].
///
/// Ordonnés par grip croissant, comme la liste de CM.
pub const TRACK_STATES: [TrackState; 6] = [
    TrackState {
        start: 86,
        transfer: 50,
        randomness: 1,
        lap_gain: 30,
        description: "A very slippery track, improves fast with more laps.",
    },
    TrackState {
        start: 89,
        transfer: 80,
        randomness: 3,
        lap_gain: 50,
        description: "Old tarmac. Bad grip won't get better soon.",
    },
    TrackState {
        start: 95,
        transfer: 90,
        randomness: 2,
        lap_gain: 132,
        description: "A clean track, gets better with more laps.",
    },
    TrackState {
        start: 96,
        transfer: 80,
        randomness: 1,
        lap_gain: 300,
        description: "A slow track that doesn't improve much.",
    },
    TrackState {
        start: 98,
        transfer: 80,
        randomness: 2,
        lap_gain: 700,
        description: "Very grippy track right from the start.",
    },
    TrackState {
        start: 100,
        transfer: 100,
        randomness: 0,
        lap_gain: 1,
        description: "Perfect track for hotlapping.",
    },
];

/// Une entrée de `cfg/templates/tracks.ini`.
pub struct TrackState {
    /// `SESSION_START` — le grip au départ, en pourcentage. C'est lui qui
    /// identifie l'état : l'écran ne retient que ce nombre.
    pub start: u32,
    /// `SESSION_TRANSFER` — ce qui se reporte d'une session à la suivante.
    pub transfer: u32,
    /// `RANDOMNESS` — la variation aléatoire.
    pub randomness: u32,
    /// `LAP_GAIN` — nombre de tours pour gagner un point de grip : plus il est
    /// grand, moins la piste s'améliore.
    pub lap_gain: u32,
    /// Telle que le jeu l'écrit. Cosmétique côté CM, jamais lue par AC.
    pub description: &'static str,
}

/// « Auto » : l'état est laissé à la météo (§9.3). Sentinelle plutôt qu'un
/// second champ, parce que l'écran n'offre qu'**un** choix parmi sept — deux
/// champs pour une seule décision finissent toujours par se contredire. Aucun
/// état réel ne vaut 0 %.
pub const GRIP_WEATHER: u32 = 0;

/// Ce que porte « Auto », repris de `TrackStateViewModelBase.CreateBuiltIn`
/// de Content Manager : le drapeau, et les valeurs de `GREEN` en repli.
///
/// Le repli n'est pas décoratif — `ToProperties()` écrit les quatre nombres
/// dans tous les cas, `WeatherDefined` compris. Une session « Auto » dont la
/// météo ne dit rien de la piste roule donc sur une piste verte, ce que la
/// description de CM énonce mot pour mot.
const WEATHER_STATE: TrackState = TrackState {
    start: 95,
    transfer: 90,
    randomness: 2,
    lap_gain: 132,
    description: "Track state specified by weather, or Green, in case weather doesn't specify track state",
};

/// L'état de piste le plus proche du grip demandé.
///
/// Par proximité et non par égalité : un preset enregistré avant que la liste
/// ne s'aligne sur celle du jeu peut porter une valeur qui n'y figure pas (92 %
/// a existé, inventé). Aucune migration à écrire — il atterrit sur l'état
/// voisin, et l'écran l'y recale à l'affichage.
///
/// **À égale distance, le plus adhérent gagne** : 92 % tombait entre 89 et 95,
/// et une piste un peu plus roulante est le repli indulgent — l'inverse
/// rendrait une session plus difficile qu'à la sauvegarde, ce qui se remarque
/// au volant alors que le contraire ne se remarque pas.
pub fn track_state_for(grip: u32) -> &'static TrackState {
    let grip = grip.clamp(1, 100);
    TRACK_STATES
        .iter()
        .min_by_key(|st| (st.start.abs_diff(grip), std::cmp::Reverse(st.start)))
        .expect("the table is never empty")
}

/// État de la piste (§9.3) — le `TrackPropertiesData` du preset, au niveau
/// racine et non dans le `ModeData`, donc commun aux quatre types de session.
///
/// **Le schéma est décodé, plus deviné**, et en deux temps. L'entrée `OPTIMUM`
/// de la table du jeu (100/100/0/1, « Perfect track for hotlapping. »)
/// reproduit exactement le `{"s":1.0,"t":1.0,"r":0.0,"g":1,"d":…}` que portent
/// les dix presets de référence : `s`, `t` et `g` s'en déduisent, mais pas `r`,
/// qu'un `RANDOMNESS` nul laissait indécidable entre les deux échelles.
///
/// C'est le panneau « Track state » de CM qui a tranché : sur `GREEN` il
/// affiche 95 %, 90 % et 2 %, là où le fichier du jeu dit `SESSION_START=95`,
/// `SESSION_TRANSFER=90` et `RANDOMNESS=2`. Les trois sont donc **le même
/// pourcentage divisé par cent**, l'aléa compris ; seul `LAP_GAIN` reste brut,
/// et il n'est d'ailleurs pas affiché en pourcentage.
///
/// `d` est cosmétique : CM l'affiche, le jeu ne la lit pas — son
/// `[DYNAMIC_TRACK]` n'a pas de clé `DESCRIPTION`. Et `w` est `WeatherDefined`,
/// le « Auto (set by weather) » de la liste de CM — voir [`GRIP_WEATHER`].
///
/// **Le nom de l'état ne part jamais.** « Green » n'est qu'un libellé
/// d'interface pour une combinaison de quatre nombres ; rien dans le preset ne
/// le transporte. Ce sont donc bien les nombres qu'il faut envoyer justes.
fn build_track_properties(s: &RaceSetup) -> Value {
    let weather_defined = s.grip == GRIP_WEATHER;
    let st = if weather_defined {
        &WEATHER_STATE
    } else {
        track_state_for(s.grip)
    };
    json!({
        "s": f64::from(st.start) / 100.0,
        "t": f64::from(st.transfer) / 100.0,
        "r": f64::from(st.randomness) / 100.0,
        "g": st.lap_gain,
        "d": st.description,
        "w": weather_defined,
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
/// `PlayerRestrictor`. Pas de grille (session solo). Valeurs de `StartType` :
/// voir `PracticeStart`.
fn mode_data_practice(s: &RaceSetup) -> String {
    let start_type = match s.practice_start {
        PracticeStart::Pit => "PIT",
        PracticeStart::Track => "TRACK",
        PracticeStart::Hotlap => "HOTLAP_START",
    };
    json!({
        "StartType": start_type,
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
/// `pitbox-weekend.cmpreset`. Utilisé dès qu'une qualification est demandée :
/// c'est le seul mode de CM qui en a une.
fn mode_data_weekend(s: &RaceSetup) -> String {
    json!({
        // `0` = phase sautée (curseur CM `[0, 90]`, libellé « Skip session »).
        // `null` ne la désactive **pas** : le `Load()` de
        // `QuickDrive_Weekend.xaml.cs` fait `r.PracticeLength ?? 15` et
        // rendait donc 15 min d'essais à toute course censée ne pas en avoir
        // — bug réel, constaté en jeu avant d'être retrouvé dans la source.
        "PracticeLength": if s.practice_enabled { s.practice_minutes } else { 0 },
        "QualificationLength": s.qualify_minutes,
        "Penalties": s.penalties,
        "JumpStartPenalty": s.jump_start_penalty,
        "LapsNumber": s.laps,
        "RaceGridSerialized": build_grid(&s.opponents).to_string(),
        "Version": 2,
    })
    .to_string()
}

/// `ModeData` pour `QuickDrive_Race.xaml` — la course sèche, sans phase
/// préparatoire. Schéma confirmé sur `pitbox-race.cmpreset` : exactement le
/// Weekend **moins** les deux durées. C'est le seul moyen d'obtenir une course
/// sans qualification, le mode Weekend n'ayant pas d'état « off » pour elle
/// (voir `RaceSetup::qualify_enabled`).
fn mode_data_race(s: &RaceSetup) -> String {
    json!({
        "Penalties": s.penalties,
        "JumpStartPenalty": s.jump_start_penalty,
        "LapsNumber": s.laps,
        "RaceGridSerialized": build_grid(&s.opponents).to_string(),
        "Version": 2,
    })
    .to_string()
}

/// `ModeData` pour `QuickDrive_Trackday.xaml` — schéma confirmé sur
/// `pitbox-trackday.cmpreset` : même grille manuelle que Course (Race, sans
/// qualification — Track day n'a pas de mode Weekend équivalent), plus
/// `SpeedLimit`. Pas de champ dédié dans `RaceSetup` pour l'instant (best
/// effort, comme `s.grip` plus bas) : `0.0` = pas de limite, seule valeur vue
/// dans le preset de référence.
fn mode_data_trackday(s: &RaceSetup) -> String {
    json!({
        "SpeedLimit": 0.0,
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
/// - **Évolution du grip / état de piste** : appliqué, voir
///   [`build_track_properties`]. Les presets de référence portent tous le même
///   `TrackPropertiesData` parce qu'ils ont tous été sauvegardés sur une piste
///   optimale, pas parce que le champ n'existe pas — c'est la table de presets
///   du jeu (`cfg/templates/tracks.ini`) qui l'a montré.
/// - **Durée en Practice** : le `ModeData` de `QuickDrive_Practice.xaml`
///   (confirmé sur `pitbox-practice.cmpreset`, et sur la classe C# CM
///   elle-même) n'a aucun champ de durée — contrairement à l'ancien race.ini
///   (`DURATION_MINUTES`), une session Practice via Quick Drive est
///   illimitée par design (on roule tant qu'on veut, sortie manuelle). Pas
///   de champ correspondant dans `RaceSetup` : rien à envoyer.
///
/// Une course se joue sur **deux** modes CM selon `qualify_enabled` : Weekend
/// quand une qualification est demandée, Race sinon (§9.3). Le mode Weekend
/// n'a pas d'état « pas de qualif » — sa durée est bornée à `[5, 90]` et son
/// `Save()` n'écrit jamais de durée nulle.
pub fn build_preset(s: &RaceSetup) -> Result<String, String> {
    let (mode_path, mode_data) = match s.session_type {
        SessionType::Practice => ("/Pages/Drive/QuickDrive_Practice.xaml", mode_data_practice(s)),
        SessionType::Hotlap => ("/Pages/Drive/QuickDrive_Hotlap.xaml", mode_data_hotlap(s)),
        SessionType::Race if s.qualify_enabled => ("/Pages/Drive/QuickDrive_Weekend.xaml", mode_data_weekend(s)),
        SessionType::Race => ("/Pages/Drive/QuickDrive_Race.xaml", mode_data_race(s)),
        SessionType::TrackDay => ("/Pages/Drive/QuickDrive_Trackday.xaml", mode_data_trackday(s)),
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
        // Rounded, not truncated: the time slider steps by ten minutes, so
        // most values are thirds of an hour that no float holds exactly —
        // truncating turns 10:00 into 09:59:59.
        "Time": (s.time_hours * 3600.0).round() as i64,
        "TimeMultipler": 1,
        "udt": s.season_date.is_some(),
        "dtv": s.season_date.as_ref().map(|d| format!("{d}T00:00:00")),
        "tpc": true,
        "TrackPropertiesData": build_track_properties(s).to_string(),
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
    use crate::launch::{AssistLevel, RaceSetup};

    fn base_setup(session_type: SessionType) -> RaceSetup {
        RaceSetup {
            car_id: "ks_praga_r1".into(),
            car_skin: None,
            driver: None,
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
            qualify_enabled: true,
            qualify_minutes: 10,
            ghost_car: false,
            practice_start: PracticeStart::Pit,
            damage: 50,
            fuel_rate: 100,
            tyre_wear: 100,
            tyre_blankets: false,
            abs: AssistLevel::Factory,
            traction_control: AssistLevel::Factory,
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
        s.practice_start = PracticeStart::Track;
        let json = build_preset(&s).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        let mode_data: Value = serde_json::from_str(v["ModeData"].as_str().unwrap()).unwrap();
        assert_eq!(mode_data["StartType"], "TRACK");
    }

    #[test]
    fn practice_start_from_hotlap_position() {
        let mut s = base_setup(SessionType::Practice);
        s.practice_start = PracticeStart::Hotlap;
        let json = build_preset(&s).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        let mode_data: Value = serde_json::from_str(v["ModeData"].as_str().unwrap()).unwrap();
        assert_eq!(mode_data["StartType"], "HOTLAP_START");
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

    /// Bug réel : `null` ne saute pas les essais libres, CM le lit comme
    /// « non renseigné » et retombe sur son défaut de 15 min. Seul `0` les
    /// saute (`QuickDrive_Weekend.xaml.cs`, curseur `[0, 90]`).
    #[test]
    fn race_without_practice_sends_zero_not_null() {
        let s = base_setup(SessionType::Race);
        let json = build_preset(&s).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        let mode_data: Value = serde_json::from_str(v["ModeData"].as_str().unwrap()).unwrap();
        assert_eq!(
            mode_data["PracticeLength"], 0,
            "pas d'essais libres = 0, jamais null (null = 15 min par défaut côté CM)"
        );
        assert_eq!(mode_data["QualificationLength"], 10, "la qualification reste demandée");
    }

    /// Le mode Weekend n'a pas d'état « pas de qualification » : sans elle,
    /// c'est l'autre mode course de CM qu'il faut viser (§9.3).
    #[test]
    fn race_without_qualification_switches_to_race_mode() {
        let mut s = base_setup(SessionType::Race);
        s.qualify_enabled = false;
        s.laps = 12;
        s.opponents = vec![Opponent {
            car_id: "ks_ferrari_488_gt3".into(),
            ai_level: 92,
            car_skin: Some("red".into()),
        }];
        let json = build_preset(&s).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["Mode"], "/Pages/Drive/QuickDrive_Race.xaml");

        let mode_data: Value = serde_json::from_str(v["ModeData"].as_str().unwrap()).unwrap();
        assert!(
            mode_data["QualificationLength"].is_null() && mode_data["PracticeLength"].is_null(),
            "le ModeData de Race ne porte aucune durée : {mode_data}"
        );
        assert_eq!(mode_data["LapsNumber"], 12, "la course elle-même est inchangée");
        let grid: Value = serde_json::from_str(mode_data["RaceGridSerialized"].as_str().unwrap()).unwrap();
        assert_eq!(grid["CarIds"][0], "ks_ferrari_488_gt3", "le plateau suit aussi ce mode");
    }

    /// Track day : grille manuelle comme Course, plus `SpeedLimit`, jamais de
    /// qualification/essais libres (pas de mode Weekend équivalent côté CM) —
    /// schéma confirmé sur `pitbox-trackday.cmpreset`.
    #[test]
    fn trackday_preset_uses_dedicated_mode_with_speed_limit_and_grid() {
        let mut s = base_setup(SessionType::TrackDay);
        s.laps = 2;
        s.opponents = vec![Opponent {
            car_id: "ks_praga_r1".into(),
            ai_level: 90,
            car_skin: None,
        }];
        let json = build_preset(&s).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["Mode"], "/Pages/Drive/QuickDrive_Trackday.xaml");

        let mode_data: Value = serde_json::from_str(v["ModeData"].as_str().unwrap()).unwrap();
        assert_eq!(mode_data["SpeedLimit"], 0.0);
        assert_eq!(mode_data["LapsNumber"], 2);
        assert!(
            mode_data["QualificationLength"].is_null() && mode_data["PracticeLength"].is_null(),
            "le ModeData de Trackday ne porte aucune durée : {mode_data}"
        );
        let grid: Value = serde_json::from_str(mode_data["RaceGridSerialized"].as_str().unwrap()).unwrap();
        assert_eq!(grid["ModeId"], "manual");
        assert_eq!(grid["CarIds"][0], "ks_praga_r1");
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
        s.abs = AssistLevel::Off;
        let json = build_preset(&s).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        let assists: Value = serde_json::from_str(v["AssistsData"].as_str().unwrap()).unwrap();
        assert_eq!(assists["Damage"], 100.0);
        assert_eq!(assists["TyreWear"], 1.0);
        assert_eq!(assists["FuelConsumption"], 2.0);
        assert_eq!(assists["Abs"], 0);
    }

    /// §9.3 — l'état de piste choisi part réellement dans le preset. Il ne
    /// partait pas : les quatre niveaux de l'écran écrivaient tous la piste
    /// optimale, donc le réglage n'avait aucun effet en jeu.
    #[test]
    fn the_chosen_grip_reaches_the_preset() {
        let mut s = base_setup(SessionType::Practice);
        s.grip = 86;
        let v: Value = serde_json::from_str(&build_preset(&s).unwrap()).unwrap();
        let track: Value = serde_json::from_str(v["TrackPropertiesData"].as_str().unwrap()).unwrap();
        assert_eq!(track["s"], 0.86, "grip de départ transmis");
        assert_eq!(track["t"], 0.5, "report de session");
        assert_eq!(track["g"], 30, "tours par point de grip");
        assert_eq!(
            track["d"], "A very slippery track, improves fast with more laps.",
            "la description suit le nombre envoyé au lieu de le contredire"
        );

        // La piste optimale doit rester exactement ce que portent les dix
        // presets de référence, sinon c'est le cas nominal qu'on a cassé.
        s.grip = 100;
        let v: Value = serde_json::from_str(&build_preset(&s).unwrap()).unwrap();
        let track: Value = serde_json::from_str(v["TrackPropertiesData"].as_str().unwrap()).unwrap();
        assert_eq!(track["s"], 1.0);
        assert_eq!(track["t"], 1.0);
        assert_eq!(track["r"], 0.0);
        assert_eq!(track["g"], 1);
        assert_eq!(track["d"], "Perfect track for hotlapping.");
    }

    /// §9.3 — « Auto » pose le drapeau que CM appelle `WeatherDefined`, et
    /// envoie tout de même quatre nombres : `ToProperties()` les écrit dans
    /// tous les cas, donc une météo muette sur la piste la laisse verte.
    #[test]
    fn the_weather_driven_state_sets_the_flag_and_falls_back_on_green() {
        let mut s = base_setup(SessionType::Practice);
        s.grip = GRIP_WEATHER;
        let v: Value = serde_json::from_str(&build_preset(&s).unwrap()).unwrap();
        let track: Value = serde_json::from_str(v["TrackPropertiesData"].as_str().unwrap()).unwrap();
        assert_eq!(track["w"], true, "drapeau posé");
        assert_eq!(track["s"], 0.95, "repli sur Green");
        assert_eq!(track["g"], 132);

        // Tout autre état laisse le drapeau à terre.
        s.grip = 100;
        let v: Value = serde_json::from_str(&build_preset(&s).unwrap()).unwrap();
        let track: Value = serde_json::from_str(v["TrackPropertiesData"].as_str().unwrap()).unwrap();
        assert_eq!(track["w"], false);
    }

    /// §9.3 — l'aléa est un pourcentage comme le grip de départ, pas une valeur
    /// brute : c'est le panneau de CM qui l'a montré, en affichant « 2 % » là
    /// où la table du jeu écrit `RANDOMNESS=2`. Se tromper d'un facteur cent
    /// ici ne se verrait qu'en course.
    #[test]
    fn randomness_travels_as_a_percentage_like_the_other_two() {
        let mut s = base_setup(SessionType::Practice);
        s.grip = 95;
        let v: Value = serde_json::from_str(&build_preset(&s).unwrap()).unwrap();
        let track: Value = serde_json::from_str(v["TrackPropertiesData"].as_str().unwrap()).unwrap();
        assert_eq!(track["s"], 0.95);
        assert_eq!(track["t"], 0.9);
        assert_eq!(track["r"], 0.02, "2 % et non 2");
    }

    /// Un preset enregistré avant que la liste ne s'aligne sur celle du jeu
    /// porte un grip qui n'y figure pas — il atterrit sur l'état voisin plutôt
    /// que de partir tel quel avec les paramètres d'une autre piste.
    #[test]
    fn a_grip_that_is_no_longer_offered_lands_on_the_nearest_state() {
        assert_eq!(
            track_state_for(92).start,
            95,
            "92 inventé -> Green, le plus proche vers le haut"
        );
        assert_eq!(track_state_for(0).start, 86, "sous la table -> la plus glissante");
        assert_eq!(track_state_for(100).start, 100, "une valeur exacte reste elle-même");
    }

    /// §9.3 — les trois niveaux d'une aide sont ceux du launcher d'AC lui-même
    /// (`0,1,2` en face de `Off,Factory,On`) : une case à cocher n'en portait
    /// que deux, et `Factory` n'était pas celui qu'elle exprimait.
    #[test]
    fn the_three_assist_levels_reach_the_preset() {
        for (level, expected) in [(AssistLevel::Off, 0), (AssistLevel::Factory, 1), (AssistLevel::On, 2)] {
            let mut s = base_setup(SessionType::Practice);
            s.abs = level;
            s.traction_control = level;
            let v: Value = serde_json::from_str(&build_preset(&s).unwrap()).unwrap();
            let assists: Value = serde_json::from_str(v["AssistsData"].as_str().unwrap()).unwrap();
            assert_eq!(assists["Abs"], expected, "ABS {level:?}");
            assert_eq!(assists["TractionControl"], expected, "antipatinage {level:?}");
        }
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
