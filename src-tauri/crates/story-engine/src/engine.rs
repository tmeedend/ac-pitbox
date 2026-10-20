//! Running a story (SPEC-story-mode, Story model): advance the flow as far as
//! it goes, deliver whatever reactive content has become true, and stop on
//! what the player must do - drive an event, answer, act in the frame.
//!
//! The host (Story Mode, the CLI, later the balance simulator) only ever
//! calls [`run`] once and then [`submit`] with what the player did. Both hand
//! back what happened, ending with what the flow now waits for. Every rule
//! the story's settings decide is read here from `Settings`, never assumed.

use std::fmt;

use crate::expr::{Criterion, Effect, Literal, Var};
use crate::load::Story;
use crate::model::{Beat, FailureMode, Node, NodeKind, Placement, Repeat, RetryCost};
use crate::state::{Entry, EventResult, Happening, Scheduled, State, Verdict, Via, Wait};
use crate::time::Span;

/// What the host must show or do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    Delivered {
        content: String,
        via: Via,
    },
    TimeMoved {
        to: crate::time::StoryDate,
    },
    /// The flow waits for this event. `difficulty_step`: how many steps of
    /// skill the launcher takes off the opponents (adaptive difficulty).
    AwaitEvent {
        event: String,
        attempt: u32,
        difficulty_step: u32,
        fee: Option<i64>,
    },
    AwaitReply {
        content: String,
    },
    AwaitAction {
        action: String,
    },
    Ended {
        ending: String,
    },
}

/// What the player did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// A session starts for this event (the flow's, or an open side event):
    /// the entry fee is paid now.
    Launch { event: String },
    /// Its result came back.
    Result { event: String, result: EventResult },
    /// The player chose a reply on a delivered content item.
    Reply { content: String, reply: String },
    /// The action the flow waits for is done.
    ActionDone { action: String },
    /// The player opened a content item.
    Seen { content: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// More steps than any story needs without waiting for the player: a
    /// cycle of beats and branches the state never leaves.
    Loop {
        node: String,
    },
    UnknownNode(String),
    /// A node with no `next` at the end of the flow.
    FallsOffFlow {
        node: String,
    },
    /// The input does not answer what the flow waits for.
    Unexpected(String),
    CannotAfford {
        fee: i64,
        money: i64,
    },
    StoryEnded,
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::Loop { node } => {
                write!(f, "the flow loops around `{node}` without ever waiting for the player")
            }
            EngineError::UnknownNode(n) => write!(f, "unknown node `{n}`"),
            EngineError::FallsOffFlow { node } => {
                write!(f, "node `{node}` leads nowhere: the flow ends without an `end` node")
            }
            EngineError::Unexpected(why) => f.write_str(why),
            EngineError::CannotAfford { fee, money } => write!(f, "entry fee {fee}, money {money}"),
            EngineError::StoryEnded => f.write_str("the story has ended"),
        }
    }
}

/// Steps without waiting before the engine calls it a loop. A real chapter
/// is a few dozen nodes; a beat-and-branch counting loop a few hundred.
const MAX_STEPS: usize = 10_000;

/// Advances the flow from where the state stands, until it waits.
pub fn run(story: &Story, state: &mut State) -> Result<Vec<Output>, EngineError> {
    let mut r = Run {
        story,
        state,
        out: Vec::new(),
    };
    r.advance()?;
    Ok(r.out)
}

/// Applies what the player did, then advances the flow. All or nothing: on
/// an error the state is left as it was, so the host can tell the player and
/// let them try again - a result refused half-way must not lose the launch.
pub fn submit(story: &Story, state: &mut State, input: Input) -> Result<Vec<Output>, EngineError> {
    if matches!(state.wait, Wait::Ended { .. }) && !matches!(input, Input::Seen { .. }) {
        return Err(EngineError::StoryEnded);
    }
    let mut next = state.clone();
    let mut r = Run {
        story,
        state: &mut next,
        out: Vec::new(),
    };
    r.apply(input)?;
    r.advance()?;
    let out = r.out;
    *state = next;
    Ok(out)
}

/// Whether a result satisfies an event's success criteria: all of them, and
/// finishing when there are none.
pub fn succeeded(criteria: &[Criterion], result: &EventResult) -> bool {
    if let Some(verdict) = result.forced {
        return verdict == Verdict::Success;
    }
    if !result.finished {
        return false;
    }
    criteria.iter().all(|c| match c {
        Criterion::Finished => true,
        Criterion::PositionAtMost(n) => result.position.is_some_and(|p| p <= *n),
        Criterion::AheadOf(driver) => result.ahead_of.iter().any(|d| d == driver),
        Criterion::LapUnder(ms) => result.best_lap_ms.is_some_and(|t| t < *ms),
    })
}

/// Adaptive difficulty (SPEC-story-mode, Hard events): a step every `every`
/// consecutive failures, up to `cap`.
pub fn difficulty_step(story: &Story, state: &State, event: &str) -> u32 {
    match story.settings().adaptive_difficulty {
        Some(a) if a.every > 0 => (state.event(event).consecutive_failures / a.every).min(a.cap),
        _ => 0,
    }
}

