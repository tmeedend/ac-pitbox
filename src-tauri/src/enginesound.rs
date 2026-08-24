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

/// Seconds decoded per candidate while looking for the idle. Enough for the
/// autocorrelation to lock onto a fundamental, short enough that scanning
/// eighty samples stays instant.
const PROBE_SECONDS: f32 = 0.4;

/// Decimation applied before autocorrelation. The fundamentals we are ranking
/// sit under 400 Hz, so a quarter of the sample rate is plenty, and it divides
/// the work by sixteen.
const PROBE_DECIMATION: usize = 4;

/// Below this, a sample is noise rather than an engine: a wind or skid loop
/// scores around 0,10, a real engine 0,53 to 0,84 (measured against the PCM16
/// bank of the same car — see `docs/fsb5-format.md`).
const PERIODIC_ENOUGH: f32 = 0.45;

/// A sample sitting at the rails is either clipped at the source or wrongly
/// decoded; either way it is a poor thing to audition.
const MAX_CLIPPED: f32 = 0.02;

/// Plausible engine fundamentals, in hertz. Below 20 Hz we are ranking noise;
/// above 400 Hz we are past anything that could pass for an idle.
const F0_RANGE: (f32, f32) = (20.0, 400.0);

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
    let mut best = 0.0f32;
    let mut best_lag = 0usize;
    for lag in min_lag..max_lag {
        let mut acc = 0.0f32;
        for i in 0..centred.len() - lag {
            acc += centred[i] * centred[i + lag];
        }
        let norm = acc / energy;
        if norm > best {
            best = norm;
            best_lag = lag;
        }
    }
    let f0 = if best_lag == 0 { 0.0 } else { rate / best_lag as f32 };
    (best, f0)
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
        if sample.seconds() < 0.5 || sample.frequency == 0 {
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
        if clipped_fraction(&pcm) > MAX_CLIPPED {
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
