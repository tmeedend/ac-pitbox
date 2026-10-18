//! Conditions, effects and success criteria, written by authors as short
//! sentences (`"reputation >= 10"`, `"rel.friend += 1"`, `"position <= 3"`)
//! and parsed into types when the story loads.
//!
//! Text rather than nested YAML maps: a condition reads the same in the file,
//! in a diff and in the editor's field, and a mistake is reported on its line.
//! This module only parses and prints; what a variable *is* (its type, whether
//! it exists in this story) is the validator's job (`validate.rs`), and what it
//! is *worth* is the state's (`state.rs`).

use std::fmt;
use std::str::FromStr;

use crate::time::StoryDate;

/// Facts every event exposes on its own (SPEC-story-mode, Variables).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventFact {
    Attempts,
    Succeeded,
    Finished,
    Position,
    BestLap,
    Dnf,
}

/// Facts every content item exposes on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentFact {
    /// Sent to the frame.
    Delivered,
    /// Opened by the player (reported by the frame).
    Seen,
    /// Id of the reply the player chose.
    Reply,
}

/// A variable a condition reads or an effect writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Var {
    Date,
    Year,
    Month,
    /// The chapter of the current flow node; compared in flow order.
    Chapter,
    Reputation,
    Followers,
    /// Only exists when the story has an economy (`settings.money`).
    Money,
    Flag(String),
    Counter(String),
    /// Relation with a character, on the story's scale.
    Rel(String),
    /// Whether the player owns that car.
    Car(String),
    Event(String, EventFact),
    Content(String, ContentFact),
}

/// The right-hand side of a comparison or an assignment, typed by its form
/// only: `warm` is a name until the validator knows it is a relation level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Literal {
    Int(i64),
    Bool(bool),
    Date(StoryDate),
    /// Milliseconds, written `1:18.200`.
    LapTime(u32),
    Name(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// `[not] <var> [<op> <literal>]`. A bare variable is a yes/no test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition {
    pub negated: bool,
    pub var: Var,
    pub test: Option<(CmpOp, Literal)>,
}

/// What a beat, an event outcome or a reply changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// `<var> = <literal>`.
    Set(Var, Literal),
    /// `<var> += n` / `<var> -= n`; on a relation, `n` steps along the scale.
    Add(Var, i64),
    /// `send <content>`: deliver a flow content item now ("Unlock:
    /// Conversation 06" in the spec's event example).
    Send(String),
}

/// One thing an event result must satisfy; all of an event's must hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Criterion {
    /// `position <= N`.
    PositionAtMost(u32),
    /// `finished`: no DNF.
    Finished,
    /// `ahead_of <driver>`: finished ahead of a named driver.
    AheadOf(String),
    /// `best_lap < 1:18.200`, in milliseconds.
    LapUnder(u32),
}

pub fn is_id(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn id(s: &str, what: &str) -> Result<String, String> {
    if is_id(s) {
        Ok(s.to_string())
    } else {
        Err(format!("invalid {what} id `{s}` (letters, digits, `_` and `-` only)"))
    }
}

impl FromStr for Var {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('.').collect();
        Ok(match parts.as_slice() {
            ["date"] => Var::Date,
            ["year"] => Var::Year,
            ["month"] => Var::Month,
            ["chapter"] => Var::Chapter,
            ["reputation"] => Var::Reputation,
            ["followers"] => Var::Followers,
            ["money"] => Var::Money,
            ["flag", name] => Var::Flag(id(name, "flag")?),
            ["counter", name] => Var::Counter(id(name, "counter")?),
            ["rel", who] => Var::Rel(id(who, "character")?),
            ["car", car] => Var::Car(id(car, "car")?),
            ["event", ev, fact] => {
                let fact = match *fact {
                    "attempts" => EventFact::Attempts,
                    "succeeded" => EventFact::Succeeded,
                    "finished" => EventFact::Finished,
                    "position" => EventFact::Position,
                    "best_lap" => EventFact::BestLap,
                    "dnf" => EventFact::Dnf,
                    _ => {
                        return Err(format!(
                            "unknown event fact `{fact}` (attempts, succeeded, finished, position, best_lap, dnf)"
                        ))
                    }
                };
                Var::Event(id(ev, "event")?, fact)
            }
            ["content", c, fact] => {
                let fact = match *fact {
                    "delivered" => ContentFact::Delivered,
                    "seen" => ContentFact::Seen,
                    "reply" => ContentFact::Reply,
                    _ => return Err(format!("unknown content fact `{fact}` (delivered, seen, reply)")),
                };
                Var::Content(id(c, "content")?, fact)
            }
            _ => return Err(format!("unknown variable `{s}`")),
        })
    }
}

