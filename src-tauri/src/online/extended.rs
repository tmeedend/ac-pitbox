//! What some servers tell beyond `/INFO`: `/api/details` ("Ce qu'un serveur
//! AC expose", level 3, of `SPEC-play-online.md`) — conditions, rules and a
//! description. AssettoServer and CM's wrapper answer it; a vanilla server
//! does not (measured: an HTML page), which is the common case, not an error.
//!
//! Field meanings are CM's (`ServerInformationExtended`,
//! `ServerInformationExtendedAssists`): assists are 0 denied / 1 factory /
//! 2 forced; fuel, damage and tyre wear are percentages; `allowedTyresOut` is
//! a count of wheels, `-1` when not checked.

use serde::Serialize;
use serde_json::Value;

use crate::http;

use super::lobby::{SERVER_AGENT, SERVER_MAX_BODY, SERVER_TIMEOUT_MS};

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Conditions {
    /// The weather's id as CSP names it (`FewClouds`), not translated: it is
    /// the server's word, like a track name.
    pub weather: Option<String>,
    pub ambient: Option<f64>,
    pub road: Option<f64>,
    /// km/h.
    pub wind_speed: Option<f64>,
    /// Degrees.
    pub wind_direction: Option<f64>,
    /// Percent.
    pub grip: Option<f64>,
}

/// A rule that departs from what an AC server does by default — the spec
/// shows rules "comme écarts par rapport à la norme", never as twelve values.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Rule {
    AbsDenied,
    AbsForced,
    TcDenied,
    TcForced,
    StabilityAllowed,
    AutoclutchDenied,
    TyreBlanketsAllowed,
    VirtualMirrorForced,
    Damage { percent: i64 },
    Fuel { percent: i64 },
    TyreWear { percent: i64 },
    TyresOut { count: i64 },
}

/// What lets a password be checked before the game is launched
/// (`SPEC-play-online.md`, "Ce qu'un serveur AC expose": "vérifier le mot de
/// passe avant de lancer le jeu"). CM's wrapper publishes, for the player's
/// password and the admin's, `sha1("apatosaur" + name + password)` — the
/// recipe of its `passwordChecksum` (gro-ove/ac-server-wrapper, `AcServer.js`)
/// and what CM's own client compares (`ServerEntry.CheckPasswordChecksum`).
/// The name is the one `/api/details` gives, without the `ℹport` the lobby
/// shows. Measured 2026-10-03: 837 locked servers publish it, all through the
/// wrapper; on 255 of them an empty admin password's checksum is exactly
/// `sha1("apatosaur" + name)`, which is what confirms the recipe.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct PasswordCheck {
    pub salt: String,
    /// Lowercase hex, the player's password and the admin's.
    pub checksums: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Extended {
    pub conditions: Conditions,
    pub rules: Vec<Rule>,
    /// The description as plain text, its BBCode removed.
    pub description: Option<String>,
    /// On a locked server that publishes it.
    pub password_check: Option<PasswordCheck>,
}

fn password_check(root: &Value) -> Option<PasswordCheck> {
    let checksums: Vec<String> = root["passwordChecksum"]
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .filter(|c| c.len() == 40 && c.chars().all(|ch| ch.is_ascii_hexdigit()))
        .map(str::to_lowercase)
        .collect();
    let salt = root["name"].as_str()?.to_string();
    (!checksums.is_empty()).then_some(PasswordCheck { salt, checksums })
}

fn int(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_f64().map(|f| f.round() as i64))
}

