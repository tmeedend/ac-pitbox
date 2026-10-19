//! Writing story files back, for the editor (SPEC-story-mode, Create Your
//! Own). The engine reads what an author typed by hand; the editor writes the
//! same files in one canonical form, so a change made in the editor shows in
//! a diff as the lines it changed.
//!
//! A file the editor writes is rewritten whole: comments and layout typed by
//! hand in that file do not survive. Every write is read back before it is
//! returned, so the editor never leaves a file the engine cannot read.

use serde::Serialize;

use crate::load::{draft, SourceFile};
use crate::model::{
    AdaptiveDifficulty, Beat, Chapter, Characters, DateDisplay, FailureMode, Manifest, Node, NodeKind, Protagonist,
    RelationScale, Requires, RetryCost, Saves, Settings, Start, Variables, STORY_FORMAT,
};
use crate::time::{Clock, StoryDate};

/// The layout the sample story is typed in: lists indented under their key,
/// and every sentence on one line, however long - a condition folded over two
/// lines would read differently in the file and in the editor's field.
fn options() -> serde_saphyr::SerializerOptions {
    serde_saphyr::ser_options! {
        compact_list_indent: false,
        prefer_block_scalars: false,
    }
}

/// `value` in canonical form, read back as the file at `path` to be sure it
/// loads: `path` decides how it is read (`load::draft`), so it must be the
/// file's real place in the story.
fn yaml<T: Serialize>(path: &str, value: &T) -> Result<String, String> {
    let text = serde_saphyr::to_string_with_options(value, options()).map_err(|e| e.to_string())?;
    let back = draft(vec![SourceFile::new(path, &text)]);
    match back.errors.into_iter().next() {
        // Not the author's mistake: the model let through a value its own
        // reader refuses. Reported rather than written.
        Some(e) => Err(format!("the written {path} does not read back: {}", e.message)),
        None => Ok(text),
    }
}

/// `flow/<nn>-<name>.yaml` for `chapter`, read back to be sure it loads.
pub fn chapter(path: &str, chapter: &Chapter) -> Result<String, String> {
    yaml(path, chapter)
}

/// `story.yaml`, read back to be sure it loads. Its defaults are left out,
/// group by group (`ManifestYaml`): a story that changed one setting shows
/// that one in its file, not the eight it left alone.
pub fn manifest(manifest: &Manifest) -> Result<String, String> {
    yaml("story.yaml", &ManifestYaml::of(manifest))
}

/// `world/characters.yaml`, read back to be sure it loads.
pub fn characters(characters: &Characters) -> Result<String, String> {
    yaml("world/characters.yaml", characters)
}

/// `world/variables.yaml`, read back to be sure it loads. Counters are
/// written in name order, whatever the order they were typed in.
pub fn variables(variables: &Variables) -> Result<String, String> {
    yaml("world/variables.yaml", variables)
}

/// `story.yaml` as written: what equals its default is left out. A group
/// that differs (`saves`, `relations`) is written whole, so the file never
/// holds half a group.
#[derive(Serialize)]
struct ManifestYaml<'a> {
    format: u32,
    id: &'a str,
    title: &'a str,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    authors: &'a [String],
    version: &'a str,
    languages: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    requires: Option<&'a Requires>,
    start: StartYaml<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    settings: Option<SettingsYaml<'a>>,
}

#[derive(Serialize)]
struct StartYaml<'a> {
    node: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    date: Option<&'a StoryDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    clock: Option<&'a Clock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reputation: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    followers: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    money: Option<i64>,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    cars: &'a [String],
}

#[derive(Serialize)]
struct SettingsYaml<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    date: Option<&'a DateDisplay>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure: Option<&'a FailureMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_attempts: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retry_cost: Option<&'a RetryCost>,
    #[serde(skip_serializing_if = "Option::is_none")]
    adaptive_difficulty: Option<&'a AdaptiveDifficulty>,
    #[serde(skip_serializing_if = "Option::is_none")]
    saves: Option<&'a Saves>,
    #[serde(skip_serializing_if = "Option::is_none")]
    money: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    protagonist: Option<&'a Protagonist>,
    #[serde(skip_serializing_if = "Option::is_none")]
    relations: Option<&'a RelationScale>,
}

/// `value`, unless it is `default`.
fn unless<'a, T: PartialEq>(value: &'a T, default: &T) -> Option<&'a T> {
    (value != default).then_some(value)
}

