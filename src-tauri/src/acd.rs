//! Reading `data.acd`, the container holding a car's physics files.
//!
//! Only ever **read**, and only from the user's own install — nothing is
//! written back and nothing is redistributed, exactly like the KN5 and FSB5
//! readers elsewhere in this crate. What it buys is two numbers no other file
//! carries in the clear: the engine's idle speed and its rev limit.
//!
//! The position on formats is the project's existing one (§2 of
//! `docs/SPEC-preview-3d-kn5.md`): the offsets, constants and arithmetic of a
//! format are technical facts; it is a third party's *code* that carries a
//! licence. The key derivation below was learned from the algorithm published
//! in [`bovis/acd_extractor`](https://github.com/bovis/acd_extractor) (Ruby),
//! then written from scratch here and **verified against measurements** rather
//! than trusted — see the tests.
//!
//! ## The container
//!
//! Plain, and legible from a hex dump:
//!
//! | | |
//! | --- | --- |
//! | `-1111` as `i32` | optional marker; when present, an `i32` version follows |
//! | `u32` | length of the entry's name |
//! | bytes | the name, ASCII, **in clear** (`engine.ini`) |
//! | `u32` | number of characters of content |
//! | `u32` × n | one character per 32-bit word, low byte significant |
//!
//! ## The cipher
//!
//! `cipher[i] = plain[i] + key[i % key.len()]`, the key being a **string**: the
//! decimal spellings of eight bytes joined by dashes, e.g.
//! `134-220-214-104-26-97-64-49` for `ks_ford_gt40`. Each of the eight is a
//! small function of the car's folder name, lowercased.
//!
//! ## Two routes, and why the slow one is kept
//!
//! [`key_for`] derives the key from the folder name: instant, and right on all
//! 299 cars of the reference install. But it **cannot** work on a car whose
//! folder was renamed after packing — the key was fixed at pack time, and the
//! name no longer produces it. That happens: mod packs rename folders.
//!
//! So [`recover_key`] stays as a second attempt, and works the other way round,
//! from the ciphertext alone. It cannot use the folder name at all — that is
//! precisely what is wrong in the case it exists for.

use std::path::Path;

/// What this module exists to find.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EngineData {
    /// `MINIMUM` of `[ENGINE_DATA]` — where the engine ticks over.
    ///
    /// **Reported as written, sign included.** A negative `MINIMUM` exists in
    /// the wild and this module does not claim to know what it means; deciding
    /// what to believe belongs to `enginesound::idle_from_minimum`, which has
    /// the rev ceiling to judge it against.
    pub idle_rev: Option<f32>,
    /// `LIMITER` — where it stops. Zero means "no limiter" and is dropped.
    pub limiter_rev: Option<f32>,
}

impl EngineData {
    fn is_empty(&self) -> bool {
        self.idle_rev.is_none() && self.limiter_rev.is_none()
    }
}

/// The marker that says a version field follows.
const VERSION_MARKER: i32 = -1111;
/// A name longer than this is a misparse, not a filename.
const MAX_NAME: usize = 260;
/// The section every `engine.ini` carries. Used as a known plaintext: it is
/// what says a key is the right one rather than merely a plausible one.
const ENGINE_SECTION: &str = "[ENGINE_DATA]";

