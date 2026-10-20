//! Story Mode, the player side (`docs/story/SPEC-story-mode.md`, « Story
//! Mode: player side »): the stories a player can play, one game in each, and
//! what moves a game on.
//!
//! Until stories come as packages (L4), the stories offered are the sample
//! Pit Box carries (`story_engine::sample`) and the author's own, those of
//! Create Your Own, played as they are on disk - an author plays what they
//! write. A story the checks find errors in does not start: it would stop
//! half-way on what the checks already know.
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
use story_engine::save::{Save, SaveError};
use story_engine::state::{State, Wait};
use story_engine::validate::{validate, Severity};
use story_engine::view::{play_view, PlayView};
use story_engine::Story;

use crate::errors;

/// Where a story is read from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Source {
    /// The sample Pit Box carries.
    Sample,
    /// A story folder of Create Your Own.
    Folder { path: PathBuf },
}

/// A story the hub offers, and the game under way in it.
#[derive(Debug, Serialize)]
pub struct Playable {
    pub source: Source,
    /// `None` when the story does not read.
    pub title: Option<String>,
    /// Errors the checks find: a story with any does not start.
    pub errors: usize,
    pub game: Option<GameInfo>,
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
}

/// What the player did, as the screen sends it. Launching an event and its
/// result come with the races (L3.2).
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Move {
    Reply { content: String, reply: String },
    ActionDone { action: String },
    Seen { content: String },
}

impl From<Move> for Input {
    fn from(m: Move) -> Input {
        match m {
            Move::Reply { content, reply } => Input::Reply { content, reply },
            Move::ActionDone { action } => Input::ActionDone { action },
            Move::Seen { content } => Input::Seen { content },
        }
    }
}

/// The sample, then the author's stories in the order of their list.
pub fn list(config_dir: &Path) -> Vec<Playable> {
    let mut sources = vec![Source::Sample];
    sources.extend(
        (super::studio::folders(config_dir).into_iter())
            .filter(|p| p.join("story.yaml").is_file())
            .map(|path| Source::Folder { path }),
    );
    sources
        .into_iter()
        .map(|source| match read(&source) {
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
                Playable {
                    title: Some(story.manifest.title.clone()),
                    errors,
                    game,
                    source,
                }
            }
            Err(_) => Playable {
                source,
                title: None,
                errors: 1,
                game: None,
            },
        })
        .collect()
}

/// Starts a new game, replacing the one under way. `seed` makes its draws:
/// stored in the save, so a reload replays them.
pub fn new_game(config_dir: &Path, source: &Source, seed: u64) -> Result<Game, String> {
    let story = playable(source)?;
    let mut state = State::new(&story, seed);
    engine::run(&story, &mut state).map_err(|e| e.to_string())?;
    write_save(config_dir, source, &story, &state)?;
    Ok(game(&story, &state, source))
}

/// The game under way, as it was left.
pub fn resume(config_dir: &Path, source: &Source) -> Result<Game, String> {
    let story = playable(source)?;
    let state = resumed(config_dir, source, &story)?;
    Ok(game(&story, &state, source))
}

/// Plays what the player did, saves, and shows where the game now stands.
/// A move the engine refuses changes nothing, on screen or on disk. The
/// story is not checked again: it was at the start of the game, and the
/// save is checked against it (`Save::check`) at every move.
pub fn play(config_dir: &Path, source: &Source, m: Move) -> Result<Game, String> {
    let story = read(source)?;
    let mut state = resumed(config_dir, source, &story)?;
    engine::submit(&story, &mut state, m.into()).map_err(|e| e.to_string())?;
    write_save(config_dir, source, &story, &state)?;
    Ok(game(&story, &state, source))
}

fn game(story: &Story, state: &State, source: &Source) -> Game {
    Game {
        view: play_view(story, state),
        characters: story.characters.clone(),
        drivers: story.drivers.clone(),
        folder: match source {
            Source::Sample => None,
            Source::Folder { path } => Some(path.clone()),
        },
    }
}

fn read(source: &Source) -> Result<Story, String> {
    let read = match source {
        Source::Sample => Story::from_sources(story_engine::sample::files()),
        Source::Folder { path } => Story::load(path),
    };
    read.map_err(|errors| {
        // Shown as the checks' count in the hub; the first is enough to log.
        let first = errors.first().map(ToString::to_string).unwrap_or_default();
        log::warn!("story mode: {source:?} does not load: {first}");
        errors::STORY_HAS_ERRORS.to_string()
    })
}

/// The story, if the checks find no error in it.
fn playable(source: &Source) -> Result<Story, String> {
    let story = read(source)?;
    if validate(&story).iter().any(|d| d.severity == Severity::Error) {
        return Err(errors::STORY_HAS_ERRORS.into());
    }
    Ok(story)
}

/// `saves/<story id>-<folder>.json` for a story of the author's, where
/// `<folder>` is a fingerprint of its folder: a copy of a story keeps its id,
/// and must not share its game. `saves/builtin/` for the sample: an author's
/// story may well be called `sample` too.
fn save_path(config_dir: &Path, source: &Source, story: &Story) -> PathBuf {
    let dir = config_dir.join("story/saves");
    match source {
        Source::Sample => dir.join("builtin").join(format!("{}.json", story_engine::sample::ID)),
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
        let listed = list(&config);
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
        let listed = list(&config);
        let entry = listed.iter().find(|p| p.source == sources[0]).unwrap();
        assert_eq!(
            entry.game.as_ref().and_then(|g| g.problem.as_deref()),
            Some(errors::STORY_SAVE_STALE),
            "listed with why it cannot resume"
        );
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
        let listed = list(&config);
        assert!(
            listed.iter().any(|p| p.source == source && p.errors > 0),
            "listed, with its errors"
        );
    }
}
