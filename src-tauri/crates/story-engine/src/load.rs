//! Reading a story folder (SPEC-story-mode, File format) into a [`Story`].
//!
//! Two steps, so that tests and the editor can build a story in memory: the
//! folder is read into [`SourceFile`]s (the crate's only I/O), then parsed.
//! Loading stops at what makes the story unreadable - a malformed file, a
//! duplicate id. Everything that is readable but wrong (a reference to
//! nothing, a node nobody reaches) is the validator's (`validate.rs`), which
//! reports all of it at once.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use serde::de::DeserializeOwned;

use crate::model::*;

/// One file of a story folder: its path inside the folder (forward slashes)
/// and its text.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: String,
    pub text: String,
}

impl SourceFile {
    pub fn new(path: &str, text: &str) -> SourceFile {
        SourceFile {
            path: path.into(),
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError {
    pub file: String,
    pub message: String,
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file, self.message)
    }
}

/// A loaded story, with its ids indexed.
#[derive(Debug, Clone)]
pub struct Story {
    pub manifest: Manifest,
    pub variables: Variables,
    pub characters: Vec<Character>,
    /// In file-name order, which is the flow order.
    pub chapters: Vec<Chapter>,
    pub content: Vec<ContentItem>,
    pub events: Vec<Event>,
    nodes: HashMap<NodeId, (usize, usize)>,
    content_index: HashMap<ContentId, usize>,
    event_index: HashMap<EventId, usize>,
}

/// The folders this lot reads. `drivers/`, `catalogs/`, `frames/`,
/// `package/` and `media/` belong to later lots and are left alone.
const READ: [&str; 4] = ["flow", "content", "events", "world"];

/// Reads every file the engine understands from a story folder.
pub fn read_dir(root: &Path) -> Result<Vec<SourceFile>, LoadError> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<SourceFile>) -> Result<(), LoadError> {
        let io = |p: &Path, e: std::io::Error| LoadError {
            file: p.display().to_string(),
            message: e.to_string(),
        };
        for entry in std::fs::read_dir(dir).map_err(|e| io(dir, e))? {
            let path = entry.map_err(|e| io(dir, e))?.path();
            if path.is_dir() {
                walk(root, &path, out)?;
            } else if matches!(path.extension().and_then(|e| e.to_str()), Some("yaml" | "story")) {
                let text = std::fs::read_to_string(&path).map_err(|e| io(&path, e))?;
                let rel = path.strip_prefix(root).unwrap_or(&path);
                let rel = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                out.push(SourceFile { path: rel, text });
            }
        }
        Ok(())
    }
    let manifest = root.join("story.yaml");
    let text = std::fs::read_to_string(&manifest).map_err(|e| LoadError {
        file: manifest.display().to_string(),
        message: e.to_string(),
    })?;
    let mut out = vec![SourceFile {
        path: "story.yaml".into(),
        text,
    }];
    for dir in READ {
        let dir = root.join(dir);
        if dir.is_dir() {
            walk(root, &dir, &mut out)?;
        }
    }
    Ok(out)
}

fn parse_yaml<T: DeserializeOwned>(file: &SourceFile) -> Result<T, LoadError> {
    serde_saphyr::from_str(&file.text).map_err(|e| LoadError {
        file: file.path.clone(),
        message: e.to_string(),
    })
}

/// Splits a `.story` file into its items. Each item is a YAML header between
/// two `---` lines, followed by its text up to the next `---`:
///
/// ```text
/// ---
/// id: invite_01
/// type: message
/// from: friend
/// ---
/// The text, opaque to the engine.
/// ```
///
/// Hence the one constraint on the text: no line made of `---` alone. The
/// text is Ink (SPEC-story-mode, Dialogue text), and the subset of it a story
/// may use has none.
fn parse_story_file(file: &SourceFile) -> Result<Vec<ContentItem>, LoadError> {
    let err = |message: String| LoadError {
        file: file.path.clone(),
        message,
    };
    let lines: Vec<&str> = file.text.lines().collect();
    let fences: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim_end() == "---")
        .map(|(i, _)| i)
        .collect();
    if lines
        .iter()
        .take(fences.first().copied().unwrap_or(lines.len()))
        .any(|l| !l.trim().is_empty())
    {
        return Err(err(
            "a .story file starts with a `---` line opening the first item's header".into(),
        ));
    }
    let mut items = Vec::new();
    for pair in fences.chunks(2) {
        let [open, close] = pair else {
            return Err(err(format!("line {}: header never closed by `---`", pair[0] + 1)));
        };
        let header = lines[open + 1..*close].join("\n");
        let body_end = fences.iter().find(|&&f| f > *close).copied().unwrap_or(lines.len());
        let body = lines[close + 1..body_end].join("\n").trim_matches('\n').to_string();
        let raw: RawContentHeader =
            serde_saphyr::from_str(&header).map_err(|e| err(format!("item header at line {}: {e}", open + 1)))?;
        items.push(raw.into_item(body, file.path.clone(), open + 1).map_err(err)?);
    }
    if items.is_empty() {
        return Err(err("no content item (a .story file starts with a `---` header)".into()));
    }
    Ok(items)
}