impl fmt::Display for Var {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Var::Date => f.write_str("date"),
            Var::Year => f.write_str("year"),
            Var::Month => f.write_str("month"),
            Var::Chapter => f.write_str("chapter"),
            Var::Reputation => f.write_str("reputation"),
            Var::Followers => f.write_str("followers"),
            Var::Money => f.write_str("money"),
            Var::Flag(n) => write!(f, "flag.{n}"),
            Var::Counter(n) => write!(f, "counter.{n}"),
            Var::Rel(n) => write!(f, "rel.{n}"),
            Var::Car(n) => write!(f, "car.{n}"),
            Var::Event(e, fact) => {
                let fact = match fact {
                    EventFact::Attempts => "attempts",
                    EventFact::Succeeded => "succeeded",
                    EventFact::Finished => "finished",
                    EventFact::Position => "position",
                    EventFact::BestLap => "best_lap",
                    EventFact::Dnf => "dnf",
                };
                write!(f, "event.{e}.{fact}")
            }
            Var::Content(c, fact) => {
                let fact = match fact {
                    ContentFact::Delivered => "delivered",
                    ContentFact::Seen => "seen",
                    ContentFact::Reply => "reply",
                };
                write!(f, "content.{c}.{fact}")
            }
        }
    }
}

/// `1:18.2`, `1:18.200`, `0:59.999` → milliseconds.
pub fn parse_lap_time(s: &str) -> Option<u32> {
    let (min, rest) = s.split_once(':')?;
    let (sec, frac) = rest.split_once('.').unwrap_or((rest, ""));
    if sec.len() != 2 || frac.len() > 3 || !frac.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let (min, sec): (u32, u32) = (min.parse().ok()?, sec.parse().ok()?);
    if sec > 59 {
        return None;
    }
    let ms = if frac.is_empty() {
        0
    } else {
        format!("{frac:0<3}").parse::<u32>().ok()?
    };
    Some(min * 60_000 + sec * 1000 + ms)
}

pub fn format_lap_time(ms: u32) -> String {
    format!("{}:{:02}.{:03}", ms / 60_000, ms / 1000 % 60, ms % 1000)
}

impl FromStr for Literal {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "true" => Literal::Bool(true),
            "false" => Literal::Bool(false),
            _ if s.contains(':') => {
                Literal::LapTime(parse_lap_time(s).ok_or_else(|| format!("invalid lap time `{s}`"))?)
            }
            // `YYYY-MM-DD` by shape, digits included: ids may hold `-` too
            // (`very-close`), and must stay names.
            _ if s.len() == 10
                && s.bytes().enumerate().all(|(i, b)| {
                    if i == 4 || i == 7 {
                        b == b'-'
                    } else {
                        b.is_ascii_digit()
                    }
                }) =>
            {
                Literal::Date(s.parse()?)
            }
            _ => match s.parse::<i64>() {
                Ok(n) => Literal::Int(n),
                Err(_) => Literal::Name(id(s, "value")?),
            },
        })
    }
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Literal::Int(n) => write!(f, "{n}"),
            Literal::Bool(b) => write!(f, "{b}"),
            Literal::Date(d) => write!(f, "{d}"),
            Literal::LapTime(ms) => f.write_str(&format_lap_time(*ms)),
            Literal::Name(n) => f.write_str(n),
        }
    }
}

impl CmpOp {
    fn parse(s: &str) -> Option<CmpOp> {
        Some(match s {
            "==" => CmpOp::Eq,
            "!=" => CmpOp::Ne,
            "<" => CmpOp::Lt,
            "<=" => CmpOp::Le,
            ">" => CmpOp::Gt,
            ">=" => CmpOp::Ge,
            _ => return None,
        })
    }

    pub fn holds<T: Ord>(self, left: T, right: T) -> bool {
        match self {
            CmpOp::Eq => left == right,
            CmpOp::Ne => left != right,
            CmpOp::Lt => left < right,
            CmpOp::Le => left <= right,
            CmpOp::Gt => left > right,
            CmpOp::Ge => left >= right,
        }
    }

    /// Whether the operator needs an order (a flag can be `==` but not `<`).
    pub fn is_ordering(self) -> bool {
        !matches!(self, CmpOp::Eq | CmpOp::Ne)
    }
}

impl fmt::Display for CmpOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            CmpOp::Eq => "==",
            CmpOp::Ne => "!=",
            CmpOp::Lt => "<",
            CmpOp::Le => "<=",
            CmpOp::Gt => ">",
            CmpOp::Ge => ">=",
        })
    }
}

impl FromStr for Condition {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut words: Vec<&str> = s.split_whitespace().collect();
        let negated = words.first() == Some(&"not");
        if negated {
            words.remove(0);
        }
        match words.as_slice() {
            [var] => Ok(Condition {
                negated,
                var: var.parse()?,
                test: None,
            }),
            [var, op, value] => {
                let op = CmpOp::parse(op).ok_or_else(|| format!("unknown operator `{op}` in `{s}`"))?;
                Ok(Condition {
                    negated,
                    var: var.parse()?,
                    test: Some((op, value.parse()?)),
                })
            }
            _ => Err(format!(
                "cannot read condition `{s}` (expected `[not] <variable> [<op> <value>]`)"
            )),
        }
    }
}

impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.negated {
            f.write_str("not ")?;
        }
        write!(f, "{}", self.var)?;
        if let Some((op, value)) = &self.test {
            write!(f, " {op} {value}")?;
        }
        Ok(())
    }
}

impl FromStr for Effect {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split_whitespace().collect::<Vec<_>>().as_slice() {
            ["send", content] => Ok(Effect::Send(id(content, "content")?)),
            [var, "=", value] => Ok(Effect::Set(var.parse()?, value.parse()?)),
            [var, op @ ("+=" | "-="), n] => {
                let n: i64 = n.parse().map_err(|_| format!("`{n}` is not a whole number in `{s}`"))?;
                Ok(Effect::Add(var.parse()?, if *op == "+=" { n } else { -n }))
            }
            _ => Err(format!(
                "cannot read effect `{s}` (expected `<variable> = <value>`, `+= n`, `-= n` or `send <content>`)"
            )),
        }
    }
}

impl fmt::Display for Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Effect::Set(var, value) => write!(f, "{var} = {value}"),
            Effect::Add(var, n) if *n < 0 => write!(f, "{var} -= {}", -n),
            Effect::Add(var, n) => write!(f, "{var} += {n}"),
            Effect::Send(c) => write!(f, "send {c}"),
        }
    }
}

impl FromStr for Criterion {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split_whitespace().collect::<Vec<_>>().as_slice() {
            ["finished"] => Ok(Criterion::Finished),
            ["position", "<=", n] => {
                n.parse().map(Criterion::PositionAtMost).map_err(|_| format!("`{n}` is not a position in `{s}`"))
            }
            ["ahead_of", driver] => Ok(Criterion::AheadOf(id(driver, "driver")?)),
            ["best_lap", "<", t] => {
                parse_lap_time(t).map(Criterion::LapUnder).ok_or_else(|| format!("invalid lap time `{t}` in `{s}`"))
            }
            _ => Err(format!(
                "cannot read success criterion `{s}` (expected `position <= N`, `finished`, `ahead_of <driver>` or `best_lap < m:ss.fff`)"
            )),
        }
    }
}

impl fmt::Display for Criterion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Criterion::PositionAtMost(n) => write!(f, "position <= {n}"),
            Criterion::Finished => f.write_str("finished"),
            Criterion::AheadOf(d) => write!(f, "ahead_of {d}"),
            Criterion::LapUnder(ms) => write!(f, "best_lap < {}", format_lap_time(*ms)),
        }
    }
}

serde_as_text!(Condition);
serde_as_text!(Effect);
serde_as_text!(Criterion);

#[cfg(test)]
mod tests {
    use super::*;

    // The written form is what the editor (L2) will write back: parsing then
    // printing must give the author's sentence again, or every save in the
    // editor would rewrite lines nobody touched.
    #[test]
    fn conditions_and_effects_round_trip_through_their_text() {
        for text in [
            "reputation >= 10",
            "not flag.sponsor_left",
            "rel.friend >= warm",
            "event.race_01.position <= 3",
            "content.invite_01.reply == accept",
            "date >= 1999-06-01",
            "event.race_01.best_lap < 1:18.200",
            "counter.garage_visits == 2",
            "chapter >= ch02",
            "rel.friend >= very-close",
        ] {
            assert_eq!(
                text.parse::<Condition>().unwrap().to_string(),
                text,
                "condition `{text}`"
            );
        }
        for text in [
            "reputation += 2",
            "money -= 500",
            "flag.met_rival = true",
            "rel.friend = cold",
            "send conv_06",
        ] {
            assert_eq!(text.parse::<Effect>().unwrap().to_string(), text, "effect `{text}`");
        }
        for text in [
            "position <= 3",
            "finished",
            "ahead_of some_driver",
            "best_lap < 1:18.200",
        ] {
            assert_eq!(
                text.parse::<Criterion>().unwrap().to_string(),
                text,
                "criterion `{text}`"
            );
        }
    }

    #[test]
    fn a_malformed_sentence_says_what_was_expected() {
        let err = "reputation is very high".parse::<Condition>().unwrap_err();
        assert!(err.contains("expected"), "{err}");
        let err = "reputation is high".parse::<Condition>().unwrap_err();
        assert!(err.contains("unknown operator `is`"), "{err}");
        let err = "event.race_01.podium".parse::<Condition>().unwrap_err();
        assert!(err.contains("podium"), "names the bad fact: {err}");
        assert!("reputation += lots".parse::<Effect>().is_err());
        assert!("position < 3".parse::<Criterion>().is_err(), "only `<=` for a position");
    }

    #[test]
    fn lap_times_accept_short_fractions() {
        assert_eq!(parse_lap_time("1:18.2"), Some(78_200));
        assert_eq!(parse_lap_time("1:18"), Some(78_000));
        assert_eq!(parse_lap_time("0:59.999"), Some(59_999));
        assert_eq!(parse_lap_time("1:78.0"), None, "seconds above 59");
        assert_eq!(format_lap_time(78_200), "1:18.200");
    }
}
