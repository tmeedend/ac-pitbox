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
use crate::model::{Beat, Chapter, Node, NodeKind, STORY_FORMAT};

/// The layout the sample story is typed in: lists indented under their key,
/// and every sentence on one line, however long - a condition folded over two
/// lines would read differently in the file and in the editor's field.
fn options() -> serde_saphyr::SerializerOptions {
    serde_saphyr::ser_options! {
        compact_list_indent: false,
        prefer_block_scalars: false,
    }
}

/// `flow/<nn>-<name>.yaml` for `chapter`, read back to be sure it loads. `path`
/// only names the file in the error.
pub fn chapter(path: &str, chapter: &Chapter) -> Result<String, String> {
    let text = serde_saphyr::to_string_with_options(chapter, options()).map_err(|e| e.to_string())?;
    let back = draft(vec![SourceFile::new(path, &text)]);
    match back.errors.into_iter().next() {
        // Not the author's mistake: the model let through a value its own
        // reader refuses. Reported rather than written.
        Some(e) => Err(format!("the written chapter does not read back: {}", e.message)),
        None => Ok(text),
    }
}

/// The files of a new story: the smallest one that validates clean, so the
/// editor opens it with nothing to fix - a chapter whose first node leads to
/// an end. `language` is the story's source language.
pub fn new_story(id: &str, title: &str, language: &str, chapter_title: &str) -> Result<Vec<SourceFile>, String> {
    #[derive(Serialize)]
    struct Manifest<'a> {
        format: u32,
        id: &'a str,
        title: &'a str,
        version: &'a str,
        languages: [&'a str; 1],
        start: Start<'a>,
    }
    #[derive(Serialize)]
    struct Start<'a> {
        node: &'a str,
    }
    let manifest = Manifest {
        format: STORY_FORMAT,
        id,
        title,
        version: "1",
        languages: [language],
        start: Start { node: "start" },
    };
    let story = serde_saphyr::to_string_with_options(&manifest, options()).map_err(|e| e.to_string())?;
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

    fn sample_chapters() -> Vec<Chapter> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("sample");
        let d = draft(read_dir(&root).expect("the sample reads"));
        assert!(d.errors.is_empty(), "the sample parses: {:?}", d.errors);
        d.chapters
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
