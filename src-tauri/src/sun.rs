//! Sunrise and sunset for the session time slider (§8.6ter).
//!
//! Picking "the lap that starts at sunset" is only useful if the times shown
//! are the ones the game will actually render. So this module does not invent
//! its own daylight model: it reads the very inputs Custom Shaders Patch reads,
//! and applies the standard solar equations to them.
//!
//! What CSP uses, verified in an actual install:
//!
//! - **Coordinates and timezone**: `extension/config/data_track_params.ini`,
//!   one section per track id (`[ks_nordschleife]` → `LATITUDE`, `LONGITUDE`,
//!   `TIMEZONE=Europe/Berlin`, plus `HEADING_ANGLE` which only rotates the
//!   world, not the sun's schedule). 445 tracks are listed there, layouts
//!   included in their parent's section — there is no per-layout entry. The
//!   Kunos `ui_track.json` files are *not* a source: their `geotags` field is
//!   the literal placeholder `["lat", "lon"]`. Mod tracks packaged with CM do
//!   carry real geotags, so those serve as a fallback for a track CSP does not
//!   list (it then has no timezone, see `TZ_FROM_LONGITUDE`).
//! - **Which date the sun trajectory follows**: `[SEASONS] ALLOW_ADJUSTMENTS`
//!   in `track_adjustments.ini` — user value in `Documents/Assetto Corsa/cfg/
//!   extension/`, shipped default in `<AC>/extension/config/`. The shipped
//!   file documents its own values: `0` = "Never (use midsummer sun
//!   trajectory)", `0.5` = "Never (use actual sun trajectory)", `1` = "With
//!   date set" (the default), `2` = "Always on (use current date if not set)".
//!   Hence `effective_date` below: at `0` the sun ignores the chosen season
//!   entirely, and at `1` it only follows a date the session actually carries
//!   (Pit Box writes one as `udt`/`dtv` in the Quick Drive preset whenever a
//!   season is picked — see `quickdrive.rs`).
//!
//! CSP computes the sun direction natively (`ac.getSunDirectionTo`); the
//! weather scripts (Sol, Pure) only *react* to its height, so they change
//! nothing here — a Pure install and a Sol install put sunset at the same
//! minute.

use std::path::{Path, PathBuf};

use chrono::{Datelike, NaiveDate, Offset, TimeZone};
use serde::Serialize;

/// Sun altitude, in degrees, that defines sunrise/sunset: the upper limb of
/// the disc touching the horizon, refraction included. Standard value.
const HORIZON_DEG: f64 = -0.833;
/// Civil twilight: the sky is still lit enough to drive without headlights.
/// It is what makes the gradient readable — sunrise alone would show a hard
/// cut between night and day where the real change takes half an hour.
const CIVIL_TWILIGHT_DEG: f64 = -6.0;
/// Earth's axial tilt (obliquity of the ecliptic).
const OBLIQUITY_DEG: f64 = 23.4397;

/// Where the coordinates came from, so the UI can say whether the times are
/// the ones CSP will use or a best guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CoordSource {
    /// `data_track_params.ini` — the file CSP itself reads.
    Csp,
    /// `ui_track.json` geotags — a track CSP does not know about.
    Geotags,
}

/// Which date the sun trajectory follows, once `ALLOW_ADJUSTMENTS` has had
/// its say.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DateBasis {
    /// The date the session carries.
    Session,
    /// No date in the session, and CSP falls back to today's real date.
    Today,
    /// CSP ignores the date and keeps the midsummer trajectory: either
    /// seasonal adjustments are off, or none is set and the setting only
    /// applies them "with date set".
    Midsummer,
}

/// Everything the launch screen needs to draw a day/night band.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackSun {
    pub latitude: f64,
    pub longitude: f64,
    /// IANA name as CSP has it, when it has one.
    pub timezone: Option<String>,
    /// Offset actually applied, summer time included, for `date`.
    pub utc_offset_hours: f64,
    pub source: CoordSource,
    /// `[SEASONS] ALLOW_ADJUSTMENTS` as read.
    pub seasonal_setting: f64,
    pub date_basis: DateBasis,
    /// Date the times below are computed for (`YYYY-MM-DD`).
    pub date: String,
    /// Local clock hours, `None` when the sun stays under or over the horizon
    /// all day (polar night / midnight sun — Rovaniemi and Kiruna both exist
    /// as AC tracks).
    pub sunrise: Option<f64>,
    pub sunset: Option<f64>,
    pub dawn: Option<f64>,
    pub dusk: Option<f64>,
    pub solar_noon: f64,
    /// True when the sun never rises that day (as opposed to never setting).
    pub polar_night: bool,
}

