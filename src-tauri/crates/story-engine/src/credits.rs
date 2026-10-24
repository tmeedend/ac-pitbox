//! The permission register (SPEC-story-mode, Content: story package and
//! dependencies): who made each file a story shows or plays, under which
//! licence, and whether its author allowed it to travel in a package.
//!
//! **One `credits.yaml` per folder that travels**: `media/credits.yaml` for
//! the files of `media/`, and `frames/skins/<id>/credits.yaml` for a skin's
//! own files - a skin copied to another story keeps its credits. A line
//! names its files by path inside that folder, `*` standing for any part of
//! a name (`icons/*.png`).
//!
//! **What needs a credit** is what the player sees or hears: a picture, a
//! video, a sound or a font. A licence text, a readme or the story's own
//! YAML travel without a line.
//!
//! **What blocks the build** (`Register::blocks`): a file with no line, a
//! line whose permission is neither granted nor needless, a line without
//! its author or its licence, a register that does not read. A line covering
//! nothing, or a file two lines cover, is only a warning.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::validate::{Diagnostic, Severity};

/// A register's file name.
pub const FILE: &str = "credits.yaml";
const MEDIA: &str = "media/";
const MEDIA_CREDITS: &str = "media/credits.yaml";

/// One line of a register.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Credit {
    /// Paths inside the register's folder; `*` stands for any part of a name.
    pub files: Vec<String>,
    /// Where it comes from: a link, "made for this story"...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub licence: Option<String>,
    pub permission: Permission,
    /// What shows the permission: "mail of 2026-03-02", a link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// The author said yes.
    Granted,
    /// Asked, no answer yet.
    Requested,
    NotAsked,
    /// The licence allows it (CC0, CC BY, OFL...), or the author of the story
    /// made it.
    NotNeeded,
}

impl Permission {
    /// Whether a package may carry a file under this permission.
    pub fn lets_travel(self) -> bool {
        matches!(self, Permission::Granted | Permission::NotNeeded)
    }
}

/// Reads a register.
pub fn parse(text: &str) -> Result<Vec<Credit>, String> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_saphyr::from_str(text).map_err(|e| e.to_string())
}

/// Whether `path`, inside the story, is a register.
pub fn is_register(path: &str) -> bool {
    path == MEDIA_CREDITS || skin_folder(path).is_some_and(|dir| path == format!("{dir}{FILE}"))
}

/// `frames/skins/<id>/` when `path` is directly inside a skin's folder.
fn skin_folder(path: &str) -> Option<String> {
    let rest = path.strip_prefix(crate::skin::SKINS)?;
    let (id, file) = rest.split_once('/')?;
    (!id.is_empty() && !file.is_empty() && !file.contains('/')).then(|| format!("{}{id}/", crate::skin::SKINS))
}

/// The register a file needs a line in, if it needs one: a picture, a
/// video, a sound or a font of `media/` or of a skin's folder.
pub fn register_for(path: &str) -> Option<String> {
    let kind = crate::scene::extension(path)?;
    let shown = [
        &crate::scene::IMAGE[..],
        &crate::scene::VIDEO,
        &crate::scene::AUDIO,
        &crate::skin::FONT,
    ]
    .iter()
    .any(|list| list.contains(&kind.as_str()));
    if !shown {
        return None;
    }
    if path.starts_with(MEDIA) {
        Some(MEDIA_CREDITS.into())
    } else {
        skin_folder(path).map(|dir| format!("{dir}{FILE}"))
    }
}

/// Whether `pattern` names `path`, both inside the register's folder: as
/// many parts, each matched with `*` standing for any run of characters.
pub fn matches(pattern: &str, path: &str) -> bool {
    let (p, f): (Vec<&str>, Vec<&str>) = (pattern.split('/').collect(), path.split('/').collect());
    p.len() == f.len() && p.iter().zip(&f).all(|(p, f)| glob(p.as_bytes(), f.as_bytes()))
}

fn glob(p: &[u8], s: &[u8]) -> bool {
    match p.split_first() {
        None => s.is_empty(),
        Some((b'*', rest)) => (0..=s.len()).any(|i| glob(rest, &s[i..])),
        Some((c, rest)) => s.first() == Some(c) && glob(rest, &s[1..]),
    }
}

/// A file that needs a credit, and the line giving it one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Coverage {
    /// Inside the story.
    pub file: String,
    /// The register it belongs in.
    pub register: String,
    /// The first line of that register naming it; `None`, no credit.
    pub line: Option<usize>,
}

/// A story's register as the Package screen shows it, and what the build
/// waits for.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Register {
    /// Every file that needs a credit, in path order.
    pub coverage: Vec<Coverage>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Register {
    /// Whether the build waits for the register.
    pub fn blocks(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }
}

