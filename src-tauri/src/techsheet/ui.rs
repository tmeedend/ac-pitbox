//! The `specs` of a `ui_car.json`, read as numbers when they are numbers
//! (FICHE§2.3, FICHE§4).
//!
//! They are free text typed by the author, Kunos included: `"--km/h"`,
//! `"500+Nm"`, `"3.4s 0-100"`. Three outcomes, never a fourth:
//!
//! - **absent** — `--`, empty, no digit, a zero: the author's own "unknown"
//!   (R4). 97 of 397 cars of the reference install say `--s 0-100`;
//! - **a quantity** — a number, a unit we know, and the author's `+` kept
//!   (`270+ km/h` is not `270 km/h`);
//! - **the text as written** — anything else (`(544+120)Bhp`, `470bhp/7000rpm`,
//!   a unit we do not know): shown as is rather than lost, and never turned
//!   into a number it might not be.
//!
//! Units are folded onto one spelling per unit (`kph` → `km/h`, `кг` → `kg`),
//! never converted: `whp` stays `whp`, see `carSpecs.ts` for why.

use serde_json::{json, Value};

/// What a spec string is worth.
#[derive(Debug, Clone, PartialEq)]
pub enum Spec {
    Quantity {
        n: f64,
        unit: Option<&'static str>,
        plus: bool,
    },
    Text(String),
}

impl Spec {
    /// The stored form: `{"n": 470, "unit": "bhp"}` (plus `"plus": true`
    /// when the author wrote one), or the text as a JSON string.
    pub fn to_json(&self) -> Value {
        match self {
            Spec::Quantity { n, unit, plus } => {
                let mut v = json!({ "n": n });
                if let Some(u) = unit {
                    v["unit"] = json!(u);
                }
                if *plus {
                    v["plus"] = json!(true);
                }
                v
            }
            Spec::Text(t) => json!(t),
        }
    }
}

/// Which spec a string is, for its units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Power,
    Torque,
    Weight,
    TopSpeed,
    PwRatio,
    /// `range`: a bare number, no unit ever written (FICHE§4).
    Range,
}

/// The one spelling of each unit the sheet knows, per kind. A unit outside
/// this table makes the value text: R4 says an unknown unit is not shown, and
/// dropping it from a number would show a number that means something else.
fn unit(kind: Kind, written: &str) -> Option<&'static str> {
    let u = written.to_lowercase().replace([' ', '*', '.'], "");
    Some(match (kind, u.as_str()) {
        (Kind::Power, "bhp") => "bhp",
        (Kind::Power, "hp") => "hp",
        (Kind::Power, "whp") => "whp",
        (Kind::Power, "ps" | "лс") => "ps",
        (Kind::Power, "cv") => "cv",
        (Kind::Power, "kw") => "kW",
        (Kind::Torque, "nm" | "нм") => "Nm",
        (Kind::Torque, "kg-m" | "kgm") => "kgm",
        (Kind::Torque, "lb-ft" | "lbft" | "ft-lb" | "ftlb") => "lb-ft",
        (Kind::Weight, "kg" | "кг") => "kg",
        (Kind::Weight, "lb" | "lbs") => "lb",
        (Kind::TopSpeed, "km/h" | "kph" | "kmh" | "км/ч") => "km/h",
        (Kind::TopSpeed, "mph") => "mph",
        (Kind::PwRatio, "kg/hp" | "kg/bhp") => "kg/hp",
        (Kind::PwRatio, "kg/cv") => "kg/cv",
        (Kind::PwRatio, "kg/ps" | "кг/лс") => "kg/ps",
        _ => return None,
    })
}

/// A number with a decimal comma or point.
fn number(s: &str) -> Option<f64> {
    s.replace(',', ".").parse().ok()
}

/// The author's "unknown": nothing, dashes, `N/A`, no digit at all.
fn says_nothing(raw: &str) -> bool {
    raw.is_empty() || raw.contains("--") || !raw.bytes().any(|b| b.is_ascii_digit())
}