/// Midsummer in the northern hemisphere — the trajectory CSP falls back to
/// when seasons are off. June solstice, not July: the setting says "midsummer
/// sun trajectory", which is the longest day, not the middle of the season.
const MIDSUMMER: (u32, u32) = (6, 21);

/// Reads `key` inside `[section]` of an INI text. Section names are compared
/// case-insensitively (track ids are lowercase in practice, but nothing
/// guarantees it), and a trailing `; comment` is stripped — the shipped CSP
/// files document every value inline (`ALLOW_ADJUSTMENTS=1 ; Use seasons…`).
fn ini_value(text: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    for line in text.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            in_section = name.eq_ignore_ascii_case(section);
            continue;
        }
        if !in_section || line.starts_with(';') || line.starts_with('/') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if k.trim().eq_ignore_ascii_case(key) {
            let v = v.split(';').next().unwrap_or("").trim();
            return Some(v.to_string());
        }
    }
    None
}

/// Track coordinates as CSP knows them, or as the mod declares them.
pub struct TrackLocation {
    pub latitude: f64,
    pub longitude: f64,
    pub timezone: Option<String>,
    pub source: CoordSource,
}

/// CSP's own table first, the mod's `ui_track.json` second.
pub fn track_location(ac_install_path: &Path, track_id: &str, layout: Option<&str>) -> Option<TrackLocation> {
    let params = ac_install_path
        .join("extension")
        .join("config")
        .join("data_track_params.ini");
    if let Ok(text) = std::fs::read_to_string(&params) {
        let lat = ini_value(&text, track_id, "LATITUDE").and_then(|v| v.parse::<f64>().ok());
        let lon = ini_value(&text, track_id, "LONGITUDE").and_then(|v| v.parse::<f64>().ok());
        if let (Some(latitude), Some(longitude)) = (lat, lon) {
            return Some(TrackLocation {
                latitude,
                longitude,
                timezone: ini_value(&text, track_id, "TIMEZONE").filter(|s| !s.is_empty()),
                source: CoordSource::Csp,
            });
        }
    }
    geotags(ac_install_path, track_id, layout).map(|(latitude, longitude)| TrackLocation {
        latitude,
        longitude,
        timezone: None,
        source: CoordSource::Geotags,
    })
}

/// `geotags` of a `ui_track.json`, layout first. Values are strings in every
/// file seen, but numbers are accepted too; Kunos' literal `["lat", "lon"]`
/// placeholder simply fails to parse, which is exactly the wanted outcome.
fn geotags(ac_install_path: &Path, track_id: &str, layout: Option<&str>) -> Option<(f64, f64)> {
    let ui = ac_install_path.join("content").join("tracks").join(track_id).join("ui");
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(l) = layout.filter(|l| !l.is_empty()) {
        candidates.push(ui.join(l).join("ui_track.json"));
    }
    candidates.push(ui.join("ui_track.json"));
    for path in candidates {
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        // `ui_json` files routinely carry a BOM and invalid escapes; only the
        // two numbers matter here, so a failed parse just moves on.
        let Ok(value) = serde_json::from_str::<serde_json::Value>(raw.trim_start_matches('\u{feff}')) else {
            continue;
        };
        let tags = value.get("geotags").and_then(|v| v.as_array())?;
        let number = |v: &serde_json::Value| -> Option<f64> {
            v.as_f64()
                .or_else(|| v.as_str().and_then(|s| s.trim().trim_end_matches('°').parse().ok()))
        };
        if let (Some(lat), Some(lon)) = (tags.first().and_then(number), tags.get(1).and_then(number)) {
            return Some((lat, lon));
        }
    }
    None
}

/// `[SEASONS] ALLOW_ADJUSTMENTS`: the user's value wins over the shipped
/// default, and `1` (CSP's own default) applies when neither file is there —
/// a CSP-less install never reaches this code anyway, the launch screen only
/// asks once a track is chosen.
pub fn seasonal_setting(ac_install_path: &Path) -> f64 {
    let user = dirs::document_dir().map(|d| {
        d.join("Assetto Corsa")
            .join("cfg")
            .join("extension")
            .join("track_adjustments.ini")
    });
    let shipped = ac_install_path
        .join("extension")
        .join("config")
        .join("track_adjustments.ini");
    for path in user.into_iter().chain(std::iter::once(shipped)) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Some(v) = ini_value(&text, "SEASONS", "ALLOW_ADJUSTMENTS").and_then(|v| v.parse::<f64>().ok()) {
                return v;
            }
        }
    }
    1.0
}

