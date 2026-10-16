use std::process::Command;

fn main() {
    stamp_build();
    tauri_build::build()
}

/// What `version_guard.rs` compares: the date of the code being built (its
/// commit, Unix seconds), and a label to name it with ("0.9.0 (78fdb92,
/// 2026-10-11)"). The commit date and not the version number: a development
/// build and the release it follows carry the same number until the next
/// bump, and they are exactly what must be told apart.
///
/// Without git (a source tarball), the date is 0 and the guard stands aside:
/// a build that cannot date itself refuses nothing and stamps nothing.
fn stamp_build() {
    let git = |args: &[&str]| -> Option<String> {
        let out = Command::new("git").args(args).output().ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let info = git(&["log", "-1", "--format=%ct %h %cs"]).unwrap_or_default();
    let mut parts = info.split(' ');
    let time: i64 = parts.next().and_then(|t| t.parse().ok()).unwrap_or(0);
    let (sha, date) = (parts.next().unwrap_or("?"), parts.next().unwrap_or("?"));
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    println!("cargo:rustc-env=PITBOX_BUILD_TIME={time}");
    println!("cargo:rustc-env=PITBOX_BUILD_LABEL={version} ({sha}, {date})");
    // Built again when the commit moves - HEAD itself (a checkout) or the
    // branch it points to (a commit). Paths as git gives them, relative to
    // this crate.
    if let Some(head) = git(&["rev-parse", "--git-path", "HEAD"]) {
        println!("cargo:rerun-if-changed={head}");
    }
    if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"]) {
        if let Some(path) = git(&["rev-parse", "--git-path", &branch]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
}