/// The departures from the defaults, in a fixed order.
fn rules(assists: &Value) -> Vec<Rule> {
    let mut out = Vec::new();
    match int(&assists["absState"]) {
        Some(0) => out.push(Rule::AbsDenied),
        Some(2) => out.push(Rule::AbsForced),
        _ => {}
    }
    match int(&assists["tcState"]) {
        Some(0) => out.push(Rule::TcDenied),
        Some(2) => out.push(Rule::TcForced),
        _ => {}
    }
    if assists["stabilityAllowed"].as_bool() == Some(true) {
        out.push(Rule::StabilityAllowed);
    }
    if assists["autoclutchAllowed"].as_bool() == Some(false) {
        out.push(Rule::AutoclutchDenied);
    }
    if assists["tyreBlanketsAllowed"].as_bool() == Some(true) {
        out.push(Rule::TyreBlanketsAllowed);
    }
    if assists["forceVirtualMirror"].as_bool() == Some(true) {
        out.push(Rule::VirtualMirrorForced);
    }
    for (key, make) in [
        ("damageMultiplier", (|p| Rule::Damage { percent: p }) as fn(i64) -> Rule),
        ("fuelRate", |p| Rule::Fuel { percent: p }),
        ("tyreWearRate", |p| Rule::TyreWear { percent: p }),
    ] {
        if let Some(p) = int(&assists[key]).filter(|&p| p != 100) {
            out.push(make(p));
        }
    }
    if let Some(count) = int(&assists["allowedTyresOut"]).filter(|&n| n >= 0 && n != 2) {
        out.push(Rule::TyresOut { count });
    }
    out
}

/// Removes BBCode tags (`[b]`, `[color=#42dfc3]`, `[url=…]`), keeping the text
/// they wrap and the line breaks; collapses the runs of blank lines servers
/// pad their descriptions with.
pub fn strip_bbcode(text: &str) -> String {
    let tag = regex::Regex::new(r"\[/?[a-zA-Z*]+(=[^\]]*)?\]").expect("static regex");
    let plain = tag.replace_all(text, "");
    let mut out: Vec<&str> = Vec::new();
    for line in plain.lines().map(str::trim) {
        if line.is_empty() && out.last().is_none_or(|l| l.is_empty()) {
            continue;
        }
        out.push(line);
    }
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out.join("\n")
}

const IMAGE_EXTENSIONS: [&str; 5] = [".png", ".jpg", ".jpeg", ".gif", ".webp"];

