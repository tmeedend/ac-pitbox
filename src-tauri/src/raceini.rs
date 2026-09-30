//! Re-injection into the `race.ini` Content Manager writes: the player's skin
//! for an offline session (SESSION§2), and the server's features for an online
//! one (`docs/online-join-research.md`). The skin is the case this module was
//! built and measured on:
//!
//! The Quick Drive preset carries no skin field for the player's car: CM falls
//! back on its own per-car memory (`CarObject.SelectedSkin`), so the skin picked
//! in Pit Box was simply ignored. Measured on a real launch (2026-08-13):
//!
//! ```text
//! +0 ms      acmanager://race/quick sent to CM
//! +1694 ms   CM rewrites Documents\Assetto Corsa\cfg\race.ini and spawns acs.exe
//! +1702 ms   we rewrite SKIN= (7 ms later) — the game then loads OUR skin
//! ```
//!
//! CM writes `race.ini` at the very instant it spawns `acs.exe`, but the game
//! only reads the file seconds later, while booting. That window is what this
//! module exploits: watch the file, and patch it the moment CM is done with it.
//! Confirmed end to end by Assetto Corsa's own `logs\log.txt`, which echoes back
//! the `SKIN=` it loaded.
//!
//! Deliberately best-effort: arriving too late simply leaves CM's skin in place,
//! exactly like before this module existed. The replacement is atomic
//! (`fs::rename`), so a half-written `race.ini` can never reach the game.
//!
//! Two traps this file is shaped around, both found on real CM output:
//! - **Section order is not predictable.** CM writes `[CAR_5]` before `[CAR_4]`,
//!   `[BENCHMARK]` after `[CAR_0]`… so "the first `SKIN=` after `[RACE]`" is
//!   wrong — the current section has to be tracked properly.
//! - **The file is UTF-8, but holds driver names with accents.** Everything here
//!   works on bytes: the patch itself is pure ASCII, and never decoding means an
//!   oddly encoded file is neither rejected on read nor corrupted on write.

use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Sections describing the player's own car. `[RACE]` holds the car itself,
/// `[CAR_0]` its grid entry (recognisable by `MODEL=-`: it inherits the model
/// from `[RACE]`). Opponents live in `[CAR_1]`… and must stay untouched — CM
/// filled them from our own grid, they are already right.
const PLAYER_SECTIONS: [&[u8]; 2] = [b"[RACE]", b"[CAR_0]"];

/// How often the watcher looks at `race.ini`. The whole window is a few hundred
/// ms of game boot, so polling has to be tight; a metadata check comes first, so
/// this costs a `stat` per tick, not a read.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// How long to wait for CM to write `race.ini`. Generous on purpose: a warm CM
/// answers in under 2 s, but a cold start (CM boot + Steam) took 24 s in
/// testing, and a slow machine can do worse.
const WATCH_TIMEOUT: Duration = Duration::from_secs(120);

/// `Documents\Assetto Corsa\cfg\race.ini` — the file the game reads to start a
/// session. Same resolution as `showroom::resolve_ac_cfg_dir`; kept local
/// because neither module owns the other.
pub fn race_ini_path() -> Option<PathBuf> {
    Some(dirs::document_dir()?.join("Assetto Corsa").join("cfg").join("race.ini"))
}

