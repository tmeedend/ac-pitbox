//! Writing story files back, for the editor (SPEC-story-mode, Create Your
//! Own). The engine reads what an author typed by hand; the editor writes the
//! same files in one canonical form, so a change made in the editor shows in
//! a diff as the lines it changed.
//!
//! A file the editor writes is rewritten whole: comments and layout typed by
//! hand in that file do not survive. Every write is read back before it is
//! returned, so the editor never leaves a file the engine cannot read.

use serde::Serialize;

use crate::expr::{Criterion, Effect};
use crate::frame::Frame;
use crate::load::{draft, SourceFile};
use crate::model::{
    AdaptiveDifficulty, Beat, Chapter, Characters, ContentFile, DateDisplay, Event, EventType, FailureMode, Manifest,
    Node, NodeKind, Protagonist, RelationScale, Requires, RetryCost, Saves, Session, Settings, Start, Variables,
    STORY_FORMAT,
};
use crate::time::{Clock, Span, StoryDate};

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

/// `events/<id>.yaml` for `event`, read back to be sure it loads. What
/// equals its default is left out, as in `story.yaml`.
pub fn event(path: &str, event: &Event) -> Result<String, String> {
    yaml(path, &EventYaml::of(event))
}

/// A `.story` file: each item's header between two `---` lines, then its
/// text as the author wrote it, trailing blank lines dropped. Read back
/// before it is returned. The text is written even when its Ink has errors:
/// those are the checks', on their lines, and a conversation typed over time
/// must be savable half-written. Never with a line made of `---` alone,
/// though, which would end it and start a header.
pub fn content_file(path: &str, file: &ContentFile) -> Result<String, String> {
    let mut text = String::new();
    for item in &file.items {
        let id = &item.header.id;
        if item.body.lines().any(|l| l.trim_end() == "---") {
            return Err(format!("content `{id}`: a line made of `---` alone would end its text"));
        }
        let header = serde_saphyr::to_string_with_options(&item.header, options()).map_err(|e| e.to_string())?;
        text.push_str("---\n");
        text.push_str(&header);
        if !header.ends_with('\n') {
            text.push('\n');
        }
        text.push_str("---\n");
        let body = item.body.trim_end();
        if !body.is_empty() {
            text.push_str(body);
            text.push('\n');
        }
    }
    let back = draft(vec![SourceFile::new(path, &text)]);
    match back.errors.into_iter().next() {
        Some(e) => Err(format!("the written {path} does not read back: {}", e.message)),
        None => Ok(text),
    }
}

/// An event as written: the id, its name and type, the session, then what
/// success takes and what each outcome does - the order an author reads it.
#[derive(Serialize)]
struct EventYaml<'a> {
    id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    #[serde(rename = "type")]
    kind: EventType,
    #[serde(skip_serializing_if = "Option::is_none")]
    session: Option<&'a Session>,
    #[serde(skip_serializing_if = "<[Criterion]>::is_empty")]
    success: &'a [Criterion],
    #[serde(skip_serializing_if = "Option::is_none")]
    on_success: Option<OnSuccessYaml<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    on_failure: Option<OnFailureYaml<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entry_fee: Option<i64>,
}

#[derive(Serialize)]
struct OnSuccessYaml<'a> {
    #[serde(skip_serializing_if = "<[Effect]>::is_empty")]
    effects: &'a [Effect],
    #[serde(skip_serializing_if = "Option::is_none")]
    advance: Option<&'a Span>,
}

#[derive(Serialize)]
struct OnFailureYaml<'a> {
    #[serde(skip_serializing_if = "<[Effect]>::is_empty")]
    effects: &'a [Effect],
    #[serde(skip_serializing_if = "Option::is_none")]
    pool: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pick: Option<u8>,
}