/// The fee a launch of `event` would cost now, if any.
pub fn fee_due(story: &Story, state: &State, event: &str) -> Option<i64> {
    let fee = story.event(event)?.entry_fee.filter(|_| story.settings().money)?;
    let side = state.side_events.contains(event) && !matches!(&state.wait, Wait::Event { event: e } if e == event);
    let pays = side || story.settings().retry_cost == RetryCost::Paid || state.event(event).paid == 0;
    pays.then_some(fee)
}

fn unexpected(why: String) -> Result<(), EngineError> {
    Err(EngineError::Unexpected(why))
}

struct Run<'a> {
    story: &'a Story,
    state: &'a mut State,
    out: Vec<Output>,
}

impl<'a> Run<'a> {
    fn log(&mut self, what: Happening) {
        self.state.history.push(Entry {
            date: self.state.date,
            clock: self.state.clock,
            what,
        });
    }

    fn node(&self, id: &str) -> Result<&'a Node, EngineError> {
        let story: &'a Story = self.story;
        story.node(id).ok_or_else(|| EngineError::UnknownNode(id.into()))
    }

    /// Moves the cursor along a node's normal exit.
    fn go_next(&mut self, from: &str) -> Result<(), EngineError> {
        let node = self.node(from)?;
        let next = self
            .story
            .next_of(node)
            .ok_or_else(|| EngineError::FallsOffFlow { node: from.into() })?;
        self.go_to(next.id.clone());
        Ok(())
    }

    fn go_to(&mut self, node: String) {
        self.state.node = node;
        self.state.wait = Wait::Running;
    }

    fn advance(&mut self) -> Result<(), EngineError> {
        self.check_reactive()?;
        let mut steps = 0;
        while self.state.wait == Wait::Running {
            steps += 1;
            if steps > MAX_STEPS {
                return Err(EngineError::Loop {
                    node: self.state.node.clone(),
                });
            }
            let id = self.state.node.clone();
            let node = self.node(&id)?;
            match &node.kind {
                NodeKind::Beat(beat) => {
                    self.beat(beat)?;
                    self.go_next(&id)?;
                }
                NodeKind::Message { content, wait_reply } => {
                    let (content, wait_reply) = (content.clone(), *wait_reply);
                    self.deliver(&content, Via::Flow);
                    if wait_reply {
                        self.state.wait = Wait::Reply { content };
                    } else {
                        self.go_next(&id)?;
                    }
                }
                NodeKind::Event { event, .. } => self.state.wait = Wait::Event { event: event.clone() },
                NodeKind::Branch { cases, otherwise } => {
                    let target = cases
                        .iter()
                        .find(|c| self.state.all_hold(self.story, &c.when))
                        .map_or(otherwise, |c| &c.goto);
                    self.go_to(target.clone());
                }
                NodeKind::Action { action } => self.state.wait = Wait::Action { action: action.clone() },
                NodeKind::SideEvents { open, close } => {
                    self.state.side_events.extend(open.iter().cloned());
                    for e in close {
                        self.state.side_events.remove(e);
                    }
                    self.go_next(&id)?;
                }
                NodeKind::End { ending } => {
                    let ending = ending.clone();
                    self.log(Happening::Ended { ending: ending.clone() });
                    self.state.wait = Wait::Ended { ending };
                }
            }
            self.check_reactive()?;
        }
        let awaiting = match &self.state.wait {
            Wait::Event { event } => Output::AwaitEvent {
                event: event.clone(),
                attempt: self.state.event(event).attempts + 1,
                difficulty_step: difficulty_step(self.story, self.state, event),
                fee: fee_due(self.story, self.state, event),
            },
            Wait::Reply { content } => Output::AwaitReply {
                content: content.clone(),
            },
            Wait::Action { action } => Output::AwaitAction { action: action.clone() },
            Wait::Ended { ending } => Output::Ended { ending: ending.clone() },
            Wait::Running => unreachable!("the loop above only exits once the flow waits"),
        };
        self.out.push(awaiting);
        Ok(())
    }

    fn beat(&mut self, beat: &Beat) -> Result<(), EngineError> {
        for effect in &beat.effects {
            self.apply_effect(effect)?;
        }
        if let Some(clock) = beat.clock {
            self.state.clock = clock;
        }
        if let Some(span) = beat.advance {
            self.move_time(span);
        }
        Ok(())
    }

    fn move_time(&mut self, span: Span) {
        let from = self.state.date;
        self.state.date = from.advance(span);
        self.log(Happening::TimeMoved { from });
        self.out.push(Output::TimeMoved { to: self.state.date });
    }

    fn deliver(&mut self, content: &str, via: Via) {
        self.state.content.entry(content.to_string()).or_default().delivered += 1;
        self.log(Happening::Delivered {
            content: content.into(),
            via: via.clone(),
        });
        self.out.push(Output::Delivered {
            content: content.into(),
            via,
        });
    }

    fn apply_effect(&mut self, effect: &Effect) -> Result<(), EngineError> {
        if let Effect::Send(content) = effect {
            self.deliver(content, Via::Effect);
            return Ok(());
        }
        let s = &mut *self.state;
        let scale = &self.story.settings().relations;
        match effect {
            Effect::Send(_) => {}
            Effect::Add(var, n) => match var {
                Var::Reputation => s.reputation += n,
                Var::Followers => s.followers += n,
                Var::Money => s.money += n,
                Var::Counter(c) => *s.counters.entry(c.clone()).or_default() += n,
                Var::Rel(who) => {
                    if let Some(level) = s.relations.get(who).and_then(|l| scale.shift(l, *n)) {
                        s.relations.insert(who.clone(), level.to_string());
                    }
                }
                _ => {}
            },
            Effect::Set(var, value) => match (var, value) {
                (Var::Reputation, Literal::Int(n)) => s.reputation = *n,
                (Var::Followers, Literal::Int(n)) => s.followers = *n,
                (Var::Money, Literal::Int(n)) => s.money = *n,
                (Var::Counter(c), Literal::Int(n)) => {
                    s.counters.insert(c.clone(), *n);
                }
                (Var::Flag(f), Literal::Bool(b)) | (Var::Car(f), Literal::Bool(b)) => {
                    let set = if matches!(var, Var::Flag(_)) {
                        &mut s.flags
                    } else {
                        &mut s.cars
                    };
                    if *b {
                        set.insert(f.clone());
                    } else {
                        set.remove(f);
                    }
                }
                (Var::Rel(who), Literal::Name(level)) if scale.rank(level).is_some() => {
                    s.relations.insert(who.clone(), level.clone());
                }
                _ => {}
            },
        }
        Ok(())
    }

    /// Delivers the reactive content that has become due (SPEC-story-mode,
    /// Reactive content). Runs after every change of state; repeats while a
    /// delivery makes something else true, since a condition can read
    /// `content.<id>.delivered`.
    fn check_reactive(&mut self) -> Result<(), EngineError> {
        let story: &'a Story = self.story;
        let mut reactive: Vec<_> = story
            .content
            .iter()
            .filter_map(|c| match &c.placement {
                Placement::Reactive {
                    when,
                    delay,
                    priority,
                    repeat,
                } => Some((c.id.as_str(), when, *delay, *priority, *repeat)),
                _ => None,
            })
            .collect();
        // Higher priority first; equal ones keep the order of the files.
        reactive.sort_by_key(|&(_, _, _, priority, _)| std::cmp::Reverse(priority));
        for _ in 0..MAX_STEPS {
            let mut changed = false;
            // Delays first: an item whose conditions no longer hold when its
            // delay runs out is dropped, and may be scheduled again later.
            let date = self.state.date;
            let (due, waiting): (Vec<Scheduled>, Vec<Scheduled>) = std::mem::take(&mut self.state.scheduled)
                .into_iter()
                .partition(|s| s.due <= date);
            self.state.scheduled = waiting;
            for s in due {
                if let Some(Placement::Reactive { when, .. }) =
                    self.story.content_item(&s.content).map(|c| &c.placement)
                {
                    if self.state.all_hold(self.story, when) {
                        self.deliver(&s.content, Via::Reactive);
                        changed = true;
                    }
                }
            }
            for &(id, when, delay, _, repeat) in &reactive {
                let holds = self.state.all_hold(self.story, when);
                let fire = match repeat {
                    Repeat::Once => holds && self.state.content_facts(id).delivered == 0,
                    Repeat::EachTime => {
                        let was = self.state.armed.contains(id);
                        if holds != was {
                            if holds {
                                self.state.armed.insert(id.to_string());
                            } else {
                                self.state.armed.remove(id);
                            }
                        }
                        holds && !was
                    }
                };
                if !fire || self.state.scheduled.iter().any(|s| s.content == id) {
                    continue;
                }
                match delay {
                    Some(span) => self.state.scheduled.push(Scheduled {
                        content: id.into(),
                        due: date.advance(span),
                    }),
                    None => {
                        self.deliver(id, Via::Reactive);
                        changed = true;
                    }
                }
            }
            if !changed {
                return Ok(());
            }
        }
        Err(EngineError::Loop {
            node: self.state.node.clone(),
        })
    }

    fn apply(&mut self, input: Input) -> Result<(), EngineError> {
        match input {
            Input::Launch { event } => self.launch(event)?,
            Input::Result { event, result } => {
                if self.state.launched.as_deref() != Some(event.as_str()) {
                    return unexpected(format!("no session was launched for `{event}`"));
                }
                self.state.launched = None;
                self.event_result(event, result)?;
            }
            Input::Reply { content, reply } => self.reply(content, reply)?,
            Input::ActionDone { action } => {
                if !matches!(&self.state.wait, Wait::Action { action: a } if *a == action) {
                    return unexpected(format!("the flow is not waiting for action `{action}`"));
                }
                self.log(Happening::ActionDone { action });
                let node = self.state.node.clone();
                self.go_next(&node)?;
            }
            Input::Seen { content } => {
                if self.state.content_facts(&content).delivered == 0 {
                    return unexpected(format!("`{content}` was never delivered"));
                }
                self.state.content.entry(content).or_default().seen = true;
            }
        }
        Ok(())
    }

    /// A session starts: the flow's event or an open side event. The entry
    /// fee is paid here, before the player drives, so that a player who
    /// cannot afford it is told before the game starts.
    fn launch(&mut self, event: String) -> Result<(), EngineError> {
        let flow_event = matches!(&self.state.wait, Wait::Event { event: e } if *e == event);
        if !flow_event && !self.state.side_events.contains(&event) {
            return unexpected(format!(
                "event `{event}` is neither the one the flow waits for nor an open side event"
            ));
        }
        if self.state.launched.as_deref() == Some(event.as_str()) {
            // Launched again before a result (the game was closed early):
            // the fee was already paid for this attempt.
            return Ok(());
        }
        if let Some(running) = &self.state.launched {
            // Its fee is paid: replacing it would lose the attempt. The host
            // closes it with a result first (a DNF when the game crashed).
            return unexpected(format!("`{running}` is still running: submit its result first"));
        }
        let fee = fee_due(self.story, self.state, &event);
        if let Some(fee) = fee {
            if self.state.money < fee {
                return Err(EngineError::CannotAfford {
                    fee,
                    money: self.state.money,
                });
            }
            self.state.money -= fee;
            self.state.events.entry(event.clone()).or_default().paid += 1;
        }
        self.state.launched = Some(event.clone());
        self.log(Happening::Launched { event, fee });
        Ok(())
    }

    /// A reply to any delivered content item that offers replies - the one
    /// the flow waits on, or a reactive message - once per item.
    fn reply(&mut self, content: String, reply: String) -> Result<(), EngineError> {
        let story: &'a Story = self.story;
        let Some(item) = story.content_item(&content) else {
            return unexpected(format!("unknown content `{content}`"));
        };
        let facts = self.state.content_facts(&content);
        if facts.delivered == 0 {
            return unexpected(format!("`{content}` was never delivered"));
        }
        if facts.reply.is_some() {
            return unexpected(format!("`{content}` already has a reply"));
        }
        let Some(chosen) = item.replies.iter().find(|r| r.id == reply) else {
            let known: Vec<_> = item.replies.iter().map(|r| r.id.as_str()).collect();
            return unexpected(format!(
                "`{content}` has no reply `{reply}` (replies: {})",
                known.join(", ")
            ));
        };
        for effect in &chosen.effects {
            self.apply_effect(effect)?;
        }
        self.state.content.entry(content.clone()).or_default().reply = Some(reply.clone());
        self.log(Happening::Replied {
            content: content.clone(),
            reply,
        });
        if matches!(&self.state.wait, Wait::Reply { content: c } if *c == content) {
            let node = self.state.node.clone();
            self.go_next(&node)?;
        }
        Ok(())
    }

    /// SPEC-story-mode, Events, retries and failure. The result is recorded
    /// first, so that a failure pool's conditions can read it ("so close"
    /// after a P2).
    fn event_result(&mut self, event_id: String, result: EventResult) -> Result<(), EngineError> {
        let story: &'a Story = self.story;
        let Some(event) = story.event(&event_id) else {
            return Err(EngineError::Unexpected(format!("unknown event `{event_id}`")));
        };
        let success = succeeded(&event.success, &result);
        let facts = self.state.events.entry(event_id.clone()).or_default();
        facts.attempts += 1;
        let attempt = facts.attempts;
        if success {
            facts.successes += 1;
            facts.consecutive_failures = 0;
        } else {
            facts.consecutive_failures += 1;
        }
        facts.last = Some(result.clone());
        self.log(Happening::Result {
            event: event_id.clone(),
            attempt,
            result,
            success,
        });

        let flow_node = match &self.state.wait {
            Wait::Event { event } if *event == event_id => Some(self.state.node.clone()),
            _ => None,
        };
        if success {
            for effect in &event.on_success.effects {
                self.apply_effect(effect)?;
            }
            // A side event never moves the story on.
            if let Some(node) = flow_node {
                if let Some(span) = event.on_success.advance {
                    self.move_time(span);
                }
                self.go_next(&node)?;
            }
            return Ok(());
        }

        for effect in &event.on_failure.effects {
            self.apply_effect(effect)?;
        }
        if let Some(pool) = &event.on_failure.pool {
            self.draw_pool(pool, event.on_failure.pick, &event_id);
        }
        let Some(node) = flow_node else { return Ok(()) };
        let NodeKind::Event { on_failure, .. } = &self.node(&node)?.kind else {
            return Ok(());
        };
        // The author's failure exit wins; without one, the story's setting.
        if let Some(target) = on_failure {
            self.go_to(target.clone());
            return Ok(());
        }
        let settings = self.story.settings();
        let retry = match settings.failure {
            FailureMode::Retry => settings.max_attempts.is_none_or(|max| attempt < max),
            FailureMode::Final => false,
        };
        if !retry {
            self.go_next(&node)?;
        }
        Ok(())
    }

    /// Sends `pick` variants of a failure pool, among those whose conditions
    /// hold, never one already sent until every eligible one has been.
    fn draw_pool(&mut self, pool: &str, pick: u8, event: &str) {
        let eligible: Vec<String> = self
            .story
            .pool_variants(pool)
            .filter(|(_, when)| self.state.all_hold(self.story, when))
            .map(|(item, _)| item.id.clone())
            .collect();
        let used = self.state.pools.entry(pool.to_string()).or_default();
        let mut fresh: Vec<String> = eligible.iter().filter(|id| !used.contains(*id)).cloned().collect();
        if fresh.is_empty() {
            used.clear();
            fresh = eligible;
        }
        for _ in 0..usize::from(pick).min(fresh.len()) {
            let id = fresh.remove(self.state.rng.below(fresh.len()));
            self.state.pools.entry(pool.to_string()).or_default().insert(id.clone());
            self.deliver(&id, Via::Pool { event: event.into() });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load::SourceFile;
    use crate::testutil::with;

    const MANIFEST: &str =
        "format: 1\nid: t\ntitle: T\nversion: \"1\"\nlanguages: [en]\nstart:\n  node: start\n  date: 2000-01-31\n  money: 250\n";
    const RACE: &str = "id: race\ntype: race\nsuccess: [\"position <= 3\"]\non_success:\n  effects: [\"reputation += 2\"]\n  advance: 1 month\non_failure:\n  pool: fail\nentry_fee: 100\n";
    const POOL: &str = "---\nid: fail_a\ntype: message\nfrom: friend\npool: fail\n---\nA.\n---\nid: fail_b\ntype: message\nfrom: family\npool: fail\n---\nB.\n---\nid: fail_c\ntype: message\npool: fail\n---\nC.\n";
    const ONE_EVENT: &str = "id: ch\nnodes:\n  - id: start\n    event: race\n  - id: after\n    end: done\n";

    /// A story with one chapter `flow`, the event `race` (P3 or better, fee
    /// 100, failure pool `fail` of three variants) and `settings` written
    /// inline (`failure: final, money: true`).
    fn story(settings: &str, flow: &str, extra: &[(&str, &str)]) -> Story {
        let mut files = vec![
            SourceFile::new("story.yaml", &format!("{MANIFEST}settings: {{ {settings} }}\n")),
            SourceFile::new("flow/01-ch.yaml", flow),
            SourceFile::new(
                "world/characters.yaml",
                "characters:\n  - id: friend\n  - id: family\n    relation: warm\n",
            ),
            SourceFile::new("world/variables.yaml", "flags: [met]\ncounters: { visits: 0 }\n"),
            SourceFile::new("events/race.yaml", RACE),
            SourceFile::new("content/pools/fail.story", POOL),
        ];
        for (path, text) in extra {
            files = with(files, path, text);
        }
        Story::from_sources(files).expect("test story loads")
    }

    fn start(story: &Story) -> State {
        let mut state = State::new(story, 7);
        run(story, &mut state).expect("runs");
        state
    }

    fn place(position: u32) -> EventResult {
        EventResult {
            finished: true,
            position: Some(position),
            ..Default::default()
        }
    }

    /// Launches `race` and submits `result`.
    fn drive(story: &Story, state: &mut State, result: EventResult) -> Vec<Output> {
        submit(story, state, Input::Launch { event: "race".into() }).expect("launch");
        submit(
            story,
            state,
            Input::Result {
                event: "race".into(),
                result,
            },
        )
        .expect("result")
    }

    fn delivered(out: &[Output]) -> Vec<&str> {
        out.iter()
            .filter_map(|o| match o {
                Output::Delivered { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect()
    }

    // Events: a success applies its effects, moves story time on and takes
    // the success exit.
    #[test]
    fn a_success_applies_its_effects_moves_time_and_goes_on() {
        let story = story("", ONE_EVENT, &[]);
        let mut state = start(&story);
        assert_eq!(state.wait, Wait::Event { event: "race".into() });
        let out = drive(&story, &mut state, place(2));
        assert_eq!(state.reputation, 2, "on_success effect");
        assert_eq!(
            state.date.to_string(),
            "2000-02-29",
            "one month after Jan 31st, leap year"
        );
        assert_eq!(
            out.last(),
            Some(&Output::Ended { ending: "done".into() }),
            "success exit"
        );
        assert_eq!(state.event("race").attempts, 1);
    }

    // Story time: failing or retrying never costs time (setting `retry`, the
    // default), and the failure pool speaks.
    #[test]
    fn a_failure_with_retry_stays_on_the_event_and_freezes_time() {
        let story = story("", ONE_EVENT, &[]);
        let mut state = start(&story);
        let out = drive(&story, &mut state, place(8));
        assert_eq!(state.date.to_string(), "2000-01-31", "time frozen");
        assert_eq!(state.reputation, 0, "no success effects");
        assert_eq!(delivered(&out).len(), 1, "one pool variant by default");
        assert!(
            matches!(out.last(), Some(Output::AwaitEvent { attempt: 2, .. })),
            "the same event again, second attempt: {out:?}"
        );
    }

    #[test]
    fn a_final_failure_carries_on_without_the_success_effects() {
        let story = story("failure: final", ONE_EVENT, &[]);
        let mut state = start(&story);
        let out = drive(&story, &mut state, place(8));
        assert_eq!(
            out.last(),
            Some(&Output::Ended { ending: "done".into() }),
            "the story goes on"
        );
        assert_eq!(
            (state.reputation, state.date.to_string().as_str()),
            (0, "2000-01-31"),
            "nothing of the success"
        );
    }

    #[test]
    fn max_attempts_turns_retries_into_a_final_result() {
        let story = story("max_attempts: 2", ONE_EVENT, &[]);
        let mut state = start(&story);
        drive(&story, &mut state, place(8));
        assert!(matches!(state.wait, Wait::Event { .. }), "first failure: retry");
        let out = drive(&story, &mut state, place(8));
        assert_eq!(
            out.last(),
            Some(&Output::Ended { ending: "done".into() }),
            "second failure: final"
        );
    }

    #[test]
    fn the_authors_failure_exit_wins_over_the_setting() {
        let flow = "id: ch\nnodes:\n  - id: start\n    event: race\n    on_failure: consolation\n  - id: after\n    end: won\n  - id: consolation\n    beat: { effects: [\"counter.visits += 1\"] }\n    next: start\n";
        let story = story("", flow, &[]);
        let mut state = start(&story);
        drive(&story, &mut state, place(8));
        assert_eq!(state.counters["visits"], 1, "the failure path ran");
        assert_eq!(
            state.wait,
            Wait::Event { event: "race".into() },
            "and led back to the event"
        );
    }

    // Failure: "a variant is not repeated until the pool is used up".
    #[test]
    fn a_failure_pool_never_repeats_a_variant_until_it_is_used_up() {
        let story = story("", ONE_EVENT, &[]);
        let mut state = start(&story);
        let mut first_round = Vec::new();
        for _ in 0..3 {
            first_round.extend(
                delivered(&drive(&story, &mut state, place(8)))
                    .into_iter()
                    .map(String::from),
            );
        }
        first_round.sort();
        assert_eq!(
            first_round,
            ["fail_a", "fail_b", "fail_c"],
            "each variant once before any repeat"
        );
        let fourth = delivered(&drive(&story, &mut state, place(8))).len();
        assert_eq!(fourth, 1, "exhausted pool starts over");
    }

    #[test]
    fn pool_variants_only_count_when_their_conditions_hold() {
        let pool = "---\nid: so_close\ntype: message\npool: fail\nwhen: [\"event.race.position == 4\"]\n---\nSo close.\n---\nid: plain\ntype: message\npool: fail\nwhen: [\"event.race.position > 4\"]\n---\nNext time.\n";
        let story = story("", ONE_EVENT, &[("content/pools/fail.story", pool)]);
        let mut state = start(&story);
        assert_eq!(
            delivered(&drive(&story, &mut state, place(4))),
            ["so_close"],
            "reads the result just recorded"
        );
        assert_eq!(delivered(&drive(&story, &mut state, place(9))), ["plain"]);
    }

    #[test]
    fn a_branch_takes_the_first_case_that_holds() {
        let flow = "id: ch\nnodes:\n  - id: start\n    branch:\n      - if: [\"money >= 1000\"]\n        goto: rich\n      - if: [\"money >= 200\", \"not flag.met\"]\n        goto: fine\n    otherwise: poor\n  - id: rich\n    end: rich\n  - id: fine\n    end: fine\n  - id: poor\n    end: poor\n";
        let story = story("money: true", flow, &[]);
        assert_eq!(
            start(&story).wait,
            Wait::Ended { ending: "fine".into() },
            "250: second case"
        );
    }

    // Reactive content: appears by itself when its conditions hold, once by
    // default, after its delay in story time.
    #[test]
    fn reactive_content_arrives_by_itself_once_and_after_its_delay() {
        let content = "---\nid: now\ntype: mail\nwhen: [\"reputation >= 2\"]\n---\nx\n---\nid: later\ntype: article\nwhen: [\"reputation >= 2\"]\ndelay: 2 weeks\n---\ny\n";
        let flow = "id: ch\nnodes:\n  - id: start\n    event: race\n  - id: more\n    beat: { effects: [\"reputation += 1\"], advance: 1 month }\n  - id: after\n    end: done\n";
        let story = story("", flow, &[("content/news.story", content)]);
        let mut state = start(&story);
        drive(&story, &mut state, place(1));
        assert_eq!(
            state.content_facts("now").delivered,
            1,
            "delivered once its condition held"
        );
        assert_eq!(
            state.content_facts("later").delivered,
            1,
            "two months passed: the delay ran out"
        );
        let delivered_at = |id: &str| {
            state.history.iter().find_map(|e| match &e.what {
                Happening::Delivered { content, .. } if content == id => Some(e.date.to_string()),
                _ => None,
            })
        };
        assert_eq!(
            delivered_at("now").as_deref(),
            Some("2000-02-29"),
            "right after the success moved time"
        );
        assert_eq!(
            delivered_at("later").as_deref(),
            Some("2000-03-29"),
            "at the next time move after its delay"
        );
    }

    #[test]
    fn each_time_content_fires_again_only_after_its_conditions_went_false() {
        let content = "---\nid: ping\ntype: message\nwhen: [\"flag.met\"]\nrepeat: each_time\n---\nx\n";
        let flow = "id: ch\nnodes:\n  - id: start\n    beat: { effects: [\"flag.met = true\"] }\n  - id: b\n    beat: { effects: [\"counter.visits += 1\"] }\n  - id: c\n    beat: { effects: [\"flag.met = false\"] }\n  - id: d\n    beat: { effects: [\"flag.met = true\"] }\n  - id: e\n    end: done\n";
        let story = story("", flow, &[("content/ping.story", content)]);
        assert_eq!(
            start(&story).content_facts("ping").delivered,
            2,
            "true, still true, false, true again"
        );
    }

    #[test]
    fn a_reply_applies_its_effects_and_releases_the_flow() {
        let content = "---\nid: invite\ntype: message\nfrom: friend\nreplies:\n  - id: accept\n    effects: [\"rel.friend += 1\"]\n  - id: decline\n    effects: [\"rel.friend -= 9\"]\n---\nCome along?\n";
        let flow = "id: ch\nnodes:\n  - id: start\n    message: invite\n    wait_reply: true\n  - id: check\n    branch:\n      - if: [\"content.invite.reply == accept\", \"rel.friend >= warm\"]\n        goto: yes_end\n    otherwise: no_end\n  - id: yes_end\n    end: yes\n  - id: no_end\n    end: no\n";
        let story = story("", flow, &[("content/invite.story", content)]);
        let mut state = start(&story);
        assert_eq!(
            state.wait,
            Wait::Reply {
                content: "invite".into()
            }
        );
        let err = submit(
            &story,
            &mut state.clone(),
            Input::Reply {
                content: "invite".into(),
                reply: "maybe".into(),
            },
        );
        assert!(matches!(err, Err(EngineError::Unexpected(_))), "unknown reply refused");
        let mut declined = state.clone();
        submit(
            &story,
            &mut declined,
            Input::Reply {
                content: "invite".into(),
                reply: "decline".into(),
            },
        )
        .unwrap();
        assert_eq!(
            declined.relations["friend"], "enemy",
            "clamped at the bottom of the scale"
        );
        submit(
            &story,
            &mut state,
            Input::Reply {
                content: "invite".into(),
                reply: "accept".into(),
            },
        )
        .unwrap();
        assert_eq!(state.relations["friend"], "warm", "neutral + 1");
        assert_eq!(state.wait, Wait::Ended { ending: "yes".into() });
    }

    // Story settings: the relation scale belongs to the story.
    #[test]
    fn a_story_can_bring_its_own_relation_scale() {
        let flow = "id: ch\nnodes:\n  - id: start\n    beat: { effects: [\"rel.friend += 1\"] }\n  - id: check\n    branch:\n      - if: [\"rel.friend == ally\"]\n        goto: ok\n    otherwise: ko\n  - id: ok\n    end: ok\n  - id: ko\n    end: ko\n";
        let story = story(
            "relations: { scale: [foe, stranger, ally], start: stranger }",
            flow,
            &[("world/characters.yaml", "characters:\n  - id: friend\n")],
        );
        assert_eq!(start(&story).wait, Wait::Ended { ending: "ok".into() });
    }

    // Retry cost: `paid` makes every attempt pay the entry fee again; `free`
    // only the first. Without an economy, nothing is paid.
    #[test]
    fn entry_fees_follow_the_retry_cost_setting() {
        let paid = story("money: true, retry_cost: paid", ONE_EVENT, &[]);
        let mut state = start(&paid);
        drive(&paid, &mut state, place(8));
        drive(&paid, &mut state, place(8));
        assert_eq!(state.money, 50, "250 - 2 × 100");
        let refused = submit(&paid, &mut state, Input::Launch { event: "race".into() });
        assert_eq!(refused, Err(EngineError::CannotAfford { fee: 100, money: 50 }));

        let free = story("money: true", ONE_EVENT, &[]);
        let mut state = start(&free);
        drive(&free, &mut state, place(8));
        drive(&free, &mut state, place(8));
        assert_eq!(state.money, 150, "only the first attempt paid");

        let no_economy = story("", ONE_EVENT, &[]);
        let mut state = start(&no_economy);
        drive(&no_economy, &mut state, place(8));
        assert_eq!(state.money, 250, "no money setting, no fee");
    }

    #[test]
    fn adaptive_difficulty_steps_every_n_failures_up_to_its_cap() {
        let story = story("adaptive_difficulty: { every: 2, cap: 1 }", ONE_EVENT, &[]);
        let mut state = start(&story);
        let steps: Vec<u32> = (0..5)
            .map(|_| match drive(&story, &mut state, place(8)).last() {
                Some(Output::AwaitEvent { difficulty_step, .. }) => *difficulty_step,
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(steps, [0, 1, 1, 1, 1], "a step after 2 failures, capped at 1");
    }

    #[test]
    fn a_side_event_pays_and_rewards_without_moving_the_story() {
        let side = "id: track_day\ntype: track_day\non_success: { effects: [\"money += 40\"], advance: 1 year }\nentry_fee: 10\n";
        let flow = "id: ch\nnodes:\n  - id: start\n    side_events: { open: [track_day] }\n  - id: wait\n    action: shop\n  - id: after\n    end: done\n";
        let story = story("money: true", flow, &[("events/track_day.yaml", side)]);
        let mut state = start(&story);
        submit(
            &story,
            &mut state,
            Input::Launch {
                event: "track_day".into(),
            },
        )
        .unwrap();
        submit(
            &story,
            &mut state,
            Input::Result {
                event: "track_day".into(),
                result: place(1),
            },
        )
        .unwrap();
        assert_eq!(state.money, 280, "250 - 10 + 40");
        assert_eq!(
            state.date.to_string(),
            "2000-01-31",
            "a side event never moves story time"
        );
        assert_eq!(state.wait, Wait::Action { action: "shop".into() }, "nor the flow");
        submit(&story, &mut state, Input::ActionDone { action: "shop".into() }).unwrap();
        assert_eq!(state.wait, Wait::Ended { ending: "done".into() });
    }

    // A refused input changes nothing, and a running session cannot be
    // replaced by another one: its fee is paid.
    #[test]
    fn a_refused_input_leaves_the_state_untouched() {
        let side = "id: track_day
type: track_day
entry_fee: 10
";
        let flow = "id: ch
nodes:
  - id: start
    side_events: { open: [track_day] }
  - id: race_node
    event: race
  - id: after
    end: done
";
        let story = story("money: true", flow, &[("events/track_day.yaml", side)]);
        let mut state = start(&story);
        submit(&story, &mut state, Input::Launch { event: "race".into() }).unwrap();
        let before = state.clone();
        let err = submit(
            &story,
            &mut state,
            Input::Launch {
                event: "track_day".into(),
            },
        );
        assert!(
            matches!(err, Err(EngineError::Unexpected(ref why)) if why.contains("still running")),
            "{err:?}"
        );
        let err = submit(
            &story,
            &mut state,
            Input::Reply {
                content: "nothing".into(),
                reply: "x".into(),
            },
        );
        assert!(err.is_err());
        assert_eq!(state, before, "nothing changed by the refused inputs");
    }

    #[test]
    fn a_result_without_a_launch_is_refused() {
        let story = story("", ONE_EVENT, &[]);
        let mut state = start(&story);
        let err = submit(
            &story,
            &mut state,
            Input::Result {
                event: "race".into(),
                result: place(1),
            },
        );
        assert!(matches!(err, Err(EngineError::Unexpected(_))), "{err:?}");
    }

    #[test]
    fn a_loop_that_never_waits_is_an_error_not_a_hang() {
        let flow = "id: ch\nnodes:\n  - id: start\n    beat: {}\n    next: start\n";
        let story = story("", flow, &[]);
        let mut state = State::new(&story, 0);
        assert_eq!(run(&story, &mut state), Err(EngineError::Loop { node: "start".into() }));
    }

    #[test]
    fn success_criteria_all_hold_and_a_forced_verdict_wins() {
        let criteria: Vec<Criterion> = ["position <= 3", "best_lap < 1:20.000"]
            .iter()
            .map(|c| c.parse().unwrap())
            .collect();
        let ok = EventResult {
            finished: true,
            position: Some(3),
            best_lap_ms: Some(79_000),
            ..Default::default()
        };
        assert!(succeeded(&criteria, &ok));
        assert!(
            !succeeded(
                &criteria,
                &EventResult {
                    best_lap_ms: Some(80_000),
                    ..ok.clone()
                }
            ),
            "lap must be under"
        );
        assert!(
            !succeeded(&[], &EventResult::default()),
            "no criterion: finishing is required"
        );
        assert!(succeeded(
            &criteria,
            &EventResult {
                forced: Some(Verdict::Success),
                ..Default::default()
            }
        ));
    }
}
