//! Story Mode, the player side (`docs/story/SPEC-story-mode.md`, « Story
//! Mode: player side »): the stories a player can play, one game in each, and
//! what moves a game on.
//!
//! The stories offered are the sample Pit Box carries
//! (`story_engine::sample`), the packages the player installed
//! (`packages.rs`), and the author's own, those of Create Your Own, played as
//! they are on disk - an author plays what they write. A story the checks
//! find errors in does not start: it would stop half-way on what the checks
//! already know.
//!
//! **One game per story**, saved after every move of the player
//! (`app_config_dir/story/saves/`), so that closing Pit Box never loses one.
//! A new game replaces it. The story's own `saves` setting (autosave per day
//! or manual slots, reload or not) comes later: it decides what a player may
//! go back to, and there is nothing to go back to with one slot.
//!
//! Every command reads the story again and resumes the save: no game lives in
//! memory between two moves, so a story edited in the studio meanwhile plays
//! on as edited - or refuses the save with the reason (`Save::check`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use story_engine::engine::{self, Input};
use story_engine::model::{Character, Driver};
use story_engine::needs::{needed, AcKind};
use story_engine::save::{Save, SaveError};
use story_engine::state::{EventResult, State, Wait};
use story_engine::validate::{validate, Severity};
use story_engine::view::{play_view, PlayView};
use story_engine::Story;

use super::holdings::{Holding, Lookup};
use crate::errors;

/// Where a story is read from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Source {
    /// The sample Pit Box carries.
    Sample,
    /// A story folder of Create Your Own.
    Folder { path: PathBuf },
    /// A package the player installed, by its story's id (`packages.rs`).
    Installed { id: String },
}

impl Source {
    /// The story's folder, where its media are; none for the sample.
    pub fn folder(&self, config_dir: &Path) -> Result<Option<PathBuf>, String> {
        match self {
            Source::Sample => Ok(None),
            Source::Folder { path } => Ok(Some(path.clone())),
            Source::Installed { id } => super::packages::folder(config_dir, id).map(Some),
        }
    }
}

/// A story the hub offers, and the game under way in it.
#[derive(Debug, Serialize)]
pub struct Playable {
    pub source: Source,
    /// `None` when the story does not read.
    pub title: Option<String>,
    /// As its `story.yaml` says; `None` when the story does not read.
    pub version: Option<String>,
    /// Errors the checks find: a story with any does not start.
    pub errors: usize,
    pub game: Option<GameInfo>,
    /// What it races on that this machine lacks, or holds in another
    /// version than the package's.
    pub content: ContentStatus,
}

/// The cars and tracks a story races on, as this machine holds them
/// (SPEC-story-mode, Content: what is missing).
#[derive(Debug, Default, Serialize)]
pub struct ContentStatus {
    /// Not here: a story missing any does not start.
    pub missing: Vec<Holding>,
    /// Here in another version than the package's: played as it is.
    pub other_version: Vec<(AcKind, String)>,
}

#[derive(Debug, Serialize)]
pub struct GameInfo {
    /// The story reached one of its endings.
    pub ended: bool,
    /// Why the game cannot resume (a save that does not read, or that the
    /// story changed under), as an i18n key; `None` when it can.
    pub problem: Option<String>,
}

/// A game as the screen shows it: the engine's view, who speaks in it and
/// who races in it.
#[derive(Debug, Serialize)]
pub struct Game {
    pub view: PlayView,
    pub characters: Vec<Character>,
    pub drivers: Vec<Driver>,
    /// The story's folder, where its media are; none for the sample.
    pub folder: Option<PathBuf>,
    /// When the session under way was launched (ms since the epoch): the
    /// game's result file is the session's only if it is newer.
    pub launched_at_ms: Option<u64>,
    /// Where the player stood in the dialogue under way when last seen.
    pub reading: Option<Reading>,
}

/// The line a dialogue was being read at (SPEC-story-mode, In-person
/// dialogues: saving during a dialogue). The engine plays a conversation from
/// choice to choice; which of its lines the player has read so far is the
/// screen's, kept beside the save so that a game reopened goes on from that
/// line. The screen keeps it only while that item is still the dialogue
/// under way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reading {
    /// The item, as an index into the game's view: a dialogue delivered again
    /// is another item.
    pub item: usize,
    /// The step shown, as an index into the item's steps.
    pub at: usize,
}

