//! Test-only helpers, compiled out of release builds.
//!
//! Almost every backend test builds a throwaway Assetto Corsa tree on the real
//! filesystem: mods manipulate junctions, hardlinks and whole directories, and
//! a mocked filesystem would prove nothing about them.
//!
//! Those trees used to be cleaned up by a trailing `remove_dir_all` call, which
//! never ran when an assertion failed — every red test leaked a directory in
//! `%TEMP%`. `temp_dir` returns a guard that cleans up on drop instead, so the
//! failing runs are cleaned up too.

use std::path::{Path, PathBuf};

use uuid::Uuid;

/// A unique temp directory, deleted when it goes out of scope.
///
/// Derefs to `Path`, so it drops in wherever a plain `PathBuf` used to sit:
/// `base.join("library")`, `&base`, `Path`-taking arguments. Only an owned
/// `PathBuf` needs spelling out, via `to_path_buf()`.
pub struct TempDir(PathBuf);

impl TempDir {
    #[allow(dead_code)]
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl std::ops::Deref for TempDir {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for TempDir {
    fn as_ref(&self) -> &Path {
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

/// Creates `%TEMP%/pitbox-<tag>-<uuid>` and returns its guard. `tag` names the
/// test family, so anything left behind by a killed process stays traceable to
/// the test that made it.
pub fn temp_dir(tag: &str) -> TempDir {
    let path = std::env::temp_dir().join(format!("pitbox-{tag}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&path).expect("create temp directory");
    TempDir(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point of the guard: a failing assertion used to leak its
    /// directory, because the trailing `remove_dir_all` never ran. (This test
    /// prints a panic message of its own — that is the panic being caught.)
    #[test]
    fn cleans_up_even_when_the_test_panics() {
        let (tx, rx) = std::sync::mpsc::channel();
        let outcome = std::panic::catch_unwind(move || {
            let base = temp_dir("droptest");
            std::fs::write(base.join("file.txt"), b"x").unwrap();
            tx.send(base.to_path_buf()).unwrap();
            panic!("simulated test failure");
        });

        assert!(outcome.is_err(), "the closure is expected to panic");
        let leaked = rx.recv().expect("path sent before the panic");
        assert!(!leaked.exists(), "the guard must clean up while the stack unwinds");
    }
}