fn nonzero(n: i64) -> Option<i64> {
    (n != 0).then_some(n)
}

impl<'a> ManifestYaml<'a> {
    fn of(m: &'a Manifest) -> Self {
        let s = &m.settings;
        let d = Settings::default();
        let settings = SettingsYaml {
            date: unless(&s.date, &d.date),
            failure: unless(&s.failure, &d.failure),
            max_attempts: s.max_attempts,
            retry_cost: unless(&s.retry_cost, &d.retry_cost),
            adaptive_difficulty: s.adaptive_difficulty.as_ref(),
            saves: unless(&s.saves, &d.saves),
            money: s.money.then_some(true),
            protagonist: unless(&s.protagonist, &d.protagonist),
            relations: unless(&s.relations, &d.relations),
        };
        ManifestYaml {
            format: m.format,
            id: &m.id,
            title: &m.title,
            authors: &m.authors,
            version: &m.version,
            languages: &m.languages,
            requires: unless(&m.requires, &Requires::default()),
            start: StartYaml {
                node: &m.start.node,
                date: unless(&m.start.date, &StoryDate::RELATIVE_START),
                clock: unless(&m.start.clock, &Clock::default()),
                reputation: nonzero(m.start.reputation),
                followers: nonzero(m.start.followers),
                money: nonzero(m.start.money),
                cars: &m.start.cars,
            },
            settings: (s != &d).then_some(settings),
        }
    }
}

/// The files of a new story: the smallest one that validates clean, so the
/// editor opens it with nothing to fix - a chapter whose first node leads to
/// an end. `language` is the story's source language.
pub fn new_story(id: &str, title: &str, language: &str, chapter_title: &str) -> Result<Vec<SourceFile>, String> {
    let story = manifest(&Manifest {
        format: STORY_FORMAT,
        id: id.into(),
        title: title.into(),
        authors: Vec::new(),
        version: "1".into(),
        languages: vec![language.into()],
        requires: Requires::default(),
        start: Start {
            node: "start".into(),
            date: StoryDate::RELATIVE_START,
            clock: Clock::default(),
            reputation: 0,
            followers: 0,
            money: 0,
            cars: Vec::new(),
        },
        settings: Settings::default(),
    })?;
    let first = Chapter {
        id: "ch01".into(),
        title: Some(chapter_title.into()),
        nodes: vec![
            Node {
                id: "start".into(),
                next: None,
                kind: NodeKind::Beat(Beat::default()),
            },
            Node {
                id: "end".into(),
                next: None,
                kind: NodeKind::End { ending: "end".into() },
            },
        ],
        file: FIRST_CHAPTER.into(),
    };
    let flow = chapter(FIRST_CHAPTER, &first)?;
    Ok(vec![
        SourceFile::new("story.yaml", &story),
        SourceFile::new(FIRST_CHAPTER, &flow),
    ])
}

