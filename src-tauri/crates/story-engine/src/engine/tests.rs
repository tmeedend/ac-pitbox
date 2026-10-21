use super::*;
use crate::frame::Frame;
use crate::load::SourceFile;
use crate::testutil::with;

const MANIFEST: &str =
        "format: 1\nid: t\ntitle: T\nversion: \"1\"\nlanguages: [en]\nstart:\n  node: start\n  frame: phone\n  date: 2000-01-31\n  money: 250\n";
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

// The frame is part of the state (SPEC-story-mode, Content types,
// templates and frames): a Beat changes or removes it, before its own
// effects, and says so once; a Beat that leaves it as it is says nothing.
#[test]
fn a_beat_changes_the_frame_before_its_effects() {
    let content = "---\nid: hello\ntype: dialogue\n---\nx\n";
    let flow = "id: ch\nnodes:\n  - id: start\n    beat: { frame: none, effects: [\"send hello\"] }\n  - id: same\n    beat: { frame: none }\n  - id: back\n    beat: { frame: phone }\n  - id: end\n    end: done\n";
    let story = story("", flow, &[("content/hello.story", content)]);
    let mut state = State::new(&story, 7);
    assert_eq!(
        state.frame,
        Some(Frame::Named("phone".into())),
        "the story's starting frame"
    );
    let out = run(&story, &mut state).unwrap();
    let changes: Vec<&Output> = (out.iter())
        .filter(|o| matches!(o, Output::FrameChanged { .. } | Output::Delivered { .. }))
        .collect();
    assert_eq!(
        changes,
        [
            &Output::FrameChanged { to: Frame::None },
            &Output::Delivered {
                content: "hello".into(),
                via: Via::Effect
            },
            &Output::FrameChanged {
                to: Frame::Named("phone".into())
            },
        ],
        "removed, then the send, nothing for the beat that kept it removed, then put back"
    );
    assert_eq!(state.frame, Some(Frame::Named("phone".into())));
}

