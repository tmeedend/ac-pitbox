//! What can be typed at the `play` prompt, parsed into engine inputs.

use story_engine::engine::Input;
use story_engine::expr::parse_lap_time;
use story_engine::state::{EventResult, Verdict, Wait};

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Inputs(Vec<Input>),
    Nothing,
    Help,
    State,
    History,
    Save(String),
    Quit,
}

/// An event result as typed: `P3`, `P3 1:18.2`, `P1 ahead:some_driver`,
/// `dnf`, `failed`, `won`.
pub fn parse_result(words: &[&str]) -> Result<EventResult, String> {
    let mut result = EventResult {
        finished: true,
        ..Default::default()
    };
    if words.is_empty() {
        return Err("type a result: P3, dnf, failed…".into());
    }
    for word in words {
        let lower = word.to_ascii_lowercase();
        match lower.as_str() {
            "dnf" => result.finished = false,
            "failed" | "fail" => result.forced = Some(Verdict::Failure),
            "won" | "success" | "ok" => result.forced = Some(Verdict::Success),
            _ if lower.starts_with('p') && lower[1..].parse::<u32>().is_ok() => {
                result.position = lower[1..].parse().ok();
            }
            _ if word.starts_with("ahead:") => result.ahead_of.push(word["ahead:".len()..].to_string()),
            _ => match parse_lap_time(word) {
                Some(ms) => result.best_lap_ms = Some(ms),
                None => return Err(format!("`{word}` is not part of a result")),
            },
        }
    }
    Ok(result)
}

fn drive(event: &str, words: &[&str]) -> Result<Command, String> {
    let result = parse_result(words)?;
    Ok(Command::Inputs(vec![
        Input::Launch { event: event.into() },
        Input::Result {
            event: event.into(),
            result,
        },
    ]))
}

pub fn parse_command(line: &str, wait: &Wait) -> Result<Command, String> {
    let words: Vec<&str> = line.split_whitespace().collect();
    match words.as_slice() {
        [] => Ok(Command::Nothing),
        ["help"] => Ok(Command::Help),
        ["state"] => Ok(Command::State),
        ["history"] => Ok(Command::History),
        ["quit"] | ["exit"] => Ok(Command::Quit),
        ["save", path] => Ok(Command::Save(path.to_string())),
        ["side", event, result @ ..] => drive(event, result),
        ["reply", content, reply] => Ok(Command::Inputs(vec![Input::Reply {
            content: content.to_string(),
            reply: reply.to_string(),
        }])),
        ["seen", content] => Ok(Command::Inputs(vec![Input::Seen {
            content: content.to_string(),
        }])),
        _ => match wait {
            Wait::Event { event } => drive(event, &words),
            Wait::Reply { content } => match words.as_slice() {
                [reply] => Ok(Command::Inputs(vec![Input::Reply {
                    content: content.clone(),
                    reply: reply.to_string(),
                }])),
                _ => Err("type one reply id".into()),
            },
            Wait::Action { action } => match words.as_slice() {
                ["done"] => Ok(Command::Inputs(vec![Input::ActionDone { action: action.clone() }])),
                _ => Err("type `done` once the action is done".into()),
            },
            Wait::Running | Wait::Ended { .. } => Err("nothing is waiting".into()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_results_cover_position_lap_dnf_and_verdicts() {
        let r = parse_result(&["P3", "1:18.2"]).unwrap();
        assert_eq!((r.finished, r.position, r.best_lap_ms), (true, Some(3), Some(78_200)));
        assert!(!parse_result(&["dnf"]).unwrap().finished);
        assert_eq!(parse_result(&["failed"]).unwrap().forced, Some(Verdict::Failure));
        assert_eq!(parse_result(&["P1", "ahead:a_rival"]).unwrap().ahead_of, ["a_rival"]);
        assert!(parse_result(&["third"]).is_err());
    }

    #[test]
    fn a_result_typed_on_an_event_launches_then_submits() {
        let wait = Wait::Event {
            event: "race_01".into(),
        };
        let Ok(Command::Inputs(inputs)) = parse_command("p2", &wait) else {
            panic!()
        };
        assert!(
            matches!(&inputs[..], [Input::Launch { .. }, Input::Result { result, .. }] if result.position == Some(2))
        );
        let action = Wait::Action { action: "shop".into() };
        assert_eq!(
            parse_command("done", &action),
            Ok(Command::Inputs(vec![Input::ActionDone { action: "shop".into() }]))
        );
        assert!(parse_command("P2", &action).is_err(), "a result is not an action");
    }
}
