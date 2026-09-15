//! What a car was built with — `electronics.ini`, sections `[ABS]` and
//! `[TRACTION_CONTROL]`.
//!
//! Read for one thing only: saying what the `Factory` assist setting is worth
//! **for the car in session** (SESSION§3). Assetto Corsa's three assist levels are
//! `Off` / `Factory` / `On`, and `Factory` means "whatever the real car had" —
//! so on a 1966 GT40 it means nothing at all, and the screen can say so
//! instead of leaving the driver to find out on track.
//!
//! ## Where the fact lives
//!
//! Not in `drivetrain.ini`, which is a reasonable guess and wrong: that file
//! holds the gearbox and the differential. `PRESENT=1` under `[ABS]` and
//! `[TRACTION_CONTROL]` of `electronics.ini` is the declaration, and the file
//! is present on every car of the reference install — including cars that have
//! neither aid, where it simply says `0`. Its presence therefore proves
//! nothing; only its content does, which is why this reads the file rather
//! than listing the container.
//!
//! ## Measured on the 311 cars of the reference install
//!
//! | | |
//! | --- | --- |
//! | both aids | 142 |
//! | ABS only | 31 |
//! | traction control only | 17 |
//! | neither | 111 |
//! | no `[ABS]`/`[TRACTION_CONTROL]` section at all | 10 (mods) |
//!
//! All four combinations occur, and the asymmetric ones are 48 cars — which is
//! why the screen names *which* aid a car has rather than answering yes or no.
//! The ten silent ones are the reason this returns an `Option`: a car that
//! does not say gets no line, never a hedged one.
//!
//! Same two-source rule as the rest of the physics files: the unpacked `data/`
//! folder first, `data.acd` after (see `steering.rs`, same shape).

use std::path::Path;

use crate::acd;

/// The section that says a `data.acd` entry decrypted into the right file
/// (see [`acd::read_text`]) — and the first of the two this module reads.
const ABS_SECTION: &str = "[ABS]";
const TC_SECTION: &str = "[TRACTION_CONTROL]";

/// What a car was equipped with when it was built.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FactoryAssists {
    pub abs: bool,
    pub traction_control: bool,
}

/// Reads `electronics.ini` for this car.
///
/// `None` when the file is absent, unreadable, refuses every key, or declares
/// neither section — each of which is a reason to say nothing, never to fail
/// and never to guess.
pub fn read(car_dir: &Path, car_id: &str) -> Option<FactoryAssists> {
    parse(&data_file(car_dir, car_id)?)
}

/// The reading itself, on text — so the tests can exercise it without a car.
fn parse(text: &str) -> Option<FactoryAssists> {
    let abs = present(text, ABS_SECTION);
    let traction_control = present(text, TC_SECTION);
    // One of the two is enough to have something to say; neither means the
    // file is not the one we think it is.
    if abs.is_none() && traction_control.is_none() {
        return None;
    }
    Some(FactoryAssists {
        abs: abs.unwrap_or(false),
        traction_control: traction_control.unwrap_or(false),
    })
}

/// `electronics.ini`, unpacked folder first — a mod that ships both has edited
/// the loose one, and it is what AC itself reads.
fn data_file(car_dir: &Path, car_id: &str) -> Option<String> {
    let loose = car_dir.join("data").join("electronics.ini");
    match std::fs::read_to_string(&loose) {
        Ok(text) => return Some(text),
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            log::warn!("electronics: {} unreadable — {e}", loose.display());
        }
        Err(_) => {}
    }
    acd::read_text(car_dir, car_id, "electronics.ini", ABS_SECTION)
}

/// `PRESENT` **inside one section**, comments stripped.
///
/// Section-scoped, unlike the section-blind readers of `steering.rs` and
/// `acd.rs`: `PRESENT` is not unique in this file. It appears under `[ABS]`,
/// under `[TRACTION_CONTROL]` and again under `[EDL]` — a section-blind scan
/// would answer the first one three times, so every car with ABS would also
/// be credited with traction control.
fn present(text: &str, section: &str) -> Option<bool> {
    let mut inside = false;
    for line in text.lines() {
        let line = line.split(';').next().unwrap_or(line).trim();
        if line.starts_with('[') {
            // A new section ends the previous one: a key missing from ours is
            // not to be picked up from the next.
            if inside {
                return None;
            }
            inside = line.eq_ignore_ascii_case(section);
            continue;
        }
        if !inside {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("PRESENT") {
            // Written as a count on a few mods rather than as a flag; anything
            // that is not zero means equipped.
            return value.trim().parse::<i32>().ok().map(|v| v != 0);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trimmed from the real `electronics.ini` of `ks_ford_gt40`, comments
    /// included: they are what a naive parser trips on, `PRESENT=0` being
    /// followed by a sentence that itself contains `1`. Only `[EDL]` departs
    /// from the file — flipped to `1`, so that borrowing it would show.
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

    /// A GT3 declares both — and `[EDL]` must not leak into either answer.
    const GT3: &str = "[ABS]\nPRESENT=1\n\n[TRACTION_CONTROL]\nPRESENT=1\n\n[EDL]\nPRESENT=0\n";

    /// SESSION§3 — `Factory` on a 1966 car means no aid at all, and the third
    /// `PRESENT` of the file must not turn that into a yes.
    #[test]
    fn edl_does_not_lend_its_present_to_the_two_aids() {
        let found = parse(GT40).expect("both sections are there");
        assert!(!found.abs, "GT40 has no ABS");
        assert!(!found.traction_control, "GT40 has no traction control");

        let found = parse(GT3).expect("both sections are there");
        assert!(found.abs, "a GT3 has ABS");
        assert!(found.traction_control, "a GT3 has traction control");
    }

    /// SESSION§3 — the asymmetric case is 48 cars of the reference install, so it
    /// is not an edge: one aid present must never imply the other.
    #[test]
    fn one_aid_alone_is_reported_alone() {
        let found = parse("[ABS]\nPRESENT=1\n\n[TRACTION_CONTROL]\nPRESENT=0\n").unwrap();
        assert!(found.abs && !found.traction_control, "ABS only");

        let found = parse("[ABS]\nPRESENT=0\n\n[TRACTION_CONTROL]\nPRESENT=1\n").unwrap();
        assert!(!found.abs && found.traction_control, "traction control only");
    }

    /// SESSION§3 — ten cars of the install declare neither section. They must
    /// produce nothing at all, since the screen shows no line rather than a
    /// hedged one.
    #[test]
    fn a_car_that_says_nothing_returns_nothing() {
        assert!(parse("[EDL]\nPRESENT=1\n").is_none(), "no aid section, no answer");
        assert!(parse("").is_none(), "empty file, no answer");
    }

    /// A section without its `PRESENT` line is silent, not a no — the next
    /// section's value must not be borrowed to fill the hole.
    #[test]
    fn a_section_without_present_does_not_borrow_the_next_one() {
        let found = parse("[ABS]\nRATE_HZ=100\n\n[TRACTION_CONTROL]\nPRESENT=1\n").unwrap();
        assert!(!found.abs, "ABS said nothing: not equipped rather than borrowed");
        assert!(found.traction_control, "traction control said yes");
    }
}
