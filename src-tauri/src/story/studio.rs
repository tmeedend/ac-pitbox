//! Create Your Own, the story editor (`docs/story/SPEC-story-mode.md`,
//! « Create Your Own: the editor »): the stories an author works on, and the
//! files the editor reads and writes in them.
//!
//! A story lives in a folder of the author's choosing - often a private git
//! repository - never in Pit Box's own data. What Pit Box keeps is only the
//! list of those folders, in `app_config_dir/story/studio.json`: nothing in
//! `overlay.sqlite` or `config.json` (CLAUDE.md, « Chantiers longs derrière
//! un drapeau »). Forgetting a story drops it from that list and touches none
//! of its files.
//!
//! The editor writes a file **whole** and at once, synchronously, in the
//! engine's canonical form (`story_engine::write`). Two safeguards come with
//! that: a file is only written when it still holds what the editor last read
//! (`Stamp`), so a change made meanwhile in a text editor is reloaded rather
//! than overwritten; and the engine reads every write back before it reaches
//! the disk.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use story_engine::load::{draft, read_dir, SourceFile};
use story_engine::model::{Chapter, Placement};
use story_engine::validate::{validate, Diagnostic, Severity};
use story_engine::Story;

use crate::errors;

/// The list of story folders, beside Pit Box's other settings.
const STUDIO_FILE: &str = "story/studio.json";

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct StudioFile {
    /// In the order they were added: the hub shows them that way.
    stories: Vec<PathBuf>,
}

fn read_studio(config_dir: &Path) -> StudioFile {
    let path = config_dir.join(STUDIO_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            // Unreadable list: start empty rather than block the hub, but say
            // so - the next write would replace it.
            log::warn!("story studio: {} unreadable, starting empty: {e}", path.display());
            StudioFile::default()
        }),
        Err(_) => StudioFile::default(),
    }
}

fn write_studio(config_dir: &Path, studio: &StudioFile) -> Result<(), String> {
    let path = config_dir.join(STUDIO_FILE);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(studio).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("writing {}: {e}", path.display()))
}

fn add_to_studio(config_dir: &Path, folder: &Path) -> Result<(), String> {
    let mut studio = read_studio(config_dir);
    if !studio.stories.iter().any(|p| p == folder) {
        studio.stories.push(folder.to_path_buf());
        write_studio(config_dir, &studio)?;
    }
    Ok(())
}

/// A story of the hub's list, as far as its folder can be read.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorySummary {
    pub path: PathBuf,
    /// `None` when `story.yaml` does not read.
    pub title: Option<String>,
    pub chapters: usize,
    pub errors: usize,
    pub warnings: usize,
    /// The folder is gone (moved, renamed, an unplugged drive).
    pub missing: bool,
}

pub fn list(config_dir: &Path) -> Vec<StorySummary> {
    read_studio(config_dir)
        .stories
        .into_iter()
        .map(|path| {
            if !path.join("story.yaml").is_file() {
                return StorySummary {
                    path,
                    title: None,
                    chapters: 0,
                    errors: 0,
                    warnings: 0,
                    missing: true,
                };
            }
            let (title, chapters, diagnostics) = match read_dir(&path) {
                Ok(files) => {
                    let diagnostics = check_files(files.clone());
                    let d = draft(files);
                    (d.manifest.map(|m| m.title), d.chapters.len(), diagnostics)
                }
                Err(e) => (None, 0, vec![load_error(e)]),
            };
            let count = |s: Severity| diagnostics.iter().filter(|d| d.severity == s).count();
            StorySummary {
                title,
                chapters,
                errors: count(Severity::Error),
                warnings: count(Severity::Warning),
                missing: false,
                path,
            }
        })
        .collect()
}

/// Adds an existing story folder to the list.
pub fn register(config_dir: &Path, folder: &Path) -> Result<(), String> {
    if !folder.join("story.yaml").is_file() {
        return Err(errors::STORY_NOT_A_STORY.into());
    }
    add_to_studio(config_dir, folder)
}

/// Drops a story from the list. Its files are left as they are.
pub fn forget(config_dir: &Path, folder: &Path) -> Result<(), String> {
    let mut studio = read_studio(config_dir);
    studio.stories.retain(|p| p != folder);
    write_studio(config_dir, &studio)
}