/// The date the sun trajectory actually follows, given the setting and the
/// date the session carries (`None` when no season is picked).
fn effective_date(setting: f64, session_date: Option<NaiveDate>, today: NaiveDate) -> (NaiveDate, DateBasis) {
    let midsummer = || {
        (
            NaiveDate::from_ymd_opt(today.year(), MIDSUMMER.0, MIDSUMMER.1).unwrap_or(today),
            DateBasis::Midsummer,
        )
    };
    if setting <= 0.0 {
        return midsummer();
    }
    match session_date {
        Some(d) => (d, DateBasis::Session),
        // 0.5 ("never adjust, but keep the actual trajectory") and 2 ("always
        // on, use current date if not set") both follow the real calendar; 1
        // only adjusts "with date set", so without one the trajectory stays
        // where vanilla AC left it — midsummer.
        None if setting == 1.0 => midsummer(),
        None => (today, DateBasis::Today),
    }
}

/// Julian day at 00:00 UT of a calendar date.
fn julian_day(date: NaiveDate) -> f64 {
    let seconds = date
        .and_hms_opt(0, 0, 0)
        .map(|dt| dt.and_utc().timestamp())
        .unwrap_or(0) as f64;
    seconds / 86_400.0 + 2_440_587.5
}

/// Sun altitude crossings for one day, as fractional hours on the track's
/// local clock. Standard low-precision solar equations (mean anomaly →
/// equation of the centre → ecliptic longitude → declination), accurate to
/// about a minute over the years AC sessions live in — far below what a
/// gradient can show.
///
/// `None` means the sun never reaches `altitude_deg` that day; the caller
/// tells polar night from midnight sun by the sign of the cosine.
fn crossings(lat: f64, lon: f64, tz_hours: f64, date: NaiveDate, altitude_deg: f64) -> (Option<f64>, Option<f64>, f64) {
    let rad = |d: f64| d.to_radians();
    // Day number since 2000-01-01 12:00 UT, shifted by longitude: the sun
    // transits earlier in UT the further east the track is (12:00 UT minus
    // longitude/15 hours), so the shift is subtracted, not added. Getting that
    // sign wrong costs twice the longitude correction — 75 minutes at Monza,
    // which is exactly the kind of error this whole module exists to avoid.
    let n = (julian_day(date) - 2_451_545.0 + 0.0008).ceil() - lon / 360.0;
    let mean_anomaly = (357.5291 + 0.985_600_28 * n).rem_euclid(360.0);
    let m = rad(mean_anomaly);
    let center = 1.9148 * m.sin() + 0.02 * (2.0 * m).sin() + 0.0003 * (3.0 * m).sin();
    let ecliptic = rad((mean_anomaly + center + 282.9372).rem_euclid(360.0));
    let transit = 2_451_545.0 + n + 0.0053 * m.sin() - 0.0069 * (2.0 * ecliptic).sin();

    let declination = (ecliptic.sin() * rad(OBLIQUITY_DEG).sin()).asin();
    let lat = rad(lat);
    let cos_hour_angle = (rad(altitude_deg).sin() - lat.sin() * declination.sin()) / (lat.cos() * declination.cos());

    // Julian date → hours on the local clock, folded back into [0, 24).
    let local = |jd: f64| ((jd - 2_440_587.5) * 24.0 + tz_hours).rem_euclid(24.0);
    let noon = local(transit);
    if !(-1.0..=1.0).contains(&cos_hour_angle) {
        return (None, None, noon);
    }
    let hour_angle = cos_hour_angle.acos().to_degrees() / 360.0;
    (
        Some(local(transit - hour_angle)),
        Some(local(transit + hour_angle)),
        noon,
    )
}

/// UTC offset for that date at that place, summer time included. Falls back to
/// the longitude's nominal zone when CSP gives no timezone (a mod track known
/// only by its geotags): wrong by an hour here and there, but never by the
/// four hours a missing offset would cost.
fn utc_offset_hours(timezone: Option<&str>, longitude: f64, date: NaiveDate) -> f64 {
    let from_name = timezone
        .and_then(|name| name.parse::<chrono_tz::Tz>().ok())
        .and_then(|tz| {
            // Noon, not midnight: a DST transition happens at night, and the
            // hour that midnight may fall into does not exist (or exists
            // twice) on those two days a year.
            let naive = date.and_hms_opt(12, 0, 0)?;
            tz.from_local_datetime(&naive)
                .earliest()
                .map(|dt| dt.offset().fix().local_minus_utc() as f64 / 3600.0)
        });
    from_name.unwrap_or_else(|| (longitude / 15.0).round())
}

