//! Which driver a car seats, and what it wears (SPEC §4.6).
//!
//! Assetto Corsa splits a driver in two, and the split is the whole difficulty
//! of ever offering a list of them:
//!
//! | | Where it lives | Who chooses it |
//! | --- | --- | --- |
//! | **mannequin** (3D) | `<AC>/content/driver/<name>.kn5` | the car, in `driver3d.ini` |
//! | **wardrobe** (textures) | `<AC>/content/texture/driver_{suit,gloves,helmet}/…` | the skin, in `skin.ini` |
//!
//! A `skin.ini` names its wardrobe **under the mannequin's own name**:
//!
//! ```ini
//! [driver_80]                  ; only read when driver3d.ini asked for driver_80
//! SUIT=\plain\red              ; → content/texture/driver_suit/plain/red/
//! GLOVES=\classicpastel\blue_lite
//! HELMET=\helmet_1985\blue
//! ```
//!
//! …and the folder it points at holds `.dds` files named exactly as the
//! mannequin's materials ask for them (`2016_Suit_DIFF.dds`,
//! `HELMET_1985.dds`). That is why a wardrobe is not portable between
//! mannequins: a modern helmet folder holds `HELMET_2012.dds`, which the
//! seventies mannequin never asks for. Any future driver picker has to offer
//! **pairs**, never a model and an outfit chosen independently.
//!
//! **Where the driver sits** is a third file again: `car.ini`, whose
//! `[GRAPHICS] DRIVEREYES` gives a pair of eyes in the car's own space. The
//! mannequin is the same seated body whatever the car, so that line is what
//! tells a formula car's driver from a saloon's — and, when its `x` is
//! negative, a right-hand-drive one. `driver3d.ini`'s own `POSITION` is a
//! fine-tuning offset added on top, and reads `0,0,0` on all 312 cars of the
//! reference install.
//!
//! Everything here is best-effort: a car whose driver cannot be resolved is
//! previewed without one, which is exactly what the preview did before this
//! module existed.

use std::path::{Path, PathBuf};

use crate::acd;

/// A resolved driver, in AC's own vocabulary — the shape a future picker will
/// produce, and the reason resolution is split in two halves: reading what the
/// car and the skin declare ([`outfit_of`]) is separate from turning that into
/// files ([`graft_for`]).
#[derive(Debug, Clone, PartialEq)]
pub struct DriverOutfit {
    /// Mannequin name, without extension: `driver`, `driver_80`, `gt-m_pro`…
    pub model: String,
    /// Where the car puts a pair of eyes, `[GRAPHICS] DRIVEREYES` of
    /// `car.ini` — the one line that says where the driver sits (see
    /// `kn5_gltf::DriverGraft::anchor`).
    pub eyes: Option<[f32; 3]>,
    /// Offset the car applies to the mannequin, `[MODEL] POSITION`.
    pub position: [f32; 3],
    /// Wardrobe paths as `skin.ini` writes them, relative to their kind's
    /// folder: `plain/red`, `helmet_1985/blue`. `None` when the skin says
    /// nothing, in which case the mannequin keeps its own textures.
    pub suit: Option<String>,
    pub gloves: Option<String>,
    pub helmet: Option<String>,
}

/// Folder under `content/texture/` each wardrobe key points into.
const SUIT_DIR: &str = "driver_suit";
const GLOVES_DIR: &str = "driver_gloves";
const HELMET_DIR: &str = "driver_helmet";

/// Section every `driver3d.ini` carries — the known plaintext that says a
/// `data.acd` key is the right one (see [`acd::read_text`]).
const MODEL_SECTION: &str = "[MODEL]";
/// Same role for `car.ini`, whose `[GRAPHICS]` section carries `DRIVEREYES`.
const GRAPHICS_SECTION: &str = "[GRAPHICS]";

/// The driver AC would seat in this car, ready to graft.
///
/// `None` — never an error — when the car names no driver, when the mannequin
/// is not installed, or when Assetto Corsa itself is not configured: a preview
/// without a driver is the normal outcome in all three cases.
pub fn resolve(ac_root: &Path, car_dir: &Path, car_id: &str, skin_dir: Option<&Path>) -> Option<kn5_gltf::DriverGraft> {
    let outfit = outfit_of(car_dir, car_id, skin_dir)?;
    graft_for(ac_root, &outfit)
}

/// Reads what the car and its skin declare, without touching the AC install.
pub fn outfit_of(car_dir: &Path, car_id: &str, skin_dir: Option<&Path>) -> Option<DriverOutfit> {
    let ini = driver3d_ini(car_dir, car_id)?;
    let model = ini_value(&ini, MODEL_SECTION, "NAME")?.to_string();
    if model.is_empty() {
        return None;
    }
    let position = ini_value(&ini, MODEL_SECTION, "POSITION")
        .and_then(parse_position)
        .unwrap_or([0.0; 3]);

    // The skin's wardrobe is read under the mannequin's name: a `skin.ini`
    // written for `driver_80` says nothing about the `driver` a CSP config may
    // have substituted, and applying it anyway would dress the wrong body.
    let wardrobe = skin_dir
        .map(|dir| dir.join("skin.ini"))
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default();
    let section = format!("[{model}]");

    Some(DriverOutfit {
        suit: wardrobe_path(&wardrobe, &section, "SUIT"),
        gloves: wardrobe_path(&wardrobe, &section, "GLOVES"),
        helmet: wardrobe_path(&wardrobe, &section, "HELMET"),
        eyes: car_ini(car_dir, car_id)
            .as_deref()
            .and_then(|text| ini_value(text, GRAPHICS_SECTION, "DRIVEREYES"))
            .and_then(parse_position),
        model,
        position,
    })
}

