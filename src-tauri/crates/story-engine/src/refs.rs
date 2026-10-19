//! The story's own names - characters, flags, counters, relation steps -
//! where they are cited, and renaming them (SPEC-story-mode, Create Your
//! Own: renaming a name renames every sentence that cites it).
//!
//! Citations are found in the **text** of the files, not in the parsed story,
//! for two reasons. A rename must change the cited word and nothing else in
//! the files the editor does not write yet (content, events): their layout
//! and comments stay as typed. And the Ink scripts of the content (Dialogue
//! text) cite the same names inside `check("…")` and `effect("…")`, which the
//! engine does not parse. The editor lists what a rename will change with the
//! same scan, so the list and the rename can never disagree.
//!
//! The declarations themselves (`world/`, the scale in `story.yaml`) are
//! edited as data and written back in canonical form, like any file the
//! editor writes. File and folder names are left alone: `content/friend/` is
//! the author's filing, not a citation.

use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::load::{draft, Draft, SourceFile};
use crate::model::Character;
use crate::write;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NameKind {
    Character,
    Flag,
    Counter,
    /// A step of the relation scale.
    Step,
}

/// A line citing a name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Usage {
    pub file: String,
    /// 1-based.
    pub line: usize,
    /// The flow node the line belongs to, in a chapter file.
    pub node: Option<String>,
    /// The content item the line belongs to, in a `.story` file.
    pub item: Option<String>,
    /// The line, trimmed.
    pub text: String,
}

/// Every line citing one declared name.
#[derive(Debug, Clone, Serialize)]
pub struct NameUsages {
    pub kind: NameKind,
    pub id: String,
    pub usages: Vec<Usage>,
}

/// A name the editor gives: a letter, then letters, digits and `_`. Stricter
/// than what the engine reads (`expr::is_id`): on the right of a comparison
/// a name starting with a digit would read as a number, and `true` as a yes.
pub fn is_name(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !matches!(s, "true" | "false")
}

fn is_id_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

/// Length of the id starting at `from`.
fn id_len(bytes: &[u8], from: usize) -> usize {
    bytes[from..].iter().take_while(|&&b| is_id_byte(b)).count()
}

/// The files that cite names: everything but the declarations.
fn cites(path: &str) -> bool {
    ["flow/", "events/", "content/"].iter().any(|dir| path.starts_with(dir))
}

/// Where `text` cites `id` as a `kind`: the byte range of the id itself.
fn citations(path: &str, text: &str, kind: NameKind, id: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    match kind {
        NameKind::Flag => prefixed(text, "flag.", id, &mut out),
        NameKind::Counter => prefixed(text, "counter.", id, &mut out),
        NameKind::Character => {
            prefixed(text, "rel.", id, &mut out);
            if path.ends_with(".story") {
                senders(text, id, &mut out);
            }
        }
        NameKind::Step => steps(text, id, &mut out),
    }
    out.sort_by_key(|r| r.start);
    out
}

/// Starts of `prefix` that begin a variable: not inside a longer word or
/// path (`event.flag.x` is an event's fact, not a flag).
fn variable_starts<'a>(text: &'a str, prefix: &'a str) -> impl Iterator<Item = usize> + 'a {
    let bytes = text.as_bytes();
    text.match_indices(prefix)
        .map(|(i, _)| i)
        .filter(move |&i| i == 0 || !(is_id_byte(bytes[i - 1]) || bytes[i - 1] == b'.'))
}

/// `flag.<id>`, `counter.<id>`, `rel.<id>`.
fn prefixed(text: &str, prefix: &str, id: &str, out: &mut Vec<Range<usize>>) {
    let bytes = text.as_bytes();
    for i in variable_starts(text, prefix) {
        let start = i + prefix.len();
        let end = start + id_len(bytes, start);
        if &text[start..end] == id {
            out.push(start..end);
        }
    }
}

