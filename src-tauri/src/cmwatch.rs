//! Content Manager left behind by a session Pit Box started (SESSION§2.7).
//!
//! After a practice, Content Manager closes on its own a few seconds after the
//! game. After a track day, a race or an online session it may not: its
//! waiting box stays up ("Waiting for extra windows to close" in its log),
//! even with "Skip results" ticked, and while it is there it ignores every
//! new request — the next session simply never starts. Measured on
//! 2026-11-11 (`docs/csp-shot-research.md`).
//!
//! The remedy is limited to what is Pit Box's own: the sessions where there is
//! nothing to read (the in-game thumbnails, GRILLE§7.1), and a Content Manager
//! that was **not running** when Pit Box started it. That one is asked to
//! close — a window message, the cross of its window, never a kill — once the
//! game has gone and it has had its usual time to close by itself. A Content
//! Manager the user had open, or opened from Pit Box's own "Open Content
//! Manager" button, is never touched; nor is one whose main window shows,
//! which means someone is using it.

use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The Content Manager process Pit Box started for a session, when none ran.
static OWNED: Mutex<Option<u32>> = Mutex::new(None);

/// How long Content Manager gets to close by itself once the game has gone:
/// about eight seconds after a practice, measured.
pub const SETTLE: Duration = Duration::from_secs(12);
/// How long it gets to obey the close request.
const CLOSE_WAIT: Duration = Duration::from_secs(10);

/// The running processes of this Content Manager executable.
fn running(cm: &Path) -> Vec<u32> {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let name = cm.file_name().map(|n| n.to_string_lossy().to_lowercase());
    sys.processes()
        .iter()
        .filter(|(_, p)| {
            p.exe().is_some_and(|exe| exe == cm)
                || name
                    .as_deref()
                    .is_some_and(|n| p.name().to_string_lossy().to_lowercase() == n)
        })
        .map(|(pid, _)| pid.as_u32())
        .collect()
}

fn alive(pid: u32) -> bool {
    let mut sys = sysinfo::System::new();
    let pid = sysinfo::Pid::from_u32(pid);
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    sys.process(pid).is_some()
}

/// Starts Content Manager for a session, and remembers it as Pit Box's own
/// when no Content Manager was running before: that process is then the
/// instance, and the session's waiting box is Pit Box's to clear.
pub fn spawn_for_session(cm: &Path, arg: &std::ffi::OsStr) -> std::io::Result<()> {
    let was_running = !running(cm).is_empty();
    let pid = crate::launch::spawn_cm(cm, Some(arg))?;
    let mut owned = OWNED.lock().unwrap_or_else(|e| e.into_inner());
    *owned = owned_after_spawn(*owned, was_running, pid);
    Ok(())
}

/// Which Content Manager is Pit Box's once it has started one: the new
/// process when none was running, else the one it already owned, if any —
/// a request handed to a running instance changes nothing about who owns it.
fn owned_after_spawn(owned: Option<u32>, was_running: bool, spawned: u32) -> Option<u32> {
    if was_running {
        owned
    } else {
        Some(spawned)
    }
}

