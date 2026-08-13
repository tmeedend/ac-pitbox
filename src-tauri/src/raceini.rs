//! Player skin re-injection into the `race.ini` Content Manager writes (§9.2).
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

fn is_player_section(name: &[u8]) -> bool {
    PLAYER_SECTIONS.iter().any(|section| name.eq_ignore_ascii_case(section))
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
    let eol: &[u8] = if ini.windows(2).any(|pair| pair == b"\r\n") {
        b"\r\n"
    } else {
        b"\n"
    };
    let mut out = Vec::with_capacity(ini.len() + 64);
    let mut in_player = false;
    let mut skin_seen = false;

    let push_skin = |out: &mut Vec<u8>, end: &[u8]| {
        out.extend_from_slice(b"SKIN=");
        out.extend_from_slice(skin.as_bytes());
        out.extend_from_slice(end);
    };

    for line in ini.split_inclusive(|&byte| byte == b'\n') {
        if let Some(name) = section_name(line) {
            // Leaving a player section that never declared a skin: add one now,
            // while we are still inside it.
            if in_player && !skin_seen {
                push_skin(&mut out, eol);
            }
            in_player = is_player_section(name);
            skin_seen = false;
            out.extend_from_slice(line);
            continue;
        }
        if in_player && trim(line).starts_with(b"SKIN=") {
            skin_seen = true;
            push_skin(&mut out, terminator(line));
            continue;
        }
        out.extend_from_slice(line);
    }
    if in_player && !skin_seen {
        push_skin(&mut out, eol);
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
///
/// Returns immediately; the work happens on its own thread and is best-effort
/// from end to end (see module docs). Every giving-up path logs a warning: on a
/// packaged build there is no console, so an unlogged failure is a bug report
/// nobody can act on.
pub fn spawn_player_skin_patcher(car_id: String, skin: String) {
    let Some(path) = race_ini_path() else {
        log::warn!("player skin: cannot resolve the Documents folder, race.ini left untouched");
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
            // keep waiting rather than stamping our skin onto another session.
            if player_car_model(&ini).as_deref() != Some(car_id.as_str()) {
                continue;
            }
            let patched = set_player_skin(&ini, &skin);
            if patched == ini {
                return; // CM already picked that skin — nothing to do.
            }
            match replace_atomically(&path, &patched) {
                Ok(()) => log::info!("player skin: race.ini patched with « {skin} » for « {car_id} »"),
                Err(e) => log::warn!("player skin: cannot rewrite race.ini ({e}), CM's skin kept"),
            }
            return;
        }
        log::warn!("player skin: Content Manager never rewrote race.ini within {WATCH_TIMEOUT:?}, CM's skin kept");
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
    /// opponents' skins — which CM filled from our own grid — must not (§9.2).
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
