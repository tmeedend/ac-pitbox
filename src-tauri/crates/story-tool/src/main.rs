//! Developer CLI for the Story Mode engine - never shipped to users.
//!
//! Plays a story folder in a terminal, the result of every event typed by
//! hand ("P3", "P3 1:18.2", "dnf", "failed"), as the editor's Playtest will
//! (SPEC-story-mode, Create Your Own). Exists so that the engine (L1) can be
//! proven on a whole story before any screen exists.
//!
//! ```text
//! story-tool check <story_dir>
//! story-tool play <story_dir> [--seed=<n>] [--load=<save.json>]
//! story-tool pack <story_dir> [--out=<file.pitstory>]
//! ```

mod input;

use std::io::{BufRead, Write};
use std::path::Path;
use std::process::ExitCode;

use story_engine::engine::{run, submit, EngineError, Input, Output};
use story_engine::save::Save;
use story_engine::state::{State, Via, Wait};
use story_engine::validate::{validate, Severity};
use story_engine::Story;
use story_engine::{credits, package};

use input::{parse_command, Command};

const USAGE: &str = "\
story-tool — check and play a Pit Box story folder

USAGE:
    story-tool check <story_dir>
    story-tool play <story_dir> [--seed=<n>] [--load=<save.json>]
    story-tool pack <story_dir> [--out=<file.pitstory>]

COMMANDS:
    check   Load and validate the story; exit code 1 on any error.
    play    Play it. Type `help` at the prompt for what can be typed.
    pack    Build a package of the story alone, `<id>-<version>.pitstory`
            unless --out says: the cars and tracks it carries are taken from
            a library, which the editor's Package section has and this tool
            has not.
";