/// The eight numbers of a car's key, from its folder name.
///
/// Faithfully the algorithm AC uses, with two things worth knowing about the
/// arithmetic:
///
/// - parts 2 and 5 multiply without ever dividing, so their intermediates run
///   away past any fixed width. Only the low byte survives, and wrapping
///   arithmetic gives exactly that — 2⁶⁴ is a multiple of 256, so wrapping and
///   then masking is the same answer a bignum would reach;
/// - parts 3, 7 and 8 divide or take a remainder, which **cannot** be done
///   modulo anything. They do not need to be: each is self-limiting (a multiply
///   followed by a divide of similar size, a remainder, a divide), so their
///   values stay in the hundreds and `i64` is roomy.
///
/// Rust truncates integer division toward zero, which is what this wants. The
/// Ruby original spells the negative case out by hand because Ruby floors
/// instead — same intent, different language getting in the way.
fn key_for(car_id: &str) -> Vec<u8> {
    let name: Vec<i64> = car_id.to_lowercase().bytes().map(i64::from).collect();
    let n = name.len();
    let at = |i: usize| name.get(i).copied().unwrap_or(0);
    // Division and remainder by a character; a folder name cannot contain a
    // NUL, but a guard costs nothing and a panic would cost the audition.
    let safe_div = |t: i64, d: i64| if d == 0 { t } else { t / d };
    let safe_rem = |t: i64, d: i64| if d == 0 { t } else { t % d };

    let mut parts = [0i64; 8];

    parts[0] = name.iter().sum();

    let mut t = 0i64;
    let mut i = 0usize;
    while i + 1 < n {
        t = t.wrapping_mul(at(i)).wrapping_sub(at(i + 1));
        i += 2;
    }
    parts[1] = t;

    t = 0;
    i = 1;
    while i + 3 < n {
        t *= at(i);
        t = safe_div(t, at(i + 1) + 0x1b);
        t += -0x1b - at(i - 1);
        // Three, not four. The original walks its index forward one, back two
        // and forward four, which nets three — and writing the four that is
        // actually in the source is how this came out wrong the first time.
        i += 3;
    }
    parts[2] = t;

    parts[3] = 0x1683 - name.iter().skip(1).sum::<i64>();

    t = 0x42;
    i = 1;
    while i + 4 < n {
        let scaled = t.wrapping_mul(at(i).wrapping_add(0xf));
        t = at(i - 1).wrapping_add(0xf).wrapping_mul(scaled).wrapping_add(0x16);
        i += 4;
    }
    parts[4] = t;

    t = 0x65;
    i = 0;
    while i + 2 < n {
        t -= at(i);
        i += 2;
    }
    parts[5] = t;

    t = 0xab;
    i = 0;
    while i + 2 < n {
        t = safe_rem(t, at(i));
        i += 2;
    }
    parts[6] = t;

    t = 0xab;
    i = 0;
    while i + 1 < n {
        t = safe_div(t, at(i)) + at(i + 1);
        i += 1;
    }
    parts[7] = t;

    let text: Vec<String> = parts.iter().map(|p| ((p & 0xFF) as u8).to_string()).collect();
    text.join("-").into_bytes()
}

/// Every entry of the container: name, and content bytes still encrypted.
fn entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut at = 0usize;

    let read_u32 = |at: usize| -> Option<u32> {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };

    if read_u32(0).map(|v| v as i32) == Some(VERSION_MARKER) {
        at = 8; // marker plus the version that follows it
    }

    while let Some(name_len) = read_u32(at) {
        at += 4;
        let name_len = name_len as usize;
        if name_len == 0 || name_len > MAX_NAME {
            break;
        }
        let Some(raw) = bytes.get(at..at + name_len) else { break };
        let name = String::from_utf8_lossy(raw).into_owned();
        at += name_len;

        let Some(count) = read_u32(at) else { break };
        at += 4;
        let count = count as usize;
        let Some(words) = bytes.get(at..at.saturating_add(count.saturating_mul(4))) else {
            break;
        };
        // One character per 32-bit word; everything above the low byte is zero.
        out.push((name, words.iter().step_by(4).copied().collect()));
        at += count * 4;
    }
    out
}

fn decrypt(data: &[u8], key: &[u8]) -> String {
    let plain: Vec<u8> = data
        .iter()
        .enumerate()
        .map(|(i, &c)| c.wrapping_sub(key[i % key.len()]))
        .collect();
    String::from_utf8_lossy(&plain).into_owned()
}

// ---------------------------------------------------------------------------
// The fallback: recovering a key from ciphertext alone
// ---------------------------------------------------------------------------