/// One spec string of `kind`. `None` is absent (R4).
pub fn parse(kind: Kind, raw: Option<&str>) -> Option<Spec> {
    let raw = raw?.trim();
    if says_nothing(raw) {
        return None;
    }
    let text = || Some(Spec::Text(raw.to_string()));
    // `+270 km/h`, `>300km/h`: said before the number, meant as after it.
    let (lead_plus, rest) = match raw.strip_prefix(['+', '>']) {
        Some(r) => (true, r.trim_start()),
        None => (false, raw),
    };
    let digits = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))
        .unwrap_or(rest.len());
    if digits == 0 {
        // `~3s`, `<5s`, `(544+120)Bhp`: a qualifier the sheet has no place for.
        return text();
    }
    let Some(n) = number(&rest[..digits]) else {
        return text();
    };
    if n == 0.0 {
        return None;
    }
    let rest = rest[digits..].trim_start();
    let (trail_plus, written) = match rest.strip_prefix('+') {
        Some(r) => (true, r.trim()),
        None => (false, rest.trim()),
    };
    let plus = lead_plus || trail_plus;
    if written.is_empty() || written == "*" {
        // A bare number: `range` is always one, and elsewhere R4 shows it
        // without the unit nobody wrote.
        return Some(Spec::Quantity { n, unit: None, plus });
    }
    match unit(kind, written) {
        Some(u) => Some(Spec::Quantity { n, unit: Some(u), plus }),
        None => text(),
    }
}

