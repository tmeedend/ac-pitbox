//! A car's physics files, the way Assetto Corsa reads them: the unpacked
//! `data/` folder first, `data.acd` after.
//!
//! Unpacked first because a mod that ships both has edited the loose one, and
//! it is what AC itself reads. Three modules each carried their own copy of
//! this rule (`driver`, `steering`, `electronics`), down to the warning
//! logged when a loose file exists but cannot be read; the tech sheet would
//! have been the fourth (FICHE§9.1).

use std::path::Path;

use crate::acd;

/// One physics file of a car, as text.
///
/// `marker` is what the decrypted `data.acd` entry must contain for its key to
/// be believed (see [`acd::read_text`]); a loose file needs no such proof.
pub fn read(car_dir: &Path, car_id: &str, name: &str, marker: &str) -> Option<String> {
    loose(car_dir, name).or_else(|| acd::read_text(car_dir, car_id, name, marker))
}

/// Several physics files of one car, the container opened at most once.
///
/// The key of a `data.acd` is established on `engine.ini`, which every car
/// has, and then trusted for every entry — so an entry that is legitimately
/// empty (`drs.ini` on most road cars) still reads, as an empty string,
/// instead of failing the marker check [`read`] would put it through.
pub struct CarData<'a> {
    dir: &'a Path,
    car_id: &'a str,
    container: std::cell::OnceCell<Option<acd::Container>>,
}

/// The entry and section that prove a container's key.
const KEY_PROOF: (&str, &str) = ("engine.ini", "[ENGINE_DATA]");

impl<'a> CarData<'a> {
    pub fn new(dir: &'a Path, car_id: &'a str) -> Self {
        Self {
            dir,
            car_id,
            container: std::cell::OnceCell::new(),
        }
    }

    fn container(&self) -> Option<&acd::Container> {
        self.container
            .get_or_init(|| acd::Container::open(self.dir, self.car_id, KEY_PROOF.0, KEY_PROOF.1))
            .as_ref()
    }

    /// One file as text, loose first.
    pub fn text(&self, name: &str) -> Option<String> {
        loose(self.dir, name).or_else(|| self.container()?.text(name))
    }

    /// Whether the car ships this file at all.
    pub fn has(&self, name: &str) -> bool {
        self.dir.join("data").join(name).is_file() || self.container().is_some_and(|c| c.has(name))
    }
}

/// `data/<name>`, when the car ships its physics unpacked. Absent is silence;
/// present but unreadable is logged, since the caller then falls back on the
/// container and would otherwise read a file the author did not mean.
fn loose(car_dir: &Path, name: &str) -> Option<String> {
    let path = car_dir.join("data").join(name);
    match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            log::warn!("car data: {} unreadable — {e}", path.display());
            None
        }
        Err(_) => None,
    }
}

/// `KEY=value` inside a named section, comments stripped. Section names are
/// compared case-insensitively — `[DRIVER_80]` and `[driver_80]` both occur.
pub fn ini_value<'a>(text: &'a str, section: &str, key: &str) -> Option<&'a str> {
    let mut inside = false;
    for line in text.lines() {
        let line = line.split(';').next().unwrap_or("").trim();
        if line.starts_with('[') {
            inside = line.eq_ignore_ascii_case(section);
            continue;
        }
        if !inside {
            continue;
        }
        if let Some((name, value)) = line.split_once('=') {
            if name.trim().eq_ignore_ascii_case(key) {
                return Some(value.trim());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The loose file wins over the container: it is the one the author
    /// edited, and the one the game reads.
    #[test]
    fn the_unpacked_file_is_read_before_the_container() {
        let base = crate::testutil::temp_dir("cardata-loose");
        std::fs::create_dir_all(base.join("data")).unwrap();
        std::fs::write(base.join("data").join("car.ini"), "[CONTROLS]\nSTEER_LOCK=400\n").unwrap();
        // A container that cannot be opened: were it consulted, nothing would come back.
        std::fs::write(base.join("data.acd"), b"not a container").unwrap();
        let text = read(&base, "any_car", "car.ini", "[CONTROLS]").expect("the loose file is read");
        assert!(
            text.contains("STEER_LOCK=400"),
            "the loose content, not the container's"
        );
    }

    /// Nothing on disk is an absence, not an error.
    #[test]
    fn a_car_without_physics_says_nothing() {
        let base = crate::testutil::temp_dir("cardata-none");
        assert!(read(&base, "any_car", "car.ini", "[CONTROLS]").is_none());
    }

    /// A key is looked up in its own section only, and the section name is
    /// case-insensitive.
    #[test]
    fn a_value_is_scoped_to_its_section() {
        let text = "[ABS]\nPRESENT=0\n[edl]\nPRESENT=1 ; comment\n";
        assert_eq!(ini_value(text, "[ABS]", "PRESENT"), Some("0"));
        assert_eq!(ini_value(text, "[EDL]", "PRESENT"), Some("1"));
        assert_eq!(ini_value(text, "[TRACTION_CONTROL]", "PRESENT"), None);
    }
}
