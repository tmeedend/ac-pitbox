//! What a car says about its steering — `car.ini`, section `[CONTROLS]`.
//!
//! Read for one thing only: saying how far the **road wheels** of the 3D
//! preview turn for a given angle at the steering wheel
//! (`docs/SPEC-preview-3d-kn5.md` §15). AC turns them from physics, so nothing
//! in the model says how far they go — no animation covers them, unlike the
//! driver's arms, which the car's own `steer.ksanim` poses.
//!
//! The two values travel **into the converted model** (`kn5_gltf::SteerLimits`)
//! rather than being applied here: the angle is not baked, it is turned at
//! render time, so it is the viewer that divides by the ratio and stops at the
//! lock.
//!
//! Same two-source rule as the rest of the physics files: the unpacked `data/`
//! folder first, `data.acd` after (see `driver::data_file`).

use std::path::Path;

use crate::acd;

/// Section `car.ini` carries the two values, and the marker that says a
/// `data.acd` entry decrypted into the right file (see [`acd::read_text`]).
const CONTROLS_SECTION: &str = "[CONTROLS]";

/// Steering travel and ratio, or what to assume when a car is silent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Steering {
    /// Steering-wheel travel **from centre to full lock**, in degrees
    /// (`STEER_LOCK`). Not the same figure as `[STEER_ANIMATION] LOCK` of
    /// `driver3d.ini`, which is the whole span the arm animation covers.
    pub lock: f32,
    /// Steering-wheel degrees per degree of road wheel (`STEER_RATIO`).
    pub ratio: f32,
}

/// Assumed when the car says nothing — see the corpus measurement below.
impl Default for Steering {
    fn default() -> Self {
        Self {
            lock: 360.0,
            ratio: 14.0,
        }
    }
}

/// Reads `car.ini` for this car, falling back on [`Steering::default`] for
/// whichever of the two values is missing or absurd.
///
/// Never an error: a preview whose wheels stay straight is a lesser fault than
/// one that refuses to convert.
pub fn read(car_dir: &Path, car_id: &str) -> Steering {
    let mut steering = Steering::default();
    let Some(text) = data_file(car_dir, car_id) else {
        return steering;
    };
    if let Some(lock) = number(&text, "STEER_LOCK").filter(|v| *v > 0.0) {
        steering.lock = lock;
    }
    if let Some(ratio) = number(&text, "STEER_RATIO").filter(|v| *v > 0.0) {
        steering.ratio = ratio;
    }
    steering
}

/// `car.ini`, unpacked folder first — a mod that ships both has edited the
/// loose one, and it is what AC itself reads.
fn data_file(car_dir: &Path, car_id: &str) -> Option<String> {
    let loose = car_dir.join("data").join("car.ini");
    match std::fs::read_to_string(&loose) {
        Ok(text) => return Some(text),
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            log::warn!("steering: {} unreadable — {e}", loose.display());
        }
        Err(_) => {}
    }
    acd::read_text(car_dir, car_id, "car.ini", CONTROLS_SECTION)
}

/// `KEY=value` anywhere in the file, comments stripped.
///
/// Section-blind on purpose: both keys are unique to `[CONTROLS]` across every
/// `car.ini` of the reference install, and a mod that indents its sections
/// oddly still gets read.
fn number(text: &str, key: &str) -> Option<f32> {
    for line in text.lines() {
        let line = line.split(';').next().unwrap_or(line).trim();
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case(key) {
            return value.trim().parse().ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_read_past_comments() {
        let text = "[CONTROLS]\n; STEER_LOCK=999\nSTEER_LOCK=400 ; le vrai\nSTEER_RATIO=12\n";
        assert_eq!(number(text, "STEER_LOCK"), Some(400.0));
        assert_eq!(number(text, "STEER_RATIO"), Some(12.0));
        assert_eq!(number(text, "STEER_ASSIST"), None);
    }

    /// Ce que les deux valeurs valent réellement, et ce qu'elles produisent
    /// comme angle de roue à fond de course.
    ///
    /// ```text
    /// PITBOX_CARS_ROOT="D:\AC-Library\cars" cargo test --lib steering -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "needs a real corpus of cars; measurement, not a check"]
    fn how_steering_is_written_across_a_corpus() {
        let Ok(root) = std::env::var("PITBOX_CARS_ROOT") else {
            eprintln!("PITBOX_CARS_ROOT unset, skipping");
            return;
        };
        let mut folders = Vec::new();
        for entry in std::fs::read_dir(&root).expect("read the corpus root").flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if path.join("data.acd").is_file() || path.join("data").is_dir() {
                folders.push(path);
                continue;
            }
            for nested in std::fs::read_dir(&path).into_iter().flatten().flatten() {
                let nested = nested.path();
                if nested.join("data.acd").is_file() || nested.join("data").is_dir() {
                    folders.push(nested);
                }
            }
        }

        let (mut read_ok, mut defaulted) = (0, 0);
        let mut locks: std::collections::BTreeMap<i32, usize> = std::collections::BTreeMap::new();
        let mut ratios: std::collections::BTreeMap<i32, usize> = std::collections::BTreeMap::new();
        let mut full_lock: Vec<f32> = Vec::new();
        for dir in &folders {
            let id = dir.file_name().unwrap_or_default().to_string_lossy().into_owned();
            // Une bibliothèque range `<mod>/<version>/`, donc le nom du dossier
            // parent est la clé de `data.acd`, pas celui de la version.
            let key = if dir.join("data.acd").is_file() && dir.parent().is_some_and(|p| p.join("data.acd").is_file()) {
                id.clone()
            } else {
                dir.parent()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| id.clone())
            };
            let steering = read(dir, &key);
            if steering == Steering::default() {
                defaulted += 1;
            } else {
                read_ok += 1;
            }
            if std::env::var("PITBOX_CAR").is_ok_and(|want| id.contains(&want) || key.contains(&want)) {
                eprintln!(
                    "  {id} : STEER_LOCK {} · STEER_RATIO {} · butée {:.1}° aux roues",
                    steering.lock,
                    steering.ratio,
                    steering.lock / steering.ratio
                );
            }
            *locks.entry(steering.lock.round() as i32).or_default() += 1;
            *ratios.entry(steering.ratio.round() as i32).or_default() += 1;
            // L'angle de roue à fond de course, tel que la vue le calcule.
            full_lock.push(if steering.ratio.abs() < f32::EPSILON {
                0.0
            } else {
                steering.lock / steering.ratio
            });
        }
        full_lock.sort_by(|a, b| a.partial_cmp(b).unwrap());
        eprintln!("{} dossiers, {read_ok} lus, {defaulted} au défaut", folders.len());
        eprintln!("STEER_LOCK  : {locks:?}");
        eprintln!("STEER_RATIO : {ratios:?}");
        if !full_lock.is_empty() {
            eprintln!(
                "angle de roue à fond de course : min {:.1}° médiane {:.1}° max {:.1}°",
                full_lock[0],
                full_lock[full_lock.len() / 2],
                full_lock[full_lock.len() - 1]
            );
        }
    }
}
