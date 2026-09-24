//! What the now-playing notification shows (MUSIQUE§5.5): title, artist,
//! album and cover, read from the track's own tags.
//!
//! Tags come through `symphonia`, which `rodio` already pulls in for its mp3
//! decoder: ID3v2 for mp3 (read by the probe, ahead of the stream), Vorbis
//! comments and embedded pictures for FLAC and Ogg (read by the container).
//! Nothing here is an error worth surfacing: a file without tags is the norm
//! for a folder of ripped ambience tracks, and the file name stands in.

use std::fs::File;
use std::io::Cursor;
use std::path::Path;

use serde::Serialize;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, MetadataRevision, StandardTagKey, StandardVisualKey};
use symphonia::core::probe::Hint;

/// Longest side of the cover sent to the front, in pixels. The notification
/// draws it at 56 CSS px; 160 covers the Big Picture zoom and a 2x screen. The
/// embedded picture itself is routinely 1000–3000 px and a few MB: sending it
/// raw through the event would mean megabytes of base64 for every track change.
const COVER_MAX_PX: u32 = 160;

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct TrackInfo {
    /// File name with its extension, so the front can recognise the bundled
    /// pack (untagged, credited in About) and fall back to the stem otherwise.
    pub file_name: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// JPEG `data:` URL, already shrunk to `COVER_MAX_PX`.
    pub cover: Option<String>,
}

/// Reads the tags of `path`. Never fails: whatever cannot be read is `None`.
pub fn read(path: &Path) -> TrackInfo {
    let mut info = TrackInfo {
        file_name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        ..TrackInfo::default()
    };
    let Ok(file) = File::open(path) else {
        return info;
    };
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let Ok(mut probed) =
        symphonia::default::get_probe().format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
    else {
        // m4a is counted by the folder scan but not enabled here: no tags,
        // the file name stands in.
        return info;
    };

    // Two places, and a file may fill either: the container's own metadata
    // (FLAC, Ogg) and what the probe read ahead of the stream (ID3v2 on mp3).
    // The container first — it is the one written for this format.
    // `fill_from` only fills what is still empty, so the order is the priority.
    if let Some(rev) = probed.format.metadata().skip_to_latest() {
        fill_from(&mut info, rev);
    }
    if let Some(mut meta) = probed.metadata.get() {
        if let Some(rev) = meta.skip_to_latest() {
            fill_from(&mut info, rev);
        }
    }
    info
}

fn fill_from(info: &mut TrackInfo, rev: &MetadataRevision) {
    for tag in rev.tags() {
        let slot = match tag.std_key {
            Some(StandardTagKey::TrackTitle) => &mut info.title,
            // Album artist only as a stand-in: on a compilation it names the
            // compiler ("Various Artists"), not who plays this track.
            Some(StandardTagKey::Artist) => &mut info.artist,
            Some(StandardTagKey::Album) => &mut info.album,
            _ => continue,
        };
        if slot.is_none() {
            *slot = non_empty(tag.value.to_string());
        }
    }
    if info.artist.is_none() {
        info.artist = rev
            .tags()
            .iter()
            .find(|t| t.std_key == Some(StandardTagKey::AlbumArtist))
            .and_then(|t| non_empty(t.value.to_string()));
    }
    if info.cover.is_none() {
        let visuals = rev.visuals();
        let front = visuals
            .iter()
            .find(|v| v.usage == Some(StandardVisualKey::FrontCover))
            .or_else(|| visuals.first());
        if let Some(visual) = front {
            info.cover = cover_data_url(&visual.data);
        }
    }
}