/// Web links found in a server's name and description: `[url=…]`, bare
/// `http(s)://…`, and `discord.gg/…` written without a scheme — the way
/// server names carry them. Unique, in order of appearance.
pub fn links(texts: &[&str]) -> Vec<String> {
    let url = regex::Regex::new(r#"(?i)\bhttps?://[^\s\]\[|"<>]+|\bdiscord\.gg/[A-Za-z0-9_-]+"#).expect("static regex");
    let mut out: Vec<String> = Vec::new();
    for text in texts {
        for m in url.find_iter(text) {
            let raw = m.as_str().trim_end_matches(['.', ',', ')', ';']);
            let link = if raw.to_lowercase().starts_with("http") {
                raw.to_string()
            } else {
                format!("https://{raw}")
            };
            // The banners of a description (`[img=…png]`) are pictures, not
            // places to go: seen on a real server, first in its list.
            let path = link.split(['?', '#']).next().unwrap_or_default().to_lowercase();
            if IMAGE_EXTENSIONS.iter().any(|ext| path.ends_with(ext)) {
                continue;
            }
            if !out.contains(&link) {
                out.push(link);
            }
        }
    }
    out
}

pub(super) fn parse(root: &Value) -> Extended {
    let num = |key: &str| root[key].as_f64();
    Extended {
        conditions: Conditions {
            weather: root["currentWeatherId"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            ambient: num("ambientTemperature"),
            road: num("roadTemperature"),
            wind_speed: num("windSpeed"),
            wind_direction: num("windDirection"),
            grip: num("grip"),
        },
        rules: rules(&root["assists"]),
        description: root["description"].as_str().map(strip_bbcode).filter(|d| !d.is_empty()),
        password_check: password_check(root),
    }
}

/// `/api/details`, read, with the answer itself — where the description as
/// written (BBCode and all, its `[url=…]` links) and the `content` block are.
/// `None` when the server does not offer it — quietly: most servers do not,
/// and a log line per vanilla server would bury the lines that matter.
pub fn fetch(ip: &str, http_port: u16, steam_id: u64) -> Option<(Extended, Value)> {
    let url = format!("http://{ip}:{http_port}/api/details?guid={steam_id}");
    let response = http::get_url(&url, SERVER_AGENT, SERVER_TIMEOUT_MS, SERVER_MAX_BODY)?;
    if response.status != 200 {
        return None;
    }
    let root: Value = serde_json::from_slice(&response.body).ok()?;
    root.is_object().then(|| (parse(&root), root))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rule: a locked server's checksums are kept with the name that salts
    /// them — a real answer (CM's wrapper, 2026-10-03), its name in Cyrillic;
    /// a server that publishes none, or garbage, gives no check.
    #[test]
    fn a_locked_server_gives_its_password_check() {
        let root = serde_json::json!({
            "name": "GL-Team - Сервер",
            "passwordChecksum": ["dbe24a8795b95c3559aa8a4caba5ea79da223078", "A6D90D3AFADC8649CA7A6924E70A73992B43ED2F"]
        });
        assert_eq!(
            password_check(&root),
            Some(PasswordCheck {
                salt: "GL-Team - Сервер".into(),
                checksums: vec![
                    "dbe24a8795b95c3559aa8a4caba5ea79da223078".into(),
                    "a6d90d3afadc8649ca7a6924e70a73992b43ed2f".into()
                ],
            })
        );
        assert_eq!(
            password_check(&serde_json::json!({ "name": "x" })),
            None,
            "no checksum published"
        );
        assert_eq!(
            password_check(&serde_json::json!({ "name": "x", "passwordChecksum": ["not a sha1"] })),
            None,
            "nothing usable"
        );
    }

    /// A real answer (an AssettoServer on LA Canyons, 2026-10-03), trimmed.
    fn answer() -> Value {
        serde_json::json!({
            "ambientTemperature": 22, "roadTemperature": 30.336649, "currentWeatherId": "FewClouds",
            "windSpeed": 0, "windDirection": 0, "grip": 100,
            "assists": { "absState": 1, "tcState": 1, "fuelRate": 0, "damageMultiplier": 0, "tyreWearRate": 0,
                         "allowedTyresOut": -1, "stabilityAllowed": false, "autoclutchAllowed": true,
                         "tyreBlanketsAllowed": true, "forceVirtualMirror": false },
            "description": " [size=18][color=#42dfc3][b]Server Information:[/b][/color][/size]\n\n\n- Fuel rate = 0%\n"
        })
    }

    /// Rule (SPEC-play-online, "Règles"): only what departs from an AC
    /// server's defaults is listed — here no damage, fuel or wear, and
    /// blankets; factory ABS and TC, and unchecked tyres out, are the norm.
    #[test]
    fn rules_are_departures_from_the_norm() {
        assert_eq!(
            parse(&answer()).rules,
            vec![
                Rule::TyreBlanketsAllowed,
                Rule::Damage { percent: 0 },
                Rule::Fuel { percent: 0 },
                Rule::TyreWear { percent: 0 },
            ]
        );
    }

    /// Rule: conditions are read as the server gives them.
    #[test]
    fn conditions_are_read() {
        let c = parse(&answer()).conditions;
        assert_eq!(c.weather.as_deref(), Some("FewClouds"));
        assert_eq!((c.ambient, c.grip), (Some(22.0), Some(100.0)));
    }

    /// Rule: BBCode is removed, its text and line breaks kept, blank runs
    /// collapsed.
    #[test]
    fn bbcode_is_stripped() {
        assert_eq!(
            parse(&answer()).description.as_deref(),
            Some("Server Information:\n\n- Fuel rate = 0%")
        );
    }

    /// Rule (SPEC-play-online, "Nom du serveur"): the Discord link a server
    /// name carries becomes a link, scheme added; duplicates are dropped.
    #[test]
    fn links_are_found_in_names_and_descriptions() {
        let name = "Cruisin USA | L.A. Canyons | cruisin.us/discord | discord.gg/AbC_12";
        let description = "[img=https://cdn.discordapp.com/attachments/1/2/banner.png]x[/img]\n\
            [url=https://cruisin.us]Site[/url] and https://discord.gg/AbC_12.";
        assert_eq!(
            links(&[name, description]),
            vec!["https://discord.gg/AbC_12", "https://cruisin.us"],
            "the banner image is not a link"
        );
    }
}
