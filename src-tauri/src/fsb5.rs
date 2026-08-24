//! Reading FMOD sound banks — the `.bank` files that carry Assetto Corsa's car
//! sounds (`content/cars/<id>/sfx/<id>.bank`).
//!
//! Exists so that a sound mod can be **auditioned without launching the game**.
//! Pure parsing and decoding: no I/O, no Tauri, no playback. The caller hands
//! in the bytes and gets PCM back.
//!
//! # What a `.bank` actually is
//!
//! A RIFF container whose payload holds an **FSB5** section: a sample table
//! plus one contiguous blob of audio data, all samples sharing a single codec.
//! Everything below was established by measurement on real banks; the method
//! for each fact is in `docs/fsb5-format.md`, and the two that cost the most
//! are repeated here because getting them wrong is silent.
//!
//! # Two facts that do not survive a guess
//!
//! **The sample header is a bit-packed `u64`, and its field boundaries are not
//! obvious.** The offset sits on 23 bits and is scaled by **32**; the sample
//! count starts at bit **34**. Both were settled against a hard constraint that
//! PCM16 provides for free — a PCM16 sample occupies exactly
//! `count * 2 * channels` bytes — over the 52 samples of a real bank. The
//! competing reading (27 bits scaled by 16) yields 51 negative lengths out of
//! 52, which is impossible.
//!
//! **FADPCM's residual step grows with the shift factor**, it does not shrink.
//! Spelled `nibble << (22 - factor)` after the nibble has been pushed to the
//! top of a 32-bit word, it reads like a right shift and behaves like a left
//! one. An exhaustive search that assumed the opposite direction never came
//! close: the answer was not in the space being searched.

use std::convert::TryFrom;

/// Codecs an FSB5 section can declare. Only the two Assetto Corsa actually
/// uses are decoded; the rest are recognised so the caller can say *why* it
/// cannot play a bank instead of failing blank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    Pcm16,
    Fadpcm,
    /// Anything else, kept as its raw value for the log line.
    Unsupported(u32),
}

impl Codec {
    fn from_raw(raw: u32) -> Self {
        match raw {
            2 => Codec::Pcm16,
            16 => Codec::Fadpcm,
            other => Codec::Unsupported(other),
        }
    }

    /// Name for logs and error messages — never shown to the user as is.
    pub fn label(self) -> String {
        match self {
            Codec::Pcm16 => "PCM16".into(),
            Codec::Fadpcm => "FADPCM".into(),
            Codec::Unsupported(2) => "PCM16".into(),
            Codec::Unsupported(15) => "VORBIS".into(),
            Codec::Unsupported(other) => format!("codec {other}"),
        }
    }
}

/// One sample of the bank. `data_offset` and `data_len` are relative to the
/// start of the data blob, not to the file.
#[derive(Debug, Clone)]
pub struct Sample {
    pub index: usize,
    /// Present only when the bank kept its name table. Sound **mods** routinely
    /// strip it, which is why picking a sample cannot rely on names alone.
    pub name: Option<String>,
    pub frequency: u32,
    pub channels: u16,
    pub sample_count: u32,
    pub data_offset: usize,
    pub data_len: usize,
    pub loop_range: Option<(u32, u32)>,
}

impl Sample {
    pub fn seconds(&self) -> f32 {
        if self.frequency == 0 {
            0.0
        } else {
            self.sample_count as f32 / self.frequency as f32
        }
    }
}

/// A parsed FSB5 section.
#[derive(Debug)]
pub struct Bank {
    pub codec: Codec,
    pub samples: Vec<Sample>,
    /// Offset of the data blob within the bytes handed to [`parse`].
    pub data_start: usize,
}

/// Sample rates, indexed by the 4-bit enum stored in the sample header.
const FREQUENCIES: [u32; 11] = [0, 8000, 11000, 11025, 16000, 22050, 24000, 32000, 44100, 48000, 96000];