/// Every file of a story parsed on its own, before ids are indexed: what the
/// editor can still show while the story as a whole does not load (a file
/// that does not read, an id used twice). A file that does not read is in
/// `errors` and nowhere else.
#[derive(Debug, Default)]
pub struct Draft {
    pub manifest: Option<Manifest>,
    pub variables: Variables,
    pub characters: Characters,
    /// In file-name order, which is the flow order.
    pub chapters: Vec<Chapter>,
    pub content: Vec<ContentItem>,
    pub events: Vec<Event>,
    pub errors: Vec<LoadError>,
}

/// Parses each file on its own, dispatched by its place in the folder; files
/// outside what this lot reads are left alone.
pub fn draft(mut files: Vec<SourceFile>) -> Draft {
    files.sort_by(|a, b| a.path.cmp(&b.path));
    for file in &mut files {
        // Some Windows editors start a UTF-8 file with a BOM: it would
        // hide the opening `---` of a .story file.
        if let Some(rest) = file.text.strip_prefix('\u{feff}') {
            file.text = rest.to_string();
        }
    }
    let mut p = Draft::default();
    for file in &files {
        let path = file.path.as_str();
        let result = if path == "story.yaml" {
            parse_yaml::<Manifest>(file).map(|m| p.manifest = Some(m))
        } else if path == "world/variables.yaml" {
            parse_yaml(file).map(|v| p.variables = v)
        } else if path == "world/characters.yaml" {
            parse_yaml(file).map(|c| p.characters = c)
        } else if in_dir(path, "flow", ".yaml").is_some_and(|rest| !rest.contains('/')) {
            parse_yaml::<Chapter>(file).map(|mut c| {
                c.file = file.path.clone();
                p.chapters.push(c)
            })
        } else if in_dir(path, "events", ".yaml").is_some_and(|rest| !rest.contains('/')) {
            parse_yaml::<Event>(file).map(|mut e| {
                e.file = file.path.clone();
                p.events.push(e)
            })
        } else if in_dir(path, "content", ".story").is_some() {
            parse_story_file(file).map(|items| p.content.extend(items))
        } else {
            Ok(())
        };
        if let Err(e) = result {
            p.errors.push(e);
        }
    }
    p
}

/// Indexes items by id, in order, reporting every id used twice.
fn index_by_id<'a>(
    what: &str,
    ids: impl Iterator<Item = (&'a str, &'a str)>,
    errors: &mut Vec<LoadError>,
) -> HashMap<String, usize> {
    let mut index = HashMap::new();
    for (i, (id, file)) in ids.enumerate() {
        if index.insert(id.to_string(), i).is_some() {
            errors.push(LoadError {
                file: file.into(),
                message: format!("{what} id `{id}` is used twice"),
            });
        }
    }
    index
}

fn in_dir<'a>(path: &'a str, dir: &str, ext: &str) -> Option<&'a str> {
    path.strip_prefix(dir)?
        .strip_prefix('/')
        .filter(|rest| rest.ends_with(ext))
}

impl Story {
    /// Reads and parses a story folder.
    pub fn load(root: &Path) -> Result<Story, Vec<LoadError>> {
        Story::from_sources(read_dir(root).map_err(|e| vec![e])?)
    }