const FIRST_CHAPTER: &str = "flow/01-chapter.yaml";

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::load::read_dir;

    fn sample() -> crate::load::Draft {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("sample");
        let d = draft(read_dir(&root).expect("the sample reads"));
        assert!(d.errors.is_empty(), "the sample parses: {:?}", d.errors);
        d
    }

    fn sample_chapters() -> Vec<Chapter> {
        sample().chapters
    }

    /// `text` read back as `path`, the way the engine reads it.
    fn reread(path: &str, text: &str) -> crate::load::Draft {
        let d = draft(vec![SourceFile::new(path, text)]);
        assert!(d.errors.is_empty(), "{path} reads back: {:?}", d.errors);
        d
    }

    // story.yaml and world/ go through the editor like a chapter: what it
    // writes reads back as what it was given, and a second write changes
    // nothing (SPEC-story-mode, Create Your Own: one canonical form).
    #[test]
    fn written_story_and_world_files_read_back_the_same_and_write_the_same() {
        let d = sample();
        let m = d.manifest.expect("manifest");
        let text = manifest(&m).unwrap();
        let back = reread("story.yaml", &text).manifest.unwrap();
        assert_eq!(format!("{back:?}"), format!("{m:?}"), "story.yaml read back");
        assert_eq!(manifest(&back).unwrap(), text, "story.yaml written again");

        let text = characters(&d.characters).unwrap();
        let back = reread("world/characters.yaml", &text).characters;
        assert_eq!(
            format!("{back:?}"),
            format!("{:?}", d.characters),
            "characters read back"
        );
        assert_eq!(characters(&back).unwrap(), text, "characters written again");

        let text = variables(&d.variables).unwrap();
        let back = reread("world/variables.yaml", &text).variables;
        assert_eq!(format!("{back:?}"), format!("{:?}", d.variables), "variables read back");
        assert_eq!(variables(&back).unwrap(), text, "variables written again");
    }

    // The canonical story.yaml leaves every default out, group by group: a
    // setting the author never touched never appears in a diff.
    #[test]
    fn story_yaml_leaves_its_defaults_out() {
        let source = "format: 1
id: s
title: S
version: \"1\"
languages: [en]
start:
  node: start
  date: 0001-01-01
  clock: \"00:00\"
  money: 0
settings:
  failure: retry
  saves: { autosave: manual }
";
        let m = reread("story.yaml", source).manifest.unwrap();
        let expected = "format: 1
id: s
title: S
version: \"1\"
languages:
  - en
start:
  node: start
settings:
  saves:
    autosave: manual
    reload: true
";
        assert_eq!(manifest(&m).unwrap(), expected, "only the changed group, written whole");
        let plain = reread(
            "story.yaml",
            "format: 1
id: s
title: S
version: \"1\"
languages: [en]
start: { node: start }
settings: {}
",
        );
        assert!(
            !manifest(&plain.manifest.unwrap()).unwrap().contains("settings"),
            "all defaults: no settings key at all"
        );
    }

    // What the editor writes reads back as the same chapter, and writing it
    // again changes nothing: a save in the editor never rewrites lines nobody
    // touched (SPEC-story-mode, File format).
    #[test]
    fn a_written_chapter_reads_back_the_same_and_writes_the_same() {
        for original in sample_chapters() {
            let text = chapter(&original.file, &original).expect("writes");
            let mut back = draft(vec![SourceFile::new(&original.file, &text)]).chapters;
            assert_eq!(back.len(), 1, "{}: one chapter read back", original.file);
            let mut back = back.remove(0);
            back.file = original.file.clone();
            assert_eq!(
                format!("{back:?}"),
                format!("{original:?}"),
                "{}: the chapter read back is the one written",
                original.file
            );
            assert_eq!(
                chapter(&back.file, &back).expect("writes again"),
                text,
                "{}: a second write is identical",
                original.file
            );
        }
    }

    // "New story" in the editor opens on a story with nothing to fix, whose
    // title is the author's own words - quotes and colons included.
    #[test]
    fn a_new_story_loads_and_validates_clean() {
        let files = new_story("my_story", "A story: \"the first\"", "fr", "Chapter 1").unwrap();
        let story = crate::Story::from_sources(files).expect("the new story loads");
        assert_eq!(crate::validate::validate(&story), [], "nothing to fix");
        assert_eq!(story.manifest.title, "A story: \"the first\"", "the title as typed");
        assert_eq!(story.manifest.languages, ["fr"], "the source language given");
    }

    // The canonical form: the node's id, its kind, then the way out; defaults
    // left out. Frozen on one node of each shape, because this is what every
    // story written in the editor will look like in a diff.
    #[test]
    fn the_written_form_is_the_one_an_author_would_type() {
        let source = "id: ch01\ntitle: Chapter 1\nnodes:\n  - id: a\n    next: c\n    beat: { effects: [\"reputation += 2\"], advance: 1 month }\n  - id: b\n    message: invite\n    wait_reply: false\n  - id: c\n    branch:\n      - if: [\"money >= 100\"]\n        goto: b\n    otherwise: d\n  - id: d\n    end: done\n";
        let parsed = draft(vec![SourceFile::new("flow/01-one.yaml", source)]);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let text = chapter("flow/01-one.yaml", &parsed.chapters[0]).unwrap();
        let expected = "id: ch01\ntitle: Chapter 1\nnodes:\n  - id: a\n    beat:\n      effects:\n        - reputation += 2\n      advance: 1 month\n    next: c\n  - id: b\n    message: invite\n  - id: c\n    branch:\n      - if:\n          - money >= 100\n        goto: b\n    otherwise: d\n  - id: d\n    end: done\n";
        assert_eq!(text, expected, "kind before `next`, `wait_reply: false` left out");
    }
}