/// Below this there is not enough text to pin a key down.
const MIN_TEXT: usize = 200;
/// Share of decrypted bytes that must look like text. Not 100 %: one accented
/// character in one comment would otherwise throw away the whole file.
const PRINTABLE_SHARE: f32 = 0.97;
/// Eight numbers of one to three digits, plus seven dashes.
const KEY_LEN: std::ops::RangeInclusive<usize> = 15..=31;
/// How many complete keys one period may be asked about before it is given up
/// on, so a stubborn file cannot cost real time. Per period, never shared.
const MAX_ASSEMBLIES: usize = 4_000;
/// How many entries to try as a source, longest first. More text means fewer
/// ambiguous positions; the longest is sometimes a binary lookup table that
/// yields nothing, so a handful are tried.
const LONGEST_TRIED: usize = 12;

/// Wide enough to accept a file with the odd oddity in it.
fn is_texty(byte: u8) -> bool {
    matches!(byte, 0x20..=0x7e | b'\t' | b'\n' | b'\r')
}

/// Narrow enough to rank one candidate above another, and the narrowness is
/// the whole point.
///
/// A key three away from the right one keeps almost everything printable — and
/// it keeps the *newlines* too, because 10 shifted by three lands on 13, a
/// carriage return that [`is_texty`] waves straight through. AC writes its
/// `.ini` files with bare line feeds, so the true key is the only one that
/// needs no carriage returns to explain itself. Accept broadly, rank strictly.
fn is_plainly_texty(byte: u8) -> bool {
    matches!(byte, 0x20..=0x7e | b'\t' | b'\n')
}

