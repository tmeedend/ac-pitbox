//! A story as the editor opens it - every file it edits, readable or not -
//! and what the checks find in it, re-read after every save.

use std::path::{Path, PathBuf};

use serde::Serialize;
use story_engine::frame::{Frames, FRAMES};
use story_engine::load::{draft_dir, Draft, SourceFile, ROSTER};
use story_engine::model::{Chapter, Characters, ContentFile, Event, Manifest, Placement, Roster, Variables};
use story_engine::refs::{self, NameUsages};
use story_engine::scene::{Scenes, SCENES};
use story_engine::skin::Skin;
use story_engine::validate::{validate, Diagnostic, Severity};
use story_engine::LoadError;
use story_engine::Story;

use super::super::history::{self, HistoryStatus};
use super::*;

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
    /// One per `frames/skins/<id>/skin.yaml`, in path order: what the Frame
    /// screen edits, and what an app or an item can wear.
    pub skins: Vec<EditedFile<Skin>>,
    /// The skins' own files, as paths inside the story: what a slot or a
    /// font can be.
    pub skin_files: Vec<String>,
    /// Every template, as the engine knows it: what the Frame screen offers.
    pub templates: Vec<TemplateInfo>,
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
    /// The permission register and what it says (`package.rs`).
    pub package: PackageView,
}

/// A template as the Frame screen offers it (`story_engine::frame::Template`,
/// `story_engine::skin::slots`): the engine's table, never copied in the
/// frontend.
#[derive(Debug, Serialize)]
pub struct TemplateInfo {
    pub name: &'static str,
    /// An app can be it; otherwise it is a way of playing on the scene.
    pub app: bool,
    /// The content types it shows.
    pub shows: Vec<&'static str>,
    /// The slots a skin for it fills.
    pub slots: &'static [&'static str],
}

fn templates() -> Vec<TemplateInfo> {
    use story_engine::frame::Template as T;
    use story_engine::model::ContentType as C;
    const TYPES: [C; 12] = [
        C::Message,
        C::Call,
        C::Mail,
        C::Article,
        C::SocialPost,
        C::Video,
        C::Listing,
        C::Statement,
        C::CalendarEntry,
        C::Dialogue,
        C::Illustration,
        C::Document,
    ];
    T::ALL
        .into_iter()
        .map(|t| TemplateInfo {
            name: t.name(),
            app: t.is_app(),
            shows: TYPES.iter().filter(|k| t.can_show(**k)).map(|k| k.name()).collect(),
            slots: story_engine::skin::slots(t),
        })
        .collect()
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
    // Drafted once, then both shown and checked: the screen and its
    // diagnostics describe the same state of the folder, read once.
    let media = media_list(&d);
    let usages = refs::all_usages(&files, &d);
    let diagnostics = verdict(folder, d.clone());
    let manifest = edited(&files, &d.errors, MANIFEST, d.manifest.take());
    let characters = edited(&files, &d.errors, CHARACTERS, Some(std::mem::take(&mut d.characters)));
    let variables = edited(&files, &d.errors, VARIABLES, Some(std::mem::take(&mut d.variables)));
    let drivers = edited(&files, &d.errors, ROSTER, Some(std::mem::take(&mut d.roster)));
    let scenes = edited(&files, &d.errors, SCENES, Some(std::mem::take(&mut d.scenes)));
    let frames = edited(&files, &d.errors, FRAMES, Some(std::mem::take(&mut d.frames)));
    let skin_files = (d.skin_files.iter()).flatten().cloned().collect();
    let skins = (files.iter())
        .filter_map(|f| story_engine::skin::skin_id(&f.path).map(|id| (f, id)))
        .map(|(f, id)| {
            let skin = d.skins.iter().find(|s| s.id == id).cloned();
            edited(&files, &d.errors, &f.path, skin)
        })
        .collect();
    let content = d.content.iter().map(ContentRef::of).collect();
    let (chapters, events, content_files) = (
        chapter_files(&files, &d),
        event_files(&files, &d),
        content_files(&files, &d),
    );
    Ok(OpenStory {
        path: folder.to_path_buf(),
        manifest,
        characters,
        variables,
        drivers,
        scenes,
        frames,
        skins,
        skin_files,
        templates: templates(),
        chapters,
        content,
        events,
        content_files,
        diagnostics,
        usages,
        media,
        history: history::with(folder, |h| h.status()),
        package: package_view(folder),
    })
}