/// Once the game has gone: lets Pit Box's own Content Manager close by itself,
/// and asks it to if it has not. Returns when it is gone, or when it is clear
/// it will not go — logged, never killed. Blocking: run off the window thread.
pub fn release_owned() {
    let Some(pid) = *OWNED.lock().unwrap_or_else(|e| e.into_inner()) else {
        return;
    };
    let settle_until = Instant::now() + SETTLE;
    while Instant::now() < settle_until {
        if !alive(pid) {
            forget(pid);
            return;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    if crate::launch::game_running() {
        // A new session already: the box belongs to it now.
        return;
    }
    match close_windows(pid) {
        CloseOutcome::InUse => {
            log::info!("cmwatch: Content Manager {pid} has its main window up, left open");
            forget(pid);
            return;
        }
        CloseOutcome::Nothing => {}
        CloseOutcome::Asked(n) => log::info!("cmwatch: asked {n} window(s) of Content Manager {pid} to close"),
    }
    let close_until = Instant::now() + CLOSE_WAIT;
    while Instant::now() < close_until {
        if !alive(pid) {
            forget(pid);
            return;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    log::warn!("cmwatch: Content Manager {pid} still running after the close request, left alone");
    forget(pid);
}

fn forget(pid: u32) {
    let mut owned = OWNED.lock().unwrap_or_else(|e| e.into_inner());
    if *owned == Some(pid) {
        *owned = None;
    }
}

#[derive(Debug, PartialEq)]
enum CloseOutcome {
    /// Its main window shows: someone uses it.
    InUse,
    /// No visible window to close.
    Nothing,
    /// That many windows were asked to close.
    Asked(usize),
}

/// Whether these visible window titles include Content Manager's main window,
/// whose title is its name — the waiting box has another ("Waiting…").
fn main_window_shown(titles: &[String]) -> bool {
    titles.iter().any(|t| t.contains("Content Manager"))
}

#[cfg(windows)]
fn close_windows(pid: u32) -> CloseOutcome {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, WM_CLOSE,
    };

    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: `lparam` is the `Vec` passed below, alive for the call.
        let windows = unsafe { &mut *(lparam.0 as *mut Vec<HWND>) };
        windows.push(hwnd);
        BOOL(1)
    }

    let mut all: Vec<HWND> = Vec::new();
    // SAFETY: the callback only pushes into the vector, which outlives the call.
    if let Err(e) = unsafe { EnumWindows(Some(collect), LPARAM(&mut all as *mut Vec<HWND> as isize)) } {
        log::warn!("cmwatch: windows not listed ({e})");
        return CloseOutcome::Nothing;
    }
    let mut own: Vec<(HWND, String)> = Vec::new();
    for hwnd in all {
        let mut owner = 0u32;
        // SAFETY: plain queries on a window handle just listed.
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut owner)) };
        if owner != pid || !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            continue;
        }
        let mut buf = [0u16; 256];
        let len = unsafe { GetWindowTextW(hwnd, &mut buf) };
        own.push((hwnd, String::from_utf16_lossy(&buf[..len.max(0) as usize])));
    }
    let titles: Vec<String> = own.iter().map(|(_, t)| t.clone()).collect();
    if main_window_shown(&titles) {
        return CloseOutcome::InUse;
    }
    if own.is_empty() {
        return CloseOutcome::Nothing;
    }
    for (hwnd, title) in &own {
        // SAFETY: posting a message to a window of another process is the
        // documented way to ask it to close; nothing is shared.
        if let Err(e) = unsafe { PostMessageW(Some(*hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) } {
            log::warn!("cmwatch: close request to « {title} » not posted ({e})");
        }
    }
    CloseOutcome::Asked(own.len())
}

#[cfg(not(windows))]
fn close_windows(_pid: u32) -> CloseOutcome {
    CloseOutcome::Nothing
}

#[cfg(test)]
mod tests {
    use super::*;

    // Rule (SESSION§2.7): Content Manager is Pit Box's only when Pit Box started
    // it while none was running; a request handed to a running instance never
    // makes it Pit Box's.
    #[test]
    fn content_manager_is_ours_only_when_none_was_running() {
        assert_eq!(owned_after_spawn(None, false, 42), Some(42), "started by Pit Box: ours");
        assert_eq!(owned_after_spawn(None, true, 42), None, "already running: the user's");
        assert_eq!(
            owned_after_spawn(Some(7), true, 42),
            Some(7),
            "a second request to our own instance keeps it ours"
        );
    }

    // Rule: a Content Manager whose main window shows is in use, and is left
    // open; its waiting box alone may be closed.
    #[test]
    fn the_main_window_means_someone_uses_it() {
        assert!(main_window_shown(&["Content Manager".into()]), "main window");
        assert!(!main_window_shown(&["Waiting…".into()]), "the waiting box alone");
        assert!(!main_window_shown(&[]), "no window");
    }
}
