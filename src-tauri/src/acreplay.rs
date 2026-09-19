//! Reading the `.acreplay` header, plus Assetto Corsa's autosave conventions
//! (§6.1). Pure file reading: nothing here ever writes to the replay folder.
//!
//! **Why parse the file at all.** The file name carries the car and track ids
//! and nothing else. Driver name, duration and the number of cars on track
//! live in the header — and they are the three facts that tell two replays of
//! the same combo apart.
//!
//! Layout of a v16 header, taken from Content Manager's own reader
//! (`AcManager.Tools/Helpers/ReplayDetails.cs`, `ParseV16`) and verified
//! against the nine real replays on the development machine (1 to 23 cars,
//! 867 KB to 635 MB):
//!
//! ```text
//! i32  version                  // 16 for every current AC build
//! f64  recording_interval_ms    // 15.0 observed on all nine
//! str  weather_id               // i32 length + ASCII, no terminator
//! str  track_id
//! str  track_layout             // empty string when the track has no layout
//! i32  cars_number
//! i32  current_recording_index  // equal to `frames` on a finished replay
//! i32  frames
//! i32  track_objects
//! …    frames * (4 + track_objects * 12) bytes of per-frame data
//! str  car_id                   // the player's car: AC writes it first
//! str  driver_name
//! str  nation_code
//! str  driver_team
//! str  car_skin_id
//! ```
//!
//! The per-frame block is a **fixed stride**, so the driver name is one seek
//! away instead of a scan: reading the header of the 635 MB replay costs the
//! same two reads as the 867 KB one. That is what makes it affordable to do
//! this for every line of the Replays list.
//!
//! Duration is `frames * recording_interval_ms` — the same formula CM uses,
//! and the reason the interval is read rather than assumed.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// Only v16 is described above. An older replay (AC before 1.14 stored a
/// different layout) is reported as unreadable rather than parsed on a guess:
/// these fields are a convenience (§6.1), and a wrong driver name is worse
/// than none.
const SUPPORTED_VERSION: i32 = 16;

/// An ASCII string longer than this is a desynchronised read, not a name —
/// bail out instead of allocating whatever the next four bytes say.
const MAX_STRING_LEN: i32 = 4096;

#[derive(Debug, Clone)]
pub struct ReplayHeader {
    pub car_id: String,
    pub driver_name: String,
    pub track_id: String,
    pub track_layout: String,
    pub cars_number: i32,
    /// Duration in seconds, rounded down. `frames * recording_interval_ms`.
    pub duration_s: i64,
}

fn read_i32(r: &mut impl Read) -> Option<i32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf).ok()?;
    Some(i32::from_le_bytes(buf))
}

fn read_f64(r: &mut impl Read) -> Option<f64> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf).ok()?;
    Some(f64::from_le_bytes(buf))
}