/// `acceleration`: the seconds of a 0-100, found wherever the author put them
/// (`3.4s 0-100`, `0 - 100 kph in 3.1 s`, `3.2s 0-100, 11.2s qtr`).
///
/// A 0-60 mph time is not a 0-100 km/h one, and is kept as text rather than
/// shown under a label it does not answer.
pub fn parse_acceleration(raw: Option<&str>) -> Option<Spec> {
    let raw = raw?.trim();
    if says_nothing(raw) {
        return None;
    }
    let lower = raw.to_lowercase();
    // `s 0-100` with no time in front: the label without its value.
    if says_nothing(&lower.replace("0-100", "").replace("0–100", "")) {
        return None;
    }
    let text = || Some(Spec::Text(raw.to_string()));
    if lower.contains("mph") || lower.contains("0-60") || lower.contains("0–60") || lower.starts_with(['~', '<']) {
        return text();
    }
    // The first number directly followed by `s` / `sec`.
    let bytes = lower.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.' || bytes[i] == b',') {
                i += 1;
            }
            let after = lower[i..].trim_start();
            let is_seconds = after.starts_with("sec")
                || (after.starts_with('s') && !after[1..].starts_with(|c: char| c.is_alphabetic()));
            if is_seconds {
                return match number(&lower[start..i]) {
                    Some(n) if n > 0.0 => Some(Spec::Quantity {
                        n,
                        unit: Some("s"),
                        plus: false,
                    }),
                    _ => text(),
                };
            }
        } else {
            i += 1;
        }
    }
    text()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(n: f64, unit: Option<&'static str>, plus: bool) -> Option<Spec> {
        Some(Spec::Quantity { n, unit, plus })
    }

    /// FICHE§9.4 — the author's dashes are absent, not a value.
    #[test]
    fn dashes_and_blanks_are_absent() {
        assert_eq!(parse_acceleration(Some("--s 0-100")), None, "Kunos' own placeholder");
        assert_eq!(parse(Kind::TopSpeed, Some("--km/h")), None);
        assert_eq!(parse(Kind::Torque, Some("N/A*")), None, "no digit");
        assert_eq!(parse(Kind::Weight, Some("")), None);
        assert_eq!(parse(Kind::Power, Some("0bhp")), None, "a zero fills a hole");
        assert_eq!(parse(Kind::Power, None), None);
    }

    /// FICHE§9.4 — the `+` of `500+Nm` is the author's, and it stays.
    #[test]
    fn the_authors_plus_is_kept() {
        assert_eq!(parse(Kind::Torque, Some("500+Nm")), q(500.0, Some("Nm"), true));
        assert_eq!(parse(Kind::TopSpeed, Some("270+ km/h")), q(270.0, Some("km/h"), true));
        assert_eq!(
            parse(Kind::TopSpeed, Some("+310km/h")),
            q(310.0, Some("km/h"), true),
            "written before"
        );
        assert_eq!(parse(Kind::TopSpeed, Some(">300 km/h")), q(300.0, Some("km/h"), true));
        assert_eq!(parse(Kind::Power, Some("470bhp")), q(470.0, Some("bhp"), false));
    }

    /// Units fold onto one spelling, never onto another unit.
    #[test]
    fn units_are_spelled_one_way_and_never_converted() {
        assert_eq!(parse(Kind::TopSpeed, Some("280kph")), q(280.0, Some("km/h"), false));
        assert_eq!(parse(Kind::Weight, Some("1525 kg*")), q(1525.0, Some("kg"), false));
        assert_eq!(parse(Kind::Weight, Some("1100кг")), q(1100.0, Some("kg"), false));
        assert_eq!(
            parse(Kind::Power, Some("620 wHP")),
            q(620.0, Some("whp"), false),
            "whp is not bhp"
        );
        assert_eq!(
            parse(Kind::PwRatio, Some("2,65kg/hp")),
            q(2.65, Some("kg/hp"), false),
            "decimal comma"
        );
    }

    /// FICHE§9.4 — `range` is a bare number.
    #[test]
    fn range_is_a_bare_number() {
        assert_eq!(parse(Kind::Range, Some("195")), q(195.0, None, false));
    }

    /// FICHE§9.4 — what cannot be read as a number is kept as written.
    #[test]
    fn an_unreadable_string_is_kept_as_is() {
        assert_eq!(
            parse(Kind::Power, Some("(544+120)Bhp")),
            Some(Spec::Text("(544+120)Bhp".into()))
        );
        assert_eq!(
            parse(Kind::Power, Some("470bhp/7000rpm")),
            Some(Spec::Text("470bhp/7000rpm".into())),
            "an rpm glued to the unit is not a unit we know"
        );
        assert_eq!(
            parse(Kind::PwRatio, Some("2.1kh/hp")),
            Some(Spec::Text("2.1kh/hp".into())),
            "a typo stays a typo"
        );
    }

    /// The seconds of a 0-100, wherever they are; a 0-60 is not one.
    #[test]
    fn acceleration_is_read_where_the_seconds_are() {
        assert_eq!(parse_acceleration(Some("3.4s 0-100")), q(3.4, Some("s"), false));
        assert_eq!(
            parse_acceleration(Some("0 - 100 kph in 3.1 s")),
            q(3.1, Some("s"), false)
        );
        assert_eq!(
            parse_acceleration(Some("3.2s 0-100, 11.2s qtr")),
            q(3.2, Some("s"), false)
        );
        assert_eq!(parse_acceleration(Some("2,8 sec")), q(2.8, Some("s"), false));
        assert_eq!(
            parse_acceleration(Some("4.1s 0-60mph")),
            Some(Spec::Text("4.1s 0-60mph".into())),
            "not a 0-100"
        );
        assert_eq!(parse_acceleration(Some("~3s")), Some(Spec::Text("~3s".into())));
        assert_eq!(parse_acceleration(Some("s 0-100")), None, "the label without its time");
    }

    /// The stored form carries the `+` only when there is one.
    #[test]
    fn the_stored_form_is_compact() {
        assert_eq!(
            q(470.0, Some("bhp"), false).unwrap().to_json(),
            json!({"n": 470.0, "unit": "bhp"})
        );
        assert_eq!(
            q(270.0, Some("km/h"), true).unwrap().to_json(),
            json!({"n": 270.0, "unit": "km/h", "plus": true})
        );
        assert_eq!(Spec::Text("x".into()).to_json(), json!("x"));
    }
}