/// What the player did, as the screen sends it. A session's result is read
/// by the screen, which knows the names it gave the grid's drivers.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Move {
    Reply {
        content: String,
        reply: String,
    },
    ActionDone {
        action: String,
    },
    Seen {
        content: String,
    },
    /// A session starts: the entry fee is paid now.
    Launch {
        event: String,
    },
    Result {
        event: String,
        result: EventResult,
    },
}

impl From<Move> for Input {
    fn from(m: Move) -> Input {
        match m {
            Move::Reply { content, reply } => Input::Reply { content, reply },
            Move::ActionDone { action } => Input::ActionDone { action },
            Move::Seen { content } => Input::Seen { content },
            Move::Launch { event } => Input::Launch { event },
            Move::Result { event, result } => Input::Result { event, result },
        }
    }
}

/// The sample, the installed packages, then the author's stories in the
/// order of their list, each with what it lacks on this machine.
pub fn list(config_dir: &Path, lookup: Lookup) -> Vec<Playable> {
    let mut sources = vec![Source::Sample];
    sources.extend(
        super::packages::ids(config_dir)
            .into_iter()
            .map(|id| Source::Installed { id }),
    );
    sources.extend(
        (super::studio::folders(config_dir).into_iter())
            .filter(|p| p.join("story.yaml").is_file())
            .map(|path| Source::Folder { path }),
    );
    sources
        .into_iter()
        .map(|source| match read(config_dir, &source) {
            Ok(story) => {
                let errors = (validate(&story).iter())
                    .filter(|d| d.severity == Severity::Error)
                    .count();
                // A save that does not read is still a game: listed, so that a
                // new game asks before replacing it.
                let game = match read_save(config_dir, &source, &story) {
                    Ok(None) => None,
                    Ok(Some(save)) => Some(GameInfo {
                        ended: matches!(save.state.wait, Wait::Ended { .. }),
                        problem: save.check(&story).err().map(|e| save_problem(&source, e)),
                    }),
                    Err(problem) => Some(GameInfo {
                        ended: false,
                        problem: Some(problem),
                    }),
                };
                let missing = (lookup(&needed(&story)).into_iter())
                    .filter(|h| h.folder.is_none())
                    .collect();
                let other_version = match &source {
                    Source::Installed { id } => super::packages::other_versions(config_dir, id),
                    _ => Vec::new(),
                };
                Playable {
                    title: Some(story.manifest.title.clone()),
                    version: Some(story.manifest.version.clone()),
                    errors,
                    game,
                    source,
                    content: ContentStatus { missing, other_version },
                }
            }
            Err(_) => Playable {
                source,
                title: None,
                version: None,
                errors: 1,
                game: None,
                content: ContentStatus::default(),
            },
        })
        .collect()
}

/// Starts a new game, replacing the one under way. `seed` makes its draws:
/// stored in the save, so a reload replays them.
pub fn new_game(config_dir: &Path, source: &Source, seed: u64) -> Result<Game, String> {
    let story = playable(config_dir, source)?;
    let mut state = State::new(&story, seed);
    engine::run(&story, &mut state).map_err(|e| e.to_string())?;
    write_save(config_dir, source, &story, &state)?;
    // The replaced game may have been left in the middle of a session, or of
    // a dialogue.
    for beside in [
        launch_stamp(config_dir, source, &story),
        reading_path(config_dir, source, &story),
    ] {
        if beside.is_file() {
            if let Err(e) = std::fs::remove_file(&beside) {
                log::warn!("story mode: {} not removed: {e}", beside.display());
            }
        }
    }
    Ok(game_in(config_dir, &story, &state, source))
}

/// Keeps where the dialogue under way is being read. Written synchronously,
/// the whole file: when the command returns, it is on disk.
pub fn set_reading(config_dir: &Path, source: &Source, reading: &Reading) -> Result<(), String> {
    let story = read(config_dir, source)?;
    let path = reading_path(config_dir, source, &story);
    let text = serde_json::to_string(reading).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| {
        log::warn!("story mode: {} not written: {e}", path.display());
        format!("writing {}: {e}", path.display())
    })
}

