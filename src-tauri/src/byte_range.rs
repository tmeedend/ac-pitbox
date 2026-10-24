//! Byte ranges for the app's own URI protocols: `carpreview` (the 3D
//! preview's models, PREVIEW§7.2) and `storymedia` (the sample story's media,
//! `story/media.rs`). A webview reads a large model, a video or a sound in
//! parts through `Range` requests: a `<video>` asks for a range before it plays,
//! and only seeks - a loop starting over included - in a file served by parts.

use std::borrow::Cow;
use tauri::http::{header, response::Builder, Response, StatusCode};

/// Answers with `bytes`, or with the part of them `range` (a `Range` header's
/// value) asks for. A range that is not a simple one gets the whole file,
/// which stays correct.
pub fn respond(
    builder: Builder,
    bytes: Cow<'_, [u8]>,
    range: Option<&str>,
) -> Result<Response<Vec<u8>>, tauri::http::Error> {
    let total = bytes.len() as u64;
    let builder = builder.header(header::ACCEPT_RANGES, "bytes");
    match range.and_then(|v| parse(v, total)) {
        Some((start, end)) => builder
            .status(StatusCode::PARTIAL_CONTENT)
            .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{total}"))
            .body(bytes[start as usize..=end as usize].to_vec()),
        None => builder.body(bytes.into_owned()),
    }
}

/// `bytes=start-end`, both included, `end` optional. `None` for anything that
/// is not a simple range: a multiple or unreadable range is served whole.
fn parse(value: &str, total: u64) -> Option<(u64, u64)> {
    let spec = value.strip_prefix("bytes=")?;
    if spec.contains(',') || total == 0 {
        return None;
    }
    let (start, end) = spec.split_once('-')?;
    let start: u64 = start.trim().parse().ok()?;
    let end: u64 = match end.trim() {
        "" => total - 1,
        text => text.parse().ok()?,
    };
    let end = end.min(total - 1);
    (start <= end).then_some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    // A correct `Range` is honoured, and anything outside the file or the
    // simple form falls back to the whole answer - never to a wrong slice,
    // which would give a truncated model or video.
    #[test]
    fn range_header_is_parsed_or_ignored() {
        assert_eq!(parse("bytes=0-99", 1000), Some((0, 99)), "explicit range");
        assert_eq!(parse("bytes=500-", 1000), Some((500, 999)), "implicit end");
        assert_eq!(
            parse("bytes=900-5000", 1000),
            Some((900, 999)),
            "an end past the file, brought back to its last byte"
        );
        assert_eq!(parse("bytes=0-0", 1), Some((0, 0)), "a one-byte file");

        assert_eq!(parse("bytes=100-50", 1000), None, "reversed range");
        assert_eq!(parse("bytes=0-10,20-30", 1000), None, "multiple ranges");
        assert_eq!(parse("octets=0-10", 1000), None, "unknown unit");
        assert_eq!(parse("bytes=0-10", 0), None, "empty file");
        assert_eq!(parse("bytes=abc-10", 1000), None, "unreadable bound");
    }

    // The answer to a range is the slice, marked partial, with where it lies
    // in the file; without one, the whole file.
    #[test]
    fn a_range_is_answered_with_its_slice() {
        let partial = respond(Response::builder(), Cow::Borrowed(b"glTFbody"), Some("bytes=4-7")).unwrap();
        assert_eq!(partial.status(), StatusCode::PARTIAL_CONTENT, "partial content");
        assert_eq!(partial.body(), b"body", "the slice asked for");
        assert_eq!(partial.headers()[header::CONTENT_RANGE], "bytes 4-7/8", "where it lies");

        let whole = respond(Response::builder(), Cow::Borrowed(b"glTFbody"), None).unwrap();
        assert_eq!(whole.status(), StatusCode::OK, "no range, the whole file");
        assert_eq!(whole.body(), b"glTFbody", "every byte");
        assert_eq!(whole.headers()[header::ACCEPT_RANGES], "bytes", "ranges are offered");
    }
}
