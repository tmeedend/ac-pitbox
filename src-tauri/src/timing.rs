//! Startup and slow-command timing - **off unless `PITBOX_TIMING` names a
//! file**, and then one JSON line per measure appended to it. What
//! `npm run bench:startup` reads (`scripts/bench-startup.mjs`).
//!
//! Permanent rather than written for each investigation: the first pass on
//! performance (2026-10-08) wrote a timing by hand, measured, then removed it,
//! three times in one session. A measure that costs that much is not taken
//! regularly, and the library only grows. Off, every call is one check of a
//! `OnceLock`.
//!
//! A line: `{"t": 412.6, "label": "setup.backup", "ms": 71.3}` - `t` the time
//! since `start` (process start, near enough), `ms` the duration of a timed
//! step, `webview` the page's own clock (`performance.now()`) for a mark sent
//! by the frontend, `n` a size (`count`).

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();
static SINK: OnceLock<Option<Mutex<PathBuf>>> = OnceLock::new();

/// The origin of every `t`. Called first thing in `run`.
pub fn start() {
    let _ = START.set(Instant::now());
}

pub fn enabled() -> bool {
    sink().is_some()
}

fn sink() -> Option<&'static Mutex<PathBuf>> {
    SINK.get_or_init(|| std::env::var_os("PITBOX_TIMING").map(|p| Mutex::new(PathBuf::from(p))))
        .as_ref()
}

fn elapsed_ms() -> f64 {
    START.get().map_or(0.0, |s| s.elapsed().as_secs_f64() * 1000.0)
}

/// Runs `work`, and records how long it took under `label`.
pub fn step<T>(label: &str, work: impl FnOnce() -> T) -> T {
    if !enabled() {
        return work();
    }
    let t = Instant::now();
    let out = work();
    record(label, Some(t.elapsed().as_secs_f64() * 1000.0), None, None);
    out
}

/// A point in time: `webview` is the page's clock when the frontend sent it.
pub fn mark(label: &str, webview: Option<f64>) {
    record(label, None, webview, None);
}

/// A size worth reading next to the times - the number of cards, without
/// which two measures of `list_library` cannot be compared.
pub fn count(label: &str, n: usize) {
    record(label, None, None, Some(n));
}

fn record(label: &str, ms: Option<f64>, webview: Option<f64>, n: Option<usize>) {
    let Some(sink) = sink() else { return };
    let line = line(elapsed_ms(), label, ms, webview, n);
    let Ok(path) = sink.lock() else { return };
    let written = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&*path)
        .and_then(|mut f| f.write_all(line.as_bytes()));
    if let Err(e) = written {
        log::warn!("timing: {} not written — {e}", path.display());
    }
}

fn line(t: f64, label: &str, ms: Option<f64>, webview: Option<f64>, n: Option<usize>) -> String {
    let round = |v: f64| (v * 10.0).round() / 10.0;
    let mut v = serde_json::json!({ "t": round(t), "label": label });
    if let Some(ms) = ms {
        v["ms"] = round(ms).into();
    }
    if let Some(w) = webview {
        v["webview"] = round(w).into();
    }
    if let Some(n) = n {
        v["n"] = n.into();
    }
    format!("{v}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rule: one line per measure, valid JSON, the fields the bench reads -
    /// and nothing for what was not measured.
    #[test]
    fn a_measure_is_one_json_line_with_only_what_was_measured() {
        let step = line(412.64, "setup.backup", Some(71.33), None, None);
        assert!(step.ends_with('\n') && !step.trim_end().contains('\n'), "one line");
        let v: serde_json::Value = serde_json::from_str(&step).unwrap();
        assert_eq!(v["label"], "setup.backup");
        assert_eq!(v["t"], 412.6, "rounded to a tenth");
        assert_eq!(v["ms"], 71.3);
        assert!(v.get("webview").is_none(), "no webview clock for a backend step");

        let mark: serde_json::Value =
            serde_json::from_str(&line(900.0, "front.cards.cars", None, Some(512.0), None)).unwrap();
        assert!(mark.get("ms").is_none(), "a mark has no duration");
        assert_eq!(mark["webview"], 512.0);

        let count: serde_json::Value = serde_json::from_str(&line(1.0, "cards", None, None, Some(385))).unwrap();
        assert_eq!(count["n"], 385);
    }
}
