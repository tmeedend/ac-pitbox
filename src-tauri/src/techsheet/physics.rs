//! What a car's physics says about it (FICHE§2.2, FICHE§5).
//!
//! Read once per version, at import or reindex, and stored as facts: the sheet
//! itself never opens a physics file (FICHE§3, R1). Every field is optional —
//! a mod's physics is less regular than Kunos', and a file that does not say
//! is a reason to fall back on the next source, never to guess.

use std::path::PathBuf;

use crate::cardata::{ini_value, CarData};

/// Everything the tech sheet reads in the physics files, raw. What it means
/// for the sheet (which source wins, how a count becomes a word) is decided
/// by the caller, [`super::facts_from_physics`].
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Physics {
    /// `drivetrain.ini [TRACTION] TYPE`, `AWD2` folded into `AWD`.
    pub traction: Option<String>,
    /// `ers.ini [FRONT_MOTORS]`: an electric front axle, which makes a
    /// rear-wheel-drive hybrid a four-wheel-drive car (FICHE§2.2, the 919
    /// Hybrid and its kin).
    pub front_motors: bool,
    /// `drivetrain.ini [GEARS] COUNT`.
    pub gears: Option<i64>,
    /// `drivetrain.ini [GEARBOX] SUPPORTS_SHIFTER`: an H-pattern is possible.
    pub h_shifter: Option<bool>,
    /// Number of `[TURBO_n]` sections of `engine.ini`. `Some(0)` is a real
    /// answer (naturally aspirated); `None` means the file was not read.
    pub turbos: Option<usize>,
    /// `engine.ini [ENGINE_DATA] LIMITER`, zero dropped.
    pub limiter: Option<f64>,
    /// `car.ini [FUEL] MAX_FUEL`, litres.
    pub max_fuel: Option<f64>,
    /// `car.ini [BASIC] TOTALMASS`, kg, driver included.
    pub total_mass: Option<f64>,
    /// `electronics.ini [ABS] PRESENT`.
    pub abs: Option<bool>,
    /// `electronics.ini [TRACTION_CONTROL] PRESENT` — `2` exists and means yes.
    pub traction_control: Option<bool>,
    /// `electronics.ini [EDL] PRESENT`.
    pub edl: Option<bool>,
    /// `drs.ini` with a `[WING_n]` section. The file alone proves nothing:
    /// it ships empty on most road cars.
    pub drs: bool,
    /// `kers.ini` with a `[KERS]` section.
    pub kers: bool,
    /// `ers.ini` with a `[KINETIC]` section.
    pub ers: bool,
    /// `ers.ini [HEAT]`: the MGU-H.
    pub ers_heat: bool,
    /// `ctrl_4ws.ini`: rear-wheel steering.
    pub four_ws: bool,
    /// `ctrl_arb_front.ini` or `ctrl_arb_rear.ini`: active anti-roll bars.
    pub active_arb: bool,
    /// `ctrl_ebb.ini`: electronic brake balance.
    pub ebb: bool,
}

/// Reads the physics of a car packed under `car_id`, through the stack of
/// its folders, the most important first (see `CarData`).
///
/// `None` when neither `data/` nor a `data.acd` that opens is there — the
/// sheet then falls back entirely on `ui_car.json` and the rules.
pub fn read(dirs: &[PathBuf], car_id: &str) -> Option<Physics> {
    let data = CarData::new(dirs, car_id);
    let engine = data.text("engine.ini");
    let drivetrain = data.text("drivetrain.ini");
    let car = data.text("car.ini");
    let electronics = data.text("electronics.ini");
    if engine.is_none() && drivetrain.is_none() && car.is_none() && electronics.is_none() {
        return None;
    }
    let electronics = electronics.unwrap_or_default();
    let ers = data.text("ers.ini").unwrap_or_default();
    Some(Physics {
        traction: drivetrain.as_deref().and_then(traction),
        front_motors: has_section(&ers, "[FRONT_MOTORS]"),
        gears: drivetrain
            .as_deref()
            .and_then(|t| number(t, "[GEARS]", "COUNT"))
            .filter(|n| *n > 0.0)
            .map(|n| n as i64),
        h_shifter: drivetrain
            .as_deref()
            .and_then(|t| number(t, "[GEARBOX]", "SUPPORTS_SHIFTER"))
            .map(|v| v != 0.0),
        turbos: engine.as_deref().map(turbo_sections),
        limiter: engine
            .as_deref()
            .and_then(|t| number(t, "[ENGINE_DATA]", "LIMITER"))
            .filter(|v| *v > 0.0),
        max_fuel: car
            .as_deref()
            .and_then(|t| number(t, "[FUEL]", "MAX_FUEL"))
            .filter(|v| *v > 0.0),
        total_mass: car
            .as_deref()
            .and_then(|t| number(t, "[BASIC]", "TOTALMASS"))
            .filter(|v| *v > 0.0),
        abs: present(&electronics, "[ABS]"),
        traction_control: present(&electronics, "[TRACTION_CONTROL]"),
        edl: present(&electronics, "[EDL]"),
        drs: data.text("drs.ini").is_some_and(|t| has_section_prefix(&t, "[WING_")),
        kers: data.text("kers.ini").is_some_and(|t| has_section(&t, "[KERS]")),
        ers: has_section(&ers, "[KINETIC]"),
        ers_heat: has_section(&ers, "[HEAT]"),
        four_ws: data.has("ctrl_4ws.ini"),
        active_arb: data.has("ctrl_arb_front.ini") || data.has("ctrl_arb_rear.ini"),
        ebb: data.has("ctrl_ebb.ini"),
    })
}