/// Turns a resolved outfit into the files the converter needs.
///
/// The wardrobe folders come **before** nothing else: they are the only
/// sources of override the graft knows about. The car skin's own loose `.dds`
/// — some mods drop `2016_Helmet_Base_D.dds` straight into the skin folder —
/// are handled a layer further down, by the texture loader, which already
/// prefers a skin file over an embedded blob for every texture in the model.
pub fn graft_for(ac_root: &Path, outfit: &DriverOutfit) -> Option<kn5_gltf::DriverGraft> {
    let model = ac_root
        .join("content")
        .join("driver")
        .join(format!("{}.kn5", outfit.model));
    if !model.is_file() {
        // Common enough to not deserve a warning at every preview: a mod car
        // may ask for a mannequin its author shipped separately, or not at all.
        log::debug!("driver: mannequin {} not installed", model.display());
        return None;
    }

    let textures = ac_root.join("content").join("texture");
    let dirs = [
        (HELMET_DIR, outfit.helmet.as_deref()),
        (GLOVES_DIR, outfit.gloves.as_deref()),
        (SUIT_DIR, outfit.suit.as_deref()),
    ];
    let texture_dirs = dirs
        .iter()
        .filter_map(|(kind, wanted)| wardrobe_dir(&textures.join(kind), (*wanted)?))
        .collect();

    Some(kn5_gltf::DriverGraft {
        model,
        anchor: outfit.eyes,
        position: outfit.position,
        texture_dirs,
    })
}

/// Joins a `skin.ini` wardrobe path onto its kind's folder, refusing anything
/// that would leave it.
///
/// The value comes out of a mod's own file, so `..` and drive letters are
/// treated as what they would be: an attempt to read outside `content/texture`.
fn wardrobe_dir(kind_dir: &Path, wanted: &str) -> Option<PathBuf> {
    let mut path = kind_dir.to_path_buf();
    for part in wanted.split(['\\', '/']).filter(|p| !p.is_empty()) {
        if part == "." || part == ".." || part.contains(':') {
            log::warn!("driver: wardrobe path `{wanted}` refused");
            return None;
        }
        path.push(part);
    }
    if path == kind_dir {
        return None;
    }
    path.is_dir().then_some(path)
}

fn driver3d_ini(car_dir: &Path, car_id: &str) -> Option<String> {
    data_file(car_dir, car_id, "driver3d.ini", MODEL_SECTION)
}

fn car_ini(car_dir: &Path, car_id: &str) -> Option<String> {
    data_file(car_dir, car_id, "car.ini", GRAPHICS_SECTION)
}

/// One of a car's physics files, from the unpacked `data/` folder or from
/// `data.acd`.
///
/// Unpacked first: a mod that ships both has edited the loose one, and it is
/// what AC itself reads.
fn data_file(car_dir: &Path, car_id: &str, name: &str, marker: &str) -> Option<String> {
    let loose = car_dir.join("data").join(name);
    match std::fs::read_to_string(&loose) {
        Ok(text) => return Some(text),
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            log::warn!("driver: {} unreadable — {e}", loose.display());
        }
        Err(_) => {}
    }
    acd::read_text(car_dir, car_id, name, marker)
}

/// Reads one wardrobe key, `None` when it is absent or empty.
fn wardrobe_path(text: &str, section: &str, key: &str) -> Option<String> {
    ini_value(text, section, key)
        .map(|v| v.trim_matches(['\\', '/']).to_string())
        .filter(|v| !v.is_empty())
}

/// `KEY=value` inside a named section, comments stripped. Section names are
/// compared case-insensitively — `[DRIVER_80]` and `[driver_80]` both occur.
fn ini_value<'a>(text: &'a str, section: &str, key: &str) -> Option<&'a str> {
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

