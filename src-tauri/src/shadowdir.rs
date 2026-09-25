//! Private copies of the config folder kept by a packaged (MSIX) app.
//!
//! Windows gives every MSIX-packaged app a private layer over `AppData`:
//! `%LOCALAPPDATA%\Packages\<package>\LocalCache\Roaming\…`. A process
//! launched *by* such an app — Pit Box started from the Claude desktop app,
//! a `tauri dev` run from its terminal, a script from its tools — sees that
//! layer merged over the real folder, file by file; a process started any
//! other way sees only the real folder. Measured (2026-09-24): writes to a
//! folder that exists for real go to the real folder, but a folder first
//! created from inside the package lands in the private layer, invisible to
//! everyone else — and so does every new file created in it afterwards.
//!
//! **This is what corrupted `overlay.sqlite` twice** (2026-08-08 and
//! 2026-09-20). SQLite keeps a database in three files, and creates `-wal`
//! and `-shm` anew after every clean close: the main file sat in one layer
//! while the WAL sat in the other, so a Pit Box launched inside the package
//! and one launched outside each checkpointed a different WAL into the same
//! database. Both corrupted files carry the signature: a WAL describing an
//! older, smaller database than the main file it is replayed onto.
//!
//! Pit Box cannot opt out of a virtualization that belongs to its parent
//! process. What it can do is notice it: a private copy of its folder under
//! any package means two diverging worlds exist, whichever side it runs on.

use std::path::{Path, PathBuf};

/// The private copies of `identifier` (Roaming and Local) held by any MSIX
/// package under `packages` (`%LOCALAPPDATA%\Packages`). Empty on a clean
/// install, which is the normal case.
pub fn find(packages: &Path, identifier: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(packages) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for package in entries.filter_map(|e| e.ok()).map(|e| e.path()) {
        for layer in ["Roaming", "Local"] {
            let shadow = package.join("LocalCache").join(layer).join(identifier);
            if shadow.is_dir() {
                found.push(shadow);
            }
        }
    }
    found.sort();
    found
}

/// Logs every private copy of the config folder. Called once at startup,
/// before the database opens: the log is the only trace a user report can
/// carry of a split that no screen shows.
pub fn warn_at_startup(identifier: &str) {
    let Some(local) = std::env::var_os("LOCALAPPDATA") else {
        return;
    };
    for shadow in find(&Path::new(&local).join("Packages"), identifier) {
        log::warn!(
            "config folder shadowed by a packaged app: {} — Pit Box launched from inside and \
             outside that app sees different files, which corrupts overlay.sqlite",
            shadow.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Protected rule: a private copy of the config folder under any MSIX
    /// package is found, in either layer, and nothing else is — the real
    /// cause of the two `overlay.sqlite` corruptions (module header).
    #[test]
    fn finds_private_copies_of_the_config_folder_only() {
        let packages = crate::testutil::temp_dir("shadowdir");
        let roaming = packages
            .join("Some.App_abc")
            .join("LocalCache")
            .join("Roaming")
            .join("com.pitbox.app");
        let local = packages
            .join("Other.App_def")
            .join("LocalCache")
            .join("Local")
            .join("com.pitbox.app");
        std::fs::create_dir_all(&roaming).unwrap();
        std::fs::create_dir_all(&local).unwrap();
        // Another app's own folder is none of our business.
        std::fs::create_dir_all(
            packages
                .join("Some.App_abc")
                .join("LocalCache")
                .join("Roaming")
                .join("other.app"),
        )
        .unwrap();

        let found = find(&packages, "com.pitbox.app");
        assert_eq!(found.len(), 2, "one copy per layer, nothing else: {found:?}");
        assert!(found.contains(&roaming), "the Roaming copy holds the database");
        assert!(found.contains(&local), "the Local copy is reported too");
    }

    /// Protected rule: no `Packages` folder at all is the ordinary case, not
    /// an error.
    #[test]
    fn a_missing_packages_folder_finds_nothing() {
        let dir = crate::testutil::temp_dir("shadowdir-none");
        assert!(find(&dir.join("Packages"), "com.pitbox.app").is_empty());
    }
}
