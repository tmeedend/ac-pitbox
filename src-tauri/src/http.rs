//! HTTP over WinHTTP: a small GET for JSON APIs, and a streamed download to
//! disk. Two callers: the Wikipedia enrichment
//! (`docs/SPEC-wikipedia-fiche-detail.md` WIKI§6) and the mod update check
//! (`cup.rs`, §4.7).
//!
//! **Why no HTTP crate.** The app had no HTTP client at all, and the two
//! requests the Wikipedia feature needs (WIKI§6.1 — one to Wikidata, one to a wiki)
//! did not justify pulling a stack of some thirty crates plus a TLS backend into a
//! Windows-only build for a decorative tab. WinHTTP ships with Windows, the
//! `windows` crate is already a dependency (registry reads, shared memory, FMOD
//! loading), and the operating system brings what the crates would have: TLS
//! against the machine's own certificate store, the user's proxy configuration,
//! redirect handling, and timeouts.
//!
//! The alternatives, kept here so the decision can be revisited rather than
//! rediscovered: `reqwest` with the `native-tls` feature (async, would also add
//! hyper/tower and five lockfile entries — the natural choice the day the app
//! stops being Windows-only), and `ureq` with `native-tls` (blocking, about ten
//! crates). Neither buys anything WinHTTP does not already do here. The mod
//! updates added a second shape of request — any URL, streamed to a file, with
//! progress — and it still fit in the same forty lines of handle management,
//! so the decision stands.
//!
//! **Blocking on purpose.** WinHTTP is synchronous, so every call here blocks
//! its thread — which is why the command facades that call this belong in
//! `spawn_blocking`, exactly like the disk-touching commands
//! (`commands/ui_prefs.rs`). Sequential requests are what WIKI§6.2 asks for anyway.

use std::path::Path;

/// What came back. A body the caller is free to fail to parse: this layer knows
/// nothing about Wikipedia, only about bytes and a status code.
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Ceiling on a response body **held in memory** (`get`), 8 MiB.
///
/// It was 2 MiB when the only thing fetched was a plain-text introduction. A
/// rendered article is another order of magnitude — a long one runs to several
/// hundred kilobytes of HTML — so the ceiling moved with the payload. It still
/// exists for the same reason: a decorative tab has no business buffering a
/// redirect gone wrong. A download to disk (`download`) has no ceiling — a mod
/// archive routinely weighs hundreds of megabytes.
const MAX_BODY: usize = 8 * 1024 * 1024;

/// Percent-encodes one query-string **value** (RFC 3986 unreserved set).
///
/// Article titles are the reason: they carry spaces, apostrophes, accents and
/// CJK — `Toyota Sprinter Trueno`, `トヨタ・AE86`. Written by hand rather than
/// taken from a crate because it is ten lines and one test, and because the
/// only inputs are values we build ourselves.
pub fn encode_query_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(*byte as char),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// An absolute URL cut into what WinHTTP asks for separately.
#[derive(Debug, PartialEq, Eq)]
pub struct UrlParts {
    pub secure: bool,
    pub host: String,
    pub port: u16,
    /// Path and query, always starting with `/`. The fragment is gone: it is
    /// never sent to a server.
    pub path: String,
}

/// Splits `http(s)://host[:port]/path?query#fragment`.
///
/// By hand rather than through `WinHttpCrackUrl`: that API fills a struct of
/// eight pointer/length pairs, which is more code than this and cannot be
/// tested off Windows. Only URLs we build ourselves reach this function, so
/// userinfo (`user:pass@`) and IPv6 literals are refused rather than parsed.
pub fn split_url(url: &str) -> Option<UrlParts> {
    let (secure, rest) = match url.strip_prefix("https://") {
        Some(rest) => (true, rest),
        None => (false, url.strip_prefix("http://")?),
    };
    let rest = rest.split('#').next().unwrap_or_default();
    let (authority, path) = match rest.find(['/', '?']) {
        Some(i) if rest[i..].starts_with('?') => (&rest[..i], format!("/{}", &rest[i..])),
        Some(i) => (&rest[..i], rest[i..].to_string()),
        None => (rest, "/".to_string()),
    };
    if authority.is_empty() || authority.contains('@') || authority.starts_with('[') {
        return None;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (host, port.parse().ok()?),
        None => (authority, if secure { 443 } else { 80 }),
    };
    if host.is_empty() {
        return None;
    }
    Some(UrlParts {
        secure,
        host: host.to_string(),
        port,
        path,
    })
}

