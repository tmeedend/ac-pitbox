//! What the editor shows without writing: the Content screen's preview of
//! an item, the Scenes screen's variant, and the Form view of a text.

use std::path::Path;

use serde::{Deserialize, Serialize};
use story_engine::frame::Frame;
use story_engine::load::draft_dir;
use story_engine::scene::{self, Period, Season, Shown};
use story_engine::Story;

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
