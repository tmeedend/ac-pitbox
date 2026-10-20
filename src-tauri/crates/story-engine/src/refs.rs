//! The story's own names - characters, flags, counters, relation steps,
//! events, drivers - where they are cited, and renaming them
//! (SPEC-story-mode, Create Your Own: renaming a name renames every sentence
//! that cites it).
//!
//! Citations are found in the **text** of the files, not in the parsed story,
//! for two reasons. A rename must change the cited word and nothing else in
//! the files the editor does not write yet (content, events): their layout
//! and comments stay as typed. And the Ink scripts of the content (Dialogue
//! text) cite the same names inside `check("…")` and `effect("…")`, which the
//! engine does not parse. The editor lists what a rename will change with the
//! same scan, so the list and the rename can never disagree.
//!
//! The declarations themselves (`world/`, the scale in `story.yaml`, an
//! event's own file) are edited as data and written back in canonical form,
//! like any file the editor writes. File and folder names are left alone:
//! `content/friend/` is the author's filing, not a citation. The one
//! exception is an event's file named after it (`events/<id>.yaml`): the
//! file *is* the event, and keeping the old name would leave anyone reading
//! the folder looking for it under the wrong one.

use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::load::{draft, Draft, SourceFile, ROSTER};
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
    Event,
    /// A driver of the roster (`drivers/roster.yaml`).
    Driver,
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

/// The files that cite names: everything but the declarations - save the
/// roster, which cites characters while it declares drivers.
fn cites(path: &str) -> bool {
    ["flow/", "events/", "content/"].iter().any(|dir| path.starts_with(dir)) || path == ROSTER
}