/// The game under way, as it was left.
pub fn resume(config_dir: &Path, source: &Source) -> Result<Game, String> {
    let story = playable(config_dir, source)?;
    let state = resumed(config_dir, source, &story)?;
    Ok(game_in(config_dir, &story, &state, source))
}

/// Plays what the player did, saves, and shows where the game now stands.
/// A move the engine refuses changes nothing, on screen or on disk. The
/// story is not checked again: it was at the start of the game, and the
/// save is checked against it (`Save::check`) at every move.
pub fn play(config_dir: &Path, source: &Source, m: Move) -> Result<Game, String> {
    let story = read(config_dir, source)?;
    let mut state = resumed(config_dir, source, &story)?;
    let launch = matches!(m, Move::Launch { .. });
    let was_launched = state.launched.clone();
    engine::submit(&story, &mut state, m.into()).map_err(|e| e.to_string())?;
    write_save(config_dir, source, &story, &state)?;
    let stamp = launch_stamp(config_dir, source, &story);
    if launch && (was_launched.is_none() || !stamp.is_file()) {
        // Launched again before a result keeps the first time: the result
        // file of the first launch, if the game wrote one, is still this
        // attempt's. A time lost meanwhile is taken again, or the result
        // could never be read.
        let now = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH))
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        std::fs::write(&stamp, now.to_string()).map_err(|e| format!("writing {}: {e}", stamp.display()))?;
    } else if state.launched.is_none() && stamp.is_file() {
        if let Err(e) = std::fs::remove_file(&stamp) {
            log::warn!("story mode: {} not removed: {e}", stamp.display());
        }
    }
    Ok(game_in(config_dir, &story, &state, source))
}

/// Beside the save: when the session under way was launched.
fn launch_stamp(config_dir: &Path, source: &Source, story: &Story) -> PathBuf {
    save_path(config_dir, source, story).with_extension("launched")
}

/// Beside the save: where the dialogue under way is being read.
fn reading_path(config_dir: &Path, source: &Source, story: &Story) -> PathBuf {
    save_path(config_dir, source, story).with_extension("reading")
}

/// `game`, with the launch time of the session under way and where its
/// dialogue is being read.
fn game_in(config_dir: &Path, story: &Story, state: &State, source: &Source) -> Game {
    let mut g = game(config_dir, story, state, source);
    // Unreadable: the dialogue starts again from its first line, no worse.
    g.reading = (std::fs::read_to_string(reading_path(config_dir, source, story)).ok())
        .and_then(|t| serde_json::from_str(&t).ok());
    if state.launched.is_some() {
        g.launched_at_ms =
            (std::fs::read_to_string(launch_stamp(config_dir, source, story)).ok()).and_then(|t| t.trim().parse().ok());
    }
    g
}

fn game(config_dir: &Path, story: &Story, state: &State, source: &Source) -> Game {
    Game {
        view: play_view(story, state),
        characters: story.characters.clone(),
        drivers: story.drivers.clone(),
        // The story was read from it a moment ago.
        folder: source.folder(config_dir).ok().flatten(),
        launched_at_ms: None,
        reading: None,
    }
}

fn read(config_dir: &Path, source: &Source) -> Result<Story, String> {
    let read = match source.folder(config_dir)? {
        None => Story::from_sources(story_engine::sample::files()),
        Some(folder) => Story::load(&folder),
    };
    read.map_err(|errors| {
        // Shown as the checks' count in the hub; the first is enough to log.
        let first = errors.first().map(ToString::to_string).unwrap_or_default();
        log::warn!("story mode: {source:?} does not load: {first}");
        errors::STORY_HAS_ERRORS.to_string()
    })
}

/// The story, if the checks find no error in it.
fn playable(config_dir: &Path, source: &Source) -> Result<Story, String> {
    let story = read(config_dir, source)?;
    if validate(&story).iter().any(|d| d.severity == Severity::Error) {
        return Err(errors::STORY_HAS_ERRORS.into());
    }
    Ok(story)
}