    /// Parses a story from its files. Collects every error it can rather than
    /// stopping at the first, so that one run shows an author all of them.
    pub fn from_sources(files: Vec<SourceFile>) -> Result<Story, Vec<LoadError>> {
        let Draft {
            manifest,
            variables,
            characters,
            chapters,
            content,
            events,
            mut errors,
        } = draft(files);

        let Some(manifest) = manifest else {
            errors.insert(
                0,
                LoadError {
                    file: "story.yaml".into(),
                    message: "missing or unreadable".into(),
                },
            );
            return Err(errors);
        };
        if manifest.format == 0 || manifest.format > STORY_FORMAT {
            errors.push(LoadError {
                file: "story.yaml".into(),
                message: format!(
                    "story format {} is not readable by this engine (it reads up to {STORY_FORMAT}): update Pit Box",
                    manifest.format
                ),
            });
        }

        index_by_id(
            "chapter",
            chapters.iter().map(|c| (c.id.as_str(), c.file.as_str())),
            &mut errors,
        );
        let positions: Vec<(usize, usize)> = (chapters.iter().enumerate())
            .flat_map(|(ci, c)| (0..c.nodes.len()).map(move |ni| (ci, ni)))
            .collect();
        let all_nodes = (chapters.iter()).flat_map(|c| c.nodes.iter().map(move |n| (n.id.as_str(), c.file.as_str())));
        let nodes = (index_by_id("node", all_nodes, &mut errors).into_iter())
            .map(|(id, i)| (id, positions[i]))
            .collect();
        let content_index = index_by_id(
            "content",
            content.iter().map(|c| (c.id.as_str(), c.file.as_str())),
            &mut errors,
        );
        let event_index = index_by_id(
            "event",
            events.iter().map(|e| (e.id.as_str(), e.file.as_str())),
            &mut errors,
        );

        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(Story {
            manifest,
            variables,
            characters: characters.characters,
            chapters,
            content,
            events,
            nodes,
            content_index,
            event_index,
        })
    }

    pub fn settings(&self) -> &Settings {
        &self.manifest.settings
    }

    pub fn node(&self, id: &str) -> Option<&Node> {
        let &(c, n) = self.nodes.get(id)?;
        Some(&self.chapters[c].nodes[n])
    }

    /// Index of the chapter holding a node, in flow order.
    pub fn chapter_of(&self, node: &str) -> Option<usize> {
        self.nodes.get(node).map(|&(c, _)| c)
    }

    pub fn chapter_rank(&self, chapter: &str) -> Option<usize> {
        self.chapters.iter().position(|c| c.id == chapter)
    }

    /// Where a node leads when it does not name its `next`: the node written
    /// after it, then the first node of the following chapter.
    pub fn fallthrough(&self, node: &str) -> Option<&Node> {
        let &(c, n) = self.nodes.get(node)?;
        if let Some(next) = self.chapters[c].nodes.get(n + 1) {
            return Some(next);
        }
        self.chapters[c + 1..].iter().find_map(|ch| ch.nodes.first())
    }

    /// The node reached after `node` along its normal exit.
    pub fn next_of(&self, node: &Node) -> Option<&Node> {
        match &node.next {
            Some(id) => self.node(id),
            None => self.fallthrough(&node.id),
        }
    }

    pub fn content_item(&self, id: &str) -> Option<&ContentItem> {
        self.content_index.get(id).map(|&i| &self.content[i])
    }

    pub fn event(&self, id: &str) -> Option<&Event> {
        self.event_index.get(id).map(|&i| &self.events[i])
    }

    pub fn character(&self, id: &str) -> Option<&Character> {
        self.characters.iter().find(|c| c.id == id)
    }

