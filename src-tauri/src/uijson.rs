//! Lecture **strictement en lecture seule** des `ui_car.json` / `ui_track.json`.
//!
//! Règle d'or (§3.0) : ces fichiers ne sont JAMAIS réécrits. On les lit comme
//! une entrée du pipeline, jamais comme une sortie.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

/// Champs natifs exploités à l'import (L1). Les specs/courbes (fiche technique)
/// relèvent de L2 et seront lues plus tard.
/// `class` et `country` sont lus dès maintenant mais consommés en L2
/// (extraction classe race/street et pays).
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct UiInfo {
    pub name: Option<String>,
    pub brand: Option<String>,
    pub year: Option<i64>,
    pub version: Option<String>,
    pub author: Option<String>,
    pub class: Option<String>,
    pub country: Option<String>,
    /// Tags bruts lus dans le fichier (origine « fichier mod », lecture seule).
    pub tags: Vec<String>,
}

fn read_json(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    // Tolère un BOM UTF-8 en tête (fréquent sur les mods).
    let text = text.trim_start_matches('\u{feff}');
    serde_json::from_str(text).ok()
}

/// Convertit une valeur JSON en chaîne (gère nombre ou chaîne).
fn as_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        }
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn parse(path: &Path) -> Option<UiInfo> {
    let v = read_json(path)?;
    let year = v.get("year").and_then(|y| match y {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    });
    let tags = v
        .get("tags")
        .and_then(|t| t.as_array())
        .map(|arr| arr.iter().filter_map(as_string).collect())
        .unwrap_or_default();
    Some(UiInfo {
        name: v.get("name").and_then(as_string),
        brand: v.get("brand").and_then(as_string),
        year,
        version: v.get("version").and_then(as_string),
        author: v.get("author").and_then(as_string),
        class: v.get("class").and_then(as_string),
        country: v.get("country").and_then(as_string),
        tags,
    })
}

/// Chemin du `ui_car.json` d'un dossier voiture.
pub fn car_ui_path(car_dir: &Path) -> PathBuf {
    car_dir.join("ui").join("ui_car.json")
}

/// Chemin du `ui_track.json` d'un circuit : à la racine `ui/`, sinon dans le
/// premier sous-dossier de layout qui en contient un.
pub fn track_ui_path(track_dir: &Path) -> Option<PathBuf> {
    let ui = track_dir.join("ui");
    let root = ui.join("ui_track.json");
    if root.is_file() {
        return Some(root);
    }
    let entries = std::fs::read_dir(&ui).ok()?;
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            let layout_json = p.join("ui_track.json");
            if layout_json.is_file() {
                return Some(layout_json);
            }
        }
    }
    None
}

pub fn read_car(car_dir: &Path) -> Option<UiInfo> {
    parse(&car_ui_path(car_dir))
}

pub fn read_track(track_dir: &Path) -> Option<UiInfo> {
    parse(&track_ui_path(track_dir)?)
}

/// Fiche technique native d'une voiture (§5bis.1), lue directement dans
/// `ui_car.json`. `specs` est un OBJET de chaînes déjà formatées (pas de
/// parsing), les courbes sont des paires `[rpm, valeur]`. Lecture seule.
#[derive(Debug, Clone, Serialize, Default)]
pub struct NativeSpecs {
    pub bhp: Option<String>,
    pub torque: Option<String>,
    pub weight: Option<String>,
    pub topspeed: Option<String>,
    pub acceleration: Option<String>,
    pub pwratio: Option<String>,
    pub range: Option<String>,
    pub description: Option<String>,
    pub country: Option<String>,
    pub author: Option<String>,
    pub year: Option<i64>,
    pub power_curve: Vec<[f64; 2]>,
    pub torque_curve: Vec<[f64; 2]>,
}

fn curve(v: &Value, key: &str) -> Vec<[f64; 2]> {
    v.get(key)
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|pt| {
                    let p = pt.as_array()?;
                    Some([p.first()?.as_f64()?, p.get(1)?.as_f64()?])
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn read_car_specs(car_dir: &Path) -> Option<NativeSpecs> {
    let v = read_json(&car_ui_path(car_dir))?;
    let specs = v.get("specs");
    let sget = |k: &str| specs.and_then(|s| s.get(k)).and_then(as_string);
    let year = v.get("year").and_then(|y| match y {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    });
    Some(NativeSpecs {
        bhp: sget("bhp"),
        torque: sget("torque"),
        weight: sget("weight"),
        topspeed: sget("topspeed"),
        acceleration: sget("acceleration"),
        pwratio: sget("pwratio"),
        range: sget("range"),
        description: v.get("description").and_then(as_string),
        country: v.get("country").and_then(as_string),
        author: v.get("author").and_then(as_string),
        year,
        power_curve: curve(&v, "powerCurve"),
        torque_curve: curve(&v, "torqueCurve"),
    })
}