/// Why a download did not produce a file.
#[derive(Debug, PartialEq, Eq)]
pub enum DownloadError {
    /// DNS, connection, TLS, timeout, or a URL we could not read. Already
    /// logged with the WinHTTP step that failed.
    Unreachable,
    /// The server answered, but not with a 200.
    Status(u16),
    /// The server answered with a web page (`text/html`) — a sign-in wall, a
    /// file host's landing page, a confirmation screen. Nothing was written:
    /// what the caller wants is a file, and this is something to show a person.
    Page,
    /// The progress callback asked to stop. The partial file is gone.
    Cancelled,
    /// Writing to disk failed. The partial file is gone.
    Io(String),
}

/// GETs `https://{host}{path}` and returns what came back.
///
/// `None` is "we could not ask" — DNS, connection, TLS, timeout, a body over
/// the ceiling. Never an error type: at this layer every failure is already a
/// non-result (WIKI§1), and the caller has nothing to display either way.
#[cfg(windows)]
pub fn get(host: &str, path: &str, user_agent: &str, timeout_ms: i32) -> Option<Response> {
    imp::get(host, path, user_agent, timeout_ms)
}

/// Streams `url` to `dest`, following redirects.
///
/// `on_progress(received, total)` is called after every chunk — `total` is the
/// announced `Content-Length`, absent when the server does not give one — and
/// stops the transfer by returning `false`. On any failure, `dest` does not
/// exist afterwards: a half-written archive is worse than none, since the next
/// step would try to open it.
#[cfg(windows)]
pub fn download(
    url: &str,
    dest: &Path,
    user_agent: &str,
    timeout_ms: i32,
    on_progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
) -> Result<u64, DownloadError> {
    let Some(parts) = split_url(url) else {
        log::warn!("http: not a URL we can fetch — {url}");
        return Err(DownloadError::Unreachable);
    };
    imp::download(&parts, dest, user_agent, timeout_ms, on_progress)
}

/// Non-Windows fallback: never actually runs (the app is Windows-only), it only
/// keeps the crate compiling elsewhere — same arrangement as
/// `music::ac_status`. The `windows` crate itself is a `cfg(windows)`
/// dependency, so the implementation below cannot even be named here.
#[cfg(not(windows))]
pub fn get(_host: &str, _path: &str, _user_agent: &str, _timeout_ms: i32) -> Option<Response> {
    None
}