/// Sun schedule for a track on the date the game will actually use.
/// `session_date` is the session's `season_date` (`YYYY-MM-DD`), if any.
pub fn track_sun(
    ac_install_path: &Path,
    track_id: &str,
    layout: Option<&str>,
    session_date: Option<&str>,
    today: NaiveDate,
) -> Option<TrackSun> {
    let location = track_location(ac_install_path, track_id, layout)?;
    let setting = seasonal_setting(ac_install_path);
    let parsed = session_date.and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
    let (date, date_basis) = effective_date(setting, parsed, today);

    let tz_hours = utc_offset_hours(location.timezone.as_deref(), location.longitude, date);
    let (sunrise, sunset, solar_noon) = crossings(location.latitude, location.longitude, tz_hours, date, HORIZON_DEG);
    let (dawn, dusk, _) = crossings(
        location.latitude,
        location.longitude,
        tz_hours,
        date,
        CIVIL_TWILIGHT_DEG,
    );
    // No sunrise means one of two opposite days; the sun's height at noon
    // settles which, and the band drawn from it is all night or all day.
    let polar_night = sunrise.is_none() && noon_altitude(location.latitude, location.longitude, date) < HORIZON_DEG;

    Some(TrackSun {
        latitude: location.latitude,
        longitude: location.longitude,
        timezone: location.timezone,
        utc_offset_hours: tz_hours,
        source: location.source,
        seasonal_setting: setting,
        date_basis,
        date: date.format("%Y-%m-%d").to_string(),
        sunrise,
        sunset,
        dawn,
        dusk,
        solar_noon,
        polar_night,
    })
}