/// A step is cited as the value of a relation: `rel.<who> <op> <step>`, in a
/// condition (`>=`, `==`…) as in an effect (`=`).
fn steps(text: &str, step: &str, out: &mut Vec<Range<usize>>) {
    let bytes = text.as_bytes();
    let blanks = |mut at: usize| {
        while bytes.get(at).is_some_and(|b| *b == b' ' || *b == b'\t') {
            at += 1;
        }
        at
    };
    for i in variable_starts(text, "rel.") {
        let who = i + "rel.".len();
        let after_who = who + id_len(bytes, who);
        if after_who == who {
            continue;
        }
        let op = blanks(after_who);
        let op_len = bytes[op..].iter().take_while(|b| b"=!<>".contains(b)).count();
        if !matches!(&text[op..op + op_len], "==" | "!=" | "<" | "<=" | ">" | ">=" | "=") {
            continue;
        }
        let start = blanks(op + op_len);
        let end = start + id_len(bytes, start);
        if &text[start..end] == step {
            out.push(start..end);
        }
    }
}

/// One line of a file, with its place in it.
struct Line<'a> {
    start: usize,
    number: usize,
    text: &'a str,
    /// In a `.story` file: inside an item's header (between its two `---`).
    header: bool,
}

fn lines<'a>(path: &str, text: &'a str) -> Vec<Line<'a>> {
    let story = path.ends_with(".story");
    let mut fences = 0;
    let mut start = 0;
    let mut out = Vec::new();
    for (i, raw) in text.split_inclusive('\n').enumerate() {
        let line = raw.trim_end_matches(['\n', '\r']);
        let fence = story && line.trim_end() == "---";
        if fence {
            fences += 1;
        }
        out.push(Line {
            start,
            number: i + 1,
            text: line,
            header: story && !fence && fences % 2 == 1,
        });
        start += raw.len();
    }
    out
}

/// `from: <character>` in the headers of a `.story` file, quoted or not.
fn senders(text: &str, who: &str, out: &mut Vec<Range<usize>>) {
    for line in lines(".story", text).iter().filter(|l| l.header) {
        let Some(rest) = line.text.strip_prefix("from:") else {
            continue;
        };
        let value = rest.trim();
        let unquoted = ["\"", "'"]
            .iter()
            .find_map(|q| value.strip_prefix(q).and_then(|v| v.strip_suffix(q)))
            .unwrap_or(value);
        if unquoted == who {
            // `rest` is a slice of `line.text`, which is one of `text`.
            let start = line.start + "from:".len() + rest.find(who).expect("found above");
            out.push(start..start + who.len());
        }
    }
}

/// `key: value` on a line, the value unquoted.
fn value_of<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let v = line.trim_start().strip_prefix(key)?.trim();
    Some(v.trim_matches(['"', '\'']))
}

/// The lines of `file` citing `id`, one entry per line.
fn usages_in(file: &SourceFile, kind: NameKind, id: &str) -> Vec<Usage> {
    let ranges = citations(&file.path, &file.text, kind, id);
    if ranges.is_empty() {
        return Vec::new();
    }
    let flow = file.path.starts_with("flow/");
    let mut out: Vec<Usage> = Vec::new();
    let (mut node, mut item) = (None, None);
    let mut ranges = ranges.into_iter().peekable();
    for line in lines(&file.path, &file.text) {
        if flow {
            node = value_of(line.text, "- id:").map(String::from).or(node);
        } else if line.header {
            item = value_of(line.text, "id:").map(String::from).or(item);
        }
        let end = line.start + line.text.len();
        let mut cited = false;
        while ranges.peek().is_some_and(|r| r.start <= end) {
            ranges.next();
            cited = true;
        }
        if cited {
            out.push(Usage {
                file: file.path.clone(),
                line: line.number,
                node: node.clone(),
                item: item.clone(),
                text: line.text.trim().to_string(),
            });
        }
    }
    out
}

