//! Test-only helpers, compiled out of every build but `cargo test`.
//!
//! Mirrors the application's own `testutil`, minus its `uuid` dependency —
//! this crate has no use for one outside tests, and a process id plus a
//! counter is enough to keep two concurrent test binaries apart.
//!
//! The guard exists for the reason the application's does: a trailing
//! `remove_dir_all` never runs when an assertion fails, so every red test used
//! to leak a directory in `%TEMP%`. Cleaning up on `Drop` covers the failing
//! runs too.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// A unique temp directory, deleted when it goes out of scope. Derefs to
/// `Path`, so `base.join(…)` and `&base` work as they would on a `PathBuf`.
pub struct TempDir(PathBuf);

impl std::ops::Deref for TempDir {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Best effort: a directory we fail to remove must never turn a green
        // test red, and Windows can hold a handle open briefly after the last
        // close (antivirus, indexer).
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

static NEXT: AtomicU32 = AtomicU32::new(0);

/// Creates `%TEMP%/kn5-gltf-<tag>-<pid>-<n>` and returns its guard. `tag`
/// names the test family, so anything left behind by a killed process stays
/// traceable to the test that made it.
pub fn temp_dir(tag: &str) -> TempDir {
    let serial = NEXT.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("kn5-gltf-{tag}-{}-{serial}", std::process::id()));
    std::fs::create_dir_all(&path).expect("create temp directory");
    TempDir(path)
}