/// `[TRACTION] TYPE`, as the rules spell drivetrains (`RWD`, `FWD`, `AWD`).
/// `AWD2` is AC's second four-wheel-drive model, the same answer for a driver.
fn traction(text: &str) -> Option<String> {
    let v = ini_value(text, "[TRACTION]", "TYPE")?.to_ascii_uppercase();
    match v.as_str() {
        "RWD" | "FWD" | "AWD" => Some(v),
        "AWD2" => Some("AWD".into()),
        _ => None,
    }
}

fn number(text: &str, section: &str, key: &str) -> Option<f64> {
    ini_value(text, section, key)?.parse().ok()
}

/// `PRESENT` of an aid section: anything but zero means equipped (a `2` is
/// written on some cars). A section without the key says nothing.
fn present(text: &str, section: &str) -> Option<bool> {
    number(text, section, "PRESENT").map(|v| v != 0.0)
}

/// Section headers of an INI, comments stripped.
fn sections(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .map(|l| l.split(';').next().unwrap_or("").trim())
        .filter(|l| l.starts_with('['))
}

fn has_section(text: &str, name: &str) -> bool {
    sections(text).any(|s| s.eq_ignore_ascii_case(name))
}

fn has_section_prefix(text: &str, prefix: &str) -> bool {
    sections(text).any(|s| s.len() > prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix))
}