/// Where `text` cites `id` as a `kind`: the byte range of the id itself.
fn citations(path: &str, text: &str, kind: NameKind, id: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    match kind {
        NameKind::Flag => prefixed(text, "flag.", id, &mut out),
        NameKind::Counter => prefixed(text, "counter.", id, &mut out),
        NameKind::Character => {
            prefixed(text, "rel.", id, &mut out);
            if path == ROSTER {
                keyed(text, "character", id, &mut out);
            }
            if path.ends_with(".story") {
                senders(text, id, &mut out);
                speakers(text, id, &mut out);
                present(text, id, &mut out);
            }
        }
        NameKind::Step => steps(text, id, &mut out),
        NameKind::Event => {
            prefixed(text, "event.", id, &mut out);
            if path.starts_with("flow/") {
                keyed(text, "event", id, &mut out);
                listed(text, &["open", "close"], id, &mut out);
            }
        }
        NameKind::Driver => {
            if path.starts_with("events/") {
                keyed(text, "driver", id, &mut out);
                after_word(text, "ahead_of", id, &mut out);
            }
        }
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

/// Starts of `key:` that begin a YAML key: not inside a longer word
/// (`side_events:`) or a path. Yields where the value after it starts.
fn key_values<'a>(text: &'a str, key: &'a str) -> impl Iterator<Item = usize> + 'a {
    let bytes = text.as_bytes();
    variable_starts(text, key).filter_map(move |i| {
        let colon = i + key.len();
        (bytes.get(colon) == Some(&b':')).then_some(colon + 1)
    })
}

/// `at`, past blanks and an opening quote.
fn skip_blanks_and_quote(bytes: &[u8], mut at: usize) -> usize {
    while bytes.get(at).is_some_and(|b| b" \t\"'".contains(b)) {
        at += 1;
    }
    at
}

/// The id at `at`, past blanks and a quote, when it is `id`.
fn id_at(text: &str, at: usize, id: &str, out: &mut Vec<Range<usize>>) -> usize {
    let bytes = text.as_bytes();
    let start = skip_blanks_and_quote(bytes, at);
    let end = start + id_len(bytes, start);
    if &text[start..end] == id {
        out.push(start..end);
    }
    end
}

/// `<word> <id>`, the word standing alone: a success criterion's driver,
/// `ahead_of rival`.
fn after_word(text: &str, word: &str, id: &str, out: &mut Vec<Range<usize>>) {
    let bytes = text.as_bytes();
    for i in variable_starts(text, word) {
        let at = i + word.len();
        if bytes.get(at).is_some_and(|b| *b == b' ' || *b == b'\t') {
            id_at(text, at, id, out);
        }
    }
}

/// `<key>: <id>`: the event an Event node launches, an opponent's driver, the
/// character a driver is.
fn keyed(text: &str, key: &str, id: &str, out: &mut Vec<Range<usize>>) {
    for value in key_values(text, key) {
        id_at(text, value, id, out);
    }
}

/// The ids listed under `keys`, in either form an author writes a list:
/// `open: [a, b]` on the line, or one `- a` per line below `open:` - the
/// events a Side events node opens or closes, the characters present in a
/// dialogue.
fn listed(text: &str, keys: &[&str], id: &str, out: &mut Vec<Range<usize>>) {
    let bytes = text.as_bytes();
    let all = lines("", text);
    let indent = |l: &Line| l.text.len() - l.text.trim_start().len();
    for &key in keys {
        for value in key_values(text, key) {
            let at = skip_blanks_and_quote(bytes, value);
            if bytes.get(at) == Some(&b'[') {
                let close = text[at..].find(']').map_or(text.len(), |n| at + n);
                let mut item = at + 1;
                while item < close {
                    let end = id_at(text, item, id, out);
                    item = text[end..close].find(',').map_or(close, |n| end + n + 1);
                }
                continue;
            }
            // A block list: the `- ` lines below, deeper than the key.
            let Some(k) = all
                .iter()
                .position(|l| l.start <= value && value <= l.start + l.text.len())
            else {
                continue;
            };
            let depth = indent(&all[k]);
            for line in all[k + 1..].iter().filter(|l| !l.text.trim().is_empty()) {
                let Some(rest) = line.text.trim_start().strip_prefix('-') else {
                    break;
                };
                if indent(line) <= depth {
                    break;
                }
                id_at(text, line.start + line.text.len() - rest.len(), id, out);
            }
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

/// A speaker in the text of a `.story` file: `who: …` at the start of a line,
/// past the markers of a choice or a gather and a line's `{check(…):`
/// condition (SPEC-story-mode, Dialogue text).
fn speakers(text: &str, who: &str, out: &mut Vec<Range<usize>>) {
    for line in lines(".story", text).iter().filter(|l| !l.header) {
        let t = line.text;
        let mut at = t.len() - t.trim_start().len();
        while let Some(c) = t[at..].chars().next() {
            let marker = c == '*' || (c == '-' && !t[at..].starts_with("->"));
            if marker || c == ' ' || c == '\t' {
                at += c.len_utf8();
            } else {
                break;
            }
        }
        if t[at..].starts_with('{') {
            // Past the conditions: the first `:` outside quotes.
            let mut quoted = false;
            let Some(colon) = t[at..].char_indices().find_map(|(i, c)| {
                if c == '"' {
                    quoted = !quoted;
                }
                (c == ':' && !quoted).then_some(i)
            }) else {
                continue;
            };
            at += colon + 1;
            at += t[at..].len() - t[at..].trim_start().len();
        }
        if crate::script::speaker_prefix(&t[at..]).is_some_and(|(name, _)| name == who) {
            out.push(line.start + at..line.start + at + who.len());
        }
    }
}

/// The characters present in a dialogue: `characters:` in an item's header.
fn present(text: &str, who: &str, out: &mut Vec<Range<usize>>) {
    let headers: Vec<Range<usize>> = (lines(".story", text).iter())
        .filter(|l| l.header)
        .map(|l| l.start..l.start + l.text.len())
        .collect();
    let mut found = Vec::new();
    listed(text, &["characters"], who, &mut found);
    out.extend(
        found
            .into_iter()
            .filter(|r| headers.iter().any(|h| h.contains(&r.start))),
    );
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
/// `d` is the draft of `files`, which tells what the story declares.
pub fn all_usages(files: &[SourceFile], d: &Draft) -> Vec<NameUsages> {
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
    names.extend(d.events.iter().map(|e| (NameKind::Event, e.id.clone())));
    names.extend(d.roster.drivers.iter().map(|x| (NameKind::Driver, x.id.clone())));
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

/// What a rename does to the story's files; nothing is written here.
#[derive(Debug, Default)]
pub struct Changes {
    /// The files that changed, with their new text, and the files added.
    pub written: Vec<SourceFile>,
    /// The files removed: an event's file, moved to its new name.
    pub removed: Vec<String>,
}

/// Renames `from` to `to` in its declaration and in every sentence citing it.
/// Refused, with the reason, when `to` is not a name or already taken, when
/// `from` is not declared, when its declaration file does not read, and when
/// the result would make a file unreadable that read before.
pub fn rename(files: &[SourceFile], kind: NameKind, from: &str, to: &str) -> Result<Changes, String> {
    if !is_name(to) {
        return Err(format!(
            "`{to}` is not a valid name (a letter, then letters, digits and `_`)"
        ));
    }
    if from == to {
        return Ok(Changes::default());
    }
    let before = draft(files.to_vec());
    let mut changed = rename_declaration(&before, kind, from, to)?;
    let mut removed = Vec::new();
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
    if kind == NameKind::Event {
        removed.extend(rename_event_file(&before, files, &mut changed, from, to)?);
    }
    // Read back as a whole: a file that read before must still read.
    let mut after: Vec<SourceFile> = (files.iter())
        .filter(|f| !changed.iter().any(|c| c.path == f.path) && !removed.contains(&f.path))
        .cloned()
        .collect();
    after.extend(changed.iter().cloned());
    if let Some(e) = (draft(after).errors.into_iter()).find(|e| !before.errors.iter().any(|b| b.file == e.file)) {
        return Err(format!("the rename would break {}: {}", e.file, e.message));
    }
    Ok(Changes {
        written: changed,
        removed,
    })
}

/// An event's own file, once its citations are renamed: its id changed,
/// written back in canonical form, and moved to `events/<to>.yaml` when it
/// was named after the old id. Returns the file it moved from, if it moved.
fn rename_event_file(
    before: &Draft,
    files: &[SourceFile],
    changed: &mut Vec<SourceFile>,
    from: &str,
    to: &str,
) -> Result<Option<String>, String> {
    let old = (before.events.iter().find(|e| e.id == from))
        .map(|e| e.file.clone())
        .ok_or_else(|| format!("`{from}` is not declared"))?;
    let new = if old == format!("events/{from}.yaml") {
        format!("events/{to}.yaml")
    } else {
        old.clone()
    };
    if new != old && files.iter().any(|f| f.path == new) {
        return Err(format!("{new} already exists"));
    }
    // The text as the citations left it: an event may cite its own facts.
    let text = match changed.iter().position(|f| f.path == old) {
        Some(i) => changed.remove(i).text,
        None => (files.iter().find(|f| f.path == old))
            .map(|f| f.text.clone())
            .unwrap_or_default(),
    };
    let mut event = (draft(vec![SourceFile::new(&old, &text)]).events.pop())
        .ok_or_else(|| format!("{old} does not read, fix it first"))?;
    event.id = to.into();
    changed.push(SourceFile::new(&new, &write::event(&new, &event)?));
    Ok((new != old).then_some(old))
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
        // Its declaration is its own file, rewritten once its citations are
        // (`rename_event_file`).
        NameKind::Event => {
            let has = |id: &str| before.events.iter().any(|e| e.id == id);
            check(has(from), has(to))?;
            if let Some(e) = before.events.iter().find(|e| e.id == from) {
                readable(&e.file)?;
            }
        }
        NameKind::Driver => {
            readable(ROSTER)?;
            let mut r = before.roster.clone();
            let has = |id: &str| r.drivers.iter().any(|x| x.id == id);
            check(has(from), has(to))?;
            for x in r.drivers.iter_mut().filter(|x| x.id == from) {
                x.id = to.into();
            }
            changed.push(SourceFile::new(ROSTER, &write::roster(&r)?));
        }
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

    /// `files` with the renamed ones replaced, and the moved ones removed.
    fn apply(files: &[SourceFile], changes: Changes) -> Vec<SourceFile> {
        let mut out: Vec<SourceFile> = (files.iter())
            .filter(|f| !changes.written.iter().any(|c| c.path == f.path) && !changes.removed.contains(&f.path))
            .cloned()
            .collect();
        out.extend(changes.written);
        out
    }

    fn written(files: Vec<SourceFile>) -> Changes {
        Changes {
            written: files,
            removed: Vec::new(),
        }
    }

    fn usages(files: &[SourceFile], kind: NameKind, id: &str) -> Vec<Usage> {
        (all_usages(files, &draft(files.to_vec())).into_iter())
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
            2,
            "a sender counts, `from: sponsor`, and so does a speaker, `sponsor: …`"
        );
    }

    // Renaming a counter renames every sentence citing it, and the story
    // still validates clean (SPEC-story-mode, Create Your Own).
    #[test]
    fn renaming_a_counter_renames_every_sentence_citing_it() {
        let files = sample();
        let changed = rename(&files, NameKind::Counter, "garage_visits", "visits").expect("renamed");
        let mut paths: Vec<_> = changed.written.iter().map(|f| f.path.as_str()).collect();
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
        let talk = story.content_item("talk_01").unwrap();
        assert_eq!(talk.characters, ["pal"], "the characters present in a dialogue");
        assert!(
            talk.script.lines().iter().all(|l| l.speaker.as_deref() == Some("pal")),
            "every speaker, a choice's body included"
        );
    }

    // A driver is cited by the grids that place them and by the criteria
    // that race against them; the character a driver is follows a rename
    // of that character.
    #[test]
    fn renaming_a_driver_follows_its_grids_and_its_rivals() {
        let files = sample();
        let race = "events/race_02.yaml";
        assert_eq!(
            usages(&files, NameKind::Driver, "rival_driver").len(),
            1,
            "its place on race 02's grid"
        );
        let files = apply(
            &files,
            written(vec![SourceFile::new(
                race,
                &file(&files, race).replace("success: [\"position <= 1\"", "success: [\"ahead_of rival_driver\""),
            )]),
        );
        let changes = rename(&files, NameKind::Driver, "rival_driver", "the_rival").expect("renamed");
        let after = apply(&files, changes);
        let story = Story::from_sources(after.clone()).expect("loads");
        assert_eq!(validate(&story), [], "nothing left pointing to the old id");
        assert!(story.driver("the_rival").is_some() && story.driver("rival_driver").is_none());
        let text = file(&after, race);
        assert!(
            text.contains("{ driver: the_rival, skin: red_livery }"),
            "the grid, as typed: {text}"
        );
        assert!(text.contains("\"ahead_of the_rival\""), "the criterion: {text}");

        let after = apply(&files, rename(&files, NameKind::Character, "rival", "foe").unwrap());
        let story = Story::from_sources(after).expect("loads");
        assert_eq!(validate(&story), [], "clean after the rename");
        assert_eq!(story.driver("rival_driver").unwrap().character.as_deref(), Some("foe"));
    }

    // A speaker is found past a choice's or a gather's markers and a line's
    // condition, never in a choice's own text.
    #[test]
    fn a_speaker_is_cited_wherever_a_line_starts() {
        let text = "---\nid: a\ntype: dialogue\ncharacters:\n  - friend\n---\nfriend: Hi.\n{check(\"rel.friend >= warm\"): friend: Pal.}\n* (yes) [friend: not a line]\n    friend: Great.\n- friend: Bye.\n";
        let mut out = Vec::new();
        speakers(text, "friend", &mut out);
        let cited: Vec<&str> = out.iter().map(|r| &text[r.clone()]).collect();
        assert_eq!(cited.len(), 4, "four lines, not the choice: {out:?}");
        let mut out = Vec::new();
        present(text, "friend", &mut out);
        assert_eq!(out.len(), 1, "the block list of the header");
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
        let broken = apply(
            &files,
            written(vec![SourceFile::new("world/variables.yaml", "flags: [oops\n")]),
        );
        assert!(rename(&broken, NameKind::Flag, "sponsor_met", "met")
            .unwrap_err()
            .contains("does not read"));
        assert!(
            rename(&files, NameKind::Flag, "sponsor_met", "sponsor_met")
                .unwrap()
                .written
                .is_empty(),
            "the same name changes nothing"
        );
    }

    // An event is cited by the node that launches it, by the Side events
    // nodes that open or close it, and by every sentence reading its facts -
    // never by a node or a `goto` that merely shares its id.
    #[test]
    fn the_usages_of_an_event_are_its_nodes_and_its_facts() {
        let places = |id: &str| -> Vec<(String, usize)> {
            (usages(&sample(), NameKind::Event, id).into_iter())
                .map(|u| (u.file, u.line))
                .collect()
        };
        let race: Vec<_> = places("race_01");
        assert_eq!(
            race,
            [
                ("content/pools/fail.story".to_string(), 14),
                ("content/rival/posts.story".to_string(), 5),
                ("flow/01-first-race.yaml".to_string(), 17),
            ],
            "the facts read and the Event node, not `- id: race_01` nor `goto: race_01`"
        );
        assert_eq!(places("track_day_01").len(), 2, "opened and closed by side events");

        let block = "id: ch01\nnodes:\n  - id: s\n    side_events:\n      open:\n        - a\n        - \"b\"\n      close: [c, b]\n";
        let mut out = Vec::new();
        listed(block, &["open", "close"], "b", &mut out);
        assert_eq!(out.len(), 2, "a block list and an inline one: {out:?}");
    }

    // Renaming an event follows its node, its side events and its facts,
    // moves its file to its new name, and leaves the story clean
    // (SPEC-story-mode, Create Your Own).
    #[test]
    fn renaming_an_event_follows_everything_citing_it_and_moves_its_file() {
        let files = sample();
        let changes = rename(&files, NameKind::Event, "track_day_01", "open_day").expect("renamed");
        assert_eq!(changes.removed, ["events/track_day_01.yaml"], "the old file goes");
        let after = apply(&files, changes);
        let story = Story::from_sources(after.clone()).expect("loads");
        assert_eq!(validate(&story), [], "nothing left pointing to the old id");
        assert_eq!(story.event("open_day").unwrap().file, "events/open_day.yaml");
        assert!(file(&after, "flow/02-second-race.yaml").contains("open: [open_day]"));

        let after = apply(&files, rename(&files, NameKind::Event, "race_01", "opener").unwrap());
        let story = Story::from_sources(after.clone()).expect("loads");
        assert_eq!(validate(&story), [], "clean after the rename");
        assert!(
            story.node("race_01").is_some(),
            "the node sharing the event's id keeps its own"
        );
        assert!(file(&after, "content/rival/posts.story").contains("event.opener.succeeded"));
        assert!(
            rename(&files, NameKind::Event, "race_01", "race_02")
                .unwrap_err()
                .contains("already exists"),
            "two events never share an id"
        );
    }
}