impl<'a> EventYaml<'a> {
    fn of(e: &'a Event) -> Self {
        let (s, f) = (&e.on_success, &e.on_failure);
        let on_success = (!s.effects.is_empty() || s.advance.is_some()).then_some(OnSuccessYaml {
            effects: &s.effects,
            advance: s.advance.as_ref(),
        });
        let pick = (f.pick != 1).then_some(f.pick);
        let on_failure = (!f.effects.is_empty() || f.pool.is_some() || pick.is_some()).then_some(OnFailureYaml {
            effects: &f.effects,
            pool: f.pool.as_deref(),
            pick,
        });
        EventYaml {
            id: &e.id,
            name: e.name.as_deref(),
            kind: e.kind,
            session: unless(&e.session, &Session::default()),
            success: &e.success,
            on_success,
            on_failure,
            entry_fee: e.entry_fee,
        }
    }
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
    /// Always written: there is no default frame to leave out.
    frame: &'a Frame,
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
                frame: &m.start.frame,
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
/// an end. `language` is the story's source language; `frame` the starting
/// frame the author picked - the editor never picks one for them.
pub fn new_story(
    id: &str,
    title: &str,
    language: &str,
    frame: Frame,
    chapter_title: &str,
) -> Result<Vec<SourceFile>, String> {
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
            frame,
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
  frame: none
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
  frame: none
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
start: { node: start, frame: phone }
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

    // A `.story` file goes through the editor like a chapter: every item of
    // every content file of the sample reads back as it was given, and the
    // file is written the same a second time.
    #[test]
    fn a_written_content_file_reads_back_the_same_and_writes_the_same() {
        let d = sample();
        let mut files: Vec<&str> = d.content.iter().map(|c| c.file.as_str()).collect();
        files.dedup();
        assert!(files.len() >= 5, "the sample's content files: {files:?}");
        for path in files {
            let file = ContentFile {
                items: d.content.iter().filter(|c| c.file == path).map(|c| c.doc()).collect(),
            };
            let text = content_file(path, &file).expect("writes");
            let back = reread(path, &text);
            let again = ContentFile {
                items: back.content.iter().map(|c| c.doc()).collect(),
            };
            assert_eq!(again, file, "{path}: read back the same");
            assert_eq!(content_file(path, &again).unwrap(), text, "{path}: written the same");
        }
    }

    // A line made of `---` alone would end the text and start a header:
    // refused rather than written.
    #[test]
    fn a_text_line_made_of_dashes_is_refused() {
        let d = sample();
        let mut doc = d.content[0].doc();
        doc.body.push_str("\n---\nsurprise");
        let refused = content_file(&d.content[0].file, &ContentFile { items: vec![doc] }).unwrap_err();
        assert!(refused.contains("`---`"), "{refused}");
    }

    // An event goes through the editor like a chapter: read back as it was
    // given, and written the same a second time (SPEC-story-mode, Create
    // Your Own: one canonical form).
    #[test]
    fn a_written_event_reads_back_the_same_and_writes_the_same() {
        let d = sample();
        assert_eq!(d.events.len(), 3, "the sample's events");
        for original in &d.events {
            let text = event(&original.file, original).expect("writes");
            let mut back = reread(&original.file, &text).events;
            let mut back = back.remove(0);
            back.file = original.file.clone();
            assert_eq!(
                format!("{back:?}"),
                format!("{original:?}"),
                "{}: the event read back is the one written",
                original.file
            );
            assert_eq!(
                event(&back.file, &back).unwrap(),
                text,
                "{}: written again",
                original.file
            );
        }
    }

    // The canonical event: defaults left out (`pick: 1`, an empty outcome,
    // an empty session), the keys in the order an author reads them.
    #[test]
    fn a_written_event_leaves_its_defaults_out() {
        let source = "id: e
type: hot_lap
on_failure: { pick: 1 }
session: {}
entry_fee: 20
name: Hot lap
";
        let e = reread("events/e.yaml", source).events.remove(0);
        assert_eq!(
            event("events/e.yaml", &e).unwrap(),
            "id: e
name: Hot lap
type: hot_lap
entry_fee: 20
",
            "nothing but what the author set"
        );
        let source = "id: r
type: race
session: { car: c, track: t, start: last, time: \"09:05\" }
success: [\"position <= 3\"]
on_failure: { pool: p, pick: 2 }
";
        let e = reread("events/r.yaml", source).events.remove(0);
        let expected = "id: r
type: race
session:
  track: t
  car: c
  start: last
  time: 09:05
success:
  - position <= 3
on_failure:
  pool: p
  pick: 2
";
        assert_eq!(
            event("events/r.yaml", &e).unwrap(),
            expected,
            "session keys in their order"
        );
    }

    // "New story" in the editor opens on a story with nothing to fix, whose
    // title is the author's own words - quotes and colons included.
    #[test]
    fn a_new_story_loads_and_validates_clean() {
        let files = new_story("my_story", "A story: \"the first\"", "fr", Frame::None, "Chapter 1").unwrap();
        let story = crate::Story::from_sources(files).expect("the new story loads");
        assert_eq!(crate::validate::validate(&story), [], "nothing to fix");
        assert_eq!(story.manifest.title, "A story: \"the first\"", "the title as typed");
        assert_eq!(story.manifest.languages, ["fr"], "the source language given");
        assert_eq!(story.manifest.start.frame, Frame::None, "the frame the author picked");
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