/// `saves/<story id>-<folder>.json` for a story of the author's, where
/// `<folder>` is a fingerprint of its folder: a copy of a story keeps its id,
/// and must not share its game. `saves/builtin/` for the sample: an author's
/// story may well be called `sample` too. `saves/installed/<story id>.json`
/// for a package: an update keeps its id, and its game.
fn save_path(config_dir: &Path, source: &Source, story: &Story) -> PathBuf {
    let dir = config_dir.join("story/saves");
    match source {
        Source::Sample => dir.join("builtin").join(format!("{}.json", story_engine::sample::ID)),
        // The id names a folder already (`package::valid_id`).
        Source::Installed { id } => dir.join("installed").join(format!("{id}.json")),
        Source::Folder { path } => {
            let id: String = (story.manifest.id.chars())
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            let folder = super::studio::stamp(&path.to_string_lossy());
            dir.join(format!("{id}-{}.json", &folder[..8]))
        }
    }
}

/// The game saved for `source`: `None` when there is none, an i18n key when
/// its file does not read.
fn read_save(config_dir: &Path, source: &Source, story: &Story) -> Result<Option<Save>, String> {
    let path = save_path(config_dir, source, story);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(None);
    };
    Save::from_json(&text).map(Some).map_err(|e| save_problem(source, e))
}

/// Why a save cannot resume, as the player reads it: an i18n key. The
/// engine's own words are a diagnostic, kept in the log.
fn save_problem(source: &Source, e: SaveError) -> String {
    log::warn!("story mode: the game of {source:?} cannot resume: {e}");
    match e {
        SaveError::Stale(_) => errors::STORY_SAVE_STALE,
        SaveError::NotASave(_) | SaveError::TooNew { .. } | SaveError::OtherStory { .. } => {
            errors::STORY_SAVE_UNREADABLE
        }
    }
    .into()
}

/// Whether the game under way in `source` would no longer resume on
/// `story`: what an update asks about before it replaces the story
/// (`packages::install`). Not when there is no game, when it ended, or when
/// it did not resume already - nothing is lost then.
pub(super) fn loses_game(config_dir: &Path, source: &Source, story: &Story) -> bool {
    let Ok(Some(save)) = read_save(config_dir, source, story) else {
        return false;
    };
    if matches!(save.state.wait, Wait::Ended { .. }) {
        return false;
    }
    match save.check(story) {
        Ok(()) => false,
        Err(e) => {
            log::warn!(
                "story mode: the game of {source:?} would not resume on version {}: {e}",
                story.manifest.version
            );
            true
        }
    }
}

fn resumed(config_dir: &Path, source: &Source, story: &Story) -> Result<State, String> {
    let save = read_save(config_dir, source, story)?.ok_or(errors::STORY_NO_GAME)?;
    save.resume(story).map_err(|e| save_problem(source, e))
}