    pub fn pool_variants<'a>(
        &'a self,
        pool: &'a str,
    ) -> impl Iterator<Item = (&'a ContentItem, &'a [crate::expr::Condition])> {
        self.content.iter().filter_map(move |item| match &item.placement {
            Placement::Pool { pool: p, when } if p == pool => Some((item, when.as_slice())),
            _ => None,
        })
    }

    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.chapters.iter().flat_map(|c| c.nodes.iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{minimal, with};

    // File format: a `.story` file holds one or more items, each a YAML
    // header between `---` lines followed by its opaque text.
    #[test]
    fn a_story_file_splits_into_headers_and_opaque_bodies() {
        let story = Story::from_sources(with(
            minimal(),
            "content/pools/fail_01.story",
            "---\nid: v1\ntype: message\nfrom: friend\npool: fail_01\n---\nFirst variant.\n\nTwo paragraphs.\n---\nid: v2\ntype: message\npool: fail_01\nwhen: [\"reputation >= 1\"]\n---\nSecond.\n",
        ))
        .expect("loads");
        let v1 = story.content_item("v1").expect("first item");
        assert_eq!(
            v1.body, "First variant.\n\nTwo paragraphs.",
            "body kept as written, outer blank lines trimmed"
        );
        assert_eq!(v1.line, 1);
        let v2 = story.content_item("v2").expect("second item");
        assert_eq!(v2.line, 10, "header line reported for diagnostics");
        assert!(matches!(&v2.placement, Placement::Pool { pool, when } if pool == "fail_01" && when.len() == 1));
        assert_eq!(story.pool_variants("fail_01").count(), 2);
    }

    #[test]
    fn a_file_saved_with_a_bom_still_reads() {
        let story = Story::from_sources(with(
            minimal(),
            "content/a.story",
            "\u{feff}---\nid: a\ntype: mail\n---\nx\n",
        ));
        assert!(story.expect("BOM ignored").content_item("a").is_some());
    }

    #[test]
    fn placement_is_told_by_the_header_keys() {
        let story = Story::from_sources(with(
            minimal(),
            "content/misc.story",
            "---\nid: flow_item\ntype: mail\n---\nx\n---\nid: reactive_item\ntype: article\nwhen: []\ndelay: 2 days\nrepeat: each_time\n---\ny\n",
        ))
        .expect("loads");
        assert!(matches!(
            story.content_item("flow_item").unwrap().placement,
            Placement::Flow
        ));
        let Placement::Reactive {
            delay,
            repeat,
            priority,
            ..
        } = &story.content_item("reactive_item").unwrap().placement
        else {
            panic!("reactive expected");
        };
        assert_eq!(
            (*delay, *repeat, *priority),
            (Some(crate::time::Span::Days(2)), Repeat::EachTime, 0)
        );
    }

    // Unknown keys are refused: a misspelt field must not silently change
    // how the story plays.
    #[test]
    fn a_misspelt_key_is_an_error_on_its_file() {
        let errors = Story::from_sources(with(
            minimal(),
            "events/race_01.yaml",
            "id: race_01\ntype: race\non_falure: { pool: fail_01 }\n",
        ))
        .unwrap_err();
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(errors[0].file, "events/race_01.yaml");
        assert!(errors[0].message.contains("on_falure"), "names the key: {}", errors[0]);
    }

    #[test]
    fn a_node_is_exactly_one_kind() {
        let errors = Story::from_sources(with(
            minimal(),
            "flow/02-two.yaml",
            "id: ch02\nnodes:\n  - id: both\n    message: m\n    event: e\n",
        ))
        .unwrap_err();
        assert!(errors[0].message.contains("exactly one"), "{}", errors[0]);
    }

    #[test]
    fn duplicate_ids_and_newer_formats_are_refused() {
        let files = with(
            minimal(),
            "flow/02-dup.yaml",
            "id: ch02\nnodes:\n  - id: start\n    end: again\n",
        );
        let errors = Story::from_sources(files).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("node id `start` is used twice")),
            "{errors:?}"
        );

        let mut files = minimal();
        files[0].text = files[0].text.replace("format: 1", "format: 2");
        let errors = Story::from_sources(files).unwrap_err();
        assert!(errors[0].message.contains("update Pit Box"), "{errors:?}");
    }

    #[test]
    fn a_node_without_next_falls_through_to_the_next_chapter() {
        let story = Story::from_sources(with(
            with(
                minimal(),
                "flow/01-one.yaml",
                "id: ch01\nnodes:\n  - id: start\n    beat: {}\n  - id: second\n    beat: {}\n",
            ),
            "flow/02-two.yaml",
            "id: ch02\nnodes:\n  - id: third\n    end: done\n",
        ))
        .expect("loads");
        let start = story.node("start").unwrap();
        assert_eq!(story.next_of(start).map(|n| n.id.as_str()), Some("second"));
        let second = story.node("second").unwrap();
        assert_eq!(
            story.next_of(second).map(|n| n.id.as_str()),
            Some("third"),
            "into the next chapter"
        );
        assert_eq!(
            story.fallthrough("third").map(|n| n.id.as_str()),
            None,
            "end of the flow"
        );
    }
}