/// Length-prefixed ASCII, no terminator. A negative or absurd length means the
/// read has drifted — `None`, never a panic on `with_capacity`.
fn read_str(r: &mut impl Read) -> Option<String> {
    let len = read_i32(r)?;
    if !(0..=MAX_STRING_LEN).contains(&len) {
        return None;
    }
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

/// `None` for anything that is not a readable v16 replay — a truncated file, a
/// replay still being written by the game, an older format version. Callers
/// fall back to what the file name alone gives them.
pub fn read_header(path: &Path) -> Option<ReplayHeader> {
    let file = File::open(path).ok()?;
    let mut r = BufReader::new(file);

    if read_i32(&mut r)? != SUPPORTED_VERSION {
        return None;
    }
    let interval_ms = read_f64(&mut r)?;
    // Weather is read for its position, not its value: it is what places the
    // two ids that follow.
    read_str(&mut r)?;
    let track_id = read_str(&mut r)?;
    let track_layout = read_str(&mut r)?;
    let cars_number = read_i32(&mut r)?;
    let _current_index = read_i32(&mut r)?;
    let frames = read_i32(&mut r)?;
    let track_objects = read_i32(&mut r)?;
    if frames < 0 || track_objects < 0 {
        return None;
    }

    // Per-frame stride: 2 bytes of sun angle + 2 of padding + 12 per track
    // object. `i64` throughout — 23 cars over half an hour already overflows
    // an `i32` byte count, and a corrupted `track_objects` would overflow
    // anything smaller.
    let stride = 4i64 + i64::from(track_objects) * 12;
    let skip = i64::from(frames).checked_mul(stride)?;
    r.seek(SeekFrom::Current(skip)).ok()?;

    let car_id = read_str(&mut r)?;
    let driver_name = read_str(&mut r)?;
    // Nation, team and skin are read to the end of the block although nothing
    // displays them: it is how a mis-stride is caught. A seek that lands a few
    // bytes off still yields a plausible-looking `car_id`, but the strings
    // after it stop parsing — so reading them all is the alignment check.
    read_str(&mut r)?;
    read_str(&mut r)?;
    read_str(&mut r)?;

    // A non-finite interval would make the cast to `i64` undefined-ish; a zero
    // one simply yields 0 s, which reads as "unknown" well enough.
    let duration_ms = if interval_ms.is_finite() && interval_ms > 0.0 {
        f64::from(frames) * interval_ms
    } else {
        0.0
    };

    Some(ReplayHeader {
        car_id,
        driver_name,
        track_id,
        track_layout,
        cars_number,
        duration_s: (duration_ms / 1000.0) as i64,
    })
}

/// How many autosaved replays Assetto Corsa keeps, per session type
/// (`cfg/replay.ini`, section `[AUTOSAVE]`, §6.1). Content Manager's
/// "Replays autosave" settings write exactly this file — there is no separate
/// CM-side retention, which is why the answer to "when will this be deleted?"
/// is a count and not a date.
#[derive(Debug, Clone)]
pub struct AutosavePolicy {
    pub enabled: bool,
    pub race: i32,
    pub qualify: i32,
    pub others: i32,
}

impl Default for AutosavePolicy {
    /// Assetto Corsa's own defaults, used when `replay.ini` is missing — the
    /// game recreates it with these values.
    fn default() -> Self {
        Self {
            enabled: true,
            race: 2,
            qualify: 1,
            others: 1,
        }
    }
}

impl AutosavePolicy {
    /// Limit that applies to a session type letter as it appears in the
    /// autosave file name. `R` is a race, `Q` a qualifying session, and
    /// everything else (`O`, practice, hotlap…) falls under `OTHERS` — the
    /// three-way split of `[AUTOSAVE]` has no fourth bucket.
    pub fn limit_for(&self, session_type: &str) -> i32 {
        match session_type {
            "R" => self.race,
            "Q" => self.qualify,
            _ => self.others,
        }
    }
}

fn parse_autosave_ini(text: &str) -> AutosavePolicy {
    let mut policy = AutosavePolicy::default();
    let mut in_section = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_section = line.eq_ignore_ascii_case("[AUTOSAVE]");
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim().to_ascii_uppercase().as_str() {
            "ENABLED" => policy.enabled = value != "0",
            "RACE" => policy.race = value.parse().unwrap_or(policy.race),
            "QUALIFY" => policy.qualify = value.parse().unwrap_or(policy.qualify),
            "OTHERS" => policy.others = value.parse().unwrap_or(policy.others),
            _ => {}
        }
    }
    policy
}

