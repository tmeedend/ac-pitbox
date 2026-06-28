//! Météo simplifiée à dégradé gracieux (§8.5). L'utilisateur choisit une
//! **intention** (Beau, Pluie…) ; l'app la traduit dans le meilleur dossier
//! météo disponible selon la stack détectée (SOL riche → vanilla limité), avec
//! **température implicite** (jamais saisie). Périmètre v1 : météo statique.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::config::AppConfig;

#[derive(Debug, Clone, Serialize)]
pub struct WeatherStack {
    pub csp: bool,
    pub sol: bool,
    pub vanilla: bool,
}

pub fn detect_stack(cfg: &AppConfig) -> WeatherStack {
    let Some(ac) = &cfg.ac_install_path else {
        return WeatherStack { csp: false, sol: false, vanilla: false };
    };
    let csp = ac.join("dwrite.dll").is_file() || ac.join("extension").is_dir();
    let weather_dir = ac.join("content").join("weather");
    let mut sol = false;
    let mut vanilla = false;
    if let Ok(entries) = std::fs::read_dir(&weather_dir) {
        for e in entries.flatten() {
            if !e.path().is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_lowercase();
            if name.starts_with("sol_") {
                sol = true;
            } else if name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                vanilla = true;
            }
        }
    }
    WeatherStack { csp, sol, vanilla }
}

/// Une intention météo et sa résolution pour l'install courante.
#[derive(Debug, Clone, Serialize)]
pub struct WeatherOption {
    pub id: String,
    pub label: String,
    pub available: bool,
    /// Dossier météo retenu pour le race.ini (si disponible).
    pub weather: Option<String>,
    /// Backend retenu, ex. "via SOL".
    pub backend: Option<String>,
    /// Raison de l'indisponibilité, le cas échéant.
    pub reason: Option<String>,
    /// La pluie est-elle impliquée (nécessite CSP pour le rendu) ?
    pub wet: bool,
}

struct Intent {
    id: &'static str,
    label: &'static str,
    sol: &'static [&'static str],
    vanilla: &'static [&'static str],
    wet: bool,
}

const INTENTS: &[Intent] = &[
    Intent { id: "clear", label: "Beau", sol: &["sol_01_clear", "sol_00_no_clouds"], vanilla: &["3_clear", "4_mid_clear"], wet: false },
    Intent { id: "few_clouds", label: "Quelques nuages", sol: &["sol_02_few_clouds", "sol_03_scattered_clouds"], vanilla: &["5_light_clouds", "4_mid_clear"], wet: false },
    Intent { id: "overcast", label: "Couvert", sol: &["sol_06_overcast", "sol_05_broken_clouds"], vanilla: &["7_heavy_clouds", "6_mid_clouds"], wet: false },
    Intent { id: "fog", label: "Brouillard", sol: &["sol_12_fog", "sol_11_mist"], vanilla: &["1_heavy_fog", "2_light_fog"], wet: false },
    Intent { id: "light_rain", label: "Pluie légère", sol: &["sol_34_light_rain", "sol_31_light_drizzle"], vanilla: &[], wet: true },
    Intent { id: "rain", label: "Pluie", sol: &["sol_35_rain", "sol_36_heavy_rain"], vanilla: &[], wet: true },
    Intent { id: "storm", label: "Orage", sol: &["sol_42_thunderstorm", "sol_43_heavy_thunderstorm"], vanilla: &[], wet: true },
];

fn installed_weathers(cfg: &AppConfig) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    if let Some(ac) = &cfg.ac_install_path {
        if let Ok(entries) = std::fs::read_dir(ac.join("content").join("weather")) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    if let Some(n) = e.file_name().to_str() {
                        set.insert(n.to_string());
                    }
                }
            }
        }
    }
    set
}

pub fn options(cfg: &AppConfig) -> Vec<WeatherOption> {
    let stack = detect_stack(cfg);
    let installed = installed_weathers(cfg);
    let pick = |cands: &[&str]| cands.iter().find(|c| installed.contains(**c)).map(|s| s.to_string());

    INTENTS
        .iter()
        .map(|it| {
            let sol_pick = pick(it.sol);
            let vanilla_pick = pick(it.vanilla);
            let (weather, backend, available, reason) = if let Some(w) = sol_pick {
                (Some(w), Some("via SOL".to_string()), true, None)
            } else if let Some(w) = vanilla_pick {
                (Some(w), Some("Standard".to_string()), true, None)
            } else {
                let reason = if it.wet {
                    "Nécessite SOL (+ CSP) pour la pluie".to_string()
                } else {
                    "Aucune météo correspondante installée".to_string()
                };
                (None, None, false, Some(reason))
            };
            // La pluie nécessite CSP pour le rendu, même via SOL.
            let (available, reason) = if it.wet && available && !stack.csp {
                (false, Some("Nécessite CSP pour le rendu de la pluie".to_string()))
            } else {
                (available, reason)
            };
            WeatherOption {
                id: it.id.to_string(),
                label: it.label.to_string(),
                available,
                weather,
                backend,
                reason,
                wet: it.wet,
            }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ImplicitTemp {
    pub ambient: i32,
    pub road: i32,
}

/// Température implicite (§8.5) déduite de l'intention + l'heure. Lecture seule.
pub fn implicit_temp(intent_id: &str, hour: f32) -> ImplicitTemp {
    // (ambient de référence à ~14h, écart piste-air) par intention.
    let (base, road_delta) = match intent_id {
        "clear" => (26, 9),
        "few_clouds" => (24, 6),
        "overcast" => (20, 3),
        "fog" => (14, 2),
        "light_rain" => (17, 1),
        "rain" => (15, 1),
        "storm" => (16, 1),
        _ => (22, 5),
    };
    // Refroidissement aux heures extrêmes (matin/soir).
    let adj = (-0.7 * (hour - 14.0).abs()).round() as i32;
    let ambient = (base + adj).clamp(5, 42);
    ImplicitTemp { ambient, road: ambient + road_delta }
}