/// What keeps one line from letting its files travel.
fn line_problems(line: &Credit) -> Vec<&'static str> {
    let blank = |field: &Option<String>| field.as_deref().is_none_or(|v| v.trim().is_empty());
    let mut problems = Vec::new();
    if line.files.is_empty() {
        problems.push("names no file");
    }
    if blank(&line.author) {
        problems.push("no author");
    }
    if blank(&line.licence) {
        problems.push("no licence");
    }
    if !line.permission.lets_travel() {
        problems.push("the build waits for its permission");
    }
    problems
}

/// The register of a story whose files are `paths` (inside the story) and
/// whose registers read as `registers`, by path.
pub fn check(paths: &BTreeSet<String>, registers: &BTreeMap<String, Result<Vec<Credit>, String>>) -> Register {
    let mut diagnostics = Vec::new();
    let mut said = |severity, file: &str, message: String| {
        diagnostics.push(Diagnostic {
            severity,
            file: file.into(),
            node: None,
            message,
        })
    };
    let lines = |register: &str| match registers.get(register) {
        Some(Ok(lines)) => lines.as_slice(),
        _ => &[],
    };
    for (register, read) in registers {
        match read {
            Err(e) => said(Severity::Error, register, format!("does not read: {e}")),
            Ok(lines) => {
                for (n, line) in lines.iter().enumerate() {
                    for problem in line_problems(line) {
                        said(
                            Severity::Error,
                            register,
                            format!("line {} ({}): {problem}", n + 1, line.files.join(", ")),
                        );
                    }
                }
            }
        }
    }

    let mut coverage = Vec::new();
    // Which lines named a file, by register, for the lines naming none.
    let mut used: BTreeMap<&str, BTreeSet<usize>> = BTreeMap::new();
    for file in paths {
        let Some(register) = register_for(file) else { continue };
        let folder = register.trim_end_matches(FILE);
        let inside = &file[folder.len()..];
        let naming: Vec<usize> = (lines(&register).iter().enumerate())
            .filter(|(_, l)| l.files.iter().any(|p| matches(p, inside)))
            .map(|(n, _)| n)
            .collect();
        match naming.as_slice() {
            [] if !matches!(registers.get(&register), Some(Err(_))) => {
                said(Severity::Error, file, format!("no credit in {register}"))
            }
            [first, second, ..] => said(
                Severity::Warning,
                file,
                format!(
                    "named by two lines of {register}: line {} counts, not line {}",
                    first + 1,
                    second + 1
                ),
            ),
            _ => {}
        }
        if let Some(r) = registers.get_key_value(&register).map(|(k, _)| k.as_str()) {
            used.entry(r).or_default().extend(&naming);
        }
        coverage.push(Coverage {
            file: file.clone(),
            line: naming.first().copied(),
            register,
        });
    }
    for (register, read) in registers {
        let Ok(lines) = read else { continue };
        for (n, line) in lines.iter().enumerate() {
            if !line.files.is_empty() && !used.get(register.as_str()).is_some_and(|u| u.contains(&n)) {
                said(
                    Severity::Warning,
                    register,
                    format!("line {} ({}) names no file of the story", n + 1, line.files.join(", ")),
                );
            }
        }
    }
    Register { coverage, diagnostics }
}