/// Written whole, through a temporary file renamed over the old one: a crash
/// half-way leaves the last save, never half of one.
fn write_save(config_dir: &Path, source: &Source, story: &Story, state: &State) -> Result<(), String> {
    let path = save_path(config_dir, source, story);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, Save::new(story, state).to_json()).map_err(|e| format!("writing {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("writing {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::story::holdings::all_held;
    use crate::testutil::temp_dir;
    use story_engine::view::Waiting;

    fn first_reply(game: &Game) -> (String, String) {
        let item = (game.view.items.iter()).find(|i| i.open).expect("a conversation waits");
        (item.content.clone(), item.choices[0].label.clone())
    }

    // A game is saved after every move: resumed later, it stands where the
    // player left it, with what they saw (SPEC-story-mode, Story Mode:
    // player side).
    #[test]
    fn a_game_is_saved_at_every_move_and_resumes_there() {
        let config = temp_dir("story-play");
        assert!(
            resume(&config, &Source::Sample).is_err(),
            "no game before the first one"
        );
        let game = new_game(&config, &Source::Sample, 7).expect("starts");
        let (content, reply) = first_reply(&game);
        let moved = play(&config, &Source::Sample, Move::Reply { content, reply }).expect("plays");
        assert!(matches!(moved.view.waiting, Waiting::Event { .. }), "on to the race");
        let back = resume(&config, &Source::Sample).expect("resumes");
        assert_eq!(
            format!("{:?}", back.view),
            format!("{:?}", moved.view),
            "the same game, as it was left"
        );
        let listed = list(&config, &all_held);
        assert!(
            matches!(
                &listed[0],
                Playable {
                    source: Source::Sample,
                    errors: 0,
                    game: Some(_),
                    ..
                }
            ),
            "the hub shows the game under way"
        );
        assert!(
            config.join("story/saves/builtin/sample.json").is_file(),
            "in Pit Box's own data"
        );
    }

    // Two copies of a story share its id but not its game; a save the story
    // changed under is listed, says why it cannot resume, and is refused with
    // a key the player can read.
    #[test]
    fn copies_keep_their_own_game_and_a_stale_one_says_why() {
        let config = temp_dir("story-play");
        let base = temp_dir("story-play-folder");
        let flow = "id: ch01\nnodes:\n  - id: start\n    action: go\n  - id: end\n    end: done\n";
        let mut sources = Vec::new();
        for name in ["a", "b"] {
            let folder = base.join(name).join("same");
            super::super::studio::create(&config, &folder, "S", "en", story_engine::frame::Frame::None, "C").unwrap();
            std::fs::write(folder.join("flow/01-chapter.yaml"), flow).unwrap();
            let yaml = std::fs::read_to_string(folder.join("story.yaml")).unwrap();
            let start = yaml
                .lines()
                .find(|l| l.trim_start().starts_with("node:"))
                .unwrap()
                .to_string();
            std::fs::write(folder.join("story.yaml"), yaml.replace(&start, "  node: start")).unwrap();
            sources.push(Source::Folder { path: folder });
        }
        new_game(&config, &sources[0], 1).expect("the first copy starts");
        assert_eq!(
            resume(&config, &sources[1]).unwrap_err(),
            errors::STORY_NO_GAME,
            "the second copy has no game of its own"
        );
        let Source::Folder { path } = &sources[0] else {
            unreachable!()
        };
        std::fs::write(path.join("flow/01-chapter.yaml"), flow.replace("start", "begin")).unwrap();
        let yaml = std::fs::read_to_string(path.join("story.yaml")).unwrap();
        std::fs::write(path.join("story.yaml"), yaml.replace("node: start", "node: begin")).unwrap();
        assert_eq!(resume(&config, &sources[0]).unwrap_err(), errors::STORY_SAVE_STALE);
        let listed = list(&config, &all_held);
        let entry = listed.iter().find(|p| p.source == sources[0]).unwrap();
        assert_eq!(
            entry.game.as_ref().and_then(|g| g.problem.as_deref()),
            Some(errors::STORY_SAVE_STALE),
            "listed with why it cannot resume"
        );
    }

    // A launch remembers when it happened, so that the game's result file
    // is read only if it is newer; launched again before a result, the first
    // time stands; the result ends the session (SPEC-story-mode, Events).
    #[test]
    fn a_launch_is_timed_until_its_result() {
        let config = temp_dir("story-play");
        let game = new_game(&config, &Source::Sample, 7).unwrap();
        let (content, reply) = first_reply(&game);
        play(&config, &Source::Sample, Move::Reply { content, reply }).unwrap();
        let launch = || {
            play(
                &config,
                &Source::Sample,
                Move::Launch {
                    event: "race_01".into(),
                },
            )
            .expect("launched")
        };
        let first = launch();
        assert_eq!(first.view.launched.as_deref(), Some("race_01"));
        let at = first.launched_at_ms.expect("timed");
        assert_eq!(
            launch().launched_at_ms,
            Some(at),
            "launched again: the first time stands"
        );
        assert_eq!(
            resume(&config, &Source::Sample).unwrap().launched_at_ms,
            Some(at),
            "kept on disk"
        );
        let result = EventResult {
            finished: true,
            position: Some(1),
            ..Default::default()
        };
        let after = play(
            &config,
            &Source::Sample,
            Move::Result {
                event: "race_01".into(),
                result,
            },
        )
        .expect("result");
        assert_eq!(
            (after.view.launched, after.launched_at_ms),
            (None, None),
            "the session is over"
        );
        assert!(
            !config.join("story/saves/builtin/sample.launched").exists(),
            "its time goes with it"
        );
    }

    // L3b.2: where a dialogue is being read is kept beside the save, comes
    // back with the game, and goes with a new game.
    #[test]
    fn where_a_dialogue_is_read_comes_back_with_the_game() {
        let config = temp_dir("story-play");
        new_game(&config, &Source::Sample, 7).unwrap();
        let reading = Reading { item: 0, at: 1 };
        set_reading(&config, &Source::Sample, &reading).expect("kept");
        assert_eq!(resume(&config, &Source::Sample).unwrap().reading, Some(reading));
        assert_eq!(
            new_game(&config, &Source::Sample, 7).unwrap().reading,
            None,
            "a new game starts no dialogue half-read"
        );
        assert!(!config.join("story/saves/builtin/sample.reading").exists());
    }

    // A move the engine refuses changes nothing on disk: the save is the one
    // before it.
    #[test]
    fn a_refused_move_leaves_the_save_alone() {
        let config = temp_dir("story-play");
        let game = new_game(&config, &Source::Sample, 7).unwrap();
        let (content, _) = first_reply(&game);
        let path = config.join("story/saves/builtin/sample.json");
        let before = std::fs::read_to_string(&path).unwrap();
        let refused = play(
            &config,
            &Source::Sample,
            Move::Reply {
                content,
                reply: "not_a_reply".into(),
            },
        );
        assert!(refused.is_err(), "not offered");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before, "save untouched");
    }

    // A story with errors does not start: it would stop half-way on what
    // the checks already say.
    #[test]
    fn a_story_with_errors_does_not_start() {
        let config = temp_dir("story-play");
        let base = temp_dir("story-play-folder");
        let folder = base.join("s");
        super::super::studio::create(&config, &folder, "S", "en", story_engine::frame::Frame::None, "C").unwrap();
        std::fs::write(
            folder.join("flow/01-chapter.yaml"),
            "id: ch01\nnodes:\n  - id: start\n    message: ghost\n",
        )
        .unwrap();
        let source = Source::Folder { path: folder.clone() };
        assert_eq!(new_game(&config, &source, 1).unwrap_err(), errors::STORY_HAS_ERRORS);
        let listed = list(&config, &all_held);
        assert!(
            listed.iter().any(|p| p.source == source && p.errors > 0),
            "listed, with its errors"
        );
    }

    // The sample plays on any install of Assetto Corsa (SPEC-story-mode,
    // Engine file format: the sample shows how everything is made): every
    // track and car its events name is Kunos's base game, never a DLC or a
    // mod. Liveries and layouts are not in the table: they were checked on
    // the game when the sample was written.
    #[test]
    fn the_sample_races_on_the_base_game_alone() {
        use crate::modscan::ModKind;
        let story = Story::from_sources(story_engine::sample::files()).expect("loads");
        let base = |kind: ModKind, id: &str| {
            assert!(
                crate::kunos_dates::is_official(kind, id) && crate::kunos_dates::pack_name(kind, id).is_none(),
                "{id} is base game content"
            )
        };
        for event in &story.events {
            let s = &event.session;
            base(ModKind::Track, s.track.as_deref().expect("a track"));
            base(ModKind::Car, s.car.as_deref().expect("a car"));
            for car in s.opponents.iter().filter_map(|o| o.car.as_deref()) {
                base(ModKind::Car, car);
            }
        }
    }

    // A story missing a car or a track it races on says which, and the DLC
    // to buy when it is the game's own (SPEC-story-mode, Content).
    #[test]
    fn what_a_story_lacks_is_listed() {
        let config = temp_dir("story-play-missing");
        let lacking = |w: &std::collections::BTreeSet<(AcKind, String)>| {
            crate::story::holdings::held_as(w, |(kind, _)| (*kind == AcKind::Track).then(|| PathBuf::from("t")))
        };
        let listed = list(&config, &lacking);
        let sample = listed.iter().find(|p| p.source == Source::Sample).unwrap();
        let missing: Vec<&str> = sample.content.missing.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(
            missing,
            ["abarth500", "bmw_1m", "bmw_m3_e30", "lotus_elise_sc"],
            "the cars, not the tracks"
        );
        assert!(
            list(&config, &all_held)[0].content.missing.is_empty(),
            "nothing lacking, nothing said"
        );
    }
}
