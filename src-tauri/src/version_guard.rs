//! Refuses to start on a base a NEWER Pit Box has opened (§10).
//!
//! The schema only ever grows (columns and tables added, `schema.rs`), so an
//! older build reads a newer base without an error - and that is the danger:
//! it silently drops the settings it does not know when it rewrites a file
//! (`config.json` and the other typed files), re-classifies the library with
//! its older engine and catalogue, which the newer one then has to do again,
//! and misreads states it predates (a mod in the showcase taken for a broken
//! one). Asked at use: a release installed to be tried, then started by
//! mistake on the base the development build had moved on; or an older
//! version reinstalled over a newer one.
//!
//! The base remembers the newest build that opened it (`meta`), dated by its
//! commit (`build.rs`) - not by its version number, which a development build
//! shares with the release it follows. An older build is refused BEFORE it
//! touches anything: no backup rotated, no migration, no write.

use std::path::Path;

use rusqlite::{Connection, OpenFlags, OptionalExtension};

/// The date of this build's code, Unix seconds - 0 when it could not be
/// dated (`build.rs`), in which case the guard stands aside.
pub const BUILD_TIME: i64 = parse_time(env!("PITBOX_BUILD_TIME"));
/// "0.9.0 (78fdb92, 2026-10-11)": what the refusal names.
pub const BUILD_LABEL: &str = env!("PITBOX_BUILD_LABEL");

const META_TIME: &str = "newest_build_time";
const META_LABEL: &str = "newest_build_label";

const fn parse_time(s: &str) -> i64 {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut n: i64 = 0;
    while i < bytes.len() {
        let d = bytes[i];
        if d < b'0' || d > b'9' {
            return 0;
        }
        n = n * 10 + (d - b'0') as i64;
        i += 1;
    }
    n
}

/// The newest build that opened the base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewestBuild {
    pub time: i64,
    pub label: String,
}

/// What the base says of the newest build that opened it, read-only - the
/// file is not created, migrated nor written. `None`: no base yet, a base
/// from before this guard, or one that cannot be read (left to the usual
/// startup, which reports it).
pub fn newest(db: &Path) -> Option<NewestBuild> {
    if !db.is_file() {
        return None;
    }
    let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    let read = |key: &str| -> Option<String> {
        conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
    };
    Some(NewestBuild {
        time: read(META_TIME)?.parse().ok()?,
        label: read(META_LABEL).unwrap_or_default(),
    })
}

/// Whether a build of `ours` must refuse a base a build of `newest` opened.
pub fn refuses(newest: Option<&NewestBuild>, ours: i64) -> bool {
    ours != 0 && newest.is_some_and(|n| n.time > ours)
}

/// Whether this build is newer than every one that opened the base - its
/// first start on it, after an update. `true` on a base no build stamped.
pub fn is_newer(newest: Option<&NewestBuild>, ours: i64) -> bool {
    ours != 0 && newest.is_none_or(|n| ours > n.time)
}

/// Records this build as the newest to open the base, when it is.
pub fn stamp(conn: &Connection, newest: Option<&NewestBuild>, ours: i64, label: &str) -> rusqlite::Result<()> {
    if !is_newer(newest, ours) {
        return Ok(());
    }
    crate::overlay::set_meta(conn, META_TIME, &ours.to_string())?;
    crate::overlay::set_meta(conn, META_LABEL, label)
}

/// The refusal, in the user's language: the six locale files are embedded,
/// since nothing of the interface exists yet - a key missing in his language
/// falls back to English, as `t()` does.
pub fn refusal_text(lang: &str, newest: &NewestBuild) -> (String, String) {
    const LOCALES: [(&str, &str); 6] = [
        ("fr", include_str!("../../src/lib/i18n/locales/fr.json")),
        ("en", include_str!("../../src/lib/i18n/locales/en.json")),
        ("it", include_str!("../../src/lib/i18n/locales/it.json")),
        ("de", include_str!("../../src/lib/i18n/locales/de.json")),
        ("es", include_str!("../../src/lib/i18n/locales/es.json")),
        ("pt", include_str!("../../src/lib/i18n/locales/pt.json")),
    ];
    let dict = |code: &str| -> Option<serde_json::Value> {
        let (_, text) = LOCALES.iter().find(|(c, _)| *c == code)?;
        serde_json::from_str(text).ok()
    };
    let (mine, english) = (dict(lang), dict("en"));
    let text = |key: &str| -> String {
        let pick = |d: &Option<serde_json::Value>| {
            key.split('.')
                .try_fold(d.as_ref()?, |v, part| v.get(part))?
                .as_str()
                .map(str::to_string)
        };
        pick(&mine)
            .or_else(|| pick(&english))
            .unwrap_or_else(|| key.to_string())
    };
    let body = text("startupGuard.body")
        .replace("{newer}", &newest.label)
        .replace("{ours}", BUILD_LABEL);
    (text("startupGuard.title"), body)
}