const PLAY_HELP: &str = "\
  When an event waits:     P3 | P3 1:18.2 | P1 ahead:<driver> | dnf | failed | won
  When a reply waits:      <label>
  When an action waits:    done
  Any time:
    side <event> <result>  drive an open side event
    reply <content> <label> reply in a conversation waiting at a choice
    seen <content>         mark a content item as opened
    state | history        show the state / what happened
    save <file.json>       write a save
    quit
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(command), Some(dir)) = (args.first(), args.get(1)) else {
        eprint!("{USAGE}");
        return ExitCode::from(2);
    };
    let option = |name: &str| {
        args.iter()
            .find_map(|a| a.strip_prefix(&format!("--{name}=")).map(String::from))
    };
    let Some(story) = load(Path::new(dir)) else {
        return ExitCode::FAILURE;
    };
    match command.as_str() {
        "check" => check(&story),
        "play" => {
            let seed = match option("seed").map(|s| s.parse::<u64>()) {
                None => 1,
                Some(Ok(seed)) => seed,
                Some(Err(_)) => {
                    eprintln!("--seed takes a whole number");
                    return ExitCode::from(2);
                }
            };
            play(&story, seed, option("load").as_deref())
        }
        "pack" => pack(&story, Path::new(dir), option("out")),
        _ => {
            eprint!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn load(dir: &Path) -> Option<Story> {
    match Story::load(dir) {
        Ok(story) => Some(story),
        Err(errors) => {
            for e in errors {
                eprintln!("error: {e}");
            }
            None
        }
    }
}

/// Prints the diagnostics; true when none is an error.
fn report(story: &Story) -> bool {
    let diags = validate(story);
    for d in &diags {
        eprintln!("{d}");
    }
    !diags.iter().any(|d| d.severity == Severity::Error)
}

fn check(story: &Story) -> ExitCode {
    let ok = report(story);
    let m = &story.manifest;
    println!(
        "{} ({} v{}): {} chapters, {} nodes, {} content items, {} events, {} scenes",
        m.title,
        m.id,
        m.version,
        story.chapters.len(),
        story.nodes().count(),
        story.content.len(),
        story.events.len(),
        story.scenes.len()
    );
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// The story's package, as the editor's Package section builds it
/// (SPEC-story-mode, Create Your Own). A story with errors, or a file
/// without its credit and permission, is never packed.
fn pack(story: &Story, dir: &Path, out: Option<String>) -> ExitCode {
    if !report(story) {
        eprintln!("the story has errors: fix them first (`story-tool check`)");
        return ExitCode::FAILURE;
    }
    let files = match package::folder_files(dir) {
        Ok(files) => files,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let register = credits::check_files(&files);
    for d in &register.diagnostics {
        eprintln!("{d}");
    }
    if register.blocks() {
        eprintln!("the permission register has errors: fix them first");
        return ExitCode::FAILURE;
    }
    let m = &story.manifest;
    let out = out.unwrap_or_else(|| format!("{}-{}.{}", m.id, m.version, package::EXTENSION));
    let built = package::pack(m, &files).and_then(|bytes| {
        std::fs::write(&out, &bytes)
            .map(|_| bytes.len())
            .map_err(|e| format!("{out}: {e}"))
    });
    match built {
        Ok(size) => {
            println!("{out}: {} v{}, {size} bytes", m.title, m.version);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn play(story: &Story, seed: u64, load: Option<&str>) -> ExitCode {
    if !report(story) {
        eprintln!("the story has errors: fix them first (`story-tool check`)");
        return ExitCode::FAILURE;
    }
    let mut state = match initial_state(story, seed, load) {
        Ok(state) => state,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    println!("== {} ==\n", story.manifest.title);
    match run(story, &mut state) {
        Ok(out) => show(story, &state, &out),
        Err(e) => return engine_failed(e),
    }

    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    loop {
        if let Wait::Ended { ending } = &state.wait {
            println!("\n== ending: {ending} ==");
            return ExitCode::SUCCESS;
        }
        print!("{} > ", prompt(story, &state));
        let _ = std::io::stdout().flush();
        let Some(Ok(line)) = lines.next() else {
            println!("\n(input ended)");
            return ExitCode::SUCCESS;
        };
        let inputs = match parse_command(&line, &state.wait) {
            Ok(Command::Inputs(inputs)) => inputs,
            Ok(Command::Nothing) => continue,
            Ok(Command::Help) => {
                print!("{PLAY_HELP}");
                continue;
            }
            Ok(Command::State) => {
                print_state(&state);
                continue;
            }
            Ok(Command::History) => {
                for entry in &state.history {
                    println!("  {} {}  {:?}", entry.date, entry.clock, entry.what);
                }
                continue;
            }
            Ok(Command::Save(path)) => {
                match std::fs::write(&path, Save::new(story, &state).to_json()) {
                    Ok(()) => println!("saved to {path}"),
                    Err(e) => println!("cannot write {path}: {e}"),
                }
                continue;
            }
            Ok(Command::Quit) => return ExitCode::SUCCESS,
            Err(why) => {
                println!("{why} (type `help`)");
                continue;
            }
        };
        match apply(story, &state, inputs) {
            Ok((next, out)) => {
                state = next;
                show(story, &state, &out);
            }
            Err(EngineError::Unexpected(why)) => println!("refused: {why}"),
            Err(EngineError::CannotAfford { fee, money }) => {
                println!("refused: the entry fee is {fee}, money is {money}")
            }
            Err(e) => return engine_failed(e),
        }
    }
}

/// A new game, or the one a save holds once it is known to fit the story.
fn initial_state(story: &Story, seed: u64, load: Option<&str>) -> Result<State, String> {
    let Some(path) = load else {
        return Ok(State::new(story, seed));
    };
    let fail = |e: String| format!("cannot load {path}: {e}");
    let text = std::fs::read_to_string(path).map_err(|e| fail(e.to_string()))?;
    let save = Save::from_json(&text).map_err(|e| fail(e.to_string()))?;
    save.resume(story).map_err(|e| fail(e.to_string()))
}

/// A typed event result is a launch then a result: both are tried on a
/// copy, so that a refused second step leaves the game untouched.
fn apply(story: &Story, state: &State, inputs: Vec<Input>) -> Result<(State, Vec<Output>), EngineError> {
    let mut next = state.clone();
    let mut out = Vec::new();
    for input in inputs {
        out.extend(submit(story, &mut next, input)?);
    }
    Ok((next, out))
}

fn engine_failed(e: EngineError) -> ExitCode {
    eprintln!("engine error: {e}");
    ExitCode::FAILURE
}

fn prompt(story: &Story, state: &State) -> String {
    match &state.wait {
        Wait::Event { event } => {
            let facts = state.event(event);
            let step = story_engine::engine::difficulty_step(story, state, event);
            let fee = story_engine::engine::fee_due(story, state, event);
            let mut p = format!("{event} (attempt {}", facts.attempts + 1);
            if step > 0 {
                p += &format!(", opponents -{step}");
            }
            if let Some(fee) = fee {
                p += &format!(", fee {fee}");
            }
            p + ")"
        }
        Wait::Reply { content } => {
            let offered = state.conversations.get(content).map(|c| c.offered.join(" / "));
            format!("reply to {content} [{}]", offered.unwrap_or_default())
        }
        Wait::Action { action } => format!("action `{action}` (type `done`)"),
        Wait::Running | Wait::Ended { .. } => String::new(),
    }
}

fn show(story: &Story, state: &State, out: &[Output]) {
    for o in out {
        match o {
            Output::Delivered { content, via } => {
                let Some(item) = story.content_item(content) else {
                    continue;
                };
                let via = match via {
                    Via::Flow => "flow".to_string(),
                    Via::Reactive => "reactive".to_string(),
                    Via::Pool { event } => format!("failure pool of {event}"),
                    Via::Effect => "sent".to_string(),
                };
                let from = item.from.as_deref().map(|f| format!(" from {f}")).unwrap_or_default();
                let clock = item.clock.unwrap_or(state.clock);
                println!("[{} {clock}] {:?}{from} — {content} ({via})", state.date, item.kind);
            }
            Output::Lines { lines, .. } => {
                for line in lines {
                    let who = line.speaker.as_deref().map(|w| format!("{w}: ")).unwrap_or_default();
                    let mut tags = String::new();
                    if let Some(e) = &line.expression {
                        tags += &format!("  ({e})");
                    }
                    println!("    {who}{}{tags}", line.text);
                }
            }
            Output::Choices { content, choices } => {
                for c in choices {
                    println!("    * ({}) {}", c.label, c.text);
                }
                if !matches!(&state.wait, Wait::Reply { content: w } if w == content) {
                    println!("    (answer any time: reply {content} <label>)");
                }
            }
            Output::TimeMoved { to } => println!("\n—— {to} ——"),
            Output::FrameChanged { to } => println!("[frame: {to}]"),
            // The variant as it shows once the beat is over, its clock set.
            Output::SceneChanged { place, .. } => match state.scene_shown(story, place) {
                Some(shown) => println!("[scene: {place}, {} — {}]", shown.variant, shown.file),
                None => println!("[scene: {place}, not declared]"),
            },
            Output::Held { content } => println!("[{content} waits for a frame that can show it]"),
            // The prompt says what is awaited.
            Output::AwaitEvent { .. }
            | Output::AwaitReply { .. }
            | Output::AwaitAction { .. }
            | Output::Ended { .. } => {}
        }
    }
}

fn print_state(state: &State) {
    println!("  node {}  ·  {} {}", state.node, state.date, state.clock);
    println!(
        "  reputation {}  followers {}  money {}",
        state.reputation, state.followers, state.money
    );
    let list = |items: Vec<String>| {
        if items.is_empty() {
            "-".to_string()
        } else {
            items.join(", ")
        }
    };
    println!("  flags: {}", list(state.flags.iter().cloned().collect()));
    println!(
        "  counters: {}",
        list(state.counters.iter().map(|(k, v)| format!("{k}={v}")).collect())
    );
    println!(
        "  relations: {}",
        list(state.relations.iter().map(|(k, v)| format!("{k}={v}")).collect())
    );
    println!("  cars: {}", list(state.cars.iter().cloned().collect()));
    println!(
        "  side events open: {}",
        list(state.side_events.iter().cloned().collect())
    );
    for (id, f) in &state.events {
        println!(
            "  event {id}: {} attempts, {} successes, {} failures in a row",
            f.attempts, f.successes, f.consecutive_failures
        );
    }
}
