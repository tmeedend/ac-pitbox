//! The frontend's side of `crate::timing`: whether it is on, and its marks.

/// Asked once by `$lib/timing`: off, the page sends no mark at all.
#[tauri::command]
pub fn timing_enabled() -> bool {
    crate::timing::enabled()
}

/// A point reached by the page, `webview` being its own clock
/// (`performance.now()`) when it got there - the IPC delivering the mark
/// later must not shift it.
#[tauri::command]
pub fn timing_mark(label: String, webview: f64) {
    crate::timing::mark(&format!("front.{label}"), Some(webview));
}