/// Shows the refusal and returns once it is dismissed. A native box: the
/// window does not exist yet, and must not - it would start the interface on
/// a base it cannot be trusted with.
#[cfg(windows)]
pub fn show_refusal(title: &str, body: &str) {
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let (title, body) = (wide(title), wide(body));
    // SAFETY: both buffers are NUL-terminated and outlive the call.
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(body.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(not(windows))]
pub fn show_refusal(title: &str, body: &str) {
    eprintln!("{title}\n{body}");
}

/// The language of the interface, before the interface exists: the one forced
/// in the settings, else the system's, else English - the rule of `t()`.
pub fn language(forced: Option<&str>) -> String {
    if let Some(code) = forced.filter(|c| !c.is_empty()) {
        return code.to_string();
    }
    system_language().unwrap_or_else(|| "en".into())
}

#[cfg(windows)]
fn system_language() -> Option<String> {
    use windows::Win32::Globalization::GetUserDefaultLocaleName;
    let mut buf = [0u16; 85];
    // SAFETY: the buffer is LOCALE_NAME_MAX_LENGTH (85) wide, as the call asks.
    let n = unsafe { GetUserDefaultLocaleName(&mut buf) };
    if n <= 0 {
        return None;
    }
    let name = String::from_utf16_lossy(&buf[..(n as usize).saturating_sub(1)]);
    Some(name.get(..2)?.to_lowercase())
}

#[cfg(not(windows))]
fn system_language() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(time: i64) -> NewestBuild {
        NewestBuild {
            time,
            label: format!("build {time}"),
        }
    }

    /// Rule (§10): a build older than the newest that opened the base is
    /// refused; the same one or a newer one is not; an undated build stands
    /// aside, and so does a base no build stamped.
    #[test]
    fn an_older_build_is_refused_and_nothing_else() {
        assert!(refuses(Some(&build(200)), 100), "older code than the base has seen");
        assert!(!refuses(Some(&build(200)), 200), "the same build again");
        assert!(!refuses(Some(&build(200)), 300), "a newer build");
        assert!(!refuses(None, 100), "a base from before the guard");
        assert!(!refuses(Some(&build(200)), 0), "a build that cannot date itself");
        assert!(is_newer(None, 100) && is_newer(Some(&build(99)), 100));
        assert!(!is_newer(Some(&build(100)), 100));
    }

    /// The base remembers the newest build, read back without creating,
    /// migrating nor writing anything - and an older one never lowers it.
    #[test]
    fn the_base_remembers_the_newest_build() {
        let base = crate::testutil::temp_dir("version-guard");
        let db = base.join("overlay.sqlite");
        assert_eq!(newest(&db), None, "no base yet");
        assert!(!db.exists(), "and none created by asking");

        let conn = crate::overlay::open(&db).unwrap();
        assert_eq!(newest(&db), None, "a base from before the guard");
        stamp(&conn, None, 200, "0.9.0 (abc, 2026-10-11)").unwrap();
        let seen = newest(&db).expect("stamped");
        assert_eq!((seen.time, seen.label.as_str()), (200, "0.9.0 (abc, 2026-10-11)"));

        stamp(&conn, Some(&seen), 100, "older").unwrap();
        assert_eq!(newest(&db).unwrap().time, 200, "an older build never lowers it");
        stamp(&conn, Some(&seen), 300, "newer").unwrap();
        assert_eq!(newest(&db).unwrap().label, "newer");
    }

    /// The refusal names both builds, in the user's language, English for a
    /// language that lacks the text.
    #[test]
    fn the_refusal_names_both_builds_in_his_language() {
        let newer = NewestBuild {
            time: 1,
            label: "0.10.0 (def, 2026-11-02)".into(),
        };
        let (title, body) = refusal_text("fr", &newer);
        assert!(!title.starts_with("startupGuard"), "a French title: {title}");
        assert!(body.contains("0.10.0 (def, 2026-11-02)"), "the newer build: {body}");
        assert!(body.contains(BUILD_LABEL), "and this one: {body}");
        let (en_title, _) = refusal_text("en", &newer);
        assert_ne!(title, en_title, "French is not English");
        assert_eq!(refusal_text("xx", &newer).0, en_title, "unknown language: English");
    }
}