/// Every key that could have encrypted this entry, likeliest first.
///
/// The search rests on the key being a *string of digits and dashes*: eleven
/// possible characters per position rather than 256, which a few thousand bytes
/// of `.ini` text collapse to one almost everywhere. What comes out is only
/// *structurally* valid — the caller decides, by decrypting `engine.ini` and
/// looking for [`ENGINE_SECTION`].
///
/// That last step is not a formality, and leaving it out cost two wrong
/// implementations. Shift `.ini` text by three and it stays printable — even
/// newlines survive, 10 landing on 13, which is a carriage return and looks
/// innocent. Worse, *section headers survive too*: the brackets sit on key
/// positions that happened to be right, so a wrong key produced `[HEADEU]` and
/// `[HNGIQE_DITA]` and scored exactly as well as the true one.
fn recover_key(data: &[u8], accept: &dyn Fn(&[u8]) -> bool) -> Option<Vec<u8>> {
    if data.len() < MIN_TEXT {
        return None;
    }
    const CANDIDATES: [u8; 11] = [b'-', b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9'];

    for period in KEY_LEN {
        let mut columns: Vec<Vec<u8>> = Vec::with_capacity(period);
        for offset in 0..period {
            let column: Vec<u8> = data.iter().skip(offset).step_by(period).copied().collect();
            let needed = column.len() as f32 * PRINTABLE_SHARE;
            let mut viable: Vec<(usize, u8)> = CANDIDATES
                .iter()
                .filter(|&&k| column.iter().filter(|&&c| is_texty(c.wrapping_sub(k))).count() as f32 >= needed)
                .map(|&k| {
                    (
                        column.iter().filter(|&&c| is_plainly_texty(c.wrapping_sub(k))).count(),
                        k,
                    )
                })
                .collect();
            if viable.is_empty() {
                columns.clear();
                break;
            }
            // Likeliest first, so the right key tends to come up early.
            viable.sort_by_key(|&(hits, _)| std::cmp::Reverse(hits));
            columns.push(viable.into_iter().map(|(_, k)| k).collect());
        }
        if columns.len() != period {
            continue;
        }
        // Judged **within** this period, never pooled across periods. Pooling
        // was tried and is quietly broken: a wrong period yields hundreds of
        // structurally valid keys, exhausts any global budget, and the right
        // period is never reached at all.
        if let Some(key) = assemble(&columns, accept) {
            return Some(key);
        }
    }
    None
}

/// Walks the per-position candidates, handing each structurally valid key to
/// `accept` and stopping at the first it takes.
///
/// Depth-first with the shape checked as it goes, so a branch dies at the
/// character that breaks it rather than at the end: a digit that pushes a
/// number over 255, an eighth dash, or a leading zero.
fn assemble(columns: &[Vec<u8>], accept: &dyn Fn(&[u8]) -> bool) -> Option<Vec<u8>> {
    fn walk(
        columns: &[Vec<u8>],
        at: usize,
        key: &mut Vec<u8>,
        accept: &dyn Fn(&[u8]) -> bool,
        budget: &mut usize,
    ) -> bool {
        if *budget == 0 {
            return false;
        }
        if at == columns.len() {
            *budget -= 1;
            return reads_as_eight_numbers(key) && accept(key);
        }
        for &candidate in &columns[at] {
            key.push(candidate);
            if could_still_read_as_eight_numbers(key) && walk(columns, at + 1, key, accept, budget) {
                return true;
            }
            key.pop();
        }
        false
    }
    let mut key = Vec::with_capacity(columns.len());
    let mut budget = MAX_ASSEMBLIES;
    walk(columns, 0, &mut key, accept, &mut budget).then_some(key)
}

fn reads_as_eight_numbers(key: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(key) else {
        return false;
    };
    let parts: Vec<&str> = text.split('-').collect();
    parts.len() == 8
        && parts.iter().all(|part| {
            // "007" would spell one number two ways; the writer never did.
            let padded = part.len() > 1 && part.starts_with('0');
            !part.is_empty() && !padded && part.parse::<u32>().is_ok_and(|value| value <= 255)
        })
}

/// Whether a partly-built key could still become a valid one.
fn could_still_read_as_eight_numbers(key: &[u8]) -> bool {
    let mut dashes = 0usize;
    let mut run: Option<u32> = None;
    for &byte in key {
        if byte == b'-' {
            if run.is_none() {
                return false; // a leading dash, or two in a row
            }
            dashes += 1;
            if dashes > 7 {
                return false;
            }
            run = None;
        } else {
            let digit = u32::from(byte - b'0');
            match run {
                None => run = Some(digit),
                Some(0) => return false, // leading zero
                Some(value) => {
                    let next = value * 10 + digit;
                    if next > 255 {
                        return false;
                    }
                    run = Some(next);
                }
            }
        }
    }
    true
}

/// Reads one `KEY=value` out of an INI section, ignoring comments.
fn ini_number(text: &str, key: &str) -> Option<f32> {
    for line in text.lines() {
        let line = line.split(';').next().unwrap_or(line).trim();
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        if !name.trim().eq_ignore_ascii_case(key) {
            continue;
        }
        if let Ok(value) = value.trim().parse::<f32>() {
            return Some(value);
        }
    }
    None
}

/// Idle speed and rev limit for a car, straight from its own physics.
///
/// `None` when the file is absent, unreadable, or refuses every key — each of
/// which is a reason to fall back on an estimate, never to fail.
pub fn read_engine_data(car_dir: &Path) -> Option<EngineData> {
    let car_id = car_dir.file_name()?.to_string_lossy().to_string();
    let bytes = std::fs::read(car_dir.join("data.acd")).ok()?;

    let mut entries = entries(&bytes);
    let engine_ciphered = entries
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("engine.ini"))
        .map(|(_, data)| data.clone())?;

    let opened = |key: &[u8]| -> Option<String> {
        let text = decrypt(&engine_ciphered, key);
        text.contains(ENGINE_SECTION).then_some(text)
    };

    // The folder name first: instant, and right on every car of the reference
    // install.
    let engine = opened(&key_for(&car_id)).or_else(|| {
        // Renamed since it was packed, then. Work from the ciphertext instead —
        // and note this route must ignore the folder name entirely, since a
        // wrong name is the whole reason for being here.
        entries.sort_by_key(|(_, data)| std::cmp::Reverse(data.len()));
        let works = |key: &[u8]| decrypt(&engine_ciphered, key).contains(ENGINE_SECTION);
        entries
            .iter()
            .take(LONGEST_TRIED)
            .find_map(|(_, data)| recover_key(data, &works))
            .and_then(|key| opened(&key))
    })?;

    let found = EngineData {
        idle_rev: ini_number(&engine, "MINIMUM").filter(|v| *v != 0.0),
        // Some cars declare no limiter with a zero rather than by omission.
        limiter_rev: ini_number(&engine, "LIMITER").filter(|v| *v > 0.0),
    };
    (!found.is_empty()).then_some(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Keys measured out of two real containers **before** the derivation was
    /// known, by solving them from the ciphertext. They are the reason
    /// [`key_for`] can be trusted: it has to reproduce numbers that were
    /// obtained without it.
    #[test]
    fn the_derived_key_matches_keys_measured_from_real_files() {
        assert_eq!(
            String::from_utf8(key_for("ks_ford_gt40")).unwrap(),
            "134-220-214-104-26-97-64-49"
        );
        assert_eq!(
            String::from_utf8(key_for("abarth500")).unwrap(),
            "7-248-6-221-246-250-21-49"
        );
    }

    /// The name is lowercased before anything else, so a container packed under
    /// one spelling opens under another.
    #[test]
    fn the_folder_name_is_case_insensitive() {
        assert_eq!(key_for("KS_Ford_GT40"), key_for("ks_ford_gt40"));
    }

    /// A name too short for the loops must come back with a key rather than a
    /// panic — a subtraction or an index away from being an arithmetic crash.
    #[test]
    fn an_absurdly_short_name_still_yields_a_key() {
        for name in ["", "a", "ab", "abc", "abcd"] {
            let key = key_for(name);
            assert!(
                reads_as_eight_numbers(&key),
                "{name:?} gave {:?}",
                String::from_utf8_lossy(&key)
            );
        }
    }

    /// Builds a container the way the game does, so the reader is exercised on
    /// its real shape rather than on a convenient one.
    fn synthetic_acd(files: &[(&str, &str)], key: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&VERSION_MARKER.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
        for (name, content) in files {
            out.extend_from_slice(&(name.len() as u32).to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(&(content.len() as u32).to_le_bytes());
            for (i, byte) in content.bytes().enumerate() {
                let ciphered = byte.wrapping_add(key[i % key.len()]);
                out.extend_from_slice(&(ciphered as u32).to_le_bytes());
            }
        }
        out
    }

    const CAR: &str = "abarth500";

    fn engine_ini(limiter: &str) -> String {
        format!(
            "[HEADER]\nVERSION=2\n\n[ENGINE_DATA]\nALTITUDE_SENSITIVITY=0.1\n\
             INERTIA=0.120\nLIMITER={limiter}\nLIMITER_HZ=30\nMINIMUM=1250\n\
             ; a comment long enough to give the solver something to chew on,\n\
             ; because a key cannot be pinned down from a handful of bytes.\n\
             [COAST_REF]\nRPM=7000\nTORQUE=60\nNON_LINEARITY=0\n"
        )
    }

    fn write_car(base: &Path, folder: &str, acd: &[u8]) -> std::path::PathBuf {
        let car = base.join(folder);
        std::fs::create_dir_all(&car).expect("create car dir");
        std::fs::write(car.join("data.acd"), acd).expect("write data.acd");
        car
    }

    #[test]
    fn reads_the_idle_and_the_limiter_out_of_an_encrypted_container() {
        let base = crate::testutil::temp_dir("acd");
        let acd = synthetic_acd(&[("engine.ini", &engine_ini("7800"))], &key_for(CAR));
        let car = write_car(&base, CAR, &acd);

        let data = read_engine_data(&car).expect("the container must give up its numbers");
        assert_eq!(data.idle_rev, Some(1250.0), "MINIMUM is the idle");
        assert_eq!(data.limiter_rev, Some(7800.0), "LIMITER is where it stops");
    }

    /// **The reason the slow route is kept.** A folder renamed after packing
    /// still holds the old key, so the name-derived one opens nothing — and the
    /// ciphertext has to answer on its own.
    #[test]
    fn a_renamed_folder_is_still_read_by_the_fallback() {
        let base = crate::testutil::temp_dir("acd-renamed");
        // With a suspensions file alongside, as a real container has. The size
        // matters: recovering a key from ciphertext needs text to work on, and
        // a couple of hundred bytes leave too many positions undecided. Real
        // physics files run to thousands, which is the case being modelled.
        // Varied, not repeated: text with a period of its own leaves whole
        // columns of the key undecided, which real physics files never do.
        let mut filler = String::from("[HEADER]\nVERSION=4\n\n[BASIC]\nWHEELBASE=2.30\n");
        for step in 0..30 {
            filler.push_str(&format!(
                "\n[NODE_{step}]\nTYPE=dwb_{step}\nBASEY=-0.0{step}\nTRACK=1.{step}5\n\
                 ROD_LENGTH=0.{step}2\nHUB_MASS=3{step}\nPROGRESSIVE_K={step}00\n\
                 ; node {step} of the front geometry, described at some length\n"
            ));
        }
        let acd = synthetic_acd(
            &[("suspensions.ini", &filler), ("engine.ini", &engine_ini("6200"))],
            &key_for(CAR),
        );
        // Packed as `abarth500`, shipped inside `abarth500_widebody`.
        let car = write_car(&base, "abarth500_widebody", &acd);

        assert!(
            !decrypt(&entries(&acd)[1].1, &key_for("abarth500_widebody")).contains(ENGINE_SECTION),
            "the premise: the new name's key must not open this file"
        );
        let data = read_engine_data(&car).expect("the fallback must rescue it");
        assert_eq!(data.limiter_rev, Some(6200.0));
        assert_eq!(data.idle_rev, Some(1250.0));
    }

    #[test]
    fn a_missing_or_unreadable_container_is_not_an_error() {
        let base = crate::testutil::temp_dir("acd-missing");
        let car = base.join(CAR);
        std::fs::create_dir_all(&car).expect("create car dir");
        assert!(read_engine_data(&car).is_none(), "no data.acd at all");

        std::fs::write(car.join("data.acd"), b"not a container").expect("write junk");
        assert!(read_engine_data(&car).is_none(), "junk must not panic or lie");
    }

    /// A limiter written as zero means the car has none, and zero rpm is not a
    /// number anything downstream should try to use.
    #[test]
    fn a_zero_limiter_counts_as_absent() {
        let base = crate::testutil::temp_dir("acd-zero");
        let acd = synthetic_acd(&[("engine.ini", &engine_ini("0"))], &key_for(CAR));
        let car = write_car(&base, CAR, &acd);

        let data = read_engine_data(&car).expect("read");
        assert_eq!(data.limiter_rev, None, "zero is not a rev limit");
        assert_eq!(data.idle_rev, Some(1250.0), "and the idle is still there");
    }

    #[test]
    fn a_commented_out_value_is_not_read() {
        assert_eq!(ini_number("; LIMITER=9000\nLIMITER=7000\n", "LIMITER"), Some(7000.0));
        assert_eq!(ini_number("LIMITER=7000 ; the real one\n", "LIMITER"), Some(7000.0));
        assert_eq!(ini_number("[ENGINE_DATA]\nOTHER=1\n", "LIMITER"), None);
    }

    /// Truncation must come back empty rather than panicking — a container can
    /// be cut short by a bad download as easily as by anything else.
    #[test]
    fn truncation_at_any_offset_is_survivable() {
        let acd = synthetic_acd(&[("engine.ini", &engine_ini("7800"))], &key_for(CAR));
        for cut in 0..acd.len().min(400) {
            let _ = entries(&acd[..cut]);
        }
    }

    /// One folder, said out loud: which route opened it, and what it holds.
    ///
    /// The bench for "this car's idle looks wrong" — a library folder is named
    /// after the *version* (`.../vrc_erc_1999_renoir_csp/v1.3`), not after the
    /// car, so the fast route cannot work there and only the recovery one can:
    ///
    /// ```text
    /// PITBOX_CAR_DIR="D:\AC-Library\cars\<mod>\<version>"     ///   cargo test --lib acd -- --ignored --nocapture one_car_folder
    /// ```
    #[test]
    #[ignore = "needs a real car folder; measurement, not a check"]
    fn one_car_folder_reports_what_it_found() {
        let Ok(dir) = std::env::var("PITBOX_CAR_DIR") else {
            eprintln!("PITBOX_CAR_DIR unset, skipping");
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        let name = dir.file_name().unwrap_or_default().to_string_lossy().into_owned();
        eprintln!(
            "
=== {} ===",
            dir.display()
        );
        eprintln!("  folder name used as the car id: {name:?}");

        let bytes = std::fs::read(dir.join("data.acd")).expect("read data.acd");
        let mut entries = entries(&bytes);
        eprintln!("  {} entries in the container", entries.len());
        let (_, ciphered) = entries
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case("engine.ini"))
            .cloned()
            .expect("engine.ini in the container");

        let fast = decrypt(&ciphered, &key_for(&name)).contains(ENGINE_SECTION);
        eprintln!("  opened by the folder name:      {fast}");

        entries.sort_by_key(|(_, data)| std::cmp::Reverse(data.len()));
        let works = |key: &[u8]| decrypt(&ciphered, key).contains(ENGINE_SECTION);
        let recovered = entries
            .iter()
            .take(LONGEST_TRIED)
            .find_map(|(_, data)| recover_key(data, &works));
        eprintln!("  recovered from the ciphertext:  {}", recovered.is_some());

        match read_engine_data(&dir) {
            Some(data) => eprintln!("  idle {:?}  limiter {:?}", data.idle_rev, data.limiter_rev),
            None => eprintln!("  nothing read — the interface falls back on an estimate"),
        }

        // The section itself, because "idle None" says nothing about whether
        // the number is absent or merely spelled otherwise.
        if let Some(key) = recovered {
            let text = decrypt(&ciphered, &key);
            eprintln!("  --- engine.ini, first 40 lines ---");
            for line in text.lines().take(40) {
                eprintln!("  | {line}");
            }
        }
    }

    /// The whole corpus, against the real files. Ignored like every test that
    /// needs the game installed:
    ///
    /// ```text
    /// PITBOX_AC_ROOT="D:\...\assettocorsa" cargo test --lib acd -- --ignored --nocapture
    /// ```
    /// How `MINIMUM` is actually written across a whole corpus, sign included.
    ///
    /// Exists because one car declares `MINIMUM=-2500` and idles, by ear, at
    /// around 2500 — a single sample proves nothing, so this counts them:
    ///
    /// ```text
    /// PITBOX_CARS_ROOT="D:\AC-Library\cars"     ///   cargo test --lib acd -- --ignored --nocapture how_minimum_is_written
    /// ```
    #[test]
    #[ignore = "needs a real corpus of cars; measurement, not a check"]
    fn how_minimum_is_written_across_a_corpus() {
        let Ok(root) = std::env::var("PITBOX_CARS_ROOT") else {
            eprintln!("PITBOX_CARS_ROOT unset, skipping");
            return;
        };
        // A library nests one level deeper than `content/cars` does
        // (`<mod>/<version>/data.acd`), so look at both depths.
        let mut folders = Vec::new();
        for entry in std::fs::read_dir(&root).expect("read the corpus root").flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if path.join("data.acd").is_file() {
                folders.push(path);
                continue;
            }
            for nested in std::fs::read_dir(&path).into_iter().flatten().flatten() {
                let nested = nested.path();
                if nested.join("data.acd").is_file() {
                    folders.push(nested);
                }
            }
        }

        let (mut opened, mut positive, mut absent) = (0, 0, 0);
        let mut negative: Vec<(String, f32, Option<f32>)> = Vec::new();
        for dir in &folders {
            let Ok(bytes) = std::fs::read(dir.join("data.acd")) else {
                continue;
            };
            let mut entries = entries(&bytes);
            let Some((_, ciphered)) = entries
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case("engine.ini"))
                .cloned()
            else {
                continue;
            };
            let name = dir.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let by_name = decrypt(&ciphered, &key_for(&name));
            let text = if by_name.contains(ENGINE_SECTION) {
                Some(by_name)
            } else {
                entries.sort_by_key(|(_, data)| std::cmp::Reverse(data.len()));
                let works = |key: &[u8]| decrypt(&ciphered, key).contains(ENGINE_SECTION);
                entries
                    .iter()
                    .take(LONGEST_TRIED)
                    .find_map(|(_, data)| recover_key(data, &works))
                    .map(|key| decrypt(&ciphered, &key))
            };
            let Some(text) = text else { continue };
            opened += 1;
            match ini_number(&text, "MINIMUM") {
                Some(v) if v > 0.0 => positive += 1,
                Some(v) if v < 0.0 => {
                    let label = dir.strip_prefix(&root).unwrap_or(dir).to_string_lossy().into_owned();
                    negative.push((label, v, ini_number(&text, "LIMITER")));
                }
                _ => absent += 1,
            }
        }

        eprintln!(
            "
=== MINIMUM across {} cars ({opened} opened) ===",
            folders.len()
        );
        eprintln!("  positive  {positive}");
        eprintln!("  negative  {}", negative.len());
        eprintln!("  absent/0  {absent}");
        for (name, value, limiter) in &negative {
            eprintln!("    {value:>8.0}  (limiter {:>6.0})  {name}", limiter.unwrap_or(0.0));
        }
    }

    #[test]
    #[ignore = "needs a real Assetto Corsa install; measurement, not a check"]
    fn every_installed_car_gives_up_its_engine_data() {
        let Ok(ac_root) = std::env::var("PITBOX_AC_ROOT") else {
            eprintln!("PITBOX_AC_ROOT unset, skipping");
            return;
        };
        let cars = std::path::PathBuf::from(ac_root).join("content").join("cars");
        let started = std::time::Instant::now();
        let (mut total, mut read, mut with_idle, mut with_limiter, mut by_name) = (0, 0, 0, 0, 0);
        let mut failures = Vec::new();

        for entry in std::fs::read_dir(&cars).expect("read content/cars").flatten() {
            let car = entry.path();
            if !car.join("data.acd").is_file() {
                continue;
            }
            total += 1;
            let name = car.file_name().unwrap().to_string_lossy().into_owned();
            // How often the fast route alone was enough.
            if let Ok(bytes) = std::fs::read(car.join("data.acd")) {
                if let Some((_, ciphered)) = entries(&bytes)
                    .into_iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case("engine.ini"))
                {
                    if decrypt(&ciphered, &key_for(&name)).contains(ENGINE_SECTION) {
                        by_name += 1;
                    }
                }
            }
            match read_engine_data(&car) {
                Some(data) => {
                    read += 1;
                    with_idle += usize::from(data.idle_rev.is_some());
                    with_limiter += usize::from(data.limiter_rev.is_some());
                }
                None => failures.push(name),
            }
        }

        eprintln!(
            "\n=== data.acd: {read} of {total} cars, in {:.2?} ===",
            started.elapsed()
        );
        eprintln!("  opened by the folder name  {by_name}");
        eprintln!("  idle read                  {with_idle}");
        eprintln!("  limiter read               {with_limiter}");
        if !failures.is_empty() {
            eprintln!("  failures: {failures:?}");
        }
        assert_eq!(read, total, "every car must give up its engine data");
    }
}
