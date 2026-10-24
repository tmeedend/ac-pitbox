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
//! the disk. Every write is recorded in the story's undo history
//! (`history.rs`).

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use story_engine::frame::{Frame, Frames, FRAMES};
use story_engine::load::{draft_dir, read_dir, Draft, SourceFile, ROSTER};
use story_engine::model::{Chapter, Characters, ContentFile, Event, Manifest, Placement, Roster, Variables};
use story_engine::refs::{self, NameKind, NameUsages};
use story_engine::scene::{self, Period, Scenes, Season, Shown, SCENES};
use story_engine::validate::{validate, Diagnostic, Severity};
use story_engine::write;
use story_engine::LoadError;
use story_engine::Story;

use super::history::{self, FileChange, HistoryStatus};
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

/// The author's story folders, in the order they were added: what Story
/// Mode offers besides the sample (`play.rs`).
pub(super) fn folders(config_dir: &Path) -> Vec<PathBuf> {
    read_studio(config_dir).stories
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
            let (title, chapters, diagnostics) = match draft_dir(&path) {
                Ok((_, d)) => {
                    let (title, chapters) = (d.manifest.as_ref().map(|m| m.title.clone()), d.chapters.len());
                    (title, chapters, verdict(d))
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
    frame: Frame,
    chapter_title: &str,
) -> Result<(), String> {
    let files = story_engine::write::new_story(&story_id(folder), title, language, frame, chapter_title)?;
    let files: Vec<(String, Vec<u8>)> = files.into_iter().map(|f| (f.path, f.text.into_bytes())).collect();
    write_new_story(config_dir, folder, &files)
}

/// Copies the sample story Pit Box carries into `folder`, which must be
/// empty, and adds it to the list: an author's own copy to read and change,
/// the way one learns how a story is made. The copy is the author's; the
/// sample Story Mode plays stays the one Pit Box carries.
pub fn copy_sample(config_dir: &Path, folder: &Path) -> Result<(), String> {
    write_new_story(config_dir, folder, &story_engine::sample::folder_files())
}

/// Writes a story's first files into `folder`, which must be empty (or not
/// exist yet), and adds it to the list.
fn write_new_story(config_dir: &Path, folder: &Path, files: &[(String, Vec<u8>)]) -> Result<(), String> {
    if folder.exists() {
        let mut entries = std::fs::read_dir(folder).map_err(|e| e.to_string())?;
        if entries.next().is_some() {
            return Err(errors::STORY_FOLDER_NOT_EMPTY.into());
        }
    }
    for (file, bytes) in files {
        let path = folder.join(file);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, bytes).map_err(|e| format!("writing {}: {e}", path.display()))?;
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
/// no longer holds it. Also the fingerprint of a story's folder in the name
/// of its save (`play.rs`).
pub(super) fn stamp(text: &str) -> String {
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

/// A content item, as the flow and event screens need it: something a
/// message node can point to, or a variant of a failure pool.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentRef {
    pub id: String,
    pub file: String,
    /// `flow`, `reactive` or `pool`: only flow content is sent by a node.
    pub placement: &'static str,
    /// Its content type, as a `.story` header names it.
    pub kind: &'static str,
    /// Its reply labels, in the order written: what a Message node's reply
    /// exits can name.
    pub labels: Vec<String>,
    /// The pool a variant belongs to.
    pub pool: Option<String>,
    pub from: Option<String>,
    /// Its conditions, as written.
    pub when: Vec<String>,
    /// The first line of its text, to recognise it.
    pub first_line: Option<String>,
}

impl ContentRef {
    fn of(c: &story_engine::model::ContentItem) -> Self {
        let (placement, pool, when) = match &c.placement {
            Placement::Flow => ("flow", None, &[][..]),
            Placement::Reactive { when, .. } => ("reactive", None, &when[..]),
            Placement::Pool { pool, when } => ("pool", Some(pool.clone()), &when[..]),
        };
        ContentRef {
            id: c.id.clone(),
            file: c.file.clone(),
            placement,
            kind: c.kind.name(),
            labels: c.script.labels().into_iter().map(String::from).collect(),
            pool,
            from: c.from.clone(),
            when: when.iter().map(ToString::to_string).collect(),
            first_line: (c.body.lines().map(str::trim))
                .find(|l| !l.is_empty())
                .map(String::from),
        }
    }
}

/// One of the story-wide files the editor writes (`story.yaml`, `world/`, the
/// roster), readable or not. A world file or a roster the story does not have
/// yet reads as empty, with an empty stamp: the first save creates it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditedFile<T> {
    pub file: String,
    pub stamp: String,
    pub value: Option<T>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenStory {
    pub path: PathBuf,
    /// `story.yaml`: title, start, settings.
    pub manifest: EditedFile<Manifest>,
    pub characters: EditedFile<Characters>,
    pub variables: EditedFile<Variables>,
    /// `drivers/roster.yaml`: optional, read as empty until the first driver.
    pub drivers: EditedFile<Roster>,
    /// `scenes/scenes.yaml`: optional, read as empty until the first place.
    pub scenes: EditedFile<Scenes>,
    /// `frames/frames.yaml`: the story's own frames; read as empty without
    /// one. Edited by hand until the Frame screen.
    pub frames: EditedFile<Frames>,
    /// The skins of `frames/skins/`, and what each dresses: what an app or
    /// an item can wear.
    pub skins: Vec<SkinRef>,
    pub chapters: Vec<ChapterFile>,
    pub content: Vec<ContentRef>,
    /// One per file of `events/`, in file-name order.
    pub events: Vec<EditedFile<Event>>,
    /// One per `.story` file of `content/`, in file-name order: what the
    /// Contents section edits.
    pub content_files: Vec<EditedFile<ContentFile>>,
    pub diagnostics: Vec<Diagnostic>,
    /// Where each character, flag, counter, relation step and event is cited.
    pub usages: Vec<NameUsages>,
    /// The files under `media/`, as paths inside it: what a portrait can be.
    pub media: Vec<String>,
    pub history: HistoryStatus,
}

/// A skin, and the template it dresses.
#[derive(Debug, Serialize)]
pub struct SkinRef {
    pub id: String,
    pub template: story_engine::frame::Template,
}

/// `file` as the editor shows it. `value` is what the draft read from it - a
/// world file the story does not have yet reads as empty - and is dropped
/// when the file does not read: the screen shows the error instead.
fn edited<T>(files: &[SourceFile], errors: &[LoadError], file: &str, value: Option<T>) -> EditedFile<T> {
    let error = (errors.iter()).find(|e| e.file == file).map(|e| e.message.clone());
    EditedFile {
        file: file.into(),
        stamp: (files.iter().find(|f| f.path == file))
            .map(|f| stamp(&f.text))
            .unwrap_or_default(),
        value: if error.is_some() { None } else { value },
        error,
    }
}

pub fn open(folder: &Path) -> Result<OpenStory, String> {
    let (files, mut d) = draft_dir(folder).map_err(|e| e.to_string())?;
    let chapter_files: Vec<SourceFile> = files.iter().filter(|f| is_chapter_path(&f.path)).cloned().collect();
    let event_files: Vec<String> = (files.iter())
        .filter(|f| is_event_path(&f.path))
        .map(|f| f.path.clone())
        .collect();
    let story_files: Vec<String> = (files.iter())
        .filter(|f| is_content_path(&f.path))
        .map(|f| f.path.clone())
        .collect();
    // Drafted once, then both shown and checked: the screen and its
    // diagnostics describe the same state of the folder, read once.
    let media = media_list(&d);
    let usages = refs::all_usages(&files, &d);
    let diagnostics = verdict(d.clone());
    let manifest = edited(&files, &d.errors, MANIFEST, d.manifest.take());
    let characters = edited(&files, &d.errors, CHARACTERS, Some(std::mem::take(&mut d.characters)));
    let variables = edited(&files, &d.errors, VARIABLES, Some(std::mem::take(&mut d.variables)));
    let drivers = edited(&files, &d.errors, ROSTER, Some(std::mem::take(&mut d.roster)));
    let scenes = edited(&files, &d.errors, SCENES, Some(std::mem::take(&mut d.scenes)));
    let frames = edited(&files, &d.errors, FRAMES, Some(std::mem::take(&mut d.frames)));
    let skins = (d.skins.iter())
        .map(|s| SkinRef {
            id: s.id.clone(),
            template: s.template,
        })
        .collect();
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
    let content = d.content.iter().map(ContentRef::of).collect();
    let mut events: Vec<EditedFile<Event>> = (event_files.iter())
        .map(|file| {
            let event = d.events.iter().find(|e| &e.file == file).cloned();
            edited(&files, &d.errors, file, event)
        })
        .collect();
    events.sort_by(|a, b| a.file.cmp(&b.file));
    let mut content_files: Vec<EditedFile<ContentFile>> = (story_files.iter())
        .map(|file| {
            let items = (d.content.iter())
                .filter(|c| &c.file == file)
                .map(|c| c.doc())
                .collect();
            edited(&files, &d.errors, file, Some(ContentFile { items }))
        })
        .collect();
    content_files.sort_by(|a, b| a.file.cmp(&b.file));
    Ok(OpenStory {
        path: folder.to_path_buf(),
        manifest,
        characters,
        variables,
        drivers,
        scenes,
        frames,
        skins,
        chapters,
        content,
        events,
        content_files,
        diagnostics,
        usages,
        media,
        history: history::with(folder, |h| h.status()),
    })
}

fn media_list(d: &Draft) -> Vec<String> {
    d.media.iter().flatten().cloned().collect()
}

const MANIFEST: &str = "story.yaml";
const CHARACTERS: &str = "world/characters.yaml";
const VARIABLES: &str = "world/variables.yaml";

/// What the studio re-reads after every save, from one reading of the folder.
#[derive(Debug, Serialize)]
pub struct Checked {
    pub diagnostics: Vec<Diagnostic>,
    /// Where each character, flag, counter, relation step and event is cited.
    pub usages: Vec<NameUsages>,
    /// Re-listed too: a picture copied into `media/` while the editor is
    /// open shows up at the next save.
    pub media: Vec<String>,
    pub history: HistoryStatus,
}

/// Everything wrong with the story, where its names are cited, and what can
/// be undone.
pub fn check(folder: &Path) -> Checked {
    let history = history::with(folder, |h| h.status());
    match draft_dir(folder) {
        Ok((files, d)) => Checked {
            usages: refs::all_usages(&files, &d),
            media: media_list(&d),
            diagnostics: verdict(d),
            history,
        },
        Err(e) => Checked {
            diagnostics: vec![load_error(e)],
            usages: Vec::new(),
            media: Vec::new(),
            history,
        },
    }
}

/// Everything wrong with a drafted story: what keeps it from loading, or
/// else what the validator finds. The messages are the engine's, in English:
/// the editor is reserved for now, and they are translated when it opens to
/// all (docs/CHANTIERS.md, Story).
fn verdict(d: Draft) -> Vec<Diagnostic> {
    match Story::from_draft(d) {
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

/// `flow/<name>.yaml`, directly in `flow/`. With `story.yaml`, the two
/// world files, the roster, the events and the content, the only files the
/// editor writes: anything else - a path climbing out of the story, another
/// folder - is refused, whatever the frontend sends.
fn is_chapter_path(path: &str) -> bool {
    is_yaml_in("flow/", path)
}

/// `events/<name>.yaml`, directly in `events/`, with the same care.
fn is_event_path(path: &str) -> bool {
    is_yaml_in("events/", path)
}

/// `content/…/<name>.story`, in `content/` or a folder of it: an author
/// files content as they like (`content/friend/`). No part of the path can
/// climb out of the story or hide a file.
fn is_content_path(path: &str) -> bool {
    path.strip_prefix("content/").is_some_and(|rest| {
        rest.ends_with(".story")
            && !rest.contains(['\\', ':'])
            && rest.split('/').all(|part| !part.is_empty() && !part.starts_with('.'))
            && rest.rsplit('/').next().is_some_and(|name| name.len() > ".story".len())
    })
}

/// A `.yaml` file directly in `dir`, whose name cannot climb out of it.
fn is_yaml_in(dir: &str, path: &str) -> bool {
    path.strip_prefix(dir).is_some_and(|name| {
        name.ends_with(".yaml")
            && name.len() > ".yaml".len()
            && !name.contains(['/', '\\', ':'])
            && !name.starts_with('.')
    })
}

/// The canonical text of `file` from the value the editor sends, in the
/// shape of that file (`story_engine::model`). Read here rather than in the
/// command's signature: a sentence the engine cannot read is then reported in
/// the engine's words alone, without Tauri's "invalid args" wrapping, and the
/// author sees what to fix.
pub fn render(file: &str, value: serde_json::Value) -> Result<String, String> {
    fn from<T: DeserializeOwned>(value: serde_json::Value) -> Result<T, String> {
        serde_json::from_value(value).map_err(|e| e.to_string())
    }
    match file {
        MANIFEST => write::manifest(&from(value)?),
        CHARACTERS => write::characters(&from(value)?),
        VARIABLES => write::variables(&from(value)?),
        ROSTER => write::roster(&from(value)?),
        SCENES => write::scenes(&from(value)?),
        _ if is_chapter_path(file) => write::chapter(file, &from::<Chapter>(value)?),
        _ if is_event_path(file) => write::event(file, &from::<Event>(value)?),
        _ if is_content_path(file) => write::content_file(file, &from::<ContentFile>(value)?),
        _ => Err(format!("not a file the editor writes: {file}")),
    }
}

/// The moment the Scenes screen's preview is set to: what picks a variant,
/// as its criteria name them. An empty weather or mood is none.
#[derive(Debug, Deserialize)]
pub struct SceneMoment {
    pub season: Season,
    pub period: Period,
    pub weather: Option<String>,
    pub mood: Option<String>,
}

/// The variant of `place` that shows at moment `m` (SPEC-story-mode, Create
/// Your Own: Scenes). `place` comes as the screen edits it, saved or not, so
/// the preview follows every keystroke; the choice is the engine's alone.
pub fn scene_pick(place: serde_json::Value, m: &SceneMoment) -> Result<Option<Shown>, String> {
    let place: scene::Place = serde_json::from_value(place).map_err(|e| e.to_string())?;
    let given = |v: &Option<String>| v.clone().filter(|s| !s.is_empty());
    let (weather, mood) = (given(&m.weather), given(&m.mood));
    Ok(scene::pick(
        &place,
        &scene::Moment {
            season: m.season,
            period: m.period,
            weather: weather.as_deref(),
            mood: mood.as_deref(),
            forced: None,
        },
    ))
}

/// The Content screen's preview of one item (SPEC-story-mode, Create Your
/// Own: Content): everything the preview column shows, worked out from the
/// story as it is on disk, so it follows every save.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewView {
    /// `scene` for what plays on the scene (a dialogue, an illustration, a
    /// document, a video or a call no app takes), `frame` for an item a
    /// frame shows, `nowhere` when none of the frames it can arrive in can
    /// show it.
    pub view: &'static str,
    /// The frame it shows in, for `frame`.
    pub frame: Option<String>,
    /// The app of that frame showing it; none when the frame has no app
    /// for its type and lists it by name.
    pub app: Option<story_engine::view::AppShown>,
    /// That frame's look: its body, status bar and sounds.
    pub phone: Option<story_engine::view::PhoneLook>,
    /// How it plays on the scene (`dialogue`, `newspaper`, `tv`…), and its
    /// skin there.
    pub staged_as: Option<story_engine::frame::Template>,
    pub skin: Option<story_engine::view::SkinShown>,
    /// Why there is nothing to show, for `nowhere`: the checks' warning.
    pub nowhere: Option<String>,
    /// Its content type, as a `.story` header names it.
    pub kind: &'static str,
    pub from: Option<String>,
    /// A dialogue's characters present.
    pub characters: Vec<String>,
    /// An illustration's picture, a path inside the story's `media/`.
    pub picture: Option<String>,
    /// How a dialogue or an illustration comes on the scene.
    pub transition: story_engine::model::Transition,
    /// A dialogue's own place, as it shows with the values set to preview
    /// with; none without one - where the flow stands is not played here.
    pub scene: Option<story_engine::scene::Shown>,
    pub steps: Vec<story_engine::engine::Step>,
    pub choices: Vec<story_engine::engine::Offer>,
    pub ended: bool,
    /// The other items it sent, named.
    pub sent: Vec<String>,
    /// What the author can set to preview with: the variables it reads,
    /// each with the story's own starting value.
    pub knobs: Vec<Knob>,
    /// What the conversation changed.
    pub changes: Vec<Change>,
    /// Story date and time of day shown on the item.
    pub date: String,
    pub clock: String,
}

/// A variable the preview can be set with, and its value at the start.
#[derive(Debug, Serialize)]
pub struct Knob {
    pub var: String,
    /// `relation`, `flag` or `number`.
    pub kind: &'static str,
    pub start: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Change {
    pub var: String,
    pub from: String,
    pub to: String,
}

/// Plays content `id` for the preview: from the story's starting state with
/// `setup` (effect sentences, `rel.friend = warm`) applied, then `replies`.
/// Reads the story from disk; nothing is written.
pub fn preview(folder: &Path, id: &str, setup: &[String], replies: &[String]) -> Result<PreviewView, String> {
    use story_engine::expr::{Effect, Var};
    let (_, d) = draft_dir(folder).map_err(|e| e.to_string())?;
    let story = Story::from_draft(d).map_err(|errors| {
        errors
            .into_iter()
            .next()
            .map_or_else(|| "the story does not load".to_string(), |e| e.to_string())
    })?;
    let item = story
        .content_item(id)
        .ok_or_else(|| format!("unknown content `{id}`"))?;
    let setup: Vec<Effect> = (setup.iter()).map(|s| s.parse()).collect::<Result<_, String>>()?;
    let played = story_engine::engine::preview(&story, &setup, id, replies).map_err(|e| e.to_string())?;
    let (before, after) = (&played.before, &played.after);

    let frames = story_engine::frame::FrameMap::new(&story).content(&story, id);
    // `none` never shows an item: the first frame that can is a named one.
    let frame = frames.iter().find(|f| f.can_show(&story, item.kind));
    // How it plays on the scene, if it does there: a video or a call does
    // when that frame has no app for it.
    let staged_as = story_engine::frame::on_scene_as(&story, frame.unwrap_or(&Frame::None), item.kind, item.layout);
    let on_scene = item.kind.on_scene() || staged_as.is_some();
    let nowhere = (!on_scene)
        .then(|| story_engine::frame::nowhere(&story, item, &frames))
        .flatten();
    let skin = (staged_as.zip(item.skin.as_deref())).and_then(|(t, s)| story_engine::view::skin_shown(&story, s, t));
    // The app of that frame the item shows in; none in a frame that lists
    // it by name, having no app for its type.
    let app = (frame.filter(|_| !on_scene))
        .and_then(|f| f.apps(&story).into_iter().find(|a| a.shows.contains(&item.kind)))
        .map(|a| {
            let skin = (a.skin.as_deref()).and_then(|s| story_engine::view::skin_shown(&story, s, a.template));
            story_engine::view::AppShown::of(a, true, skin)
        });
    // The phone it shows in, dressed as the story starts it.
    let phone = (frame.filter(|_| !on_scene)).and_then(|f| story_engine::view::phone_look(&story, before, f));
    let view = match (on_scene, &nowhere) {
        (true, _) => "scene",
        (false, Some(_)) => "nowhere",
        (false, None) => "frame",
    };

    // A knob starts from the story's own starting value, not from what the
    // author set: the panel tells a value set from one left alone.
    let start = story_engine::state::State::new(&story, 0);
    let money = story.settings().money;
    let knobs = (item.reads().into_iter())
        .filter_map(|var| {
            let (kind, start) = match var {
                Var::Rel(who) => ("relation", start.relations.get(who)?.clone()),
                Var::Flag(f) => ("flag", start.flags.contains(f).to_string()),
                Var::Counter(c) => ("number", start.counters.get(c)?.to_string()),
                Var::Reputation => ("number", start.reputation.to_string()),
                Var::Followers => ("number", start.followers.to_string()),
                Var::Money if money => ("number", start.money.to_string()),
                _ => return None,
            };
            Some(Knob {
                var: var.to_string(),
                kind,
                start,
            })
        })
        .collect();

    Ok(PreviewView {
        view,
        frame: frame.filter(|_| view == "frame").map(ToString::to_string),
        app,
        phone,
        staged_as,
        skin,
        nowhere,
        kind: item.kind.name(),
        from: item.from.clone(),
        characters: item.characters.clone(),
        picture: item.picture.clone(),
        transition: item.transition.unwrap_or_default(),
        scene: (item.scene.as_deref()).and_then(|p| before.scene_shown(&story, p)),
        steps: played.steps.clone(),
        choices: played.choices.clone(),
        ended: played.ended,
        sent: played.sent.clone(),
        knobs,
        changes: changes(&story, before, after),
        date: after.date.to_string(),
        clock: item.clock.unwrap_or(after.clock).to_string(),
    })
}

/// What changed between two states, in the variables an author sets.
fn changes(story: &Story, before: &story_engine::state::State, after: &story_engine::state::State) -> Vec<Change> {
    let mut out = Vec::new();
    let mut push = |var: String, from: String, to: String| {
        if from != to {
            out.push(Change { var, from, to });
        }
    };
    push(
        "reputation".into(),
        before.reputation.to_string(),
        after.reputation.to_string(),
    );
    push(
        "followers".into(),
        before.followers.to_string(),
        after.followers.to_string(),
    );
    if story.settings().money {
        push("money".into(), before.money.to_string(), after.money.to_string());
    }
    for (who, level) in &after.relations {
        let was = before.relations.get(who).cloned().unwrap_or_default();
        push(format!("rel.{who}"), was, level.clone());
    }
    for flag in before.flags.union(&after.flags) {
        let (was, is) = (before.flags.contains(flag), after.flags.contains(flag));
        push(format!("flag.{flag}"), was.to_string(), is.to_string());
    }
    for (counter, n) in &after.counters {
        let was = before.counters.get(counter).copied().unwrap_or_default();
        push(format!("counter.{counter}"), was.to_string(), n.to_string());
    }
    out
}

/// A content item's text as the Form view edits it: its tree, and what
/// does not read, by line of the text (1 is its first line).
#[derive(Debug, Serialize)]
pub struct ScriptRead {
    pub script: story_engine::script::Script,
    pub errors: Vec<ScriptLineError>,
}

#[derive(Debug, Serialize)]
pub struct ScriptLineError {
    pub line: usize,
    pub message: String,
}

/// Reads a content item's text into the tree the Form view edits. The one
/// reader of Ink is the engine's: the frontend never parses it.
pub fn read_script(text: &str) -> ScriptRead {
    let (script, errors) = story_engine::script::parse(text, 1);
    ScriptRead {
        script,
        errors: (errors.into_iter())
            .map(|e| ScriptLineError {
                line: e.line,
                message: e.message,
            })
            .collect(),
    }
}

/// Writes the tree the Form view edited back as Ink, in canonical form. A
/// sentence that does not read (a condition, an effect) is refused in the
/// engine's words, like a save.
pub fn write_script(value: serde_json::Value) -> Result<String, String> {
    let script: story_engine::script::Script = serde_json::from_value(value).map_err(|e| e.to_string())?;
    Ok(story_engine::script::print(&script))
}

/// Writes one file of the story. `expected` is the stamp the editor read it
/// with, `None` for a file the story does not have yet (a new chapter, a
/// first character), which must not exist. Returns the new stamp.
pub fn save_file(
    folder: &Path,
    file: &str,
    expected: Option<&str>,
    value: serde_json::Value,
) -> Result<String, String> {
    let writes = matches!(file, MANIFEST | CHARACTERS | VARIABLES | ROSTER | SCENES)
        || is_chapter_path(file)
        || is_event_path(file)
        || is_content_path(file);
    if !writes {
        return Err(format!("not a file the editor writes: {file}"));
    }
    write_guarded(folder, file, expected, || render(file, value))
}

/// Deletes an event's file, or a `.story` file whose last item goes, only
/// if it still holds what the editor read. What cited it is left as it is,
/// for the checks to report. Recorded in the undo history, so Ctrl+Z brings
/// it back.
pub fn remove_file(folder: &Path, file: &str, expected: &str) -> Result<(), String> {
    if !is_event_path(file) && !is_content_path(file) {
        return Err(format!("not a file the editor removes: {file}"));
    }
    let path = folder.join(file);
    let on_disk = std::fs::read_to_string(&path).ok();
    if on_disk.as_deref().map(stamp).as_deref() != Some(expected) {
        return Err(errors::STORY_FILE_CHANGED.into());
    }
    std::fs::remove_file(&path).map_err(|e| format!("removing {}: {e}", path.display()))?;
    history::with(folder, |h| {
        h.record(vec![FileChange {
            file: file.into(),
            before: on_disk,
            after: None,
        }])
    });
    Ok(())
}

/// Renames a character, flag, counter, relation step, event or driver, and every
/// sentence citing it, in every file of the story (`story_engine::refs`).
/// Unlike a save it takes no stamp: it reads the files as they are now and
/// changes only the cited words, so an edit made meanwhile in a text editor
/// is kept, not overwritten. Nothing is written unless the whole story reads
/// back. An event's file named after it moves to its new name.
pub fn rename(folder: &Path, kind: NameKind, from: &str, to: &str) -> Result<(), String> {
    let files = read_dir(folder).map_err(|e| e.to_string())?;
    let changes = refs::rename(&files, kind, from, to)?;
    let mut edit = Vec::new();
    for file in changes.written {
        let path = folder.join(&file.path);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, &file.text).map_err(|e| format!("writing {}: {e}", path.display()))?;
        edit.push(FileChange {
            before: files.iter().find(|f| f.path == file.path).map(|f| f.text.clone()),
            after: Some(file.text),
            file: file.path,
        });
    }
    // Removed last: the moved file is already written under its new name.
    for file in changes.removed {
        let path = folder.join(&file);
        let before = std::fs::read_to_string(&path).ok();
        std::fs::remove_file(&path).map_err(|e| format!("removing {}: {e}", path.display()))?;
        edit.push(FileChange {
            file,
            before,
            after: None,
        });
    }
    // One edit, however many files: Ctrl+Z undoes the rename whole.
    history::with(folder, |h| h.record(edit));
    Ok(())
}

/// Writes `file` with the text `render` gives, only if the file still holds
/// what the editor read (`expected`, its stamp; `None`: the file must not
/// exist yet). The text is rendered after the check, so a refused write costs
/// nothing. Recorded in the undo history; returns the new stamp.
fn write_guarded(
    folder: &Path,
    file: &str,
    expected: Option<&str>,
    render: impl FnOnce() -> Result<String, String>,
) -> Result<String, String> {
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
    let text = render()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, &text).map_err(|e| format!("writing {}: {e}", path.display()))?;
    let new_stamp = stamp(&text);
    history::with(folder, |h| {
        h.record(vec![FileChange {
            file: file.into(),
            before: on_disk,
            after: Some(text),
        }])
    });
    Ok(new_stamp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::temp_dir;

    fn json<T: Serialize>(value: &T) -> serde_json::Value {
        serde_json::to_value(value).expect("serializes")
    }

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
        create(&config, &folder, "My story", "en", Frame::None, "Chapter 1").expect("created");
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
            create(&config, &folder, "S", "en", Frame::None, "C").unwrap_err(),
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
        create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
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
        create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
        let (mut chapter, stamp) = chapter_of(&folder, "flow/01-chapter.yaml");
        chapter.nodes[0].next = Some("nowhere".into());
        let new_stamp = save_file(&folder, "flow/01-chapter.yaml", Some(&stamp), json(&chapter)).expect("saved");
        let (back, read_stamp) = chapter_of(&folder, "flow/01-chapter.yaml");
        assert_eq!(read_stamp, new_stamp, "the returned stamp is the file's");
        assert_eq!(back.nodes[0].next.as_deref(), Some("nowhere"));
        let d = check(&folder).diagnostics;
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
        create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
        let (chapter, stamp) = chapter_of(&folder, "flow/01-chapter.yaml");
        let path = folder.join("flow/01-chapter.yaml");
        let by_hand = format!("# typed by hand\n{}", std::fs::read_to_string(&path).unwrap());
        std::fs::write(&path, &by_hand).unwrap();
        assert_eq!(
            save_file(&folder, "flow/01-chapter.yaml", Some(&stamp), json(&chapter)).unwrap_err(),
            errors::STORY_FILE_CHANGED
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), by_hand, "the hand edit kept");
        assert_eq!(
            save_file(&folder, "flow/01-chapter.yaml", None, json(&chapter)).unwrap_err(),
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
        let why = render("flow/01-chapter.yaml", bad).unwrap_err();
        assert!(why.contains("cannot read effect `money +=== 1`"), "{why}");
        let good = serde_json::json!({ "id": "ch01", "nodes": [{ "id": "start", "end": "done" }] });
        assert!(render("flow/01-chapter.yaml", good).is_ok());
        let bad_date = serde_json::json!({ "characters": [{ "id": "a", "relation": "warm", "age": 3 }] });
        assert!(
            render("world/characters.yaml", bad_date).unwrap_err().contains("age"),
            "an unknown key named, in a world file too"
        );
    }

    // Only the story's own files are written, inside the story: its chapters,
    // story.yaml and the two world files.
    #[test]
    fn only_the_story_files_are_written() {
        let base = temp_dir("studio-story");
        for bad in [
            "flow/../story.yaml",
            "flow/sub/01.yaml",
            "flow/.yaml",
            "flow/01.txt",
            "flow/..\\x.yaml",
            "events/../story.yaml",
            "events/sub/race.yaml",
            "world/../story.yaml",
            "world/economy.yaml",
            "drivers/careers.yaml",
        ] {
            assert!(!is_chapter_path(bad), "{bad} is not a chapter");
            assert!(
                save_file(&base, bad, None, serde_json::json!({})).is_err(),
                "{bad} refused"
            );
            assert!(!base.join(bad).exists(), "{bad} not written");
        }
        assert!(is_chapter_path("flow/02-chapter.yaml"));
        assert!(is_event_path("events/race_01.yaml") && !is_chapter_path("events/race_01.yaml"));
        assert!(is_content_path("content/friend/invite.story") && is_content_path("content/a.story"));
        for bad in [
            "content/../story.yaml",
            "content/.hidden.story",
            "content//a.story",
            "content/.story",
            "flow/a.story",
        ] {
            assert!(!is_content_path(bad), "{bad} is not a content file the editor writes");
        }
        assert!(
            remove_file(&base, "flow/01-chapter.yaml", "").is_err(),
            "only an event's file is removed"
        );
    }

    // An event is created, edited and read back like a chapter; deleting it
    // leaves the node citing it for the checks, and Ctrl+Z brings it back
    // (SPEC-story-mode, Create Your Own).
    #[test]
    fn an_event_is_saved_removed_and_restored() {
        let config = temp_dir("studio-config");
        let base = temp_dir("studio-story");
        let folder = base.join("s");
        create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
        let file = "events/race_01.yaml";
        let event = serde_json::json!({
            "id": "race_01", "name": "Race 01", "type": "race",
            "session": { "track": "some_track", "car": "some_car", "laps": 8 },
            "success": ["position <= 3"],
            "on_success": { "effects": [], "advance": null },
            "on_failure": { "effects": [], "pool": null, "pick": 1 },
            "entry_fee": null,
        });
        let stamp = save_file(&folder, file, None, event).expect("created");
        let opened = open(&folder).unwrap();
        let read = opened.events[0].value.as_ref().expect("reads");
        assert_eq!((read.id.as_str(), read.session.laps), ("race_01", Some(8)));
        assert_eq!(opened.events[0].stamp, stamp, "the stamp the save returned");

        let (mut chapter, cstamp) = chapter_of(&folder, "flow/01-chapter.yaml");
        chapter.nodes[0].kind = story_engine::model::NodeKind::Event {
            event: "race_01".into(),
            on_failure: None,
        };
        save_file(&folder, "flow/01-chapter.yaml", Some(&cstamp), json(&chapter)).unwrap();
        assert!(check(&folder).diagnostics.is_empty(), "{:#?}", check(&folder));

        assert_eq!(
            remove_file(&folder, file, "0").unwrap_err(),
            errors::STORY_FILE_CHANGED,
            "never removed on an old stamp"
        );
        remove_file(&folder, file, &stamp).expect("removed");
        assert!(!folder.join(file).exists());
        assert!(
            (check(&folder).diagnostics.iter()).any(|d| d.message.contains("unknown event `race_01`")),
            "the node citing it is reported"
        );
        history::with(&folder, |h| h.undo(&folder)).unwrap();
        assert!(folder.join(file).exists(), "Ctrl+Z brings it back");
    }

    // Renaming an event moves its file and follows its node; Ctrl+Z puts
    // both back at once.
    #[test]
    fn renaming_an_event_moves_its_file_and_undoes_whole() {
        let config = temp_dir("studio-config");
        let base = temp_dir("studio-story");
        let folder = base.join("s");
        create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
        std::fs::create_dir_all(folder.join("events")).unwrap();
        std::fs::write(
            folder.join("events/race_01.yaml"),
            "id: race_01\ntype: race\nsession: { track: t, car: c }\n",
        )
        .unwrap();
        let (mut chapter, cstamp) = chapter_of(&folder, "flow/01-chapter.yaml");
        chapter.nodes[0].kind = story_engine::model::NodeKind::Event {
            event: "race_01".into(),
            on_failure: None,
        };
        save_file(&folder, "flow/01-chapter.yaml", Some(&cstamp), json(&chapter)).unwrap();

        rename(&folder, NameKind::Event, "race_01", "opener").expect("renamed");
        assert!(!folder.join("events/race_01.yaml").exists(), "the old file is gone");
        assert!(folder.join("events/opener.yaml").exists(), "under its new name");
        assert!(check(&folder).diagnostics.is_empty(), "{:#?}", check(&folder));

        history::with(&folder, |h| h.undo(&folder)).unwrap();
        assert!(folder.join("events/race_01.yaml").exists(), "back under its old name");
        assert!(!folder.join("events/opener.yaml").exists(), "the new one undone");
        assert!(check(&folder).diagnostics.is_empty(), "the node follows back");
    }

    // story.yaml and the world files are saved like a chapter: canonical,
    // read back, guarded by their stamp; a world file the story did not have
    // yet is created by its first save.
    #[test]
    fn story_yaml_and_world_files_save_and_read_back() {
        let config = temp_dir("studio-config");
        let base = temp_dir("studio-story");
        let folder = base.join("s");
        create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
        let opened = open(&folder).unwrap();
        assert_eq!(opened.characters.stamp, "", "no characters file yet");
        assert!(opened.characters.value.unwrap().characters.is_empty(), "read as empty");
        let mut manifest = opened.manifest.value.expect("story.yaml reads");
        manifest.title = "Renamed".into();
        manifest.settings.failure = story_engine::model::FailureMode::Final;
        save_file(&folder, MANIFEST, Some(&opened.manifest.stamp), json(&manifest)).expect("saved");
        let characters = serde_json::json!({ "characters": [{ "id": "friend", "name": "A friend" }] });
        save_file(&folder, CHARACTERS, None, characters.clone()).expect("created");
        assert_eq!(
            save_file(&folder, CHARACTERS, None, characters).unwrap_err(),
            errors::STORY_FILE_CHANGED,
            "a second \"first save\" never replaces it"
        );
        let opened = open(&folder).unwrap();
        let m = opened.manifest.value.unwrap();
        assert_eq!(m.title, "Renamed");
        assert_eq!(m.settings.failure, story_engine::model::FailureMode::Final);
        assert_eq!(opened.characters.value.unwrap().characters[0].id, "friend");
        assert_ne!(opened.characters.stamp, "", "now on disk");
        assert_eq!(list(&config)[0].title.as_deref(), Some("Renamed"), "the hub follows");
        assert!(check(&folder).diagnostics.is_empty(), "still clean");
    }

    // The sample copied into Create Your Own opens clean, portraits
    // included, and refuses a folder that is not empty like a new story.
    #[test]
    fn the_sample_is_copied_whole_into_an_empty_folder() {
        let config = temp_dir("studio-config");
        let base = temp_dir("studio-story");
        let folder = base.join("sample");
        copy_sample(&config, &folder).expect("copied");
        let opened = open(&folder).unwrap();
        assert!(opened.diagnostics.is_empty(), "clean: {:?}", opened.diagnostics);
        assert!(
            opened.media.iter().any(|m| m == "portraits/friend_neutral.png"),
            "with its portraits: {:?}",
            opened.media
        );
        assert_eq!(list(&config).len(), 1, "listed in Create Your Own");
        assert_eq!(
            copy_sample(&config, &folder).unwrap_err(),
            errors::STORY_FOLDER_NOT_EMPTY,
            "never over an existing story"
        );
    }

    // The roster goes through the editor like a world file: read as empty
    // until its first save, then checked with the rest - here a grid naming
    // a driver it does not have (SPEC-story-mode, Drivers).
    #[test]
    fn the_roster_is_created_read_back_and_checked() {
        let config = temp_dir("studio-config");
        let base = temp_dir("studio-story");
        let folder = base.join("s");
        create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
        let opened = open(&folder).unwrap();
        assert_eq!(opened.drivers.stamp, "", "no roster yet");
        assert!(opened.drivers.value.unwrap().drivers.is_empty(), "read as empty");
        let event = serde_json::json!({
            "id": "r", "type": "race",
            "session": { "track": "t", "car": "c", "opponents": [{ "driver": "rival" }] },
        });
        save_file(&folder, "events/r.yaml", None, event).expect("event created");
        let unknown =
            |folder: &Path| (check(folder).diagnostics.iter()).any(|d| d.message.contains("unknown driver `rival`"));
        assert!(unknown(&folder), "the grid names a driver the story lacks");
        let roster = serde_json::json!({ "drivers": [{ "id": "rival", "name": "R", "ai_level": 95 }] });
        save_file(&folder, ROSTER, None, roster).expect("roster created");
        let opened = open(&folder).unwrap();
        let drivers = opened.drivers.value.expect("reads").drivers;
        assert_eq!((drivers[0].id.as_str(), drivers[0].ai_level), ("rival", Some(95)));
        assert!(!unknown(&folder), "the driver is known now");
        assert!(
            (opened.usages.iter()).any(|u| u.kind == NameKind::Driver && u.id == "rival" && u.usages.len() == 1),
            "the grid cites the driver"
        );
    }

    // A `.story` file goes through the editor like an event: created by its
    // first save, read back as items, guarded by its stamp, and removed when
    // its last item goes (SPEC-story-mode, Create Your Own).
    #[test]
    fn a_content_file_is_created_read_back_and_removed() {
        let config = temp_dir("studio-config");
        let base = temp_dir("studio-story");
        let folder = base.join("s");
        create(&config, &folder, "S", "en", Frame::Named("phone".into()), "C").unwrap();
        let file = "content/friend/ask.story";
        let doc = serde_json::json!({ "items": [{
            "header": { "id": "ask", "type": "message", "when": ["reputation >= 1"] },
            "body": "Coming?\n* (yes) [Yes.]\n* (no) [No.]\n"
        }] });
        let stamp = save_file(&folder, file, None, doc).expect("created");
        let opened = open(&folder).unwrap();
        let entry = opened.content_files.iter().find(|f| f.file == file).expect("listed");
        let items = &entry.value.as_ref().expect("reads").items;
        assert_eq!(items[0].header.id, "ask");
        assert!(items[0].body.contains("* (yes) [Yes.]"), "the text as written");
        assert_eq!(opened.content[0].labels, ["yes", "no"], "its labels, for the flow");
        assert!(
            check(&folder).diagnostics.is_empty(),
            "{:#?}",
            check(&folder).diagnostics
        );
        remove_file(&folder, file, &stamp).expect("removed");
        assert!(!folder.join(file).exists());
    }

    // The Scenes screen (SPEC-story-mode, Create Your Own: Scenes): the
    // story's places come with the story, and its preview asks the engine
    // which variant shows at the moment the author set, on the place as the
    // screen holds it.
    #[test]
    fn the_scenes_come_with_the_story_and_the_preview_picks_their_variant() {
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/story-engine/sample");
        let story = open(&sample).expect("opens");
        let places = story.scenes.value.expect("the sample has scenes");
        let garage = places.scenes.iter().find(|p| p.id == "garage").expect("a garage");
        let garage = serde_json::to_value(garage).unwrap();
        let at = |period: &str, mood: Option<&str>| {
            let m: SceneMoment = serde_json::from_value(serde_json::json!({
                "season": "spring", "period": period, "weather": null, "mood": mood
            }))
            .unwrap();
            scene_pick(garage.clone(), &m).unwrap().map(|s| s.variant)
        };
        assert_eq!(at("morning", None).as_deref(), Some("day"), "the default, last");
        assert_eq!(at("night", Some("")).as_deref(), Some("night"), "an empty mood is none");
        assert_eq!(
            at("morning", Some("tense")).as_deref(),
            Some("tense"),
            "the first that holds"
        );
        let empty = serde_json::json!({ "id": "x", "variants": [] });
        let m: SceneMoment =
            serde_json::from_value(serde_json::json!({ "season": "winter", "period": "night" })).unwrap();
        assert_eq!(
            scene_pick(empty, &m).unwrap(),
            None,
            "a place with no variant shows nothing"
        );
        assert!(
            scene_pick(serde_json::json!({ "id": "x" }), &m).is_err(),
            "a place that does not read says why"
        );

        // The Scenes screen saves the file: a story without one gets it.
        let config = temp_dir("studio-scenes-config");
        let base = temp_dir("studio-scenes");
        let folder = base.join("s");
        create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
        let places =
            serde_json::json!({ "scenes": [{ "id": "attic", "variants": [{ "id": "default", "file": "a.webp" }] }] });
        save_file(&folder, SCENES, None, places).expect("the editor writes scenes/scenes.yaml");
        let opened = open(&folder).unwrap();
        assert_eq!(opened.scenes.value.unwrap().scenes[0].id, "attic", "read back");
    }

    // The preview (SPEC-story-mode, Create Your Own: Content): a dialogue on
    // the scene, a message in its frame, and the checks' warning for an item
    // no frame can show; the values it reads can be set, and what the
    // conversation changes is listed.
    #[test]
    fn the_preview_picks_its_view_and_lists_what_can_be_set_and_what_changed() {
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/story-engine/sample");
        let talk = preview(&sample, "talk_01", &[], &["rest".into()]).expect("previews");
        assert_eq!(
            (talk.view, talk.characters.as_slice()),
            ("scene", &["friend".to_string()][..])
        );
        assert!(talk.ended, "`rest` ends it");
        let place = talk.scene.as_ref().expect("its own place");
        assert_eq!(
            (place.place.as_str(), place.variant.as_str()),
            ("garage", "day"),
            "the garage at the story's start, 09:00"
        );
        assert_eq!(
            talk.changes,
            [Change {
                var: "rel.friend".into(),
                from: "neutral".into(),
                to: "cold".into()
            }]
        );
        let invite = preview(&sample, "invite_01", &[], &[]).unwrap();
        assert_eq!((invite.view, invite.frame.as_deref()), ("frame", Some("basic_phone")));
        let app = invite.app.expect("the app showing it");
        assert_eq!(
            (app.id.as_str(), app.name.as_deref()),
            ("messages", Some("Messages")),
            "the basic phone's own"
        );
        assert!(invite.scene.is_none(), "a message has no place of its own");
        let offer = preview(&sample, "sponsor_offer", &["flag.sponsor_met = true".into()], &[]).unwrap();
        let knobs: Vec<(&str, &str)> = offer.knobs.iter().map(|k| (k.var.as_str(), k.kind)).collect();
        assert_eq!(knobs, [("reputation", "number"), ("flag.sponsor_met", "flag")]);
        assert_eq!(offer.knobs[1].start, "false", "the story's own start, whatever was set");

        let config = temp_dir("studio-config");
        let base = temp_dir("studio-story");
        let folder = base.join("s");
        create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
        let doc = serde_json::json!({ "items": [{ "header": { "id": "note", "type": "mail", "when": [] }, "body": "Hi.\n" }] });
        save_file(&folder, "content/note.story", None, doc).unwrap();
        let note = preview(&folder, "note", &[], &[]).unwrap();
        assert_eq!(note.view, "nowhere", "a visual novel has nowhere to show a mail");
        assert!(
            note.nowhere.unwrap().contains("no frame it can arrive in"),
            "the checks' warning"
        );
        assert!(
            preview(&folder, "note", &["money +== 1".into()], &[]).is_err(),
            "a setting that does not read"
        );
    }

    // The Form view edits the tree the engine reads, and gets back the text
    // the engine writes: the same conversation, in canonical form.
    #[test]
    fn a_script_goes_through_the_form_view_and_back() {
        let text = "friend: Hi.\n* (yes) [Yes.]\n    ~ effect(\"reputation += 1\")\n* (no) [No.]\n";
        let read = read_script(text);
        assert!(read.errors.is_empty(), "{:?}", read.errors);
        let tree = serde_json::to_value(&read.script).unwrap();
        assert_eq!(write_script(tree).unwrap(), text, "the canonical form of what was read");
        let broken = read_script("friend: Hi.\n-> away\n");
        assert_eq!(broken.errors[0].line, 2, "by line of the text");
        let bad = serde_json::json!({ "items": [{ "kind": "effect", "effect": "money +== 1" }] });
        assert!(
            write_script(bad).unwrap_err().contains("cannot read effect"),
            "in the engine's words"
        );
    }

    // A portrait is a file of the story's `media/`: the studio lists them for
    // the picker, and its checks see one that points to nothing - including a
    // picture added while the editor is open.
    #[test]
    fn the_studio_lists_media_and_checks_portraits_against_them() {
        let config = temp_dir("studio-config");
        let base = temp_dir("studio-story");
        let folder = base.join("s");
        create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
        let characters = serde_json::json!({ "characters": [{ "id": "friend", "portraits": {
            "neutral": "portraits/friend.png", "happy": "portraits/happy.png" } }] });
        save_file(&folder, CHARACTERS, None, characters).expect("saved");
        std::fs::create_dir_all(folder.join("media/portraits")).unwrap();
        std::fs::write(folder.join("media/portraits/friend.png"), b"png").unwrap();

        let opened = open(&folder).unwrap();
        assert_eq!(opened.media, ["portraits/friend.png"], "listed inside media/");
        let missing =
            |d: &[Diagnostic]| (d.iter()).any(|x| x.message.contains("`media/portraits/happy.png` does not exist"));
        assert!(missing(&opened.diagnostics), "{:#?}", opened.diagnostics);

        std::fs::write(folder.join("media/portraits/happy.png"), b"png").unwrap();
        let checked = check(&folder);
        assert_eq!(checked.media.len(), 2, "re-listed at the next check");
        assert!(checked.diagnostics.is_empty(), "{:#?}", checked.diagnostics);
    }

    // A rename reaches every file citing the name, and keeps what was typed
    // by hand around it (SPEC-story-mode, Create Your Own).
    #[test]
    fn a_rename_follows_into_every_file_and_keeps_hand_edits() {
        let config = temp_dir("studio-config");
        let base = temp_dir("studio-story");
        let folder = base.join("s");
        create(&config, &folder, "S", "en", Frame::Named("phone".into()), "C").unwrap();
        save_file(
            &folder,
            VARIABLES,
            None,
            serde_json::json!({ "counters": { "visits": 0 } }),
        )
        .unwrap();
        let (mut chapter, stamp) = chapter_of(&folder, "flow/01-chapter.yaml");
        chapter.nodes[0].kind = story_engine::model::NodeKind::Beat(
            serde_json::from_value(serde_json::json!({ "effects": ["counter.visits += 1"] })).unwrap(),
        );
        save_file(&folder, "flow/01-chapter.yaml", Some(&stamp), json(&chapter)).unwrap();
        let content = "---
id: remark
type: message
when: [\"counter.visits >= 2\"]
---

    A line, indented by hand.
";
        std::fs::create_dir_all(folder.join("content")).unwrap();
        std::fs::write(folder.join("content/remark.story"), content).unwrap();
        let cited = |id: &str| {
            let u = check(&folder).usages;
            u.into_iter().find(|n| n.id == id).map(|n| n.usages.len())
        };
        assert_eq!(cited("visits"), Some(2), "the beat and the reactive message");

        rename(&folder, NameKind::Counter, "visits", "trips").expect("renamed");
        assert_eq!(cited("trips"), Some(2), "both citations followed");
        assert_eq!(cited("visits"), None, "the old name is gone");
        assert_eq!(
            std::fs::read_to_string(folder.join("content/remark.story")).unwrap(),
            content.replace("counter.visits", "counter.trips"),
            "only the cited word changed in the hand-typed file"
        );
        assert!(check(&folder).diagnostics.is_empty(), "clean: {:#?}", check(&folder));
        assert!(
            rename(&folder, NameKind::Counter, "trips", "9lives").is_err(),
            "an invalid name is refused"
        );
    }

    // Saves and renames land in one undo history, undone last first; a
    // rename that touched several files comes back whole (SPEC-story-mode,
    // Create Your Own).
    #[test]
    fn saves_and_renames_are_undone_last_first() {
        let config = temp_dir("studio-config");
        let base = temp_dir("studio-story");
        let folder = base.join("s");
        create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
        let undo =
            |n: usize| (0..n).for_each(|_| assert!(!history::with(&folder, |h| h.undo(&folder)).unwrap().is_empty()));
        save_file(
            &folder,
            VARIABLES,
            None,
            serde_json::json!({ "counters": { "visits": 0 } }),
        )
        .unwrap();
        let (mut chapter, stamp) = chapter_of(&folder, "flow/01-chapter.yaml");
        let original = std::fs::read_to_string(folder.join("flow/01-chapter.yaml")).unwrap();
        chapter.nodes[0].kind = story_engine::model::NodeKind::Beat(
            serde_json::from_value(serde_json::json!({ "effects": ["counter.visits += 1"] })).unwrap(),
        );
        save_file(&folder, "flow/01-chapter.yaml", Some(&stamp), json(&chapter)).unwrap();
        rename(&folder, NameKind::Counter, "visits", "trips").unwrap();
        assert!(check(&folder).history.can_undo, "the editor can offer Ctrl+Z");

        undo(1);
        assert_eq!(check(&folder).usages[0].id, "visits", "the rename undone: declaration");
        assert!(
            std::fs::read_to_string(folder.join("flow/01-chapter.yaml"))
                .unwrap()
                .contains("counter.visits"),
            "and the sentence citing it"
        );
        undo(2);
        assert_eq!(
            std::fs::read_to_string(folder.join("flow/01-chapter.yaml")).unwrap(),
            original,
            "the chapter as created"
        );
        assert!(
            !folder.join(VARIABLES).exists(),
            "the world file the first save created is gone"
        );
        assert!(check(&folder).history.can_redo && !check(&folder).history.can_undo);
    }

    // One chapter that does not read hides neither the others nor itself:
    // it is listed with its error.
    #[test]
    fn an_unreadable_chapter_is_listed_with_its_error() {
        let config = temp_dir("studio-config");
        let base = temp_dir("studio-story");
        let folder = base.join("s");
        create(&config, &folder, "S", "en", Frame::None, "C").unwrap();
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