/// Guard against a corrupt count sending us into a multi-gigabyte allocation.
/// A car bank holds tens of samples; a thousand is already absurd.
const MAX_SAMPLES: u32 = 4096;

fn read_u32(bytes: &[u8], at: usize) -> Result<u32, String> {
    bytes
        .get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| format!("truncated bank: no u32 at {at}"))
}

fn read_u64(bytes: &[u8], at: usize) -> Result<u64, String> {
    bytes
        .get(at..at + 8)
        .map(|b| u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
        .ok_or_else(|| format!("truncated bank: no u64 at {at}"))
}

/// Parses the FSB5 section of a `.bank` file.
///
/// The section is located by scanning for its magic rather than by walking the
/// RIFF chunk tree: the tree carries FMOD Studio's own metadata, which we have
/// no use for, and skipping it removes a whole format from the surface we have
/// to keep working.
pub fn parse(bytes: &[u8]) -> Result<Bank, String> {
    let base = bytes
        .windows(4)
        .position(|w| w == b"FSB5")
        .ok_or_else(|| "not an FMOD bank: no FSB5 section".to_string())?;

    let version = read_u32(bytes, base + 4)?;
    let count = read_u32(bytes, base + 8)?;
    let headers_size = read_u32(bytes, base + 12)? as usize;
    let names_size = read_u32(bytes, base + 16)? as usize;
    let data_size = read_u32(bytes, base + 20)? as usize;
    let codec = Codec::from_raw(read_u32(bytes, base + 24)?);

    if count > MAX_SAMPLES {
        return Err(format!("implausible sample count: {count}"));
    }
    // Version 0 carries one extra word before the sample table.
    let header_size = if version == 0 { 64 } else { 60 };
    let headers_start = base + header_size;
    let names_start = headers_start + headers_size;
    let data_start = names_start + names_size;

    let mut samples = Vec::with_capacity(count as usize);
    let mut at = headers_start;
    for index in 0..count as usize {
        let raw = read_u64(bytes, at)?;
        at += 8;

        let has_chunks = raw & 1 == 1;
        let freq_enum = ((raw >> 1) & 0x0f) as usize;
        let channels = u16::try_from((raw >> 5) & 0x03).unwrap_or(0) + 1;
        // 23 bits scaled by 32, and the count from bit 34 — see the module
        // header for how those two were settled.
        let data_offset = ((raw >> 7) & 0x7f_ffff) as usize * 32;
        let sample_count = ((raw >> 34) & 0x3fff_ffff) as u32;

        let mut sample = Sample {
            index,
            name: None,
            frequency: FREQUENCIES.get(freq_enum).copied().unwrap_or(44100),
            channels,
            sample_count,
            data_offset,
            data_len: 0,
            loop_range: None,
        };

        // A chain of optional chunks may override what the packed word said.
        let mut more = has_chunks;
        while more {
            let head = read_u32(bytes, at)?;
            at += 4;
            more = head & 1 == 1;
            let size = ((head >> 1) & 0xff_ffff) as usize;
            let kind = (head >> 25) & 0x7f;
            let body = bytes
                .get(at..at + size)
                .ok_or_else(|| format!("truncated chunk on sample {index}"))?;
            match kind {
                1 if size >= 1 => sample.channels = body[0] as u16,
                2 if size >= 4 => sample.frequency = read_u32(body, 0)?,
                3 if size >= 8 => sample.loop_range = Some((read_u32(body, 0)?, read_u32(body, 4)?)),
                _ => {}
            }
            at += size;
        }
        samples.push(sample);
    }

    // A sample's length is only knowable from where the next one starts: the
    // format stores it nowhere. The last one runs to the end of the blob.
    for i in 0..samples.len() {
        let next = samples.get(i + 1).map(|s| s.data_offset).unwrap_or(data_size);
        samples[i].data_len = next.saturating_sub(samples[i].data_offset);
    }

    if names_size > 0 {
        let table = bytes
            .get(names_start..names_start + names_size)
            .ok_or_else(|| "truncated name table".to_string())?;
        for (i, sample) in samples.iter_mut().enumerate() {
            let Ok(at) = read_u32(table, i * 4) else { break };
            let at = at as usize;
            if at >= table.len() {
                continue;
            }
            let end = table[at..]
                .iter()
                .position(|&b| b == 0)
                .map(|n| at + n)
                .unwrap_or(table.len());
            let name = String::from_utf8_lossy(&table[at..end]).into_owned();
            if !name.is_empty() {
                sample.name = Some(name);
            }
        }
    }

    Ok(Bank {
        codec,
        samples,
        data_start,
    })
}

// ---------------------------------------------------------------- FADPCM

/// FADPCM predictor coefficients. Only the first five are ever used — which is
/// itself a check on the parse: a misaligned coefficient field would spread the
/// observed indices over all sixteen values instead of over 0..=4.
const FADPCM_COEFS: [(i32, i32); 8] = [(0, 0), (60, 0), (122, 60), (115, 52), (98, 55), (0, 0), (0, 0), (0, 0)];

/// Bytes one FADPCM frame occupies, per channel.
const FADPCM_FRAME: usize = 0x8c;
/// Samples one FADPCM frame yields, per channel: `(0x8c - 0x0c) * 2`.
const FADPCM_FRAME_SAMPLES: usize = 256;

/// Decodes one channel of a FADPCM sample.
///
/// The frame is 140 bytes: eight 4-bit coefficient indices, eight 4-bit shift
/// factors, two `i16` of predictor history, then 128 bytes read as eight
/// sub-blocks of 32 nibbles. Channels are interleaved **by frame**, not by
/// sample.
///
/// The history in the header seeds the frame, which is what makes frames
/// independently decodable. It is *not* a continuity checkpoint — measured, it
/// does not match the previous frame's last two samples, so it cannot be used
/// to verify a decode.
pub fn decode_fadpcm(data: &[u8], sample: &Sample, channel: u16, max_samples: usize) -> Vec<i16> {
    let channels = sample.channels.max(1) as usize;
    let channel = (channel as usize).min(channels - 1);
    let frames = (max_samples.min(sample.sample_count as usize)).div_ceil(FADPCM_FRAME_SAMPLES);
    let mut out = Vec::with_capacity(frames * FADPCM_FRAME_SAMPLES);

    for frame in 0..frames {
        let at = (frame * channels + channel) * FADPCM_FRAME;
        let Some(f) = data.get(at..at + FADPCM_FRAME) else {
            break;
        };

        let coefs = u32::from_le_bytes([f[0], f[1], f[2], f[3]]);
        let shifts = u32::from_le_bytes([f[4], f[5], f[6], f[7]]);
        let mut h1 = i16::from_le_bytes([f[8], f[9]]) as i32;
        let mut h2 = i16::from_le_bytes([f[10], f[11]]) as i32;

        for block in 0..8 {
            let index = ((coefs >> (block * 4)) & 0x0f) as usize;
            let (c1, c2) = FADPCM_COEFS[index & 7];
            let shift = 22 - ((shifts >> (block * 4)) & 0x0f) as i32;
            for group in 0..4 {
                let g = 12 + block * 16 + group * 4;
                let word = i32::from_le_bytes([f[g], f[g + 1], f[g + 2], f[g + 3]]);
                for nibble in 0..8 {
                    // The `<< 28` carries the sign: the nibble is pushed to the
                    // top of the word so the arithmetic shift that follows
                    // extends it, with no separate conversion.
                    let residual = ((word >> (nibble * 4)) << 28) >> shift;
                    let value = (residual + c1 * h1 - c2 * h2) >> 6;
                    let value = value.clamp(i16::MIN as i32, i16::MAX as i32);
                    out.push(value as i16);
                    h2 = h1;
                    h1 = value;
                }
            }
        }
    }
    out.truncate(max_samples.min(sample.sample_count as usize));
    out
}

/// Decodes one channel of a PCM16 sample — a de-interleave, nothing more.
pub fn decode_pcm16(data: &[u8], sample: &Sample, channel: u16, max_samples: usize) -> Vec<i16> {
    let channels = sample.channels.max(1) as usize;
    let channel = (channel as usize).min(channels - 1);
    let wanted = max_samples.min(sample.sample_count as usize);
    let mut out = Vec::with_capacity(wanted);
    for i in 0..wanted {
        let at = (i * channels + channel) * 2;
        let Some(b) = data.get(at..at + 2) else { break };
        out.push(i16::from_le_bytes([b[0], b[1]]));
    }
    out
}

/// Decodes one channel, given the original bytes the bank was parsed from.
pub fn decode_with_bytes(
    bytes: &[u8],
    bank: &Bank,
    sample: &Sample,
    channel: u16,
    max_samples: usize,
) -> Result<Vec<i16>, String> {
    let from = bank.data_start + sample.data_offset;
    let to = (from + sample.data_len).min(bytes.len());
    let data = bytes
        .get(from..to)
        .ok_or_else(|| format!("sample {} runs past the end of the bank", sample.index))?;
    match bank.codec {
        Codec::Pcm16 => Ok(decode_pcm16(data, sample, channel, max_samples)),
        Codec::Fadpcm => Ok(decode_fadpcm(data, sample, channel, max_samples)),
        other => Err(format!("unsupported codec: {}", other.label())),
    }
}

// ------------------------------------------------------------------- WAV

/// Wraps mono PCM in a WAV header, so the webview can decode it with
/// `decodeAudioData` and loop it without any codec of its own.
pub fn to_wav(pcm: &[i16], frequency: u32) -> Vec<u8> {
    let bytes = pcm.len() * 2;
    let mut out = Vec::with_capacity(44 + bytes);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((36 + bytes) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&frequency.to_le_bytes());
    out.extend_from_slice(&(frequency * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(bytes as u32).to_le_bytes());
    for s in pcm {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a minimal FSB5 section so the container tests do not depend on a
    /// game install. One sample, PCM16, with a name.
    fn synth_bank(codec: u32, samples: &[(u32, u16, usize, u32, Option<&str>)], data: &[u8]) -> Vec<u8> {
        // Name table: one u32 offset per sample, then the zero-terminated names.
        let named = samples.iter().any(|s| s.4.is_some());
        let mut names = Vec::new();
        if named {
            let mut offsets = Vec::new();
            let mut body = Vec::new();
            let head = samples.len() * 4;
            for s in samples {
                offsets.push((head + body.len()) as u32);
                body.extend_from_slice(s.4.unwrap_or("").as_bytes());
                body.push(0);
            }
            for o in offsets {
                names.extend_from_slice(&o.to_le_bytes());
            }
            names.extend_from_slice(&body);
        }

        let mut headers = Vec::new();
        for (freq_enum, channels, offset, count, _) in samples {
            let raw: u64 = ((*freq_enum as u64 & 0x0f) << 1)
                | (((*channels as u64 - 1) & 0x03) << 5)
                | (((*offset as u64 / 32) & 0x7f_ffff) << 7)
                | ((*count as u64 & 0x3fff_ffff) << 34);
            headers.extend_from_slice(&raw.to_le_bytes());
        }

        let mut out = Vec::new();
        out.extend_from_slice(b"FSB5");
        out.extend_from_slice(&1u32.to_le_bytes()); // version
        out.extend_from_slice(&(samples.len() as u32).to_le_bytes());
        out.extend_from_slice(&(headers.len() as u32).to_le_bytes());
        out.extend_from_slice(&(names.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&codec.to_le_bytes());
        out.extend_from_slice(&[0u8; 32]); // zero + hash + dummy, up to 60 bytes
        assert_eq!(out.len(), 60, "en-tête FSB5 de 60 octets");
        out.extend_from_slice(&headers);
        out.extend_from_slice(&names);
        out.extend_from_slice(data);
        out
    }

    /// The bit layout of the packed sample header — the fact that cost the most
    /// to establish, and the one a refactor would silently break.
    #[test]
    fn sample_header_fields_round_trip() {
        let pcm: Vec<u8> = (0..512u32).flat_map(|i| (i as i16).to_le_bytes()).collect();
        let bytes = synth_bank(2, &[(9, 2, 0, 256, Some("idle_1383"))], &pcm);
        let bank = parse(&bytes).expect("bank parses");
        assert_eq!(bank.codec, Codec::Pcm16, "codec read from the header");
        assert_eq!(bank.samples.len(), 1, "one sample");
        let s = &bank.samples[0];
        assert_eq!(s.frequency, 48000, "frequency enum 9 is 48 kHz");
        assert_eq!(s.channels, 2, "channels stored minus one");
        assert_eq!(s.sample_count, 256, "count survives the round trip");
        assert_eq!(s.name.as_deref(), Some("idle_1383"), "name table read");
    }

    /// A PCM16 sample occupies exactly `count * 2 * channels` bytes. That
    /// identity is what settled the offset scale against a real bank, so it is
    /// worth holding a parser to it.
    #[test]
    fn pcm16_length_matches_count_times_channels() {
        let pcm: Vec<u8> = vec![0; 128 * 2 * 2];
        let bytes = synth_bank(2, &[(8, 2, 0, 128, None)], &pcm);
        let bank = parse(&bytes).expect("bank parses");
        let s = &bank.samples[0];
        assert_eq!(
            s.data_len,
            s.sample_count as usize * 2 * s.channels as usize,
            "longueur PCM16 = count x 2 x canaux"
        );
    }

    /// Builds one FADPCM frame with the given coefficient index, shift factor
    /// and nibbles, history at zero.
    fn synth_frame(coef_index: u32, shift_factor: u32, nibbles: &[u8; 256]) -> Vec<u8> {
        let mut coefs = 0u32;
        let mut shifts = 0u32;
        for block in 0..8 {
            coefs |= (coef_index & 0x0f) << (block * 4);
            shifts |= (shift_factor & 0x0f) << (block * 4);
        }
        let mut f = Vec::with_capacity(FADPCM_FRAME);
        f.extend_from_slice(&coefs.to_le_bytes());
        f.extend_from_slice(&shifts.to_le_bytes());
        f.extend_from_slice(&0i16.to_le_bytes()); // hist1
        f.extend_from_slice(&0i16.to_le_bytes()); // hist2
        for pair in nibbles.chunks(2) {
            f.push((pair[0] & 0x0f) | ((pair[1] & 0x0f) << 4));
        }
        assert_eq!(f.len(), FADPCM_FRAME, "trame de 140 octets");
        f
    }

    fn fake_sample(channels: u16, count: u32) -> Sample {
        Sample {
            index: 0,
            name: None,
            frequency: 48000,
            channels,
            sample_count: count,
            data_offset: 0,
            data_len: 0,
            loop_range: None,
        }
    }

    /// The residual, isolated: with coefficient index 0 the predictor
    /// contributes nothing, and with shift factor 0 the arithmetic collapses to
    /// "the nibble, signed". Anything else means the shift direction is wrong —
    /// the exact mistake that made an exhaustive search miss the answer.
    #[test]
    fn fadpcm_residual_is_the_signed_nibble_at_shift_zero() {
        let mut nibbles = [0u8; 256];
        for (i, n) in nibbles.iter_mut().enumerate() {
            *n = (i % 16) as u8;
        }
        let frame = synth_frame(0, 0, &nibbles);
        let sample = fake_sample(1, 256);
        let pcm = decode_fadpcm(&frame, &sample, 0, 256);
        assert_eq!(pcm.len(), 256, "une trame rend 256 échantillons");
        let expected: Vec<i16> = (0..256)
            .map(|i| {
                if i % 16 < 8 {
                    (i % 16) as i16
                } else {
                    (i % 16) as i16 - 16
                }
            })
            .collect();
        assert_eq!(pcm, expected, "nibble signé, sans prédiction ni mise à l'échelle");
    }

    /// The step **doubles** with the shift factor. Written the other way round
    /// the decoder produces plausible-looking noise, which is why this is a
    /// test and not a comment.
    #[test]
    fn fadpcm_step_grows_with_the_shift_factor() {
        let mut nibbles = [0u8; 256];
        nibbles[0] = 1;
        let sample = fake_sample(1, 256);
        let mut steps = Vec::new();
        for factor in 0..5 {
            let frame = synth_frame(0, factor, &nibbles);
            steps.push(decode_fadpcm(&frame, &sample, 0, 1)[0]);
        }
        assert_eq!(steps, vec![1, 2, 4, 8, 16], "un cran de facteur double le pas");
    }

    /// Channels are interleaved by frame, not by sample: the second channel's
    /// frame starts 140 bytes after the first, not two bytes.
    #[test]
    fn fadpcm_channels_are_interleaved_by_frame() {
        let mut left = [0u8; 256];
        left[0] = 3;
        let mut right = [0u8; 256];
        right[0] = 5;
        let mut data = synth_frame(0, 0, &left);
        data.extend_from_slice(&synth_frame(0, 0, &right));
        let sample = fake_sample(2, 256);
        assert_eq!(decode_fadpcm(&data, &sample, 0, 1)[0], 3, "canal gauche");
        assert_eq!(decode_fadpcm(&data, &sample, 1, 1)[0], 5, "canal droit");
    }

    /// The predictor feeds on the two previous samples, second coefficient
    /// subtracted. Index 1 is `(60, 0)`, so it only leans on `h1`.
    #[test]
    fn fadpcm_predictor_leans_on_the_history() {
        let mut nibbles = [0u8; 256];
        nibbles[0] = 4; // premier échantillon : 4 << 6 >> 6 = 4
        let frame = synth_frame(1, 0, &nibbles);
        let sample = fake_sample(1, 256);
        let pcm = decode_fadpcm(&frame, &sample, 0, 3);
        assert_eq!(pcm[0], 4, "sans historique, le résidu seul");
        // Ensuite le résidu est nul : l'échantillon ne vient que de 60*h1>>6.
        assert_eq!(pcm[1], (60 * 4) >> 6, "60 x h1 / 64");
        assert_eq!(pcm[2] as i32, (60 * pcm[1] as i32) >> 6, "et ainsi de suite");
    }

    /// A truncated bank must be refused, not read past its end.
    #[test]
    fn truncated_bank_is_refused() {
        let pcm: Vec<u8> = vec![0; 64];
        let bytes = synth_bank(2, &[(8, 1, 0, 32, None)], &pcm);
        for cut in [4, 20, 40, 61] {
            assert!(parse(&bytes[..cut]).is_err(), "un bank coupé à {cut} octets est refusé");
        }
    }

    /// An unknown codec is named rather than silently producing silence.
    #[test]
    fn unsupported_codec_is_named() {
        let bytes = synth_bank(15, &[(8, 1, 0, 32, None)], &[0; 64]);
        let bank = parse(&bytes).expect("le conteneur se lit quand même");
        assert_eq!(bank.codec, Codec::Unsupported(15), "Vorbis reconnu sans être décodé");
        let err = decode_with_bytes(&bytes, &bank, &bank.samples[0], 0, 16).unwrap_err();
        assert!(err.contains("VORBIS"), "l'erreur nomme le codec : {err}");
    }

    /// The WAV header the webview will read back.
    #[test]
    fn wav_header_is_well_formed() {
        let pcm = vec![0i16, 1, -1, 32767];
        let wav = to_wav(&pcm, 48000);
        assert_eq!(&wav[0..4], b"RIFF", "magic");
        assert_eq!(&wav[8..12], b"WAVE", "type");
        assert_eq!(
            u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]),
            48000,
            "fréquence"
        );
        assert_eq!(
            u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]),
            8,
            "taille des données"
        );
        assert_eq!(wav.len(), 44 + 8, "en-tête + données");
    }
}