/// Where each declared name is cited, for every name the story declares.
pub fn all_usages(files: &[SourceFile]) -> Vec<NameUsages> {
    let d = draft(files.to_vec());
    let mut names: Vec<(NameKind, String)> = Vec::new();
    names.extend(
        d.characters
            .characters
            .iter()
            .map(|c| (NameKind::Character, c.id.clone())),
    );
    names.extend(d.variables.flags.iter().map(|f| (NameKind::Flag, f.clone())));
    names.extend(d.variables.counters.keys().map(|c| (NameKind::Counter, c.clone())));
    if let Some(m) = &d.manifest {
        names.extend(m.settings.relations.scale.iter().map(|s| (NameKind::Step, s.clone())));
    }
    names
        .into_iter()
        .map(|(kind, id)| {
            let mut usages: Vec<Usage> = (files.iter().filter(|f| cites(&f.path)))
                .flat_map(|f| usages_in(f, kind, &id))
                .collect();
            usages.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
            NameUsages { kind, id, usages }
        })
        .collect()
}

/// Renames `from` to `to` in its declaration and in every sentence citing it.
/// Returns the files that changed, with their new text; nothing is written
/// here. Refused, with the reason, when `to` is not a name or already taken,
/// when `from` is not declared, when its declaration file does not read, and
/// when the result would make a file unreadable that read before.
pub fn rename(files: &[SourceFile], kind: NameKind, from: &str, to: &str) -> Result<Vec<SourceFile>, String> {
    if !is_name(to) {
        return Err(format!(
            "`{to}` is not a valid name (a letter, then letters, digits and `_`)"
        ));
    }
    if from == to {
        return Ok(Vec::new());
    }
    let before = draft(files.to_vec());
    let mut changed = rename_declaration(&before, kind, from, to)?;
    for file in files.iter().filter(|f| cites(&f.path)) {
        let ranges = citations(&file.path, &file.text, kind, from);
        if ranges.is_empty() {
            continue;
        }
        let mut text = file.text.clone();
        for r in ranges.into_iter().rev() {
            text.replace_range(r, to);
        }
        changed.push(SourceFile::new(&file.path, &text));
    }
    // Read back as a whole: a file that read before must still read.
    let mut after: Vec<SourceFile> = (files.iter())
        .filter(|f| !changed.iter().any(|c| c.path == f.path))
        .cloned()
        .collect();
    after.extend(changed.iter().cloned());
    if let Some(e) = (draft(after).errors.into_iter()).find(|e| !before.errors.iter().any(|b| b.file == e.file)) {
        return Err(format!("the rename would break {}: {}", e.file, e.message));
    }
    Ok(changed)
}