/// Sun altitude at solar noon — the highest it gets that day. Only used to
/// tell polar night from midnight sun.
fn noon_altitude(lat: f64, lon: f64, date: NaiveDate) -> f64 {
    let rad = |d: f64| d.to_radians();
    let n = (julian_day(date) - 2_451_545.0 + 0.0008).ceil() - lon / 360.0;
    let mean_anomaly = (357.5291 + 0.985_600_28 * n).rem_euclid(360.0);
    let m = rad(mean_anomaly);
    let center = 1.9148 * m.sin() + 0.02 * (2.0 * m).sin() + 0.0003 * (3.0 * m).sin();
    let ecliptic = rad((mean_anomaly + center + 282.9372).rem_euclid(360.0));
    let declination = (ecliptic.sin() * rad(OBLIQUITY_DEG).sin()).asin().to_degrees();
    90.0 - (lat - declination).abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    /// Minutes between an hour value and an expected `HH:MM`.
    fn minutes_off(hours: f64, expected_h: f64, expected_m: f64) -> f64 {
        (hours * 60.0 - (expected_h * 60.0 + expected_m)).abs()
    }

    // §8.6ter — sunrise/sunset must match the real world within a couple of
    // minutes, otherwise "start at sunset" puts the car in the dark. Reference
    // values: Monza, 15 July 2026, CEST (UTC+2) — 05:47 / 21:07.
    #[test]
    fn monza_midsummer_matches_real_sunrise_and_sunset() {
        let (rise, set, noon) = crossings(45.464199, 9.19034, 2.0, date(2026, 7, 15), HORIZON_DEG);
        let (rise, set) = (rise.unwrap(), set.unwrap());
        assert!(
            minutes_off(rise, 5.0, 47.0) < 3.0,
            "sunrise {rise} should be near 05:47"
        );
        assert!(minutes_off(set, 21.0, 7.0) < 3.0, "sunset {set} should be near 21:07");
        assert!(
            minutes_off(noon, 13.0, 27.0) < 3.0,
            "solar noon {noon} should be near 13:27"
        );
    }

    // Same equations on a winter day and a different meridian: Silverstone,
    // 21 December 2026, GMT — 08:12 / 15:56.
    #[test]
    fn silverstone_winter_solstice_matches_real_times() {
        let (rise, set, _) = crossings(52.090672, -1.02193, 0.0, date(2026, 12, 21), HORIZON_DEG);
        let (rise, set) = (rise.unwrap(), set.unwrap());
        assert!(
            minutes_off(rise, 8.0, 12.0) < 3.0,
            "sunrise {rise} should be near 08:12"
        );
        assert!(minutes_off(set, 15.0, 56.0) < 3.0, "sunset {set} should be near 15:56");
    }

    // Civil twilight brackets the sun's crossing on both sides — the whole
    // point of the second pair of times.
    #[test]
    fn civil_twilight_brackets_sunrise_and_sunset() {
        let (rise, set, _) = crossings(45.464199, 9.19034, 2.0, date(2026, 7, 15), HORIZON_DEG);
        let (dawn, dusk, _) = crossings(45.464199, 9.19034, 2.0, date(2026, 7, 15), CIVIL_TWILIGHT_DEG);
        assert!(dawn.unwrap() < rise.unwrap(), "dawn comes before sunrise");
        assert!(dusk.unwrap() > set.unwrap(), "dusk comes after sunset");
    }

    // Above the polar circle the sun does not cross the horizon at all: no
    // times rather than nonsense ones.
    #[test]
    fn polar_day_and_night_have_no_crossings() {
        let (rise, set, _) = crossings(78.22, 15.65, 1.0, date(2026, 6, 21), HORIZON_DEG);
        assert!(
            rise.is_none() && set.is_none(),
            "midnight sun never crosses the horizon"
        );
        assert!(
            noon_altitude(78.22, 15.65, date(2026, 6, 21)) > HORIZON_DEG,
            "sun stays up"
        );
        let (rise, set, _) = crossings(78.22, 15.65, 1.0, date(2026, 12, 21), HORIZON_DEG);
        assert!(
            rise.is_none() && set.is_none(),
            "polar night never crosses the horizon either"
        );
        assert!(
            noon_altitude(78.22, 15.65, date(2026, 12, 21)) < HORIZON_DEG,
            "sun stays down"
        );
    }

    // Summer time is not optional: the same instant reads an hour later on the
    // clock, and that hour is exactly what makes a sunset lap start too early.
    #[test]
    fn timezone_offset_follows_summer_time() {
        assert_eq!(utc_offset_hours(Some("Europe/Berlin"), 9.19, date(2026, 7, 15)), 2.0);
        assert_eq!(utc_offset_hours(Some("Europe/Berlin"), 9.19, date(2026, 1, 15)), 1.0);
        assert_eq!(utc_offset_hours(Some("Europe/London"), -1.02, date(2026, 12, 21)), 0.0);
    }

    // A track known only by its geotags has no timezone; the nominal zone of
    // its meridian beats having none at all.
    #[test]
    fn missing_timezone_falls_back_to_longitude() {
        assert_eq!(utc_offset_hours(None, 9.19, date(2026, 7, 15)), 1.0);
        assert_eq!(utc_offset_hours(Some("Not/AZone"), 139.7, date(2026, 7, 15)), 9.0);
    }

    // §8.6ter — ALLOW_ADJUSTMENTS decides which date the sun follows, and the
    // four documented values do not agree with each other.
    #[test]
    fn seasonal_setting_decides_the_date_basis() {
        let today = date(2026, 3, 10);
        let session = Some(date(2026, 12, 24));
        assert_eq!(
            effective_date(0.0, session, today).1,
            DateBasis::Midsummer,
            "0 ignores the date"
        );
        assert_eq!(
            effective_date(1.0, session, today).0,
            date(2026, 12, 24),
            "1 follows a date that is set"
        );
        assert_eq!(
            effective_date(1.0, None, today).1,
            DateBasis::Midsummer,
            "1 without a date stays midsummer"
        );
        assert_eq!(
            effective_date(0.5, None, today).1,
            DateBasis::Today,
            "0.5 keeps the actual trajectory"
        );
        assert_eq!(
            effective_date(2.0, None, today).1,
            DateBasis::Today,
            "2 falls back to the real date"
        );
        assert_eq!(
            effective_date(1.0, None, today).0,
            date(2026, 6, 21),
            "midsummer is the June solstice"
        );
    }

    // Not a check: reads the tracks of a real install and prints what the band
    // will draw for each. The equations are pinned by the tests above; what
    // this one exercises is the plumbing around them — sections found, summer
    // time applied, geotags picked up for the one mod track CSP ignores.
    // `PITBOX_AC_ROOT=D:\...ssettocorsa cargo test --lib sun -- --ignored --nocapture`
    #[test]
    #[ignore = "needs a real Assetto Corsa install; measurement, not a check"]
    fn every_installed_track_gives_up_its_sun() {
        let Ok(ac_root) = std::env::var("PITBOX_AC_ROOT") else {
            eprintln!("PITBOX_AC_ROOT unset, skipping");
            return;
        };
        let ac = std::path::PathBuf::from(ac_root);
        eprintln!("ALLOW_ADJUSTMENTS = {}", seasonal_setting(&ac));
        let mut entries: Vec<_> = std::fs::read_dir(ac.join("content").join("tracks"))
            .expect("tracks folder")
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        entries.sort();
        let fmt = |h: Option<f64>| match h {
            // Total minutes, not truncated hours plus rounded minutes: the
            // latter prints "04:60" (Brands Hatch, mid-July).
            Some(h) => {
                let total = (h * 60.0).round() as u32;
                format!("{:02}:{:02}", (total / 60) % 24, total % 60)
            }
            None => "  —  ".to_string(),
        };
        for id in entries {
            match track_sun(&ac, &id, None, Some("2026-07-15"), date(2026, 7, 15)) {
                Some(s) => eprintln!(
                    "{id:<32} {:>8.3},{:>8.3} {:<18} UTC{:+.0} {} → {}  [{:?}]",
                    s.latitude,
                    s.longitude,
                    s.timezone.clone().unwrap_or_else(|| "-".into()),
                    s.utc_offset_hours,
                    fmt(s.sunrise),
                    fmt(s.sunset),
                    s.source
                ),
                None => eprintln!("{id:<32} no location"),
            }
        }
    }

    // The shipped CSP files document every value inline; a comment left in the
    // parsed value would make every number unparseable.
    #[test]
    fn ini_value_reads_sections_and_strips_comments() {
        let text = "[SEASONS]\nALLOW_ADJUSTMENTS=0.5 ; Use seasons; more prose\n\n[OTHER]\nALLOW_ADJUSTMENTS=2\n";
        assert_eq!(ini_value(text, "SEASONS", "ALLOW_ADJUSTMENTS").as_deref(), Some("0.5"));
        assert_eq!(ini_value(text, "OTHER", "ALLOW_ADJUSTMENTS").as_deref(), Some("2"));
        assert_eq!(ini_value(text, "MISSING", "ALLOW_ADJUSTMENTS"), None);
    }

    // Real shape of `data_track_params.ini`, and the layout of an install:
    // section per track id, no per-layout entry.
    #[test]
    fn track_location_reads_csp_params_before_geotags() {
        let dir = crate::testutil::temp_dir("sun-params");
        let cfg = dir.join("extension").join("config");
        std::fs::create_dir_all(&cfg).unwrap();
        std::fs::write(
            cfg.join("data_track_params.ini"),
            "[ks_nordschleife]\nHEADING_ANGLE=-10\nLATITUDE=50.3356\nLONGITUDE=6.9475\nTIMEZONE=Europe/Berlin\n",
        )
        .unwrap();
        let loc = track_location(&dir, "ks_nordschleife", None).expect("track found");
        assert_eq!(loc.source, CoordSource::Csp, "CSP's own table is the reference");
        assert_eq!(loc.timezone.as_deref(), Some("Europe/Berlin"));
        assert!((loc.latitude - 50.3356).abs() < 1e-6, "latitude read as written");
    }

    // A mod track CSP does not list: its own geotags carry it. Kunos' literal
    // ["lat", "lon"] placeholder must NOT be taken for coordinates.
    #[test]
    fn geotags_fallback_ignores_the_kunos_placeholder() {
        let dir = crate::testutil::temp_dir("sun-geotags");
        let ui = dir.join("content").join("tracks").join("rmi_mdpietra").join("ui");
        std::fs::create_dir_all(&ui).unwrap();
        std::fs::write(
            ui.join("ui_track.json"),
            r#"{"name": "Montagna", "geotags": ["44.8976", "8.8637"]}"#,
        )
        .unwrap();
        let loc = track_location(&dir, "rmi_mdpietra", None).expect("geotags found");
        assert_eq!(loc.source, CoordSource::Geotags);
        assert!(loc.timezone.is_none(), "geotags carry no timezone");

        let ui = dir.join("content").join("tracks").join("spa").join("ui");
        std::fs::create_dir_all(&ui).unwrap();
        std::fs::write(ui.join("ui_track.json"), r#"{"geotags": ["lat", "lon"]}"#).unwrap();
        assert!(
            track_location(&dir, "spa", None).is_none(),
            "the placeholder is not a location"
        );
    }
}