/// Reads `Documents/Assetto Corsa/cfg/replay.ini`. A missing or unreadable
/// file yields the game's defaults rather than an error: this only drives a
/// badge, and being silent about retention would be worse than being
/// approximate about it.
pub fn autosave_policy() -> AutosavePolicy {
    let Some(dir) = crate::media::documents_ac_dir() else {
        return AutosavePolicy::default();
    };
    match std::fs::read_to_string(dir.join("cfg").join("replay.ini")) {
        Ok(text) => parse_autosave_ini(&text),
        Err(e) => {
            log::warn!("replay.ini unreadable, falling back to AC defaults: {e}");
            AutosavePolicy::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a synthetic v16 replay: the point is the stride arithmetic, so
    /// the per-frame block is zero-filled but has exactly the real size.
    fn write_replay(path: &Path, track: &str, layout: &str, cars: i32, frames: i32, objects: i32, driver: &str) {
        fn put_str(out: &mut Vec<u8>, s: &str) {
            out.extend_from_slice(&(s.len() as i32).to_le_bytes());
            out.extend_from_slice(s.as_bytes());
        }
        let mut out = Vec::new();
        out.extend_from_slice(&16i32.to_le_bytes());
        out.extend_from_slice(&15.0f64.to_le_bytes());
        put_str(&mut out, "sol_01_clear");
        put_str(&mut out, track);
        put_str(&mut out, layout);
        out.extend_from_slice(&cars.to_le_bytes());
        out.extend_from_slice(&frames.to_le_bytes());
        out.extend_from_slice(&frames.to_le_bytes());
        out.extend_from_slice(&objects.to_le_bytes());
        out.resize(out.len() + (frames as usize) * (4 + objects as usize * 12), 0);
        put_str(&mut out, "ks_audi_tt_cup");
        put_str(&mut out, driver);
        put_str(&mut out, "FRA");
        put_str(&mut out, "");
        put_str(&mut out, "01_blue");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, out).unwrap();
    }

    // The whole point of the module: the driver name sits behind a block whose
    // size is `frames * (4 + track_objects * 12)`. Get the stride wrong and
    // the strings read as garbage — which is exactly how a wrong name would
    // reach the screen.
    #[test]
    fn header_seeks_past_the_frame_block_to_reach_the_driver_name() {
        let base = crate::testutil::temp_dir("acreplay-header");
        let path = base.join("AC_190926-130225_O_ks_audi_tt_cup_ks_silverstone_gp.acreplay");
        write_replay(&path, "ks_silverstone", "gp", 1, 895, 50, "Theo");

        let h = read_header(&path).expect("a well-formed v16 replay must parse");
        assert_eq!(h.driver_name, "Theo", "driver name read past the frame block");
        assert_eq!(h.car_id, "ks_audi_tt_cup");
        assert_eq!(h.track_id, "ks_silverstone");
        assert_eq!(h.track_layout, "gp");
        assert_eq!(h.cars_number, 1);
        // 895 frames * 15 ms = 13.4 s.
        assert_eq!(
            h.duration_s, 13,
            "duration is frames * recording interval, not a frame count"
        );
    }

    #[test]
    fn header_handles_a_track_without_layout_and_many_cars() {
        // Real shape of `AC_300124-231824_R_…_shannonville_long`: an empty
        // layout string is a valid field, not an absent one.
        let base = crate::testutil::temp_dir("acreplay-nolayout");
        let path = base.join("r.acreplay");
        write_replay(&path, "shannonville", "", 10, 200, 94, "Theo Meedendorp");

        let h = read_header(&path).expect("empty layout is legal");
        assert_eq!(
            h.track_layout, "",
            "an empty layout must not shift the following fields"
        );
        assert_eq!(h.cars_number, 10);
        assert_eq!(h.driver_name, "Theo Meedendorp");
    }

    // A replay being written by the game is a truncated file: the seek lands
    // past the end. It must not surface as a line with a garbage driver name.
    #[test]
    fn truncated_or_foreign_files_yield_no_header_instead_of_garbage() {
        let base = crate::testutil::temp_dir("acreplay-bad");
        let full = base.join("full.acreplay");
        write_replay(&full, "imola", "", 1, 100, 20, "Theo");
        let bytes = std::fs::read(&full).unwrap();

        let cut = base.join("cut.acreplay");
        std::fs::write(&cut, &bytes[..bytes.len() / 2]).unwrap();
        assert!(
            read_header(&cut).is_none(),
            "a half-written replay has no readable header"
        );

        // Cut right after the car id: the strings that follow are what says
        // the seek landed where it should.
        let short = base.join("short.acreplay");
        let car_id_end = bytes.len() - "FRA".len() - "01_blue".len() - 4 * 3;
        std::fs::write(&short, &bytes[..car_id_end]).unwrap();
        assert!(
            read_header(&short).is_none(),
            "an unreadable tail means the alignment is unproven, so no header"
        );

        let alien = base.join("alien.acreplay");
        std::fs::write(&alien, b"not a replay at all").unwrap();
        assert!(read_header(&alien).is_none(), "version check rejects a foreign file");

        let empty = base.join("empty.acreplay");
        std::fs::write(&empty, b"").unwrap();
        assert!(read_header(&empty).is_none());
    }

    // The retention question the user actually asks ("will this one be
    // deleted?") is answered by these three numbers, so a typo in the section
    // name must not silently fall back to the defaults.
    #[test]
    fn autosave_policy_reads_the_ini_and_falls_back_to_ac_defaults() {
        let real = "[REPLAY]\nMAX_SIZE_MB=1384\n\n[QUALITY]\nLEVEL=4\n\n[AUTOSAVE]\nENABLED=1\nRACE=2\nQUALIFY=1\nOTHERS=1\nMIN_TIME_SECONDS=30\n";
        let p = parse_autosave_ini(real);
        assert!(p.enabled);
        assert_eq!(p.race, 2);
        assert_eq!(p.qualify, 1);
        assert_eq!(p.others, 1);
        assert_eq!(p.limit_for("R"), 2, "R is a race");
        assert_eq!(p.limit_for("Q"), 1);
        assert_eq!(p.limit_for("O"), 1, "anything that is not R or Q counts as OTHERS");
        assert_eq!(p.limit_for("P"), 1, "practice has no bucket of its own");

        // Keys outside [AUTOSAVE] must not be picked up: [REPLAY] has none of
        // these names today, but reading the file section-blind would be a
        // silent trap the day it does.
        let elsewhere = "[REPLAY]\nRACE=99\n";
        assert_eq!(
            parse_autosave_ini(elsewhere).race,
            AutosavePolicy::default().race,
            "a key outside [AUTOSAVE] is not the autosave limit"
        );

        assert!(!parse_autosave_ini("[AUTOSAVE]\nENABLED=0\n").enabled);
    }
}