/// Starts a new story in `folder`, which must be empty (or not exist yet):
/// the editor never mixes a story into files it did not write.
pub fn create(
    config_dir: &Path,
    folder: &Path,
    title: &str,
    language: &str,
    chapter_title: &str,
) -> Result<(), String> {
    if folder.exists() {
        let mut entries = std::fs::read_dir(folder).map_err(|e| e.to_string())?;
        if entries.next().is_some() {
            return Err(errors::STORY_FOLDER_NOT_EMPTY.into());
        }
    }
    let files = story_engine::write::new_story(&story_id(folder), title, language, chapter_title)?;
    for file in files {
        let path = folder.join(&file.path);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, file.text).map_err(|e| format!("writing {}: {e}", path.display()))?;
    }
    add_to_studio(config_dir, folder)
}

/// A story's id from its folder name: lower case, letters, digits and `_`.
fn story_id(folder: &Path) -> String {
    let name = folder
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let id: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let id = id.trim_matches('_').to_string();
    if id.is_empty() {
        "story".into()
    } else {
        id
    }
}

/// What a file held when the editor read it: a write is refused if the file
/// no longer holds it.
fn stamp(text: &str) -> String {
    // FNV-1a: a fingerprint, not a security measure.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in text.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// One chapter file, readable or not: an unreadable one is shown with its
/// error, so that one typo in a file does not hide the others.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterFile {
    pub file: String,
    pub stamp: String,
    pub chapter: Option<Chapter>,
    pub error: Option<String>,
}

/// A content item, as the flow screen needs it: something a message node can
/// point to.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentRef {
    pub id: String,
    pub file: String,
    /// `flow`, `reactive` or `pool`: only flow content is sent by a node.
    pub placement: &'static str,
    pub has_replies: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenStory {
    pub path: PathBuf,
    pub title: Option<String>,
    pub start: Option<String>,
    pub chapters: Vec<ChapterFile>,
    pub content: Vec<ContentRef>,
    pub events: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn open(folder: &Path) -> Result<OpenStory, String> {
    let files = read_dir(folder).map_err(|e| e.to_string())?;
    let chapter_files: Vec<SourceFile> = files.iter().filter(|f| is_chapter_path(&f.path)).cloned().collect();
    let diagnostics = check_files(files.clone());
    let d = draft(files);
    let chapters = chapter_files
        .iter()
        .map(|f| {
            let chapter = d.chapters.iter().find(|c| c.file == f.path).cloned();
            let error = (d.errors.iter()).find(|e| e.file == f.path).map(|e| e.message.clone());
            ChapterFile {
                file: f.path.clone(),
                stamp: stamp(&f.text),
                chapter,
                error,
            }
        })
        .collect();
    let content = (d.content.iter())
        .map(|c| ContentRef {
            id: c.id.clone(),
            file: c.file.clone(),
            placement: match c.placement {
                Placement::Flow => "flow",
                Placement::Reactive { .. } => "reactive",
                Placement::Pool { .. } => "pool",
            },
            has_replies: !c.replies.is_empty(),
        })
        .collect();
    Ok(OpenStory {
        path: folder.to_path_buf(),
        title: d.manifest.as_ref().map(|m| m.title.clone()),
        start: d.manifest.as_ref().map(|m| m.start.node.clone()),
        chapters,
        content,
        events: d.events.iter().map(|e| e.id.clone()).collect(),
        diagnostics,
    })
}

/// Everything wrong with the story: what keeps it from loading, or else what
/// the validator finds. The messages are the engine's, in English: the
/// editor is reserved for now, and they are translated when it opens to all
/// (docs/CHANTIERS.md, Story).
pub fn check(folder: &Path) -> Vec<Diagnostic> {
    match read_dir(folder) {
        Ok(files) => check_files(files),
        Err(e) => vec![load_error(e)],
    }
}

/// `check` on files already read: the screen and its diagnostics then
/// describe the same state of the folder, read once.
fn check_files(files: Vec<SourceFile>) -> Vec<Diagnostic> {
    match Story::from_sources(files) {
        Ok(story) => validate(&story),
        Err(errors) => errors.into_iter().map(load_error).collect(),
    }
}

fn load_error(e: story_engine::LoadError) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        file: e.file,
        node: None,
        message: e.message,
    }
}