fn non_empty(s: String) -> Option<String> {
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

/// Decodes the embedded picture, shrinks it and re-encodes it as a JPEG
/// `data:` URL. `None` on a format the `image` features here do not cover
/// (jpeg and png only — what virtually every tagger embeds).
fn cover_data_url(data: &[u8]) -> Option<String> {
    let img = match image::load_from_memory(data) {
        Ok(img) => img,
        Err(e) => {
            log::warn!("music: embedded cover unreadable, notification shown without it: {e}");
            return None;
        }
    };
    let thumb = img.thumbnail(COVER_MAX_PX, COVER_MAX_PX).into_rgb8();
    let mut jpeg = Vec::new();
    if let Err(e) = thumb.write_to(&mut Cursor::new(&mut jpeg), image::ImageFormat::Jpeg) {
        log::warn!("music: cover thumbnail encoding failed: {e}");
        return None;
    }
    Some(format!("data:image/jpeg;base64,{}", crate::enginesound::base64(&jpeg)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal ID3v2.3 tag: header + one text frame per `(id, text)`, text in
    /// ISO-8859-1 (encoding byte 0). Enough for the probe to read it ahead of
    /// whatever follows.
    fn id3v2(frames: &[(&[u8; 4], &str)]) -> Vec<u8> {
        let mut body = Vec::new();
        for (id, text) in frames {
            let payload_len = 1 + text.len();
            body.extend_from_slice(*id);
            body.extend_from_slice(&(payload_len as u32).to_be_bytes());
            body.extend_from_slice(&[0, 0]);
            body.push(0);
            body.extend_from_slice(text.as_bytes());
        }
        let size = body.len() as u32;
        // Synchsafe size: 7 bits per byte.
        let synchsafe = [
            ((size >> 21) & 0x7f) as u8,
            ((size >> 14) & 0x7f) as u8,
            ((size >> 7) & 0x7f) as u8,
            (size & 0x7f) as u8,
        ];
        let mut out = b"ID3\x03\x00\x00".to_vec();
        out.extend_from_slice(&synchsafe);
        out.extend_from_slice(&body);
        out
    }

    /// A few silent MPEG-1 Layer III frames (128 kbps, 44.1 kHz, 417 bytes
    /// each): enough for the mp3 reader to accept the stream after the tag.
    fn silent_mp3_frames() -> Vec<u8> {
        let mut out = Vec::new();
        for _ in 0..8 {
            let mut frame = vec![0u8; 417];
            frame[..4].copy_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
            out.extend_from_slice(&frame);
        }
        out
    }

    // MUSIQUE§5.5: an ID3v2-tagged mp3 gives its title, artist and album.
    #[test]
    fn reads_id3v2_title_artist_album() {
        let dir = crate::testutil::temp_dir("music-tags-id3");
        let path = dir.join("01 - track.mp3");
        let mut bytes = id3v2(&[(b"TIT2", "Night Drive"), (b"TPE1", "Some Band"), (b"TALB", "Laps")]);
        bytes.extend_from_slice(&silent_mp3_frames());
        std::fs::write(&path, bytes).unwrap();

        let info = read(&path);
        assert_eq!(
            info.file_name, "01 - track.mp3",
            "file name kept for the front's fallback"
        );
        assert_eq!(info.title.as_deref(), Some("Night Drive"), "TIT2 is the title");
        assert_eq!(info.artist.as_deref(), Some("Some Band"), "TPE1 is the artist");
        assert_eq!(info.album.as_deref(), Some("Laps"), "TALB is the album");
        assert_eq!(info.cover, None, "no APIC frame, no cover");
    }

    // MUSIQUE§5.5: an untagged or unreadable file is not an error — only the
    // file name comes back, and the front shows it.
    #[test]
    fn untagged_or_unreadable_file_yields_file_name_only() {
        let dir = crate::testutil::temp_dir("music-tags-none");
        let bare = dir.join("menu-ambience.mp3");
        std::fs::write(&bare, silent_mp3_frames()).unwrap();
        let garbage = dir.join("broken.flac");
        std::fs::write(&garbage, b"not audio at all").unwrap();

        let info = read(&bare);
        assert_eq!(info.file_name, "menu-ambience.mp3", "file name always present");
        assert_eq!(
            (info.title, info.artist, info.cover),
            (None, None, None),
            "nothing invented"
        );

        let info = read(&garbage);
        assert_eq!(info.file_name, "broken.flac", "unreadable file still named");
        assert_eq!(info.title, None, "unreadable file has no title");

        let info = read(&dir.join("missing.mp3"));
        assert_eq!(info.file_name, "missing.mp3", "missing file still named");
    }

    // The cover is shrunk before it travels: a raw 1000 px cover would be
    // megabytes of base64 in every track-change event.
    #[test]
    fn cover_is_shrunk_to_a_jpeg_data_url() {
        let big = image::RgbImage::from_pixel(1000, 600, image::Rgb([200, 30, 30]));
        let mut png = Vec::new();
        big.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();

        let url = cover_data_url(&png).expect("a png cover decodes");
        assert!(
            url.starts_with("data:image/jpeg;base64,"),
            "re-encoded as JPEG data URL"
        );
        assert!(url.len() < 20_000, "thumbnail stays small, got {} bytes", url.len());
        assert_eq!(cover_data_url(b"not an image"), None, "garbage gives no cover");
    }
}
