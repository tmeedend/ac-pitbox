//! Lecture du kilométrage Content Manager (§6.5), source fragile mais riche.
//!
//! On lit le **journal de sessions** de CM (`Profile (Sessions).data` sous
//! `%LOCALAPPDATA%\AcTools Content Manager\Progress\`), un flux de lignes JSON
//! (une par session) avec `CarId`, `TrackId`, `Distance` (en mètres). On somme
//! `Distance` par voiture et par circuit. On évite ainsi le binaire compressé
//! `Profile.data`/`Values.data` (format « Storage » de CM, difficile à parser).
//!
//! Fragilités assumées (§6.5) : 0 km ≠ « jamais essayé » (resets de CM), d'où la
//! combinaison avec le marqueur propre de l'app.

use std::collections::HashMap;

use serde::Deserialize;

/// Distances cumulées par identifiant (voiture ou base de circuit).
#[derive(Debug, Clone, Default)]
pub struct CmUsage {
    /// Mètres cumulés, clé = `CarId` ou base de `TrackId` (avant le `/` de layout).
    pub distance_m: HashMap<String, f64>,
}

impl CmUsage {
    /// Distance en km pour un id de mod (voiture ou circuit), si connue de CM.
    pub fn km(&self, id: &str) -> Option<f64> {
        self.distance_m.get(id).map(|m| m / 1000.0)
    }
}

#[derive(Deserialize)]
struct Session {
    #[serde(rename = "CarId")]
    car_id: Option<String>,
    #[serde(rename = "TrackId")]
    track_id: Option<String>,
    #[serde(rename = "Distance")]
    distance: Option<f64>,
}

/// Chemin du journal de sessions CM (`%LOCALAPPDATA%\AcTools Content Manager\…`).
fn sessions_path() -> Option<std::path::PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    Some(
        std::path::Path::new(&local)
            .join("AcTools Content Manager")
            .join("Progress")
            .join("Profile (Sessions).data"),
    )
}

/// Lit et agrège le kilométrage CM. Renvoie une structure vide si le fichier est
/// absent/illisible (CM jamais utilisé, chemins non standard…).
pub fn read() -> CmUsage {
    let mut usage = CmUsage::default();
    let Some(path) = sessions_path() else {
        return usage;
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return usage;
    };
    let text = String::from_utf8_lossy(&bytes);
    for obj in json_objects(&text) {
        let Ok(s) = serde_json::from_str::<Session>(obj) else {
            continue;
        };
        let Some(dist) = s.distance else { continue };
        if let Some(car) = s.car_id.filter(|c| !c.is_empty()) {
            *usage.distance_m.entry(car).or_insert(0.0) += dist;
        }
        if let Some(track) = s.track_id.filter(|t| !t.is_empty()) {
            // `TrackId` inclut le layout (`spa/2022`) : on agrège sur la base.
            let base = track.split('/').next().unwrap_or(&track).to_string();
            *usage.distance_m.entry(base).or_insert(0.0) += dist;
        }
    }
    usage
}

/// Extrait les objets JSON `{…}` d'un flux contenant des octets de séparation
/// non-JSON entre les enregistrements. Suit les chaînes et les échappements pour
/// ne pas couper sur une accolade littérale.
fn json_objects(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut in_str = false;
    let mut escaped = false;
    for (i, &b) in bytes.iter().enumerate() {
        if in_str {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => {
                if depth == 0 {
                    start = i;
                }
                depth += 1;
            }
            b'}' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    out.push(&text[start..=i]);
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregates_sessions() {
        let raw = "\u{1}{\"CarId\":\"ks_porsche\",\"TrackId\":\"spa/2022\",\"Distance\":11943.3}\n\u{2}{\"CarId\":\"ks_porsche\",\"TrackId\":\"spa\",\"Distance\":2000.0}\n";
        let objs = json_objects(raw);
        assert_eq!(objs.len(), 2);
        let mut u = CmUsage::default();
        for o in objs {
            let s: Session = serde_json::from_str(o).unwrap();
            let d = s.distance.unwrap();
            *u.distance_m.entry(s.car_id.unwrap()).or_insert(0.0) += d;
            let base = s.track_id.unwrap().split('/').next().unwrap().to_string();
            *u.distance_m.entry(base).or_insert(0.0) += d;
        }
        // Voiture : 11943.3 + 2000 = 13943.3 m ≈ 13.94 km.
        assert!((u.km("ks_porsche").unwrap() - 13.9433).abs() < 1e-3);
        // Circuit spa (les deux sessions, layout agrégé).
        assert!((u.km("spa").unwrap() - 13.9433).abs() < 1e-3);
        assert!(u.km("inconnu").is_none());
    }
}