/// The chapters, each readable or not, in path order.
fn chapter_files(files: &[SourceFile], d: &Draft) -> Vec<ChapterFile> {
    (files.iter().filter(|f| is_chapter_path(&f.path)))
        .map(|f| ChapterFile {
            file: f.path.clone(),
            stamp: stamp(&f.text),
            chapter: d.chapters.iter().find(|c| c.file == f.path).cloned(),
            error: (d.errors.iter()).find(|e| e.file == f.path).map(|e| e.message.clone()),
        })
        .collect()
}

/// One per file of `events/`, in file-name order.
fn event_files(files: &[SourceFile], d: &Draft) -> Vec<EditedFile<Event>> {
    let mut events: Vec<EditedFile<Event>> = (files.iter().filter(|f| is_event_path(&f.path)))
        .map(|f| {
            let event = d.events.iter().find(|e| e.file == f.path).cloned();
            edited(files, &d.errors, &f.path, event)
        })
        .collect();
    events.sort_by(|a, b| a.file.cmp(&b.file));
    events
}

/// One per `.story` file of `content/`, all its items, in file-name order.
fn content_files(files: &[SourceFile], d: &Draft) -> Vec<EditedFile<ContentFile>> {
    let mut out: Vec<EditedFile<ContentFile>> = (files.iter().filter(|f| is_content_path(&f.path)))
        .map(|f| {
            let items = (d.content.iter())
                .filter(|c| c.file == f.path)
                .map(|c| c.doc())
                .collect();
            edited(files, &d.errors, &f.path, Some(ContentFile { items }))
        })
        .collect();
    out.sort_by(|a, b| a.file.cmp(&b.file));
    out
}

pub(super) fn media_list(d: &Draft) -> Vec<String> {
    d.media.iter().flatten().cloned().collect()
}

/// What the studio re-reads after every save, from one reading of the folder.
#[derive(Debug, Serialize)]
pub struct Checked {
    pub diagnostics: Vec<Diagnostic>,
    /// Where each character, flag, counter, relation step and event is cited.
    pub usages: Vec<NameUsages>,
    /// Re-listed too: a picture copied into `media/` while the editor is
    /// open shows up at the next save.
    pub media: Vec<String>,
    /// Re-listed too, like `media`.
    pub skin_files: Vec<String>,
    pub history: HistoryStatus,
    /// Re-read too: a register written, or a file added to `media/`.
    pub package: PackageView,
}

/// Everything wrong with the story, where its names are cited, and what can
/// be undone.
pub fn check(folder: &Path) -> Checked {
    let history = history::with(folder, |h| h.status());
    match draft_dir(folder) {
        Ok((files, d)) => Checked {
            usages: refs::all_usages(&files, &d),
            media: media_list(&d),
            skin_files: (d.skin_files.iter()).flatten().cloned().collect(),
            diagnostics: verdict(folder, d),
            history,
            package: package_view(folder),
        },
        Err(e) => Checked {
            diagnostics: vec![load_error(e)],
            usages: Vec::new(),
            media: Vec::new(),
            skin_files: Vec::new(),
            history,
            package: package_view(folder),
        },
    }
}

/// Everything wrong with a drafted story: what keeps it from loading, or
/// else what the validator finds. The messages are the engine's, in English:
/// the editor is reserved for now, and they are translated when it opens to
/// all (docs/CHANTIERS.md, Story).
pub(super) fn verdict(folder: &Path, d: Draft) -> Vec<Diagnostic> {
    match Story::from_draft(d) {
        // The engine's checks, then what only the pictures can say.
        Ok(story) => {
            let mut found = validate(&story);
            found.extend(crate::story::frames::image_checks(folder, &story));
            found
        }
        Err(errors) => errors.into_iter().map(load_error).collect(),
    }
}

pub(super) fn load_error(e: story_engine::LoadError) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        file: e.file,
        node: None,
        message: e.message,
    }
}