/// The declaration files of `from`, renamed and written back in canonical
/// form: the world file declaring it, or for a step the scale in
/// `story.yaml` and the characters starting on it.
fn rename_declaration(before: &Draft, kind: NameKind, from: &str, to: &str) -> Result<Vec<SourceFile>, String> {
    let readable = |path: &str| match before.errors.iter().find(|e| e.file == path) {
        Some(e) => Err(format!("{path} does not read, fix it first: {}", e.message)),
        None => Ok(()),
    };
    let check = |declared: bool, taken: bool| match (declared, taken) {
        (false, _) => Err(format!("`{from}` is not declared")),
        (_, true) => Err(format!("`{to}` already exists")),
        _ => Ok(()),
    };
    let mut changed = Vec::new();
    match kind {
        NameKind::Character => {
            readable("world/characters.yaml")?;
            let mut c = before.characters.clone();
            let has = |id: &str| c.characters.iter().any(|x| x.id == id);
            check(has(from), has(to))?;
            for x in c.characters.iter_mut().filter(|x| x.id == from) {
                x.id = to.into();
            }
            changed.push(SourceFile::new("world/characters.yaml", &write::characters(&c)?));
        }
        NameKind::Flag | NameKind::Counter => {
            readable("world/variables.yaml")?;
            let mut v = before.variables.clone();
            let declared = match kind {
                NameKind::Flag => v.flags.iter().any(|f| f == from),
                _ => v.counters.contains_key(from),
            };
            // A flag and a counter of the same name read apart (`flag.x`,
            // `counter.x`) but the validator warns about them: not made here.
            check(declared, v.flags.iter().any(|f| f == to) || v.counters.contains_key(to))?;
            if kind == NameKind::Flag {
                for f in v.flags.iter_mut().filter(|f| *f == from) {
                    *f = to.into();
                }
            } else if let Some(start) = v.counters.remove(from) {
                v.counters.insert(to.into(), start);
            }
            changed.push(SourceFile::new("world/variables.yaml", &write::variables(&v)?));
        }
        NameKind::Step => {
            readable("story.yaml")?;
            let mut m = before.manifest.clone().ok_or("story.yaml is missing")?;
            let scale = &mut m.settings.relations;
            check(scale.rank(from).is_some(), scale.rank(to).is_some())?;
            for s in scale.scale.iter_mut().filter(|s| *s == from) {
                *s = to.into();
            }
            if scale.start == from {
                scale.start = to.into();
            }
            changed.push(SourceFile::new("story.yaml", &write::manifest(&m)?));
            // A character's starting relation is a step too.
            let mut c = before.characters.clone();
            let starts_there = |x: &Character| x.relation.as_deref() == Some(from);
            if c.characters.iter().any(starts_there) {
                readable("world/characters.yaml")?;
                for x in c.characters.iter_mut().filter(|x| starts_there(x)) {
                    x.relation = Some(to.into());
                }
                changed.push(SourceFile::new("world/characters.yaml", &write::characters(&c)?));
            }
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::load::read_dir;
    use crate::validate::validate;
    use crate::Story;

    fn sample() -> Vec<SourceFile> {
        read_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("sample")).expect("the sample reads")
    }

    /// `files` with the renamed ones replaced.
    fn apply(files: &[SourceFile], changed: Vec<SourceFile>) -> Vec<SourceFile> {
        let mut out: Vec<SourceFile> = (files.iter())
            .filter(|f| !changed.iter().any(|c| c.path == f.path))
            .cloned()
            .collect();
        out.extend(changed);
        out
    }

    fn usages(files: &[SourceFile], kind: NameKind, id: &str) -> Vec<Usage> {
        (all_usages(files).into_iter())
            .find(|n| n.kind == kind && n.id == id)
            .map(|n| n.usages)
            .unwrap_or_default()
    }

    fn file<'a>(files: &'a [SourceFile], path: &str) -> &'a str {
        &files.iter().find(|f| f.path == path).expect(path).text
    }

    // What the editor lists under a name is every line citing it, with the
    // node or content item it belongs to - so the author can go there.
    #[test]
    fn the_usages_of_a_name_are_the_lines_citing_it() {
        let found = usages(&sample(), NameKind::Counter, "garage_visits");
        let places: Vec<_> = (found.iter())
            .map(|u| (u.file.as_str(), u.node.as_deref(), u.item.as_deref()))
            .collect();
        assert_eq!(
            places,
            [
                ("content/rival/posts.story", None, Some("garage_remark")),
                ("flow/01-first-race.yaml", Some("count_visit"), None),
                ("flow/01-first-race.yaml", Some("enough_visits"), None),
            ],
            "{found:#?}"
        );
        assert_eq!(found[1].text, "beat: { effects: [\"counter.garage_visits += 1\"] }");
        assert_eq!(
            usages(&sample(), NameKind::Character, "sponsor").len(),
            1,
            "a sender counts: `from: sponsor`"
        );
    }

    // Renaming a counter renames every sentence citing it, and the story
    // still validates clean (SPEC-story-mode, Create Your Own).
    #[test]
    fn renaming_a_counter_renames_every_sentence_citing_it() {
        let files = sample();
        let changed = rename(&files, NameKind::Counter, "garage_visits", "visits").expect("renamed");
        let mut paths: Vec<_> = changed.iter().map(|f| f.path.as_str()).collect();
        paths.sort();
        assert_eq!(
            paths,
            [
                "content/rival/posts.story",
                "flow/01-first-race.yaml",
                "world/variables.yaml"
            ],
            "the declaration and each citing file, once"
        );
        let after = apply(&files, changed);
        let story = Story::from_sources(after.clone()).expect("loads");
        assert_eq!(validate(&story), [], "nothing left pointing to the old name");
        assert_eq!(
            usages(&after, NameKind::Counter, "visits").len(),
            3,
            "the three lines followed"
        );
        assert_eq!(story.variables.counters.get("visits"), Some(&0), "starting value kept");
    }

    // In a file the editor does not write, only the cited word changes: a
    // longer name, a fact of another kind, comments and text stay as typed.
    #[test]
    fn a_rename_changes_the_cited_word_and_nothing_else() {
        let text = "---\nid: a\ntype: message\nfrom: friend\nwhen: [\"counter.visits >= 1\", \"counter.visits_total > 2\"]\n---\n# counter.visits in a comment is a citation too\nFriend: from: friend, said the line.\n~ check(\"rel.friend >= warm\")\nevent.counter.visits.x stays\n";
        let mut out = Vec::new();
        prefixed(text, "counter.", "visits", &mut out);
        let words: Vec<_> = out.iter().map(|r| &text[r.start - 8..r.end]).collect();
        assert_eq!(
            words,
            ["counter.visits", "counter.visits"],
            "not `visits_total`, not `event.counter`"
        );
        assert_eq!(out.len(), 2);

        let who = citations("content/a.story", text, NameKind::Character, "friend");
        let mut renamed = text.to_string();
        for r in who.into_iter().rev() {
            renamed.replace_range(r, "pal");
        }
        assert!(renamed.contains("from: pal\n"), "the header's sender: {renamed}");
        assert!(
            renamed.contains("check(\"rel.pal >= warm\")"),
            "a sentence in the script"
        );
        assert!(
            renamed.contains("Friend: from: friend, said the line."),
            "a line of dialogue is not a header: {renamed}"
        );
    }

    // A relation step is renamed on the scale, in the characters starting
    // on it, and where a sentence compares or sets a relation to it.
    #[test]
    fn renaming_a_step_follows_the_scale_the_characters_and_the_sentences() {
        let files = sample();
        let changed = rename(&files, NameKind::Step, "warm", "fond").expect("renamed");
        let after = apply(&files, changed);
        let story = Story::from_sources(after.clone()).expect("loads");
        assert_eq!(validate(&story), [], "clean after the rename");
        let scale = &story.settings().relations.scale;
        assert_eq!(scale, &["enemy", "cold", "neutral", "fond", "close"], "the scale");
        assert_eq!(story.character("parent").unwrap().relation.as_deref(), Some("fond"));
        assert!(file(&after, "content/pools/fail.story").contains("rel.friend >= fond"));
        assert!(
            file(&after, "content/pools/fail.story").contains("rel.rival <= cold"),
            "another step untouched"
        );
    }

    #[test]
    fn renaming_a_character_follows_its_relation_and_its_messages() {
        let files = sample();
        let after = apply(&files, rename(&files, NameKind::Character, "friend", "pal").unwrap());
        let story = Story::from_sources(after.clone()).expect("loads");
        assert_eq!(validate(&story), [], "clean after the rename");
        assert_eq!(story.content_item("invite_01").unwrap().from.as_deref(), Some("pal"));
        assert!(story.character("friend").is_none() && story.character("pal").is_some());
    }

    #[test]
    fn a_rename_is_refused_with_its_reason() {
        let files = sample();
        let refused = |kind, from, to| rename(&files, kind, from, to).unwrap_err();
        assert!(refused(NameKind::Flag, "sponsor_met", "2fast").contains("not a valid name"));
        assert!(refused(NameKind::Flag, "sponsor_met", "true").contains("not a valid name"));
        assert!(refused(NameKind::Character, "friend", "rival").contains("already exists"));
        assert!(
            refused(NameKind::Flag, "sponsor_met", "garage_visits").contains("already exists"),
            "a flag does not take a counter's name"
        );
        assert!(refused(NameKind::Counter, "ghost", "x").contains("not declared"));
        let broken = apply(&files, vec![SourceFile::new("world/variables.yaml", "flags: [oops\n")]);
        assert!(rename(&broken, NameKind::Flag, "sponsor_met", "met")
            .unwrap_err()
            .contains("does not read"));
        assert!(
            rename(&files, NameKind::Flag, "sponsor_met", "sponsor_met")
                .unwrap()
                .is_empty(),
            "the same name changes nothing"
        );
    }
}
