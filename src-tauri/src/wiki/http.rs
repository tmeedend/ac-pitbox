//! One HTTPS GET, over WinHTTP (`docs/SPEC-wikipedia-fiche-detail.md` §6).
//!
//! **Why no HTTP crate.** The app had no HTTP client at all, and the two
//! requests this feature needs (§6.1 — one to Wikidata, one to a wiki) did not
//! justify pulling a stack of some thirty crates plus a TLS backend into a
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
//! crates). Neither buys anything WinHTTP does not already do here; both would
//! win the moment this file needs to grow.
//!
//! **Blocking on purpose.** WinHTTP is synchronous, so every call here blocks
//! its thread — which is why the command facades that will eventually call this
//! belong in `spawn_blocking`, exactly like the disk-touching commands
//! (`commands/ui_prefs.rs`). Sequential requests are what §6.2 asks for anyway.

/// What came back. A body the caller is free to fail to parse: this layer knows
/// nothing about Wikipedia, only about bytes and a status code.
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Ceiling on a response body, 8 MiB.
///
/// It was 2 MiB when the only thing fetched was a plain-text introduction. A
/// rendered article is another order of magnitude — a long one runs to several
/// hundred kilobytes of HTML — so the ceiling moved with the payload. It still
/// exists for the same reason: a decorative tab has no business buffering a
/// redirect gone wrong.
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

/// GETs `https://{host}{path}` and returns what came back.
///
/// `None` is "we could not ask" — DNS, connection, TLS, timeout, a body over
/// the ceiling. Never an error type: at this layer every failure is already a
/// non-result (§1), and the caller has nothing to display either way.
#[cfg(windows)]
pub fn get(host: &str, path: &str, user_agent: &str, timeout_ms: i32) -> Option<Response> {
    imp::get(host, path, user_agent, timeout_ms)
}

/// Non-Windows fallback: never actually runs (the app is Windows-only), it only
/// keeps the crate compiling elsewhere — same arrangement as
/// `music::ac_status`. The `windows` crate itself is a `cfg(windows)`
/// dependency, so the implementation below cannot even be named here.
#[cfg(not(windows))]
pub fn get(_host: &str, _path: &str, _user_agent: &str, _timeout_ms: i32) -> Option<Response> {
    None
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;

    use windows::core::PCWSTR;
    use windows::Win32::Networking::WinHttp::{
        WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryDataAvailable,
        WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetOption,
        WinHttpSetTimeouts, INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE,
        WINHTTP_OPTION_DECOMPRESSION, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
    };

    use super::{Response, MAX_BODY};

    /// gzip + deflate. The `windows` crate exposes the option but not this
    /// flag combination (`WINHTTP_DECOMPRESSION_FLAG_ALL` in `winhttp.h`).
    const DECOMPRESSION_ALL: u32 = 0x0000_0003;

    /// Closes a WinHTTP handle on the way out — including the five early
    /// returns below, which is the whole reason it exists.
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
                log::warn!("wiki/http: {} — {e}", $step);
                return None;
            }
        };
    }

    pub fn get(host: &str, path: &str, user_agent: &str, timeout_ms: i32) -> Option<Response> {
        let agent = wide(user_agent);
        let host_w = wide(host);
        let path_w = wide(path);
        let verb = wide("GET");
        // Trailing CRLF is what WinHTTP expects between header lines; no NUL,
        // the length is taken from the slice.
        let headers: Vec<u16> = "Accept: application/json\r\n".encode_utf16().collect();

        unsafe {
            // The User-Agent lives on the session, so every request carries it
            // — §6.2 makes it mandatory, and Wikimedia blocks requests without
            // an identifiable one.
            let session = WinHttpOpen(
                PCWSTR(agent.as_ptr()),
                WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                PCWSTR::null(),
                PCWSTR::null(),
                0,
            );
            if session.is_null() {
                log::warn!("wiki/http: WinHttpOpen — {}", std::io::Error::last_os_error());
                return None;
            }
            let session = Handle(session);

            // §6.2 asks for a short timeout, and it has to cover every phase:
            // a name that never resolves hangs exactly as long as a server that
            // never answers.
            win_step!(
                "WinHttpSetTimeouts",
                WinHttpSetTimeouts(session.0, timeout_ms, timeout_ms, timeout_ms, timeout_ms)
            );

            let connect = WinHttpConnect(session.0, PCWSTR(host_w.as_ptr()), INTERNET_DEFAULT_HTTPS_PORT, 0);
            if connect.is_null() {
                log::warn!("wiki/http: WinHttpConnect {host} — {}", std::io::Error::last_os_error());
                return None;
            }
            let connect = Handle(connect);

            // WINHTTP_FLAG_SECURE is the only thing making this HTTPS; without
            // it WinHTTP would happily talk cleartext to port 443. Redirects
            // are followed by WinHTTP's default policy, which refuses an
            // https→http downgrade — worth keeping as it is.
            let request = WinHttpOpenRequest(
                connect.0,
                PCWSTR(verb.as_ptr()),
                PCWSTR(path_w.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                WINHTTP_FLAG_SECURE,
            );
            if request.is_null() {
                log::warn!("wiki/http: WinHttpOpenRequest — {}", std::io::Error::last_os_error());
                return None;
            }
            let request = Handle(request);

            // Best-effort by design: on a Windows too old to know the option,
            // the answer simply arrives uncompressed. Logged rather than
            // swallowed — a packaged build has no console, so an untraced
            // failure leaves nothing to diagnose afterwards.
            let flags = DECOMPRESSION_ALL;
            if let Err(e) = WinHttpSetOption(
                Some(request.0 as *const c_void),
                WINHTTP_OPTION_DECOMPRESSION,
                Some(&flags.to_ne_bytes()),
            ) {
                log::warn!("wiki/http: decompression unavailable, plain response — {e}");
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

            let mut body: Vec<u8> = Vec::new();
            loop {
                let mut available: u32 = 0;
                win_step!(
                    "WinHttpQueryDataAvailable",
                    WinHttpQueryDataAvailable(request.0, &mut available)
                );
                if available == 0 {
                    break;
                }
                if body.len().saturating_add(available as usize) > MAX_BODY {
                    log::warn!("wiki/http: response over {MAX_BODY} bytes, dropped");
                    return None;
                }
                let mut chunk = vec![0u8; available as usize];
                let mut read: u32 = 0;
                win_step!(
                    "WinHttpReadData",
                    WinHttpReadData(request.0, chunk.as_mut_ptr() as *mut c_void, available, &mut read,)
                );
                if read == 0 {
                    break;
                }
                chunk.truncate(read as usize);
                body.extend_from_slice(&chunk);
            }

            Some(Response {
                status: status as u16,
                body,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rule (§6.1): a title goes into the query string intact, whatever it is
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
}