#[cfg(not(windows))]
pub fn download(
    _url: &str,
    _dest: &Path,
    _user_agent: &str,
    _timeout_ms: i32,
    _on_progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
) -> Result<u64, DownloadError> {
    Err(DownloadError::Unreachable)
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::io::Write;
    use std::path::Path;

    use windows::core::PCWSTR;
    use windows::Win32::Networking::WinHttp::{
        WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryDataAvailable,
        WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetOption,
        WinHttpSetTimeouts, INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE,
        WINHTTP_OPEN_REQUEST_FLAGS, WINHTTP_OPTION_DECOMPRESSION, WINHTTP_QUERY_CONTENT_LENGTH,
        WINHTTP_QUERY_CONTENT_TYPE, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
    };

    use super::{DownloadError, Response, UrlParts, MAX_BODY};

    /// gzip + deflate. The `windows` crate exposes the option but not this
    /// flag combination (`WINHTTP_DECOMPRESSION_FLAG_ALL` in `winhttp.h`).
    const DECOMPRESSION_ALL: u32 = 0x0000_0003;

    /// Closes a WinHTTP handle on the way out — including the early returns
    /// below, which is the whole reason it exists.
    struct Handle(*mut c_void);

    impl Drop for Handle {
        fn drop(&mut self) {
            // Nothing useful to do about a failed close, and it happens after
            // the answer is already in hand.
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }

    /// A request whose response headers have arrived. Field order is drop
    /// order: the request closes before its connection, the connection
    /// before its session.
    struct Opened {
        request: Handle,
        _connect: Handle,
        _session: Handle,
        status: u16,
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// A WinHTTP call that returns a `BOOL`: log what failed and give up.
    /// Spelled as a macro because the alternative is the same six lines eight
    /// times over, and because the failure message has to name the step — an
    /// `Err` with no context is unreadable in a user's log file.
    macro_rules! win_step {
        ($step:literal, $call:expr) => {
            if let Err(e) = $call {
                log::warn!("http: {} — {e}", $step);
                return None;
            }
        };
    }

    /// Sends one GET and waits for the response headers. `decompress` asks
    /// the server for gzip — right for a JSON answer, pointless for an archive
    /// that is already compressed, and it would make `Content-Length` describe
    /// the wire rather than the file.
    fn open(url: &UrlParts, user_agent: &str, timeout_ms: i32, accept: &str, decompress: bool) -> Option<Opened> {
        let agent = wide(user_agent);
        let host_w = wide(&url.host);
        let path_w = wide(&url.path);
        let verb = wide("GET");
        // Trailing CRLF is what WinHTTP expects between header lines; no NUL,
        // the length is taken from the slice.
        let headers: Vec<u16> = format!("Accept: {accept}\r\n").encode_utf16().collect();

        unsafe {
            // The User-Agent lives on the session, so every request carries it
            // — WIKI§6.2 makes it mandatory, and Wikimedia blocks requests without
            // an identifiable one.
            let session = WinHttpOpen(
                PCWSTR(agent.as_ptr()),
                WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                PCWSTR::null(),
                PCWSTR::null(),
                0,
            );
            if session.is_null() {
                log::warn!("http: WinHttpOpen — {}", std::io::Error::last_os_error());
                return None;
            }
            let session = Handle(session);

            // WIKI§6.2 asks for a short timeout, and it has to cover every phase:
            // a name that never resolves hangs exactly as long as a server that
            // never answers. For a download, the receive timeout bounds each
            // read, not the whole transfer.
            win_step!(
                "WinHttpSetTimeouts",
                WinHttpSetTimeouts(session.0, timeout_ms, timeout_ms, timeout_ms, timeout_ms)
            );

            let connect = WinHttpConnect(session.0, PCWSTR(host_w.as_ptr()), url.port, 0);
            if connect.is_null() {
                log::warn!(
                    "http: WinHttpConnect {} — {}",
                    url.host,
                    std::io::Error::last_os_error()
                );
                return None;
            }
            let connect = Handle(connect);

            // WINHTTP_FLAG_SECURE is the only thing making this HTTPS; without
            // it WinHTTP would happily talk cleartext to port 443. Redirects
            // are followed by WinHTTP's default policy, which refuses an
            // https→http downgrade — worth keeping as it is.
            let flags = if url.secure {
                WINHTTP_FLAG_SECURE
            } else {
                WINHTTP_OPEN_REQUEST_FLAGS(0)
            };
            let request = WinHttpOpenRequest(
                connect.0,
                PCWSTR(verb.as_ptr()),
                PCWSTR(path_w.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                flags,
            );
            if request.is_null() {
                log::warn!("http: WinHttpOpenRequest — {}", std::io::Error::last_os_error());
                return None;
            }
            let request = Handle(request);

            // Best-effort by design: on a Windows too old to know the option,
            // the answer simply arrives uncompressed. Logged rather than
            // swallowed — a packaged build has no console, so an untraced
            // failure leaves nothing to diagnose afterwards.
            if decompress {
                let flags = DECOMPRESSION_ALL;
                if let Err(e) = WinHttpSetOption(
                    Some(request.0 as *const c_void),
                    WINHTTP_OPTION_DECOMPRESSION,
                    Some(&flags.to_ne_bytes()),
                ) {
                    log::warn!("http: decompression unavailable, plain response — {e}");
                }
            }

            win_step!(
                "WinHttpSendRequest",
                WinHttpSendRequest(request.0, Some(&headers), None, 0, 0, 0)
            );
            win_step!(
                "WinHttpReceiveResponse",
                WinHttpReceiveResponse(request.0, std::ptr::null_mut())
            );

            let mut status: u32 = 0;
            let mut status_len = std::mem::size_of::<u32>() as u32;
            win_step!(
                "WinHttpQueryHeaders(status)",
                WinHttpQueryHeaders(
                    request.0,
                    WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                    PCWSTR::null(),
                    Some(std::ptr::addr_of_mut!(status) as *mut c_void),
                    &mut status_len,
                    std::ptr::null_mut(),
                )
            );

            Some(Opened {
                request,
                _connect: connect,
                _session: session,
                status: status as u16,
            })
        }
    }

    /// One response header as text, `None` when the server did not send it.
    fn header(request: &Handle, info: u32) -> Option<String> {
        unsafe {
            // First call with no buffer: WinHTTP answers "insufficient buffer"
            // and writes the size it needs, in bytes. An absent header answers
            // the same call with a different error and leaves the size at 0 —
            // which is the only case this function reports.
            let mut len: u32 = 0;
            if WinHttpQueryHeaders(request.0, info, PCWSTR::null(), None, &mut len, std::ptr::null_mut()).is_ok()
                || len == 0
            {
                return None;
            }
            let mut buf = vec![0u16; len as usize / 2 + 1];
            if let Err(e) = WinHttpQueryHeaders(
                request.0,
                info,
                PCWSTR::null(),
                Some(buf.as_mut_ptr() as *mut c_void),
                &mut len,
                std::ptr::null_mut(),
            ) {
                log::warn!("http: WinHttpQueryHeaders({info}) — {e}");
                return None;
            }
            buf.truncate(len as usize / 2);
            Some(String::from_utf16_lossy(&buf))
        }
    }

    /// Reads the body chunk by chunk and hands each one over. `Some(true)` =
    /// read to the end, `Some(false)` = `sink` asked to stop, `None` = WinHTTP
    /// failed (already logged).
    fn read_body(request: &Handle, sink: &mut dyn FnMut(&[u8]) -> bool) -> Option<bool> {
        unsafe {
            loop {
                let mut available: u32 = 0;
                win_step!(
                    "WinHttpQueryDataAvailable",
                    WinHttpQueryDataAvailable(request.0, &mut available)
                );
                if available == 0 {
                    return Some(true);
                }
                let mut chunk = vec![0u8; available as usize];
                let mut read: u32 = 0;
                win_step!(
                    "WinHttpReadData",
                    WinHttpReadData(request.0, chunk.as_mut_ptr() as *mut c_void, available, &mut read,)
                );
                if read == 0 {
                    return Some(true);
                }
                if !sink(&chunk[..read as usize]) {
                    return Some(false);
                }
            }
        }
    }

    pub fn get(host: &str, path: &str, user_agent: &str, timeout_ms: i32) -> Option<Response> {
        let url = UrlParts {
            secure: true,
            host: host.to_string(),
            port: INTERNET_DEFAULT_HTTPS_PORT,
            path: path.to_string(),
        };
        let opened = open(&url, user_agent, timeout_ms, "application/json", true)?;
        let mut body: Vec<u8> = Vec::new();
        let mut too_big = false;
        read_body(&opened.request, &mut |chunk| {
            if body.len().saturating_add(chunk.len()) > MAX_BODY {
                too_big = true;
                return false;
            }
            body.extend_from_slice(chunk);
            true
        })?;
        if too_big {
            log::warn!("http: response over {MAX_BODY} bytes, dropped");
            return None;
        }
        Some(Response {
            status: opened.status,
            body,
        })
    }

    pub fn download(
        url: &UrlParts,
        dest: &Path,
        user_agent: &str,
        timeout_ms: i32,
        on_progress: &mut dyn FnMut(u64, Option<u64>) -> bool,
    ) -> Result<u64, DownloadError> {
        let opened = open(url, user_agent, timeout_ms, "*/*", false).ok_or(DownloadError::Unreachable)?;
        if opened.status != 200 {
            log::warn!("http: {}{} answered {}", url.host, url.path, opened.status);
            return Err(DownloadError::Status(opened.status));
        }
        // Checked on the header, before a single byte is written: a file
        // host's landing page must not be mistaken for the file, and there
        // is no point streaming it to disk to find out.
        if header(&opened.request, WINHTTP_QUERY_CONTENT_TYPE)
            .is_some_and(|t| t.trim().to_ascii_lowercase().starts_with("text/html"))
        {
            return Err(DownloadError::Page);
        }
        let total = header(&opened.request, WINHTTP_QUERY_CONTENT_LENGTH).and_then(|l| l.trim().parse::<u64>().ok());

        let file = std::fs::File::create(dest).map_err(|e| DownloadError::Io(e.to_string()))?;
        let mut out = std::io::BufWriter::with_capacity(1 << 20, file);
        let mut received: u64 = 0;
        let mut write_error: Option<String> = None;
        let finished = read_body(&opened.request, &mut |chunk| {
            if let Err(e) = out.write_all(chunk) {
                write_error = Some(e.to_string());
                return false;
            }
            received += chunk.len() as u64;
            on_progress(received, total)
        });
        let flushed = out.flush().map_err(|e| e.to_string());
        drop(out);

        let outcome = match (finished, write_error, flushed) {
            (_, Some(e), _) | (Some(true), None, Err(e)) => Err(DownloadError::Io(e)),
            (None, None, _) => Err(DownloadError::Unreachable),
            (Some(false), None, _) => Err(DownloadError::Cancelled),
            // A connection that drops mid-transfer ends the body early without
            // any WinHTTP error: only the announced length can tell.
            (Some(true), None, Ok(())) if total.is_some_and(|t| t != received) => {
                log::warn!(
                    "http: {}{} ended after {received} of {} bytes",
                    url.host,
                    url.path,
                    total.unwrap_or_default()
                );
                Err(DownloadError::Unreachable)
            }
            (Some(true), None, Ok(())) => Ok(received),
        };
        if outcome.is_err() {
            if let Err(e) = std::fs::remove_file(dest) {
                log::warn!("http: partial download {} left behind — {e}", dest.display());
            }
        }
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rule (WIKI§6.1): a title goes into the query string intact, whatever it is
    /// made of. The three cases are real titles of the same car, measured on
    /// the API.
    #[test]
    fn a_title_survives_the_query_string() {
        assert_eq!(encode_query_value("Toyota AE86"), "Toyota%20AE86", "spaces");
        assert_eq!(
            encode_query_value("Toyota Sprinter Trueno"),
            "Toyota%20Sprinter%20Trueno",
            "the French title of the same entity"
        );
        assert_eq!(
            encode_query_value("トヨタ・AE86"),
            "%E3%83%88%E3%83%A8%E3%82%BF%E3%83%BBAE86",
            "UTF-8 encoded byte by byte, not dropped"
        );
    }

    /// Rule: the unreserved set stays literal — encoding it would work but
    /// would make every logged URL unreadable.
    #[test]
    fn unreserved_characters_are_left_alone() {
        assert_eq!(encode_query_value("Q1377219"), "Q1377219");
        assert_eq!(encode_query_value("a-b_c.d~e"), "a-b_c.d~e");
    }

    /// Rule: separators that would otherwise change the meaning of the query
    /// are encoded — a title containing `&` must not start a new parameter.
    #[test]
    fn query_separators_are_neutralised() {
        assert_eq!(encode_query_value("A&B=C"), "A%26B%3DC", "no parameter injection");
        assert_eq!(encode_query_value("a|b"), "a%7Cb", "the API's own list separator");
    }

    /// Rule: a URL is cut where WinHTTP expects it, scheme deciding both TLS
    /// and the default port. The two real redirect targets of the CUP registry
    /// (§4.7) are the cases: a query string, and a fragment that carries the
    /// decryption key of a file host and must never reach a server.
    #[test]
    fn a_url_splits_into_what_winhttp_asks_for() {
        assert_eq!(
            split_url("https://files.acstuff.club/cup/hsrc/hsrc_subaru_gc8.7z?key=uCaI7tnK"),
            Some(UrlParts {
                secure: true,
                host: "files.acstuff.club".into(),
                port: 443,
                path: "/cup/hsrc/hsrc_subaru_gc8.7z?key=uCaI7tnK".into(),
            }),
            "query kept with the path"
        );
        assert_eq!(
            split_url("https://mega.nz/file/KoogkRaS#uRIlMYMQ").map(|u| u.path),
            Some("/file/KoogkRaS".into()),
            "fragment dropped"
        );
        assert_eq!(
            split_url("http://example.com:8080"),
            Some(UrlParts {
                secure: false,
                host: "example.com".into(),
                port: 8080,
                path: "/".into(),
            }),
            "explicit port, empty path"
        );
        assert_eq!(
            split_url("https://example.com?x=1").map(|u| u.path),
            Some("/?x=1".into()),
            "a query straight after the host still gets its slash"
        );
    }

    /// Rule: anything that is not a plain http(s) URL is refused rather than
    /// guessed at — no scheme, another scheme, credentials, an empty host.
    #[test]
    fn a_url_we_do_not_build_is_refused() {
        assert_eq!(split_url("acstuff.club/cup/"), None, "no scheme");
        assert_eq!(split_url("ftp://example.com/a.zip"), None, "other scheme");
        assert_eq!(split_url("https://user:pw@example.com/"), None, "credentials");
        assert_eq!(split_url("https:///path"), None, "empty host");
        assert_eq!(
            split_url("https://example.com:http/"),
            None,
            "port that is not a number"
        );
    }
}