/// The register of a story whose files are `files`, with their bytes: a
/// story already in memory, or read whole (`package::folder_files`).
pub fn check_files(files: &[(String, Vec<u8>)]) -> Register {
    let paths = files.iter().map(|(p, _)| p.clone()).collect();
    let registers = (files.iter())
        .filter(|(p, _)| is_register(p))
        .map(|(p, b)| {
            let read = std::str::from_utf8(b).map_err(|e| e.to_string()).and_then(parse);
            (p.clone(), read)
        })
        .collect();
    check(&paths, &registers)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(files: &[&str], permission: Permission) -> Credit {
        Credit {
            files: files.iter().map(|f| f.to_string()).collect(),
            source: None,
            author: Some("Someone".into()),
            licence: Some("CC0 1.0".into()),
            permission,
            proof: None,
        }
    }

    fn paths(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|p| p.to_string()).collect()
    }

    fn messages(r: &Register, severity: Severity) -> Vec<String> {
        (r.diagnostics.iter())
            .filter(|d| d.severity == severity)
            .map(|d| format!("{}: {}", d.file, d.message))
            .collect()
    }

    // What the player sees or hears needs a credit, in the register of the
    // folder it travels with; the rest does not (module doc).
    #[test]
    fn a_shown_file_needs_a_line_in_the_register_of_its_folder() {
        assert_eq!(
            register_for("media/scenes/a.webp").as_deref(),
            Some("media/credits.yaml")
        );
        assert_eq!(register_for("media/a/b/c.MP3").as_deref(), Some("media/credits.yaml"));
        assert_eq!(
            register_for("frames/skins/chat/Inter.ttf").as_deref(),
            Some("frames/skins/chat/credits.yaml")
        );
        for none in [
            "media/OFL.txt",
            "media/credits.yaml",
            "frames/skins/chat/skin.yaml",
            "story.yaml",
            "flow/a.png",
        ] {
            assert_eq!(register_for(none), None, "{none}");
        }
        assert!(is_register("media/credits.yaml") && is_register("frames/skins/chat/credits.yaml"));
        assert!(!is_register("media/x/credits.yaml") && !is_register("credits.yaml"));
    }

    #[test]
    fn a_star_stands_for_any_part_of_a_name() {
        assert!(matches("icons/*.png", "icons/mail.png"));
        assert!(matches("*", "a.png"));
        assert!(matches("tap.ogg", "tap.ogg"));
        assert!(matches("garage_*.webp", "garage_day.webp"));
        assert!(
            !matches("icons/*.png", "icons/sub/mail.png"),
            "a star stays inside its part"
        );
        assert!(!matches("*.png", "icons/mail.png"));
        assert!(!matches("tap.ogg", "tap.ogg.bak"));
    }

    // An item without permission blocks the build (SPEC-story-mode,
    // Content: permission register).
    #[test]
    fn the_build_waits_for_every_credit_and_permission() {
        let files = paths(&[
            "media/a.png",
            "media/b.png",
            "media/c.png",
            "media/notes.txt",
            "story.yaml",
        ]);
        let registers = BTreeMap::from([(
            "media/credits.yaml".to_string(),
            Ok(vec![
                line(&["a.png"], Permission::NotNeeded),
                line(&["b.png"], Permission::Requested),
            ]),
        )]);
        let r = check(&files, &registers);
        assert!(r.blocks());
        assert_eq!(
            messages(&r, Severity::Error),
            vec![
                "media/credits.yaml: line 2 (b.png): the build waits for its permission",
                "media/c.png: no credit in media/credits.yaml",
            ]
        );
        assert_eq!(
            (r.coverage.iter().map(|c| (c.file.as_str(), c.line))).collect::<Vec<_>>(),
            vec![
                ("media/a.png", Some(0)),
                ("media/b.png", Some(1)),
                ("media/c.png", None)
            ],
            "the notes need no credit"
        );

        let all = BTreeMap::from([(
            "media/credits.yaml".to_string(),
            Ok(vec![line(&["*.png"], Permission::Granted)]),
        )]);
        assert!(!check(&files, &all).blocks(), "one line can credit many files");
    }

    #[test]
    fn a_line_says_who_and_under_which_licence() {
        let mut bare = line(&["a.png"], Permission::NotNeeded);
        bare.author = None;
        bare.licence = Some(" ".into());
        let r = check(
            &paths(&["media/a.png"]),
            &BTreeMap::from([("media/credits.yaml".to_string(), Ok(vec![bare]))]),
        );
        assert_eq!(
            messages(&r, Severity::Error),
            vec![
                "media/credits.yaml: line 1 (a.png): no author",
                "media/credits.yaml: line 1 (a.png): no licence"
            ]
        );
    }

    #[test]
    fn a_register_that_does_not_read_blocks_without_one_error_per_file() {
        let r = check(
            &paths(&["media/a.png", "media/b.png"]),
            &BTreeMap::from([("media/credits.yaml".to_string(), Err("bad".into()))]),
        );
        assert_eq!(
            messages(&r, Severity::Error),
            vec!["media/credits.yaml: does not read: bad"]
        );
    }

    #[test]
    fn a_line_naming_nothing_or_a_file_named_twice_is_a_warning() {
        let r = check(
            &paths(&["media/a.png"]),
            &BTreeMap::from([(
                "media/credits.yaml".to_string(),
                Ok(vec![
                    line(&["a.png"], Permission::NotNeeded),
                    line(&["*.png"], Permission::NotNeeded),
                    line(&["gone.png"], Permission::NotNeeded),
                ]),
            )]),
        );
        assert!(!r.blocks());
        assert_eq!(
            messages(&r, Severity::Warning),
            vec![
                "media/a.png: named by two lines of media/credits.yaml: line 1 counts, not line 2",
                "media/credits.yaml: line 3 (gone.png) names no file of the story",
            ]
        );
    }

    #[test]
    fn a_skin_carries_its_own_register() {
        let files = paths(&[
            "frames/skins/chat/skin.yaml",
            "frames/skins/chat/bubble.png",
            "media/a.png",
        ]);
        let registers = BTreeMap::from([
            (
                "frames/skins/chat/credits.yaml".to_string(),
                Ok(vec![line(&["bubble.png"], Permission::NotNeeded)]),
            ),
            (
                "media/credits.yaml".to_string(),
                Ok(vec![line(&["a.png"], Permission::NotNeeded)]),
            ),
        ]);
        assert!(check(&files, &registers).diagnostics.is_empty());
    }

    // The sample shows how everything is made: its register is complete.
    #[test]
    fn the_sample_has_a_credit_for_every_file() {
        let r = check_files(&crate::sample::folder_files());
        assert!(r.diagnostics.is_empty(), "{:#?}", r.diagnostics);
        assert!(r.coverage.len() > 10, "the sample's media are all counted");
    }
}