fn trim(line: &[u8]) -> &[u8] {
    let mut start = 0;
    let mut end = line.len();
    while start < end && line[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && line[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    &line[start..end]
}

/// The `[NAME]` a line declares, if it declares one.
fn section_name(line: &[u8]) -> Option<&[u8]> {
    let trimmed = trim(line);
    match (trimmed.first(), trimmed.last()) {
        (Some(b'['), Some(b']')) => Some(trimmed),
        _ => None,
    }
}

fn is_one_of(name: &[u8], sections: &[&[u8]]) -> bool {
    sections.iter().any(|section| name.eq_ignore_ascii_case(section))
}

/// The line ending of a line, so a patched line keeps the one it had.
fn terminator(line: &[u8]) -> &[u8] {
    if line.ends_with(b"\r\n") {
        b"\r\n"
    } else if line.ends_with(b"\n") {
        b"\n"
    } else {
        b""
    }
}

/// Replaces `SKIN=` in `[RACE]` and `[CAR_0]` only, leaving every other byte —
/// opponents' skins included — exactly as it was. A player section without a
/// `SKIN=` line gets one appended rather than being silently skipped.
pub fn set_player_skin(ini: &[u8], skin: &str) -> Vec<u8> {
    set_key(ini, &PLAYER_SECTIONS, b"SKIN", skin)
}

/// Sets `[REMOTE] __FEATURES=` — the capabilities a multiplayer server declares
/// in its `/JSON`, comma-separated as Content Manager's own join writes them.
///
/// CSP reads this line to decide what to send in the handshake: without
/// `STEAM_TICKET` in it, no Steam ticket goes out and an AssettoServer refuses
/// the connection (`ACP_AUTH_FAILED`). CM's native join writes it; the
/// `race/online` route Pit Box uses does not — measured, see
/// `docs/online-join-research.md`.
pub fn set_remote_features(ini: &[u8], features: &[String]) -> Vec<u8> {
    set_key(ini, &[b"[REMOTE]"], b"__FEATURES", &features.join(","))
}

/// Watches `race.ini` and injects the server's features into `[REMOTE]` as
/// soon as Content Manager has written the online session for `car_id`.
pub fn spawn_remote_features_patcher(car_id: String, features: Vec<String>) {
    spawn_patcher("server features", car_id, move |ini| {
        set_remote_features(ini, &features)
    });
}

/// Sets `key=value` in each of `sections`, and nowhere else: an existing line
/// is replaced in place, a missing one appended at the end of its section.
/// Every other byte comes back as it was.
fn set_key(ini: &[u8], sections: &[&[u8]], key: &[u8], value: &str) -> Vec<u8> {
    let eol: &[u8] = if ini.windows(2).any(|pair| pair == b"\r\n") {
        b"\r\n"
    } else {
        b"\n"
    };
    let mut prefix = key.to_vec();
    prefix.push(b'=');
    let mut out = Vec::with_capacity(ini.len() + 64);
    let mut inside = false;
    let mut key_seen = false;

    let push_line = |out: &mut Vec<u8>, end: &[u8]| {
        out.extend_from_slice(&prefix);
        out.extend_from_slice(value.as_bytes());
        out.extend_from_slice(end);
    };

    for line in ini.split_inclusive(|&byte| byte == b'\n') {
        if let Some(name) = section_name(line) {
            // Leaving a target section that never declared the key: add it
            // now, while we are still inside it.
            if inside && !key_seen {
                push_line(&mut out, eol);
            }
            inside = is_one_of(name, sections);
            key_seen = false;
            out.extend_from_slice(line);
            continue;
        }
        if inside && trim(line).starts_with(&prefix) {
            key_seen = true;
            push_line(&mut out, terminator(line));
            continue;
        }
        out.extend_from_slice(line);
    }
    if inside && !key_seen {
        push_line(&mut out, eol);
    }
    out
}

/// `[RACE] MODEL=` — the player's car id. Used as a guard: we only patch a
/// `race.ini` that actually describes the session we just asked CM for.
pub fn player_car_model(ini: &[u8]) -> Option<String> {
    let mut in_race = false;
    for line in ini.split_inclusive(|&byte| byte == b'\n') {
        if let Some(name) = section_name(line) {
            in_race = name.eq_ignore_ascii_case(b"[RACE]");
            continue;
        }
        let trimmed = trim(line);
        if in_race {
            if let Some(value) = trimmed.strip_prefix(b"MODEL=") {
                return String::from_utf8(value.to_vec()).ok();
            }
        }
    }
    None
}

/// Writes `patched` over `race.ini` atomically: the game reads either the old
/// file or the new one, never a torn one. Same directory on purpose —
/// `fs::rename` only replaces in place within a volume.
fn replace_atomically(path: &std::path::Path, patched: &[u8]) -> std::io::Result<()> {
    let temp = path.with_extension("ini.pitbox-tmp");
    std::fs::write(&temp, patched)?;
    std::fs::rename(&temp, path)
}

/// Watches `race.ini` and injects `skin` into the player's sections as soon as
/// Content Manager has written it.
pub fn spawn_player_skin_patcher(car_id: String, skin: String) {
    spawn_patcher("player skin", car_id, move |ini| set_player_skin(ini, &skin));
}

/// Watches `race.ini` and applies `patch` to it as soon as Content Manager has
/// written the session for `car_id`. `what` names the patch in the log.
///
/// Returns immediately; the work happens on its own thread and is best-effort
/// from end to end (see module docs). Every giving-up path logs a warning: on a
/// packaged build there is no console, so an unlogged failure is a bug report
/// nobody can act on.
fn spawn_patcher(what: &'static str, car_id: String, patch: impl Fn(&[u8]) -> Vec<u8> + Send + 'static) {
    let Some(path) = race_ini_path() else {
        log::warn!("{what}: cannot resolve the Documents folder, race.ini left untouched");
        return;
    };
    // Baseline taken here, before CM had time to write: the watcher fires on the
    // first change, which is CM's own write.
    let baseline = std::fs::metadata(&path).ok().map(|m| (m.len(), m.modified().ok()));

    std::thread::spawn(move || {
        let deadline = Instant::now() + WATCH_TIMEOUT;
        while Instant::now() < deadline {
            std::thread::sleep(POLL_INTERVAL);
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            if baseline == Some((meta.len(), meta.modified().ok())) {
                continue;
            }
            let Ok(ini) = std::fs::read(&path) else {
                continue;
            };
            // Someone else's race.ini (an unrelated CM launch, a leftover):
            // keep waiting rather than stamping our patch onto another session.
            if player_car_model(&ini).as_deref() != Some(car_id.as_str()) {
                continue;
            }
            let patched = patch(&ini);
            if patched == ini {
                return; // CM already wrote what we wanted — nothing to do.
            }
            match replace_atomically(&path, &patched) {
                Ok(()) => log::info!("{what}: race.ini patched for « {car_id} »"),
                Err(e) => log::warn!("{what}: cannot rewrite race.ini ({e}), CM's version kept"),
            }
            return;
        }
        log::warn!("{what}: Content Manager never rewrote race.ini within {WATCH_TIMEOUT:?}, CM's version kept");
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Faithful to real CM output: sections in scrambled order, CRLF endings,
    /// player in `[RACE]`/`[CAR_0]` (`MODEL=-`), opponents in `[CAR_n]`.
    const RACE: &[u8] = b"[RACE]\r\nMODEL=ks_praga_r1\r\nSKIN=00_blue\r\nTRACK=magione\r\n\
[CAR_5]\r\nMODEL=ks_audi_r8_lms\r\nSKIN=53_neo_racing\r\nAI_LEVEL=97\r\n\
[CAR_0]\r\nSETUP=\r\nSKIN=00_blue\r\nMODEL=-\r\nDRIVER_NAME=Player\r\n\
[BENCHMARK]\r\nACTIVE=0\r\n\
[CAR_1]\r\nMODEL=ks_bmw_m4_gt3\r\nSKIN=09_team_mando\r\nAI_LEVEL=93\r\n";

    /// The rule this whole module exists for: the player's skin changes, the
    /// opponents' skins — which CM filled from our own grid — must not (SESSION§2).
    #[test]
    fn only_player_sections_are_patched() {
        let out = set_player_skin(RACE, "12_endurance");
        let text = String::from_utf8(out).unwrap();
        assert_eq!(
            text.matches("SKIN=12_endurance").count(),
            2,
            "exactly the two player lines carry the new skin"
        );
        assert!(text.contains("SKIN=53_neo_racing"), "opponent [CAR_5] skin preserved");
        assert!(text.contains("SKIN=09_team_mando"), "opponent [CAR_1] skin preserved");
    }

    /// Sections come out of CM unordered, so tracking the current section is the
    /// only correct way to find `[CAR_0]` — never "the first SKIN= after [RACE]".
    #[test]
    fn patching_survives_scrambled_section_order() {
        let out = set_player_skin(RACE, "red");
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.contains("[CAR_0]\r\nSETUP=\r\nSKIN=red\r\n"),
            "[CAR_0] patched even though [CAR_5] came before it"
        );
    }

    /// The game re-reads this file whole: anything but the two skin lines must
    /// come back byte for byte, line endings included.
    #[test]
    fn everything_else_is_preserved_byte_for_byte() {
        let out = set_player_skin(RACE, "00_blue");
        assert_eq!(out, RACE, "patching with the skin already in place is a no-op");
    }

    /// Driver names carry accents, and a file written in another encoding must
    /// still survive a round trip — hence bytes rather than `String` throughout.
    #[test]
    fn non_utf8_bytes_are_left_alone() {
        let mut ini = b"[RACE]\r\nMODEL=abarth500\r\nSKIN=white\r\n[CAR_0]\r\nSKIN=white\r\nDRIVER_NAME=Th".to_vec();
        ini.push(0xE9); // « é » in Windows-1252: invalid UTF-8 on purpose
        ini.extend_from_slice(b"o\r\n");
        let out = set_player_skin(&ini, "red");
        assert!(
            out.windows(4).any(|w| w == [b'T', b'h', 0xE9, b'o']),
            "the raw driver-name bytes are untouched"
        );
        assert_eq!(
            out.windows(9).filter(|w| *w == b"SKIN=red\r").count(),
            2,
            "both player sections still patched"
        );
    }

    /// A player section without a `SKIN=` line would silently drop the feature;
    /// the line is added instead, inside the section it belongs to.
    #[test]
    fn skin_line_is_added_when_missing() {
        let ini = b"[RACE]\r\nMODEL=abarth500\r\n[CAR_0]\r\nMODEL=-\r\n[CAR_1]\r\nSKIN=other\r\n";
        let out = set_player_skin(ini, "red");
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.starts_with("[RACE]\r\nMODEL=abarth500\r\nSKIN=red\r\n[CAR_0]"),
            "skin added at the end of [RACE], before the next section: {text}"
        );
        assert!(text.contains("[CAR_0]\r\nMODEL=-\r\nSKIN=red\r\n"), "same for [CAR_0]");
        assert!(text.contains("SKIN=other"), "opponent untouched");
    }

    /// What CM's `race/online` writes, measured on a real join: a `[REMOTE]`
    /// section with no `__FEATURES` line, which is what gets an AssettoServer to
    /// refuse us. The line lands inside `[REMOTE]`, before the next section.
    const ONLINE: &[u8] = b"[CAR_0]\r\nSKIN=05_sunburst_yellow\r\nMODEL=-\r\n\
[RACE]\r\nMODEL=ks_mazda_miata\r\nTRACK=la_canyons\r\n\
[REMOTE]\r\nACTIVE=1\r\nREQUESTED_CAR=ks_mazda_miata\r\n__CM_EXTENDED=0\r\nSERVER_PORT=9602\r\n\
[SESSION_0]\r\nNAME=Nothing\r\n";

    /// Rule (online-join-research.md): the server's features reach CSP through
    /// `[REMOTE] __FEATURES`, comma-separated, and nothing else moves.
    #[test]
    fn remote_features_land_in_the_remote_section() {
        let features = vec!["STEAM_TICKET".to_string(), "WEATHERFX_V1".to_string()];
        let text = String::from_utf8(set_remote_features(ONLINE, &features)).unwrap();
        assert!(
            text.contains("SERVER_PORT=9602\r\n__FEATURES=STEAM_TICKET,WEATHERFX_V1\r\n[SESSION_0]"),
            "appended at the end of [REMOTE], before the next section: {text}"
        );
        assert_eq!(text.matches("__FEATURES=").count(), 1, "written once, in [REMOTE] only");
        assert!(
            text.contains("[CAR_0]\r\nSKIN=05_sunburst_yellow\r\n"),
            "other sections untouched"
        );
    }

    /// A second pass (or a CM that one day writes the line itself) replaces
    /// the value instead of stacking a duplicate the game would read twice.
    #[test]
    fn remote_features_replace_an_existing_line() {
        let once = set_remote_features(ONLINE, &["EMOJI".to_string()]);
        let twice = String::from_utf8(set_remote_features(&once, &["STEAM_TICKET".to_string()])).unwrap();
        assert_eq!(twice.matches("__FEATURES=").count(), 1, "still a single line");
        assert!(twice.contains("__FEATURES=STEAM_TICKET\r\n"), "with the new value");
    }

    /// The guard that keeps us from stamping a skin onto somebody else's
    /// session: the file has to describe the car we just asked CM for.
    #[test]
    fn player_car_model_reads_the_race_section() {
        assert_eq!(player_car_model(RACE).as_deref(), Some("ks_praga_r1"));
        assert_eq!(
            player_car_model(b"[CAR_5]\r\nMODEL=ks_audi_r8_lms\r\n").as_deref(),
            None,
            "MODEL= outside [RACE] is an opponent, not the player"
        );
    }
}