/// `flow/<name>.yaml`, directly in `flow/`: the only files this lot writes.
/// Anything else - a path climbing out of the story, another folder - is
/// refused, whatever the frontend sends.
fn is_chapter_path(path: &str) -> bool {
    path.strip_prefix("flow/").is_some_and(|name| {
        name.ends_with(".yaml")
            && name.len() > ".yaml".len()
            && !name.contains(['/', '\\', ':'])
            && !name.starts_with('.')
    })
}

/// A chapter as the editor sends it, read here rather than in the command's
/// signature: a sentence the engine cannot read is then reported in the
/// engine's words alone, without Tauri's "invalid args" wrapping, and the
/// author sees what to fix.
pub fn chapter_from_json(value: serde_json::Value) -> Result<Chapter, String> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}

/// Writes one chapter. `expected` is the stamp the editor read the file with,
/// `None` for a new chapter, whose file must not exist yet. Returns the new
/// stamp.
pub fn save_chapter(folder: &Path, file: &str, expected: Option<&str>, chapter: &Chapter) -> Result<String, String> {
    if !is_chapter_path(file) {
        return Err(format!("not a chapter file: {file}"));
    }
    let path = folder.join(file);
    let on_disk = std::fs::read_to_string(&path).ok();
    let unchanged = match (expected, &on_disk) {
        (None, None) => true,
        (Some(expected), Some(text)) => stamp(text) == expected,
        _ => false,
    };
    if !unchanged {
        return Err(errors::STORY_FILE_CHANGED.into());
    }
    let text = story_engine::write::chapter(file, chapter)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, &text).map_err(|e| format!("writing {}: {e}", path.display()))?;
    Ok(stamp(&text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;

    fn chapter_of(folder: &Path, file: &str) -> (Chapter, String) {
        let open = open(folder).expect("opens");
        let c = open
            .chapters
            .into_iter()
            .find(|c| c.file == file)
            .expect("chapter listed");
        (c.chapter.expect("readable"), c.stamp)
    }

    // "New story" opens on a story with nothing to fix, and the hub lists it
    // (SPEC-story-mode, Create Your Own).
    #[test]
    fn a_new_story_is_listed_and_clean() {
        let config = temp_dir("studio-config");
        let base = temp_dir("studio-stories");
        let folder = base.join("My Story");
        create(&config, &folder, "My story", "en", "Chapter 1").expect("created");
        let listed = list(&config);
        assert_eq!(listed.len(), 1, "listed once");
        assert_eq!(listed[0].title.as_deref(), Some("My story"));
        assert_eq!((listed[0].errors, listed[0].warnings), (0, 0), "nothing to fix");
        assert_eq!(story_id(&folder), "my_story", "id from the folder name");
    }

    // The editor never mixes a story into files it did not write.
    #[test]
    fn a_new_story_refuses_a_folder_that_is_not_empty() {
        let config = temp_dir("studio-config");
        let folder = temp_dir("studio-full");
        std::fs::write(folder.join("notes.txt"), "mine").unwrap();
        assert_eq!(
            create(&config, &folder, "S", "en", "C").unwrap_err(),
            errors::STORY_FOLDER_NOT_EMPTY
        );
        assert_eq!(
            std::fs::read_to_string(folder.join("notes.txt")).unwrap(),
            "mine",
            "the folder's own file untouched"
        );
        assert!(list(&config).is_empty(), "not listed");
    }

    // Forgetting a story only drops it from the list (SPEC-story-mode,
    // Create Your Own): its files stay.
    #[test]
    fn forgetting_a_story_keeps_its_files() {
        let config = temp_dir("studio-config");
        let base = temp_dir("studio-story");
        let folder = base.join("s");
        create(&config, &folder, "S", "en", "C").unwrap();
        forget(&config, &folder).unwrap();
        assert!(list(&config).is_empty(), "no longer listed");
        assert!(folder.join("story.yaml").is_file(), "story.yaml still there");
        register(&config, &folder).expect("opens again");
        assert_eq!(list(&config).len(), 1);
        assert_eq!(
            register(&config, &folder.join("flow")).unwrap_err(),
            errors::STORY_NOT_A_STORY,
            "a folder without story.yaml is not a story"
        );
    }

    // A save writes the chapter, which reads back; the diagnostics follow.
    #[test]
    fn a_saved_chapter_reads_back_and_is_checked() {
        let config = temp_dir("studio-config");
        let base = temp_dir("studio-story");
        let folder = base.join("s");
        create(&config, &folder, "S", "en", "C").unwrap();
        let (mut chapter, stamp) = chapter_of(&folder, "flow/01-chapter.yaml");
        chapter.nodes[0].next = Some("nowhere".into());
        let new_stamp = save_chapter(&folder, "flow/01-chapter.yaml", Some(&stamp), &chapter).expect("saved");
        let (back, read_stamp) = chapter_of(&folder, "flow/01-chapter.yaml");
        assert_eq!(read_stamp, new_stamp, "the returned stamp is the file's");
        assert_eq!(back.nodes[0].next.as_deref(), Some("nowhere"));
        let d = check(&folder);
        assert!(
            d.iter()
                .any(|d| d.node.as_deref() == Some("start") && d.message.contains("nowhere")),
            "the broken link is reported on its node: {d:#?}"
        );
    }

    // A file changed outside Pit Box since the editor read it is not
    // overwritten: the editor reloads it instead.
    #[test]
    fn a_chapter_changed_outside_is_never_overwritten() {
        let config = temp_dir("studio-config");
        let base = temp_dir("studio-story");
        let folder = base.join("s");
        create(&config, &folder, "S", "en", "C").unwrap();
        let (chapter, stamp) = chapter_of(&folder, "flow/01-chapter.yaml");
        let path = folder.join("flow/01-chapter.yaml");
        let by_hand = format!("# typed by hand\n{}", std::fs::read_to_string(&path).unwrap());
        std::fs::write(&path, &by_hand).unwrap();
        assert_eq!(
            save_chapter(&folder, "flow/01-chapter.yaml", Some(&stamp), &chapter).unwrap_err(),
            errors::STORY_FILE_CHANGED
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), by_hand, "the hand edit kept");
        assert_eq!(
            save_chapter(&folder, "flow/01-chapter.yaml", None, &chapter).unwrap_err(),
            errors::STORY_FILE_CHANGED,
            "a new chapter never replaces an existing file"
        );
    }

    // A sentence that does not read is refused with the engine's words, the
    // ones the author needs to fix it.
    #[test]
    fn a_chapter_that_does_not_read_says_why_in_the_engine_words() {
        let bad = serde_json::json!({
            "id": "ch01",
            "nodes": [{ "id": "start", "beat": { "effects": ["money +=== 1"] } }]
        });
        let why = chapter_from_json(bad).unwrap_err();
        assert!(why.contains("cannot read effect `money +=== 1`"), "{why}");
        let good = serde_json::json!({ "id": "ch01", "nodes": [{ "id": "start", "end": "done" }] });
        assert!(chapter_from_json(good).is_ok());
    }

    // Only chapter files are written, inside the story.
    #[test]
    fn only_chapter_files_are_written() {
        for bad in [
            "story.yaml",
            "flow/../story.yaml",
            "flow/sub/01.yaml",
            "flow/.yaml",
            "flow/01.txt",
            "flow/..\\x.yaml",
            "events/race.yaml",
        ] {
            assert!(!is_chapter_path(bad), "{bad} refused");
        }
        assert!(is_chapter_path("flow/02-chapter.yaml"));
    }

    // One chapter that does not read hides neither the others nor itself:
    // it is listed with its error.
    #[test]
    fn an_unreadable_chapter_is_listed_with_its_error() {
        let config = temp_dir("studio-config");
        let base = temp_dir("studio-story");
        let folder = base.join("s");
        create(&config, &folder, "S", "en", "C").unwrap();
        std::fs::write(folder.join("flow/02-broken.yaml"), "id: ch02\nnodes: [oops\n").unwrap();
        let open = open(&folder).unwrap();
        assert_eq!(open.chapters.len(), 2, "both chapters listed");
        assert!(open.chapters[0].chapter.is_some(), "the first still reads");
        assert!(open.chapters[1].error.is_some(), "the second carries its error");
        assert!(
            open.diagnostics.iter().any(|d| d.file == "flow/02-broken.yaml"),
            "and the check names it"
        );
    }
}