/// `[TURBO_0]`, `[TURBO_1]`… — not `[TURBO_BOOST_THRESHOLD]` or the like.
fn turbo_sections(text: &str) -> usize {
    sections(text)
        .filter(|s| {
            let upper = s.to_ascii_uppercase();
            upper
                .strip_prefix("[TURBO_")
                .and_then(|rest| rest.strip_suffix(']'))
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn car(files: &[(&str, &str)]) -> crate::testutil::TempDir {
        let base = crate::testutil::temp_dir("techsheet-physics");
        let data = base.join("data");
        std::fs::create_dir_all(&data).unwrap();
        for (name, text) in files {
            std::fs::write(data.join(name), text).unwrap();
        }
        base
    }

    const ENGINE_NA: &str = "[ENGINE_DATA]\nLIMITER=8300\nMINIMUM=1000\n";
    const DRIVETRAIN_RWD: &str = "[TRACTION]\nTYPE=RWD\n[GEARS]\nCOUNT=6\n[GEARBOX]\nSUPPORTS_SHIFTER=0\n";

    /// FICHE§2.2 — the five disagreements between physics and rules were the
    /// front-motor hybrids: `TYPE=RWD` for the combustion engine, and an
    /// electric front axle in `ers.ini`. They are four-wheel-drive cars.
    #[test]
    fn a_hybrid_with_front_motors_reads_as_rear_drive_plus_front_motors() {
        let dir = car(&[
            ("engine.ini", ENGINE_NA),
            ("drivetrain.ini", DRIVETRAIN_RWD),
            ("ers.ini", "[KINETIC]\nMAX_KJ=4000\n[FRONT_MOTORS]\nMAX_TORQUE=300\n"),
        ]);
        let p = read(&[dir.to_path_buf()], "hybrid").expect("physics read");
        assert_eq!(p.traction.as_deref(), Some("RWD"), "the engine drives the rear");
        assert!(p.front_motors, "the front axle is electric");
        assert!(p.ers && !p.ers_heat, "MGU-K only");
    }

    /// FICHE§2.2 — `drs.ini` ships empty on most road cars; only a
    /// `[WING_n]` section is a driver-operated wing.
    #[test]
    fn an_empty_drs_file_is_no_drs() {
        let empty = car(&[("engine.ini", ENGINE_NA), ("drs.ini", "")]);
        assert!(
            !read(&[empty.to_path_buf()], "c").unwrap().drs,
            "an empty drs.ini is not DRS"
        );
        let wing = car(&[("engine.ini", ENGINE_NA), ("drs.ini", "[WING_3]\nDRS_ANGLE=0\n")]);
        assert!(
            read(&[wing.to_path_buf()], "c").unwrap().drs,
            "a [WING_3] section is DRS"
        );
    }

    /// FICHE§5 — `PRESENT=2` exists on traction control and means equipped;
    /// the `[EDL]` section's own `PRESENT` must not leak into the others.
    #[test]
    fn aids_read_their_own_present_line() {
        let dir = car(&[
            ("engine.ini", ENGINE_NA),
            (
                "electronics.ini",
                "[ABS]\nPRESENT=0\n[TRACTION_CONTROL]\nPRESENT=2\n[EDL]\nPRESENT=1\n",
            ),
        ]);
        let p = read(&[dir.to_path_buf()], "c").unwrap();
        assert_eq!(p.abs, Some(false), "no ABS");
        assert_eq!(p.traction_control, Some(true), "PRESENT=2 is a yes");
        assert_eq!(p.edl, Some(true), "EDL");
    }

    /// Trimmed from the real `electronics.ini` of `ks_ford_gt40`, comments
    /// included: they are what a naive parser trips on, `PRESENT=0` being
    /// followed by a sentence that itself contains `1`. Only `[EDL]` departs
    /// from the file — flipped to `1`, so that borrowing it would show.
    /// (Moved from `electronics.rs` with the reading, SESSION§3.)
    const GT40: &str = "\
[ABS]
SLIP_RATIO_LIMIT=0.12\t\t; Slipratio limit before ABS engages
CURVE=\t\t\t; Leave blank for a single level
PRESENT=0\t\t\t; 1 if present in car, 0 if not present
ACTIVE=0\t\t\t; 1 will make the car start with ABS active

[TRACTION_CONTROL]
SLIP_RATIO_LIMIT=0.10\t\t; Slipratio limit before TC engages
PRESENT=0\t\t\t; 1 if present in car, 0 if not present
RATE_HZ=100

[EDL]
PRESENT=1\t\t\t; a third PRESENT, and the reason this parser is scoped
MAX_SPIN_POWER=0.8
";

    /// SESSION§3 — `Factory` on a 1966 car means no aid at all, and the third
    /// `PRESENT` of the file must not turn that into a yes.
    #[test]
    fn edl_does_not_lend_its_present_to_the_two_aids() {
        assert_eq!(present(GT40, "[ABS]"), Some(false), "GT40 has no ABS");
        assert_eq!(
            present(GT40, "[TRACTION_CONTROL]"),
            Some(false),
            "GT40 has no traction control"
        );
        assert_eq!(present(GT40, "[EDL]"), Some(true), "its EDL is its own");
    }

    /// A section without its `PRESENT` line is silent, not a no — the next
    /// section's value must not be borrowed to fill the hole.
    #[test]
    fn a_section_without_present_does_not_borrow_the_next_one() {
        let text = "[ABS]\nRATE_HZ=100\n\n[TRACTION_CONTROL]\nPRESENT=1\n";
        assert_eq!(present(text, "[ABS]"), None, "ABS said nothing");
        assert_eq!(present(text, "[TRACTION_CONTROL]"), Some(true));
    }

    /// Turbo sections are counted exactly: a key named `TURBO_…` or an
    /// oddly named section is not a turbo.
    #[test]
    fn turbo_sections_are_counted_exactly() {
        let text =
            "[ENGINE_DATA]\nLIMITER=7000\n[TURBO_0]\nMAX_BOOST=1\n[TURBO_1]\n[TURBO_BOOST_THRESHOLD]\n; [TURBO_2]\n";
        assert_eq!(turbo_sections(text), 2);
        assert_eq!(turbo_sections(ENGINE_NA), 0, "no section, naturally aspirated");
    }

    /// The numbers the sheet shows come straight out of their sections.
    #[test]
    fn gears_limiter_tank_and_mass_are_read() {
        let dir = car(&[
            ("engine.ini", ENGINE_NA),
            ("drivetrain.ini", DRIVETRAIN_RWD),
            ("car.ini", "[BASIC]\nTOTALMASS=1320\n[FUEL]\nMAX_FUEL=110 ; litres\n"),
        ]);
        let p = read(&[dir.to_path_buf()], "c").unwrap();
        assert_eq!(p.gears, Some(6));
        assert_eq!(p.h_shifter, Some(false), "no H-pattern");
        assert_eq!(p.limiter, Some(8300.0));
        assert_eq!(p.max_fuel, Some(110.0));
        assert_eq!(p.total_mass, Some(1320.0));
        assert_eq!(p.turbos, Some(0));
    }

    /// No physics at all is `None`, so the caller falls back entirely.
    #[test]
    fn a_car_without_physics_reads_as_none() {
        let dir = crate::testutil::temp_dir("techsheet-nophysics");
        assert!(read(&[dir.to_path_buf()], "c").is_none());
    }
}
