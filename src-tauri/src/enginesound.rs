//! Auditioning a car's engine sound **without launching the game**.
//!
//! The "Engine sound" block of the car sheet lists the original sound and the
//! installed sound mods. Comparing two of them used to mean activating one,
//! starting a session, listening, coming back and doing it again. This module
//! reads the `.bank` straight from disk and hands back a short idle loop.
//!
//! **Auditioning never deploys anything.** It reads; it does not touch
//! `content/`. That separation is the whole point — the same list already has a
//! radio button that *does* replace the game's `sfx/` folder, and confusing the
//! two would install mods on a click meant to preview them.
//!
//! Format facts and how they were established: `docs/fsb5-format.md`.

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Serialize;

use crate::config::AppConfig;
use crate::fsb5::{self, Bank, Codec, Sample};

/// Seconds of audio handed to the webview. An idle loop is steady, so a couple
/// of seconds says everything a longer clip would; and the whole thing travels
/// base64-encoded inside the command's result, so length is bytes on the wire.
const CLIP_SECONDS: f32 = 2.5;

/// Seconds decoded per candidate while looking for the idle.
///
/// 0,4 s was not enough and it showed: on one real bank the fundamental came
/// out at 61 Hz against 79 Hz measured over a longer window, and that wrong low
/// reading won the "lowest fundamental" contest. Measured over 91 banks, 1,2 s
/// also happens to be *faster* than 0,4 s was, because the minimum duration
/// below skips more candidates outright.
const PROBE_SECONDS: f32 = 1.2;

/// A candidate shorter than this is not an engine layer. Doors, gear shifts and
/// backfires live below it.
const MIN_CANDIDATE_SECONDS: f32 = 1.0;

/// Decimation applied before autocorrelation. Fundamentals top out at 600 Hz,
/// so an eighth of 48 kHz leaves a wide margin and divides the work by 64.
const PROBE_DECIMATION: usize = 8;

/// Below this, a sample is noise rather than an engine: a wind or skid loop
/// scores around 0,10, a real engine 0,53 to 0,84 (measured against the PCM16
/// bank of the same car — see `docs/fsb5-format.md`).
const PERIODIC_ENOUGH: f32 = 0.5;

/// How steady a candidate's level has to be over time.
///
/// An idle loop is flat; a starter, a rev or a one-shot swells and dies. On the
/// CSP variant of one real car the app was playing the **ignition sound** on a
/// loop — it scored 0,30 here where every genuine engine layer scores under
/// 0,15.
const MAX_STATIONARITY: f32 = 0.15;

/// Fraction of the autocorrelation peak that identifies the **fundamental**
/// rather than one of its multiples.
///
/// This one number decides whether the whole thing works, and it took a
/// calibration to find. The global maximum of the autocorrelation of an engine
/// loop falls on the **loop period**, not on the firing period: a two-second
/// overrun loop peaks at 20 Hz whatever its engine speed. Ranking by "lowest
/// fundamental" then ranks by "longest loop", which is how a 4000 rpm layer
/// came out below an idle.
///
/// Calibrated against the engine speed Kunos writes into its sample names —
/// for a four-stroke, `f0 × 60 / rpm` must equal half the cylinder count, so it
/// must be **constant** across one car's samples. On the BMW 1M (inline six,
/// expected 3,00):
///
/// | règle | rapport mesuré | dispersion |
/// | --- | --- | --- |
/// | maximum global | 0,56 | 25 % |
/// | plus petit retard ≥ 0,5 × pic | 2,58 | 38 % |
/// | **plus petit retard ≥ 0,3 × pic** | **3,23** | **10 %** |
const FUNDAMENTAL_RATIO: f32 = 0.3;

/// A sample sitting at the rails is either clipped at the source or wrongly
/// decoded; either way it is a poor thing to audition.
const MAX_CLIPPED: f32 = 0.02;

/// Plausible firing frequencies, in hertz. A V12 at 8000 rpm fires at 800 Hz,
/// but nothing that high can pass for an idle; below 20 Hz we are measuring a
/// loop, not an engine.
const F0_RANGE: (f32, f32) = (20.0, 600.0);

