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
//! ## Read from the tech sheet
//!
//! The file itself is read once, with the rest of the car's physics, by
//! `techsheet::physics` (FICHE§5), and stored; this module asks the sheet.
//! So a correction made on the sheet holds on the session screen too, and a
//! car whose files are gone still answers (FICHE§9.1).

use rusqlite::Connection;

/// What a car was equipped with when it was built.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FactoryAssists {
    pub abs: bool,
    pub traction_control: bool,
}

/// ABS and traction control of this car, as its tech sheet has them.
///
/// `None` when the sheet knows neither — the physics did not say, or has not
/// been read yet — which is a reason to say nothing, never to guess.
pub fn read(conn: &Connection, car_id: &str) -> Option<FactoryAssists> {
    let (abs, traction_control) = match crate::techsheet::factory_assists(conn, car_id) {
        Ok(found) => found,
        Err(e) => {
            log::warn!("electronics: tech sheet of {car_id} unreadable — {e}");
            return None;
        }
    };
    // One of the two is enough to have something to say.
    if abs.is_none() && traction_control.is_none() {
        return None;
    }
    Some(FactoryAssists {
        abs: abs.unwrap_or(false),
        traction_control: traction_control.unwrap_or(false),
    })
}