#[test]
fn a_reply_applies_its_effects_and_releases_the_flow() {
    let content = "---\nid: invite\ntype: message\nfrom: friend\n---\nCome along?\n* (accept) [Sure.]\n    ~ effect(\"rel.friend += 1\")\n* (decline) [No.]\n    ~ effect(\"rel.friend -= 9\")\n";
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

// Reply exits (SPEC-story-mode, In-person dialogues): the conversation
// ends, then the node leaves along the exit of the most recent reply
// that has one; a reply without an exit takes `next`. Here the player
// chooses `yes`, then `sure` under it: `sure` has no exit, `yes` does.
#[test]
fn a_message_leaves_by_the_latest_reply_that_has_an_exit() {
    let content = "---\nid: ask\ntype: dialogue\ncharacters: [friend]\n---\nfriend: Coming?\n* (yes) [Yes.]\n    * * (sure) [Sure.]\n    * * (maybe) [Maybe.]\n* (no) [No.]\n";
    let flow = "id: ch\nnodes:\n  - id: start\n    message: ask\n    on_reply: { yes: went, no: stayed }\n  - id: default\n    end: default\n  - id: went\n    end: went\n  - id: stayed\n    end: stayed\n";
    let story = story("", flow, &[("content/ask.story", content)]);
    let reply = |state: &mut State, label: &str| {
        submit(
            &story,
            state,
            Input::Reply {
                content: "ask".into(),
                reply: label.into(),
            },
        )
    };
    let mut state = start(&story);
    assert_eq!(state.wait, Wait::Reply { content: "ask".into() });
    let out = reply(&mut state, "yes").unwrap();
    assert!(
        out.iter()
            .any(|o| matches!(o, Output::Choices { choices, .. } if choices.len() == 2)),
        "the nested choice point: {out:?}"
    );
    assert_eq!(state.wait, Wait::Reply { content: "ask".into() }, "still talking");
    reply(&mut state, "sure").unwrap();
    assert_eq!(state.wait, Wait::Ended { ending: "went".into() }, "`yes` has the exit");
    assert_eq!(state.content_facts("ask").chosen, ["yes", "sure"]);

    let mut state = start(&story);
    assert!(
        matches!(reply(&mut state.clone(), "sure"), Err(EngineError::Unexpected(_))),
        "a label not offered here is refused"
    );
    reply(&mut state, "no").unwrap();
    assert_eq!(
        state.wait,
        Wait::Ended {
            ending: "stayed".into()
        }
    );
}

// A reply is tested against every label chosen in the conversation, and
// a reactive message is answered whenever the player likes, the flow
// going on meanwhile.
#[test]
fn a_reactive_conversation_is_answered_later_and_every_reply_counts() {
    let content = "---\nid: ping\ntype: message\nfrom: friend\nwhen: [\"flag.met\"]\n---\nfriend: Hey.\n* (wave) [Wave.]\n    * * (smile) [Smile.]\n";
    let flow = "id: ch\nnodes:\n  - id: start\n    beat: { effects: [\"flag.met = true\"] }\n  - id: act\n    action: wait\n  - id: check\n    branch:\n      - if: [\"content.ping.reply == wave\", \"content.ping.reply == smile\"]\n        goto: both\n    otherwise: neither\n  - id: both\n    end: both\n  - id: neither\n    end: neither\n";
    let story = story("", flow, &[("content/ping.story", content)]);
    let mut state = start(&story);
    assert_eq!(
        state.wait,
        Wait::Action { action: "wait".into() },
        "the flow did not wait"
    );
    for label in ["wave", "smile"] {
        submit(
            &story,
            &mut state,
            Input::Reply {
                content: "ping".into(),
                reply: label.into(),
            },
        )
        .unwrap();
    }
    assert!(
        matches!(
            submit(
                &story,
                &mut state.clone(),
                Input::Reply {
                    content: "ping".into(),
                    reply: "wave".into()
                }
            ),
            Err(EngineError::Unexpected(_))
        ),
        "an ended conversation waits for no reply"
    );
    submit(&story, &mut state, Input::ActionDone { action: "wait".into() }).unwrap();
    assert_eq!(state.wait, Wait::Ended { ending: "both".into() }, "both labels count");
}

// A new delivery is a new conversation: the replies of the last one are
// forgotten.
#[test]
fn a_new_delivery_forgets_the_last_replies() {
    let content = "---\nid: ask\ntype: message\nfrom: friend\n---\n* (yes) [Yes.]\n";
    let flow = "id: ch\nnodes:\n  - id: start\n    message: ask\n    wait_reply: true\n  - id: again\n    beat: { effects: [\"send ask\"] }\n  - id: wait\n    action: look\n  - id: end\n    end: done\n";
    let story = story("", flow, &[("content/ask.story", content)]);
    let mut state = start(&story);
    submit(
        &story,
        &mut state,
        Input::Reply {
            content: "ask".into(),
            reply: "yes".into(),
        },
    )
    .unwrap();
    let facts = state.content_facts("ask");
    assert_eq!(
        (facts.delivered, facts.chosen.len()),
        (2, 0),
        "sent again, its replies reset"
    );
    assert!(state.conversations.contains_key("ask"), "and waiting again");
}

// Two items sending each other would never stop: an error, not a hang.
#[test]
fn items_sending_each_other_are_a_loop_not_a_hang() {
    let content =
        "---\nid: a\ntype: message\n---\n~ effect(\"send b\")\n---\nid: b\ntype: message\n---\n~ effect(\"send a\")\n";
    let flow = "id: ch\nnodes:\n  - id: start\n    message: a\n  - id: end\n    end: done\n";
    let story = story("", flow, &[("content/loop.story", content)]);
    let mut state = State::new(&story, 7);
    assert!(matches!(run(&story, &mut state), Err(EngineError::Loop { .. })));
}

// The Content screen's preview (SPEC-story-mode, Create Your Own): one
// item played on its own, the values the author set applied first, the
// replies chosen so far replayed; the flow never moves.
#[test]
fn a_preview_plays_one_item_on_the_values_set_and_leaves_the_flow() {
    let content = "---\nid: ask\ntype: message\nfrom: friend\n---\n{check(\"flag.met\"): friend: Hi again.}\nfriend: Coming?\n* (yes) [Yes.]\n    ~ effect(\"rel.friend += 1\")\n    ~ effect(\"send other\")\n    friend: Great.\n* (no) [No.]\n---\nid: other\ntype: message\n---\nx\n";
    let story = story("", ONE_EVENT, &[("content/ask.story", content)]);
    let said = |p: &Preview| -> Vec<String> {
        (p.steps.iter())
            .map(|s| match s {
                Step::Said(l) => l.text.clone(),
                Step::Chose(c) => format!("> {}", c.label),
            })
            .collect()
    };

    let first = preview(&story, &[], "ask", &[]).unwrap();
    assert_eq!(said(&first), ["Coming?"], "the conditional line hidden");
    assert_eq!(
        first.choices.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(),
        ["yes", "no"]
    );
    assert!(!first.ended);

    let met: Effect = "flag.met = true".parse().unwrap();
    let shown = preview(&story, &[met], "ask", &["yes".into()]).unwrap();
    assert_eq!(
        said(&shown),
        ["Hi again.", "Coming?", "> yes", "Great."],
        "set, then replayed"
    );
    assert!(shown.ended && shown.choices.is_empty());
    assert_eq!(shown.sent, ["other"], "what it sent is named, not shown");
    assert_eq!(
        (
            shown.before.relations["friend"].as_str(),
            shown.after.relations["friend"].as_str()
        ),
        ("neutral", "warm"),
        "what the conversation changed"
    );
    assert_eq!(shown.after.node, "start", "the flow did not move");
    assert_eq!(shown.after.wait, Wait::Running);

    let stale = preview(&story, &[], "ask", &["gone".into(), "yes".into()]).unwrap();
    assert_eq!(
        said(&stale),
        ["Coming?"],
        "a reply no longer offered stops the replay there"
    );
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
    let side =
        "id: track_day\ntype: track_day\non_success: { effects: [\"money += 40\"], advance: 1 year }\nentry_fee: 10\n";
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