/// Base64, written here rather than pulled in as a dependency: it is twenty
/// lines, it has no edge cases at this size, and the crate would exist in the
/// tree for this one call.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let packed = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(ALPHABET[(packed >> 18) as usize & 63] as char);
        out.push(ALPHABET[(packed >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(packed >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[packed as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// What the frontend needs to play one clip.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineClip {
    /// A complete WAV file, base64. Small enough to travel in the result rather
    /// than through a custom protocol like the 3D preview's `.glb` does.
    pub wav: String,
    pub frequency: u32,
    pub seconds: f32,
    /// Diagnostics, never shown as is: which sample of which codec was picked,
    /// and how. A user reporting "it played the horn" needs this in the log.
    pub codec: String,
    pub sample_index: usize,
    pub sample_name: Option<String>,
    pub picked_by: &'static str,
}

/// What one bank holds, read on demand — the part of a sound mod's sheet that
/// no other tool shows, because it means decoding the container.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BankFacts {
    pub file_name: String,
    pub codec: String,
    pub sample_count: usize,
    /// Highest sample rate found — banks are homogeneous in practice.
    pub frequency: u32,
    /// Total playing time of every sample, seconds.
    pub seconds: f32,
    /// Whether the bank kept its sample name table. Sound mods routinely strip
    /// it, and its absence is why the idle has to be found by measurement.
    pub named: bool,
    pub size_bytes: u64,
}

/// Everything the sound mod's sheet displays.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundDetail {
    pub id: String,
    pub name: String,
    pub parent_id: String,
    /// Nom lisible de la voiture cible, quand elle est connue de la base.
    pub parent_name: Option<String>,
    pub author: Option<String>,
    pub source_archive: Option<String>,
    pub imported_at: String,
    pub is_active: bool,
    pub removable: bool,
    pub size_bytes: u64,
    /// `None` quand le bank est illisible : la fiche reste utile, elle dit
    /// simplement qu'elle n'a pas pu l'ouvrir.
    pub bank: Option<BankFacts>,
}

/// Lit la fiche d'un mod de son.
pub fn detail(conn: &Connection, cfg: &AppConfig, sub_id: &str) -> Result<SoundDetail, String> {
    let sub = crate::overlay::get_sub_mod(conn, sub_id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::SOUND_NOT_FOUND)?;
    if sub.sub_type != "SOUND" {
        return Err(crate::errors::NOT_A_SOUND_MOD.into());
    }
    let dir = crate::libpath::resolve(cfg.library_path.as_deref(), &sub.library_path)
        .ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    let parent_name = crate::overlay::get_mod(conn, &sub.parent_id)
        .ok()
        .flatten()
        .and_then(|m| m.display_name);

    Ok(SoundDetail {
        id: sub.id,
        name: sub.name,
        parent_id: sub.parent_id,
        parent_name,
        author: sub.author,
        source_archive: sub.source_archive,
        imported_at: sub.imported_at,
        is_active: sub.is_active,
        removable: sub.removable,
        size_bytes: crate::inspect::dir_size_bytes(&dir),
        bank: bank_facts(&dir),
    })
}

/// Ouvre le bank et résume ce qu'il contient. Best-effort : un bank illisible
/// ne doit pas empêcher la fiche de s'afficher, il doit se voir comme tel.
fn bank_facts(dir: &Path) -> Option<BankFacts> {
    let path = find_bank(dir)?;
    let size_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let bytes = std::fs::read(&path).ok()?;
    let bank = match fsb5::parse(&bytes) {
        Ok(b) => b,
        Err(e) => {
            log::warn!("bank illisible ({}): {e}", path.display());
            return None;
        }
    };
    Some(BankFacts {
        file_name: path.file_name()?.to_string_lossy().into_owned(),
        codec: bank.codec.label(),
        sample_count: bank.samples.len(),
        frequency: bank.samples.iter().map(|s| s.frequency).max().unwrap_or(0),
        seconds: bank.samples.iter().map(|s| s.seconds()).sum(),
        named: bank.samples.iter().any(|s| s.name.is_some()),
        size_bytes,
    })
}

/// Dossier des ressources d'un mod de son — le même que celui où l'import
/// range ses annexes.
pub fn resources_dir(conn: &Connection, cfg: &AppConfig, sub_id: &str) -> Result<PathBuf, String> {
    let sub = crate::overlay::get_sub_mod(conn, sub_id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::SOUND_NOT_FOUND)?;
    let library = cfg.library_path.as_ref().ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    Ok(crate::resources::resources_dir_for(
        library,
        "sounds",
        &[&sub.parent_id, &sub.name],
    ))
}

/// Finds the `.bank` inside a sound folder. AC puts exactly one there, next to
/// `GUIDs.txt`; the largest wins if a pack ever ships more.
fn find_bank(dir: &Path) -> Option<PathBuf> {
    let mut best: Option<(u64, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("bank"))
            != Some(true)
        {
            continue;
        }
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        if best.as_ref().is_none_or(|(s, _)| size > *s) {
            best = Some((size, path));
        }
    }
    best.map(|(_, p)| p)
}

/// Where a given entry of the "Engine sound" list lives on disk.
///
/// `None` means the original, and the original is **not** always the car's
/// current `sfx/`: once a mod has been activated, that folder holds the mod and
/// the true original sits in the backup `activate_sound` made. Reading the live
/// folder would silently audition the active mod under the name "Origine".
fn sound_dir(conn: &Connection, cfg: &AppConfig, parent_id: &str, sub_id: Option<&str>) -> Result<PathBuf, String> {
    match sub_id {
        Some(id) => {
            let sub = crate::overlay::get_sub_mod(conn, id)
                .map_err(|e| e.to_string())?
                .ok_or(crate::errors::SOUND_NOT_FOUND)?;
            if sub.sub_type != "SOUND" {
                return Err(crate::errors::NOT_A_SOUND_MOD.into());
            }
            crate::libpath::resolve(cfg.library_path.as_deref(), &sub.library_path)
                .ok_or_else(|| crate::errors::LIBRARY_NOT_CONFIGURED.into())
        }
        None => {
            let backup = crate::submods::sound_backup_dir(cfg, parent_id)?;
            if backup.is_dir() {
                return Ok(backup);
            }
            crate::submods::parent_subdir(conn, cfg, parent_id, "sfx")
                .ok_or_else(|| crate::errors::TARGET_CAR_UNKNOWN.into())
        }
    }
}

/// Everything the native FMOD path needs, resolved **without loading a DLL**.
///
/// Kept here rather than in `fmod/` because it is the same directory lookup the
/// WAV path already does (`sound_dir`): whichever route ends up playing, "which
/// bank is this entry?" has exactly one answer.
pub struct NativeTarget {
    pub ac_root: PathBuf,
    pub bank: PathBuf,
    pub guid: crate::fmod::guids::Guid,
    pub event_path: String,
}

/// Resolves a list entry to a playable FMOD event.
///
/// Every failure here is a **fallback trigger**, not something to show: no game
/// configured, no bank, no engine event in the table. The caller drops back to
/// the in-house decoder (§4.1) and the message only ever reaches the log, which
/// is why these strings stay raw diagnostics rather than becoming i18n keys.
pub fn native_target(
    conn: &Connection,
    cfg: &AppConfig,
    parent_id: &str,
    sub_id: Option<&str>,
    view: crate::fmod::guids::EngineView,
) -> Result<NativeTarget, String> {
    let ac_root = cfg
        .ac_install_path
        .clone()
        .ok_or_else(|| "no Assetto Corsa install configured".to_string())?;
    let dir = sound_dir(conn, cfg, parent_id, sub_id)?;
    let bank = find_bank(&dir).ok_or_else(|| format!("no .bank in {}", dir.display()))?;
    let (event_path, guid) = crate::fmod::guids::resolve_engine_event(&dir, Some(&ac_root), parent_id, view)
        .ok_or_else(|| {
            format!(
                "no engine event for {parent_id} in any GUIDs.txt near {}",
                dir.display()
            )
        })?;
    Ok(NativeTarget {
        ac_root,
        bank,
        guid,
        event_path,
    })
}

/// How periodic a signal is, and at what lag — the measure that tells an engine
/// from wind without knowing anything about either.
///
/// Returns the normalised autocorrelation peak and the fundamental it implies.
fn periodicity(pcm: &[i16], frequency: u32) -> (f32, f32) {
    if pcm.len() < 4 * PROBE_DECIMATION {
        return (0.0, 0.0);
    }
    // Decimate by averaging: cheaper than filtering, and the fundamentals we
    // rank are far below the new Nyquist anyway.
    let decimated: Vec<f32> = pcm
        .chunks(PROBE_DECIMATION)
        .map(|c| c.iter().map(|&v| v as f32).sum::<f32>() / c.len() as f32)
        .collect();
    let rate = frequency as f32 / PROBE_DECIMATION as f32;
    let mean = decimated.iter().sum::<f32>() / decimated.len() as f32;
    let centred: Vec<f32> = decimated.iter().map(|v| v - mean).collect();
    let energy: f32 = centred.iter().map(|v| v * v).sum();
    if energy <= f32::EPSILON {
        return (0.0, 0.0);
    }

    let min_lag = (rate / F0_RANGE.1).floor().max(2.0) as usize;
    let max_lag = ((rate / F0_RANGE.0).ceil() as usize).min(centred.len() / 2);
    if max_lag <= min_lag {
        return (0.0, 0.0);
    }
    let mut curve = vec![0.0f32; max_lag];
    let mut best = 0.0f32;
    for lag in min_lag..max_lag {
        let mut acc = 0.0f32;
        for i in 0..centred.len() - lag {
            acc += centred[i] * centred[i + lag];
        }
        curve[lag] = acc / energy;
        if curve[lag] > best {
            best = curve[lag];
        }
    }
    // **Le plus petit** retard qui approche le pic, pas le pic lui-même : le
    // maximum global tombe sur la période de la boucle, et tous ses multiples
    // sont presque aussi forts. Voir `FUNDAMENTAL_RATIO`.
    //
    // Et on ne cherche qu'**après la première descente sous zéro** : juste après
    // le retard nul l'autocorrélation est encore élevée, et un signal très pur y
    // franchirait n'importe quel seuil bien avant sa vraie période. Sans cette
    // garde, un son pur de 60 Hz est mesuré à 600.
    let from = (min_lag..max_lag).find(|&l| curve[l] <= 0.0).unwrap_or(min_lag);
    // Et c'est le premier **maximum local** au-delà du seuil, pas le premier
    // franchissement : le seuil est franchi avant le sommet, ce qui surestime la
    // fréquence — un son pur de 60 Hz ressortait à 75.
    let threshold = best * FUNDAMENTAL_RATIO;
    let lag = ((from + 1)..max_lag.saturating_sub(1))
        .find(|&l| curve[l] >= threshold && curve[l] >= curve[l - 1] && curve[l] >= curve[l + 1])
        .unwrap_or(0);
    let f0 = if lag == 0 { 0.0 } else { rate / lag as f32 };
    (best, f0)
}

/// How steady the level is over time — see [`MAX_STATIONARITY`].
///
/// The standard deviation of the level across 50 ms windows, over its mean. A
/// loop that holds its level lands near zero; anything that starts, swells or
/// dies away lands well above.
fn stationarity(pcm: &[i16], frequency: u32) -> f32 {
    let window = ((frequency as f32 * 0.05) as usize).max(1);
    let levels: Vec<f32> = pcm
        .chunks_exact(window)
        .map(|w| (w.iter().map(|&v| (v as f32) * (v as f32)).sum::<f32>() / window as f32).sqrt())
        .collect();
    if levels.len() < 4 {
        return 1.0;
    }
    let mean = levels.iter().sum::<f32>() / levels.len() as f32;
    if mean < 1.0 {
        return 1.0;
    }
    let variance = levels.iter().map(|l| (l - mean) * (l - mean)).sum::<f32>() / levels.len() as f32;
    variance.sqrt() / mean
}

fn clipped_fraction(pcm: &[i16]) -> f32 {
    if pcm.is_empty() {
        return 1.0;
    }
    let n = pcm.iter().filter(|&&v| v >= i16::MAX - 8 || v <= i16::MIN + 8).count();
    n as f32 / pcm.len() as f32
}

/// The engine speed a Kunos sample name carries, if any.
///
/// Kunos writes it in clear: `idle_1383`, `mk1_idle_1655a`, `5167b_off`. The
/// trailing digit run is the RPM.
fn rpm_in_name(name: &str) -> Option<u32> {
    let digits: String = name
        .chars()
        .rev()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let value: u32 = digits.chars().rev().collect::<String>().parse().ok()?;
    (300..=20000).contains(&value).then_some(value)
}

/// Picks the sample that best passes for an idle.
///
/// Two strategies, because half the corpus has no name table at all. Kunos
/// banks name their samples and write the engine speed into the name; sound
/// **mods** routinely strip the whole table, and their `GUIDs.txt` only lists
/// *event* names (`engine_int`), which say nothing about samples.
///
/// So when names are missing, the sound itself is measured: decode a fraction
/// of a second of every candidate, keep the ones that are strongly periodic and
/// unclipped — that alone throws out doors, horns, wind and skids — and take the
/// lowest fundamental among them. The idle is the slowest engine layer.
fn pick_idle(bytes: &[u8], bank: &Bank) -> Option<(Sample, &'static str)> {
    // By name first: it is exact when it is there, and free.
    let named: Vec<&Sample> = bank.samples.iter().filter(|s| s.name.is_some()).collect();
    if !named.is_empty() {
        let mut idles: Vec<&&Sample> = named
            .iter()
            .filter(|s| {
                let name = s.name.as_deref().unwrap_or("");
                name.to_ascii_lowercase().contains("idle") && !name.to_ascii_lowercase().contains("_off")
            })
            .collect();
        if !idles.is_empty() {
            // Several idle layers: the slowest one, by the speed in its name.
            idles.sort_by_key(|s| rpm_in_name(s.name.as_deref().unwrap_or("")).unwrap_or(u32::MAX));
            return Some(((**idles[0]).clone(), "name"));
        }
    }

    // Otherwise, measure. `_off` layers are overrun, not idle.
    let mut best: Option<(f32, Sample)> = None;
    for sample in &bank.samples {
        if sample.seconds() < MIN_CANDIDATE_SECONDS || sample.frequency == 0 {
            continue;
        }
        if sample
            .name
            .as_deref()
            .is_some_and(|n| n.to_ascii_lowercase().contains("_off"))
        {
            continue;
        }
        let probe = (sample.frequency as f32 * PROBE_SECONDS) as usize;
        let Ok(pcm) = fsb5::decode_with_bytes(bytes, bank, sample, 0, probe) else {
            continue;
        };
        if clipped_fraction(&pcm) > MAX_CLIPPED || stationarity(&pcm, sample.frequency) > MAX_STATIONARITY {
            continue;
        }
        let (peak, f0) = periodicity(&pcm, sample.frequency);
        if peak < PERIODIC_ENOUGH || f0 < F0_RANGE.0 || f0 > F0_RANGE.1 {
            continue;
        }
        if best.as_ref().is_none_or(|(low, _)| f0 < *low) {
            best = Some((f0, sample.clone()));
        }
    }
    best.map(|(_, s)| (s, "pitch"))
}

/// Reads one entry of the "Engine sound" list and returns a short idle loop.
pub fn audition(
    conn: &Connection,
    cfg: &AppConfig,
    parent_id: &str,
    sub_id: Option<&str>,
) -> Result<EngineClip, String> {
    let dir = sound_dir(conn, cfg, parent_id, sub_id)?;
    let path = find_bank(&dir).ok_or(crate::errors::SOUND_BANK_MISSING)?;
    let bytes = std::fs::read(&path).map_err(|e| format!("lecture de {}: {e}", path.display()))?;
    let bank = fsb5::parse(&bytes)?;

    if matches!(bank.codec, Codec::Unsupported(_)) {
        log::warn!(
            "aperçu sonore impossible pour {}: codec {} non décodé",
            path.display(),
            bank.codec.label()
        );
        return Err(crate::errors::SOUND_CODEC_UNSUPPORTED.into());
    }

    let (sample, picked_by) = pick_idle(&bytes, &bank).ok_or(crate::errors::SOUND_NO_ENGINE_SAMPLE)?;
    let wanted = (sample.frequency as f32 * CLIP_SECONDS) as usize;
    let pcm = fsb5::decode_with_bytes(&bytes, &bank, &sample, 0, wanted)?;
    if pcm.is_empty() {
        return Err(crate::errors::SOUND_NO_ENGINE_SAMPLE.into());
    }

    let seconds = pcm.len() as f32 / sample.frequency as f32;
    let wav = fsb5::to_wav(&pcm, sample.frequency);
    Ok(EngineClip {
        wav: base64(&wav),
        frequency: sample.frequency,
        seconds,
        codec: bank.codec.label(),
        sample_index: sample.index,
        sample_name: sample.name.clone(),
        picked_by,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les trois cas de bourrage, qui sont tout ce que base64 a de piégeux.
    #[test]
    fn base64_pads_like_everyone_else() {
        assert_eq!(base64(b""), "", "rien");
        assert_eq!(base64(b"f"), "Zg==", "un octet, deux signes égal");
        assert_eq!(base64(b"fo"), "Zm8=", "deux octets, un signe égal");
        assert_eq!(base64(b"foo"), "Zm9v", "trois octets, aucun");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy", "deux groupes pleins");
        assert_eq!(
            base64(&[0xff, 0xfe, 0xfd]),
            "//79",
            "les deux derniers caractères de l'alphabet"
        );
    }

    #[test]
    fn rpm_is_read_from_the_trailing_digits() {
        assert_eq!(rpm_in_name("idle_1383"), Some(1383), "le suffixe numérique");
        assert_eq!(
            rpm_in_name("mk1_idle_1655a"),
            Some(1655),
            "une lettre après les chiffres"
        );
        assert_eq!(rpm_in_name("5167b_off"), Some(5167), "des chiffres au milieu");
        assert_eq!(rpm_in_name("horn"), None, "aucun chiffre");
        assert_eq!(rpm_in_name("flutter_4"), None, "trop bas pour être un régime");
    }

    /// A pure tone is perfectly periodic; white noise is not. That contrast is
    /// what separates an engine loop from a door slam when no name is available.
    #[test]
    fn periodicity_tells_a_tone_from_noise() {
        let rate = 48000u32;
        let tone: Vec<i16> = (0..24000)
            .map(|i| ((i as f32 * std::f32::consts::TAU * 60.0 / rate as f32).sin() * 8000.0) as i16)
            .collect();
        let (peak, f0) = periodicity(&tone, rate);
        assert!(peak > 0.8, "un son pur est franchement périodique (mesuré {peak})");
        assert!(
            (f0 - 60.0).abs() < 4.0,
            "la fondamentale est retrouvée (mesuré {f0} Hz)"
        );

        // Bruit déterministe : un générateur congruentiel, pour que le test ne
        // dépende pas du hasard.
        let mut state = 12345u32;
        let noise: Vec<i16> = (0..24000)
            .map(|_| {
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                ((state >> 16) as i16).wrapping_sub(16384) / 4
            })
            .collect();
        let (peak, _) = periodicity(&noise, rate);
        assert!(peak < 0.3, "du bruit ne l'est pas (mesuré {peak})");
    }

    /// **Le piège qui rendait tout le choix faux.** Une couche moteur est une
    /// boucle : son autocorrélation culmine sur la période de la **boucle**, pas
    /// sur celle de l'allumage. Ranger par « fondamentale la plus basse »
    /// revenait donc à ranger par « boucle la plus longue », et un lâcher de gaz
    /// à 4000 tr/min sortait sous un ralenti.
    #[test]
    fn periodicity_finds_the_firing_period_not_the_loop_period() {
        let rate = 48000u32;
        let tone = 150.0f32; // la « période d'allumage »
        let loop_hz = 25.0f32; // la boucle : six cycles
        let pcm: Vec<i16> = (0..48000)
            .map(|i| {
                let t = i as f32 / rate as f32;
                // Une enveloppe qui se répète à 25 Hz par-dessus un son à 150 Hz :
                // le signal se répète exactement toutes les 1/25 s.
                let envelope = 0.5 + 0.5 * (t * std::f32::consts::TAU * loop_hz).sin().abs();
                ((t * std::f32::consts::TAU * tone).sin() * envelope * 9000.0) as i16
            })
            .collect();
        let (peak, f0) = periodicity(&pcm, rate);
        assert!(peak > 0.5, "le signal est franchement périodique (mesuré {peak})");
        assert!(
            (f0 - tone).abs() < tone * 0.15,
            "c'est la période d'allumage qui est rendue, pas celle de la boucle (mesuré {f0} Hz pour {tone} attendus)"
        );
    }

    /// Un ralenti tient son niveau ; un démarreur enfle et meurt. C'est ce qui
    /// distinguait les deux sur la variante CSP d'une vraie voiture, où l'app
    /// jouait le son de mise en route en boucle.
    #[test]
    fn stationarity_separates_a_steady_loop_from_a_swell() {
        let rate = 48000u32;
        let steady: Vec<i16> = (0..48000)
            .map(|i| ((i as f32 / rate as f32 * std::f32::consts::TAU * 80.0).sin() * 8000.0) as i16)
            .collect();
        let swell: Vec<i16> = (0..48000)
            .map(|i| {
                let t = i as f32 / 48000.0;
                ((t * std::f32::consts::TAU * 80.0).sin() * 8000.0 * t) as i16
            })
            .collect();
        let flat = stationarity(&steady, rate);
        let rising = stationarity(&swell, rate);
        assert!(flat < MAX_STATIONARITY, "une boucle stable passe (mesuré {flat})");
        assert!(
            rising > MAX_STATIONARITY,
            "un son qui enfle est écarté (mesuré {rising})"
        );
    }

    #[test]
    fn clipping_is_counted_at_both_rails() {
        assert_eq!(clipped_fraction(&[0, 0, 0, 0]), 0.0, "rien en butée");
        assert_eq!(
            clipped_fraction(&[i16::MAX, 0, i16::MIN, 0]),
            0.5,
            "les deux butées comptent"
        );
        assert_eq!(clipped_fraction(&[]), 1.0, "un signal vide est inutilisable");
    }
}