/// `POSITION=x,y,z`, in metres. A malformed one is dropped rather than
/// half-read: a driver an axis off is worse than a driver at the origin.
fn parse_position(value: &str) -> Option<[f32; 3]> {
    let mut out = [0.0f32; 3];
    let mut parts = value.split(',');
    for slot in &mut out {
        *slot = parts.next()?.trim().parse().ok()?;
    }
    parts.next().is_none().then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAR_INI: &str = "\
[BASIC]
TOTALMASS=1050

[GRAPHICS]
DRIVEREYES=0.330737,1.19075,-0.490002
ONBOARD_EXPOSURE=20
";

    const DRIVER3D: &str = "\
[MODEL]
NAME=driver_80
POSITION=-0.01, 0.02, 0.03

[STEER_ANIMATION]
NAME=steer.ksanim

[HIDE_OBJECT_0]
NAME=DRIVER:HELMET1985
";

    const SKIN_INI: &str = "\
[driver_80]
SUIT=\\plain\\red
GLOVES=\\classicpastel\\blue_lite
HELMET=\\helmet_1985\\blue

[CREW]
SUIT=\\type1\\black_black
";

    /// A car folder with a loose `data/driver3d.ini` and one skin.
    fn fake_car(base: &Path, skin_ini: Option<&str>) -> (PathBuf, Option<PathBuf>) {
        let car = base.join("ks_fake");
        std::fs::create_dir_all(car.join("data")).expect("car data folder");
        std::fs::write(car.join("data").join("driver3d.ini"), DRIVER3D).expect("driver3d.ini");
        std::fs::write(car.join("data").join("car.ini"), CAR_INI).expect("car.ini");
        let skin = skin_ini.map(|text| {
            let dir = car.join("skins").join("red");
            std::fs::create_dir_all(&dir).expect("skin folder");
            std::fs::write(dir.join("skin.ini"), text).expect("skin.ini");
            dir
        });
        (car, skin)
    }

    // Rule: the mannequin comes from the car's driver3d.ini, the wardrobe from
    // the skin's skin.ini — and the wardrobe is read under the mannequin's own
    // name (§4.6).
    #[test]
    fn the_car_names_the_mannequin_and_the_skin_dresses_it() {
        let base = crate::testutil::temp_dir("driver-outfit");
        let (car, skin) = fake_car(&base, Some(SKIN_INI));

        let outfit = outfit_of(&car, "ks_fake", skin.as_deref()).expect("an outfit");

        assert_eq!(outfit.model, "driver_80", "mannequin read from [MODEL] NAME");
        assert_eq!(outfit.position, [-0.01, 0.02, 0.03], "POSITION read as three metres");
        assert_eq!(
            outfit.eyes,
            Some([0.330737, 1.19075, -0.490002]),
            "DRIVEREYES read from car.ini — the one line that seats the mannequin"
        );
        assert_eq!(outfit.suit.as_deref(), Some("plain\\red"), "leading separator stripped");
        assert_eq!(outfit.gloves.as_deref(), Some("classicpastel\\blue_lite"));
        assert_eq!(outfit.helmet.as_deref(), Some("helmet_1985\\blue"));
    }

    // Rule: a `skin.ini` written for another mannequin dresses nobody — its
    // file names would not match the materials of the one actually loaded.
    #[test]
    fn a_wardrobe_written_for_another_mannequin_is_ignored() {
        let base = crate::testutil::temp_dir("driver-other");
        let (car, skin) = fake_car(&base, Some("[driver]\nSUIT=\\sparco\\red\n"));

        let outfit = outfit_of(&car, "ks_fake", skin.as_deref()).expect("an outfit");

        assert_eq!(outfit.model, "driver_80", "the car still names its mannequin");
        assert_eq!(outfit.suit, None, "the [driver] section is not ours");
    }

    // Rule: no skin, or a skin without `skin.ini`, still yields a driver — he
    // simply wears what the mannequin was shipped with.
    #[test]
    fn a_skin_without_a_wardrobe_still_yields_a_driver() {
        let base = crate::testutil::temp_dir("driver-bare");
        let (car, _) = fake_car(&base, None);

        let outfit = outfit_of(&car, "ks_fake", None).expect("an outfit");

        assert_eq!(outfit.model, "driver_80");
        assert!(
            outfit.suit.is_none() && outfit.gloves.is_none() && outfit.helmet.is_none(),
            "nothing to dress it with"
        );
    }

    // Rule: a wardrobe path never leaves `content/texture/<kind>` — the value
    // comes out of a mod's own file.
    #[test]
    fn a_wardrobe_path_cannot_escape_its_folder() {
        let base = crate::testutil::temp_dir("driver-escape");
        let kind = base.join("driver_suit");
        let inside = kind.join("plain").join("red");
        std::fs::create_dir_all(&inside).expect("wardrobe folder");

        assert_eq!(wardrobe_dir(&kind, "plain\\red"), Some(inside), "an ordinary path");
        assert_eq!(wardrobe_dir(&kind, "..\\..\\windows"), None, "climbing out is refused");
        assert_eq!(wardrobe_dir(&kind, "C:\\windows"), None, "an absolute path is refused");
        assert_eq!(wardrobe_dir(&kind, "plain\\green"), None, "a folder that is not there");
    }

    // Rule: a malformed POSITION is dropped whole, never half-read.
    #[test]
    fn a_malformed_position_falls_back_to_the_origin() {
        assert_eq!(parse_position("0, 0, 0"), Some([0.0; 3]));
        assert_eq!(parse_position("-0.0,0.1,0.2"), Some([-0.0, 0.1, 0.2]));
        assert_eq!(parse_position("0, 0"), None, "two numbers are not a position");
        assert_eq!(parse_position("0, 0, 0, 0"), None, "nor are four");
        assert_eq!(parse_position("0, x, 0"), None, "nor is a word");
    }
}
