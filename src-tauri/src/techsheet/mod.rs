//! The tech sheet of a car (FICHE§): every value in the base, one source per
//! value, and a fixed order between sources.
//!
//! Three writers, each owning its own facts (FICHE§6.2):
//!
//! - [`record`] — what the **files** of one version say (`physics`, `ui`,
//!   and the stock content table for a year), at import, reindex and backfill;
//! - [`store_harmonized`] — what the **rules** deduce, and the country as the
//!   harmonisation settles it, at every harmonisation;
//! - [`save_user`] — what the **user** decided, in a table nothing recomputes
//!   (R6).
//!
//! And one reader, [`effective`], which applies R2 and is the only place the
//! order between sources is written. The five spec columns of `mods` are a
//! cache of its answer ([`refresh_cache`]), so the library filters and sorts on
//! exactly what the sheet shows.

pub mod physics;
mod store;
mod ui;

#[cfg(test)]
mod measure;

use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub use store::pending_cars;

/// Where a value comes from (R2, R5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// Decided by the user (`tech_user`), never stored as a fact.
    User,
    Physics,
    Ui,
    /// The table of the game's own content (`kunos_dates`): a year.
    Table,
    Rules,
    /// Worked out from two other values (a power-to-weight ratio). Never stored.
    Computed,
}

impl Source {
    fn as_str(self) -> &'static str {
        match self {
            Source::User => "user",
            Source::Physics => "physics",
            Source::Ui => "ui",
            Source::Table => "table",
            Source::Rules => "rules",
            Source::Computed => "computed",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "physics" => Source::Physics,
            "ui" => Source::Ui,
            "table" => Source::Table,
            "rules" => Source::Rules,
            _ => return None,
        })
    }
}

/// One thing one source says about one field.
#[derive(Debug, Clone, PartialEq)]
pub struct Fact {
    pub field: &'static str,
    pub source: Source,
    pub value: Value,
}

fn fact(field: &'static str, source: Source, value: Value) -> Fact {
    Fact { field, source, value }
}

/// The field keys, as persisted in `tech_facts` and `tech_user`. **Never
/// renamed**: they are rows in users' databases.
pub mod field {
    pub const POWER: &str = "power";
    pub const TORQUE: &str = "torque";
    pub const WEIGHT: &str = "weight";
    pub const PWRATIO: &str = "pwratio";
    pub const TOPSPEED: &str = "topspeed";
    pub const ACCELERATION: &str = "acceleration";
    pub const ENGINE_CONFIG: &str = "engine_config";
    pub const ASPIRATION: &str = "aspiration";
    pub const ENGINE_POS: &str = "engine_pos";
    pub const RPM_LIMIT: &str = "rpm_limit";
    pub const DRIVETRAIN: &str = "drivetrain";
    pub const GEARBOX: &str = "gearbox";
    pub const GEARS: &str = "gears";
    pub const FUEL_TANK: &str = "fuel_tank";
    pub const RANGE: &str = "range";
    pub const COUNTRY: &str = "country";
    pub const YEAR: &str = "year";
    pub const POWER_CURVE: &str = "power_curve";
    pub const TORQUE_CURVE: &str = "torque_curve";
    pub const ABS: &str = "aid.abs";
    pub const TC: &str = "aid.tc";
    pub const EDL: &str = "aid.edl";
    pub const DRS: &str = "aid.drs";
    pub const KERS: &str = "aid.kers";
    pub const ERS: &str = "aid.ers";
    /// The MGU-H of an ERS: a detail of the ERS chip, not an aid of its own.
    pub const ERS_HEAT: &str = "aid.ers_heat";
    pub const FOUR_WS: &str = "aid.4ws";
    pub const ACTIVE_ARB: &str = "aid.active_arb";
    pub const EBB: &str = "aid.ebb";
    /// Marks a version as read, whatever the files said: what makes the
    /// backfill idempotent for a car whose files yield nothing (FICHE§9.3).
    pub const RECORDED: &str = "_recorded";
}

/// The key figures, their fixed unit when the user types them (FICHE§8).
const KEY_FIGURES: [(&str, &str); 6] = [
    (field::POWER, "bhp"),
    (field::TORQUE, "Nm"),
    (field::WEIGHT, "kg"),
    (field::PWRATIO, "kg/hp"),
    (field::TOPSPEED, "km/h"),
    (field::ACCELERATION, "s"),
];

/// Plain numbers the user may type (their unit is the field's own).
const NUMBERS: [&str; 3] = [field::RPM_LIMIT, field::FUEL_TANK, field::RANGE];
const INTEGERS: [&str; 2] = [field::GEARS, field::YEAR];
/// Closed lists (FICHE§8); the codes are the rules' own.
const CHOICES: [&str; 6] = [
    field::ENGINE_CONFIG,
    field::ASPIRATION,
    field::ENGINE_POS,
    field::DRIVETRAIN,
    field::GEARBOX,
    field::COUNTRY,
];
const AIDS: [&str; 9] = [
    field::ABS,
    field::TC,
    field::EDL,
    field::DRS,
    field::KERS,
    field::ERS,
    field::FOUR_WS,
    field::ACTIVE_ARB,
    field::EBB,
];

/// The columns of `mods` that cache the effective value (FICHE§6.3).
const CACHED: [&str; 5] = [
    field::DRIVETRAIN,
    field::ASPIRATION,
    field::GEARBOX,
    field::ENGINE_CONFIG,
    field::ENGINE_POS,
];

/// Gearboxes the physics cannot tell apart from one another (FICHE§2.2): only
/// the tags know them, so the tags win over the physics for these.
const TAGGED_GEARBOXES: [&str; 4] = ["SEQUENTIAL", "DCT", "SEMIAUTO", "AUTO"];

// --- The files ----------------------------------------------------------------

/// A name or a tag that says twin turbo (FICHE§10): two `[TURBO_n]` sections
/// are not enough on their own, an author may model one turbo in two stages.
fn says_twin_turbo(text: &str) -> bool {
    let t = text.to_lowercase();
    ["twin turbo", "twin-turbo", "twinturbo", "biturbo", "bi-turbo"]
        .iter()
        .any(|k| t.contains(k))
}

/// What the physics says, as sheet facts.
fn physics_facts(p: &physics::Physics, twin_hint: bool) -> Vec<Fact> {
    use Source::Physics as P;
    let mut out = Vec::new();
    // The electric front axle makes a four-wheel drive of a rear-drive hybrid
    // (FICHE§2.2): the two files are read together, or the 919 is "RWD".
    let drive = if p.front_motors {
        Some("AWD".to_string())
    } else {
        p.traction.clone()
    };
    if let Some(d) = drive {
        out.push(fact(field::DRIVETRAIN, P, json!(d)));
    }
    if let Some(g) = p.gears {
        out.push(fact(field::GEARS, P, json!(g)));
    }
    // No H-pattern says "not a manual", and not which of the others: "paddles"
    // is exact in both cases it cannot tell apart (FICHE§4).
    if let Some(h) = p.h_shifter {
        out.push(fact(field::GEARBOX, P, json!(if h { "MANUAL" } else { "PADDLES" })));
    }
    if let Some(n) = p.turbos {
        let v = match n {
            0 => "NA",
            n if n >= 2 && twin_hint => "TWIN_TURBO",
            _ => "TURBO",
        };
        out.push(fact(field::ASPIRATION, P, json!(v)));
    }
    if let Some(v) = p.limiter {
        out.push(fact(field::RPM_LIMIT, P, json!(v)));
    }
    if let Some(v) = p.max_fuel {
        out.push(fact(field::FUEL_TANK, P, json!(v)));
    }
    for (f, v) in [
        (field::ABS, p.abs),
        (field::TC, p.traction_control),
        (field::EDL, p.edl),
    ] {
        if let Some(v) = v {
            out.push(fact(f, P, json!(v)));
        }
    }
    // Present or unknown: "no DRS" on a hatchback is noise (FICHE§5), so an
    // absence is never stored as a no.
    for (f, v) in [
        (field::DRS, p.drs),
        (field::KERS, p.kers),
        (field::ERS, p.ers),
        (field::ERS_HEAT, p.ers && p.ers_heat),
        (field::FOUR_WS, p.four_ws),
        (field::ACTIVE_ARB, p.active_arb),
        (field::EBB, p.ebb),
    ] {
        if v {
            out.push(fact(f, P, json!(true)));
        }
    }
    out
}

/// What `ui_car.json` says, as sheet facts.
fn ui_facts(specs: &crate::uijson::NativeSpecs) -> Vec<Fact> {
    use ui::Kind;
    let mut out = Vec::new();
    let mut add = |f: &'static str, spec: Option<ui::Spec>| {
        if let Some(s) = spec {
            out.push(fact(f, Source::Ui, s.to_json()));
        }
    };
    add(field::POWER, ui::parse(Kind::Power, specs.bhp.as_deref()));
    add(field::TORQUE, ui::parse(Kind::Torque, specs.torque.as_deref()));
    add(field::WEIGHT, ui::parse(Kind::Weight, specs.weight.as_deref()));
    add(field::PWRATIO, ui::parse(Kind::PwRatio, specs.pwratio.as_deref()));
    add(field::TOPSPEED, ui::parse(Kind::TopSpeed, specs.topspeed.as_deref()));
    add(
        field::ACCELERATION,
        ui::parse_acceleration(specs.acceleration.as_deref()),
    );
    // A range is a number of km: its only meaning is the number, so it is
    // stored as one (FICHE§10: consistent with km at race pace).
    if let Some(ui::Spec::Quantity { n, .. }) = ui::parse(Kind::Range, specs.range.as_deref()) {
        out.push(fact(field::RANGE, Source::Ui, json!(n)));
    }
    if let Some(y) = specs.year.filter(|y| *y > 0) {
        out.push(fact(field::YEAR, Source::Ui, json!(y)));
    }
    // The curves travel with the sheet (FICHE§9.1): a mod whose files are gone
    // keeps its curve.
    if specs.power_curve.len() > 1 {
        out.push(fact(field::POWER_CURVE, Source::Ui, json!(specs.power_curve)));
    }
    if specs.torque_curve.len() > 1 {
        out.push(fact(field::TORQUE_CURVE, Source::Ui, json!(specs.torque_curve)));
    }
    out
}

/// Everything the files of the car in `dir` say. No base involved: the
/// backfill reads without holding the lock.
pub fn read_files(dir: &Path, car_id: &str, stock: bool) -> Vec<Fact> {
    let info = crate::uijson::read_car(dir).unwrap_or_default();
    let twin_hint = info.tags.iter().any(|t| says_twin_turbo(t)) || info.name.as_deref().is_some_and(says_twin_turbo);
    let mut facts = Vec::new();
    if let Some(p) = physics::read(dir, car_id) {
        facts.extend(physics_facts(&p, twin_hint));
    }
    if let Some(specs) = crate::uijson::read_car_specs(dir) {
        facts.extend(ui_facts(&specs));
    }
    // The game's own content has a year even when its file does not say.
    if stock {
        if let Some(y) = crate::kunos_dates::car_year(car_id) {
            facts.push(fact(field::YEAR, Source::Table, json!(y)));
        }
    }
    facts.push(fact(field::RECORDED, Source::Physics, json!(true)));
    facts
}

/// The facts key a mod's files are stored under: the active version, or `''`
/// for content indexed from the game folder, whose synthetic versions are
/// recreated at every reindex (FICHE§6.1).
pub fn version_key(stock: bool, active_version: Option<&str>) -> String {
    if stock {
        String::new()
    } else {
        active_version.unwrap_or_default().to_string()
    }
}

/// Writes what the files of one version say, then refreshes the cache.
pub fn record(conn: &Connection, mod_id: &str, version: &str, dir: &Path, stock: bool) -> rusqlite::Result<()> {
    let facts = read_files(dir, mod_id, stock);
    store_files(conn, mod_id, version, &facts)
}

/// The writing half of [`record`], for a caller that read without the lock.
pub fn store_files(conn: &Connection, mod_id: &str, version: &str, facts: &[Fact]) -> rusqlite::Result<()> {
    store::replace_file_facts(conn, mod_id, version, facts)?;
    refresh_cache(conn, mod_id)
}

// --- The rules ------------------------------------------------------------------

/// Writes what the harmonisation settled for a car: the spec fields its rules
/// deduced, and the country, the file's first (R2), both folded onto the
/// game's spelling as `harmonize::final_country` does.
pub fn store_harmonized(
    conn: &Connection,
    mod_id: &str,
    h: &crate::rules::Harmonized,
    native_country: Option<&str>,
    rules: &crate::rules::Rules,
) -> rusqlite::Result<()> {
    let mut facts = Vec::new();
    for (f, v) in [
        (field::DRIVETRAIN, &h.drivetrain),
        (field::ASPIRATION, &h.aspiration),
        (field::ENGINE_CONFIG, &h.engine_config),
        (field::ENGINE_POS, &h.engine_pos),
        (field::GEARBOX, &h.gearbox),
    ] {
        if let Some(v) = v.as_deref().filter(|v| !v.is_empty()) {
            facts.push(fact(f, Source::Rules, json!(v)));
        }
    }
    let canonical = |c: &str| crate::rules::canonical_country(c, &rules.country_aliases);
    if let Some(c) = native_country.filter(|c| !c.trim().is_empty()).and_then(canonical) {
        facts.push(fact(field::COUNTRY, Source::Ui, json!(c)));
    }
    if let Some(c) = h.country.as_deref().and_then(canonical) {
        facts.push(fact(field::COUNTRY, Source::Rules, json!(c)));
    }
    store::replace_harmonized_facts(conn, mod_id, &facts)?;
    refresh_cache(conn, mod_id)
}

// --- The effective value ------------------------------------------------------------

/// A value and where it came from.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Resolved {
    pub value: Value,
    pub source: Source,
}

/// The sheet as shown: every field that has a value, and which fields the
/// user decided (a value, or a forced "unknown", R6).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct TechSheet {
    pub values: BTreeMap<String, Resolved>,
    pub edited: Vec<String>,
}

/// The order of R2, user aside. Fields a source does not know simply have no
/// fact from it.
const ORDER: [Source; 4] = [Source::Physics, Source::Ui, Source::Table, Source::Rules];

type Facts = BTreeMap<(String, Source), Value>;

fn first_of(facts: &Facts, f: &str) -> Option<Resolved> {
    ORDER.iter().find_map(|s| {
        facts.get(&(f.to_string(), *s)).map(|v| Resolved {
            value: v.clone(),
            source: *s,
        })
    })
}

/// R2 for one field, with the two nuances the measurement asked for.
fn resolve_field(facts: &Facts, f: &str) -> Option<Resolved> {
    let rules = facts.get(&(f.to_string(), Source::Rules));
    let physics = facts.get(&(f.to_string(), Source::Physics));
    let from_rules = |v: &Value| Resolved {
        value: v.clone(),
        source: Source::Rules,
    };
    match f {
        // Sequential, dual-clutch, automatic: only the tags tell them apart
        // (FICHE§4), the physics saying at most "no H-pattern".
        field::GEARBOX => {
            if let Some(v) = rules.filter(|v| v.as_str().is_some_and(|s| TAGGED_GEARBOXES.contains(&s))) {
                return Some(from_rules(v));
            }
        }
        // A supercharger is no `[TURBO_n]`: the 16 supercharged cars of the
        // reference install have none (FICHE§10). "No turbo" then leaves the
        // word to the tags.
        field::ASPIRATION => {
            let supercharged = rules.is_some_and(|v| v == "SUPERCHARGED");
            if supercharged && physics.is_none_or(|v| v == "NA") {
                return rules.map(from_rules);
            }
        }
        _ => {}
    }
    first_of(facts, f)
}

/// Every field the sheet knows. `_recorded` and the like stay out.
fn is_shown(f: &str) -> bool {
    !f.starts_with('_')
}

/// Applies R2 to the facts and the user's decisions. Pure, hence testable.
fn resolve(facts: &Facts, user: &BTreeMap<String, Option<Value>>) -> TechSheet {
    let mut sheet = TechSheet::default();
    let fields: std::collections::BTreeSet<&str> = facts
        .keys()
        .map(|(f, _)| f.as_str())
        .chain(user.keys().map(String::as_str))
        .filter(|f| is_shown(f))
        .collect();
    for f in fields {
        let resolved = match user.get(f) {
            // The user's word, "unknown" included (R6).
            Some(Some(v)) => Some(Resolved {
                value: v.clone(),
                source: Source::User,
            }),
            Some(None) => None,
            None => resolve_field(facts, f),
        };
        if let Some(r) = resolved {
            sheet.values.insert(f.to_string(), r);
        }
    }
    sheet.edited = user.keys().cloned().collect();
    if !sheet.values.contains_key(field::PWRATIO) && !user.contains_key(field::PWRATIO) {
        if let Some(r) = computed_ratio(&sheet) {
            sheet.values.insert(field::PWRATIO.to_string(), r);
        }
    }
    sheet
}

/// Weight over power, when both are read with units that allow it (FICHE§4):
/// `bhp` or `hp` and `kg` — `whp` or `ps` would make it another ratio.
fn computed_ratio(sheet: &TechSheet) -> Option<Resolved> {
    let num = |f: &str, units: &[&str]| -> Option<f64> {
        let v = &sheet.values.get(f)?.value;
        let unit = v.get("unit")?.as_str()?;
        units.contains(&unit).then(|| v.get("n")?.as_f64()).flatten()
    };
    let power = num(field::POWER, &["bhp", "hp"])?;
    let weight = num(field::WEIGHT, &["kg"])?;
    (power > 0.0).then(|| Resolved {
        value: json!({ "n": (weight / power * 100.0).round() / 100.0, "unit": "kg/hp" }),
        source: Source::Computed,
    })
}

/// The sheet of a car, as shown (FICHE§6.3). The only reading of the tables.
pub fn effective(conn: &Connection, mod_id: &str) -> rusqlite::Result<TechSheet> {
    let key = store::version_key_of(conn, mod_id)?;
    let facts = store::facts(conn, mod_id, &key)?;
    let user = store::user(conn, mod_id)?;
    Ok(resolve(&facts, &user))
}

/// Rewrites the five spec columns of `mods` from [`effective`], with where
/// each came from — the ≈ of the library columns (R5).
///
/// A car with neither facts nor decisions is left alone: that is a base
/// written before this module, whose columns the backfill will replace.
pub fn refresh_cache(conn: &Connection, mod_id: &str) -> rusqlite::Result<()> {
    if !store::has_anything(conn, mod_id)? {
        return Ok(());
    }
    let sheet = effective(conn, mod_id)?;
    let mut values = Vec::new();
    let mut marks = serde_json::Map::new();
    for f in CACHED {
        let r = sheet.values.get(f);
        values.push(r.and_then(|r| r.value.as_str()).map(str::to_string));
        if let Some(r) = r.filter(|r| matches!(r.source, Source::Rules | Source::User)) {
            marks.insert(f.to_string(), json!(r.source.as_str()));
        }
    }
    store::write_cache(conn, mod_id, &values, &Value::Object(marks).to_string())
}

// --- The user ---------------------------------------------------------------------

/// One decision of the edit mode (FICHE§8).
#[derive(Debug, Clone, Deserialize)]
pub struct Edit {
    pub field: String,
    /// The value, or `null` for "unknown". Ignored when `revert`.
    #[serde(default)]
    pub value: Value,
    /// "↺ revenir": forget the decision, back to the mod's value.
    #[serde(default)]
    pub revert: bool,
}

/// Checks a typed value and puts it in its stored form. `Err` names the field.
fn normalize(f: &str, v: Value) -> Result<Value, String> {
    if v.is_null() {
        return Ok(Value::Null);
    }
    let bad = || format!("techsheet: invalid value for {f}: {v}");
    let positive = |v: &Value| v.as_f64().filter(|n| n.is_finite() && *n > 0.0);
    if let Some((_, unit)) = KEY_FIGURES.iter().find(|(k, _)| *k == f) {
        let n = positive(&v).ok_or_else(bad)?;
        return Ok(json!({ "n": n, "unit": unit }));
    }
    if NUMBERS.contains(&f) {
        return positive(&v).map(|n| json!(n)).ok_or_else(bad);
    }
    if INTEGERS.contains(&f) {
        return v
            .as_i64()
            .or_else(|| positive(&v).filter(|n| n.fract() == 0.0).map(|n| n as i64))
            .filter(|n| *n > 0)
            .map(|n| json!(n))
            .ok_or_else(bad);
    }
    if CHOICES.contains(&f) {
        return v
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty() && s.len() <= 64)
            .map(|s| json!(s))
            .ok_or_else(bad);
    }
    if AIDS.contains(&f) {
        return v.as_bool().map(|b| json!(b)).ok_or_else(bad);
    }
    Err(format!("techsheet: {f} is not editable"))
}

/// Applies the decisions of one "Enregistrer" (FICHE§8), refreshes the cache,
/// and notes it in the mod's history — which fields, never the values: the
/// history tells, it does not keep.
///
/// Every value is checked before anything is written, so a bad one leaves the
/// sheet as it was rather than half saved.
pub fn save_user(conn: &Connection, mod_id: &str, edits: Vec<Edit>) -> Result<(), String> {
    let mut checked = Vec::new();
    for e in edits {
        let value = if e.revert {
            None
        } else {
            Some(normalize(&e.field, e.value)?)
        };
        checked.push((e.field, value));
    }
    if checked.is_empty() {
        return Ok(());
    }
    let mut fields: Vec<&str> = Vec::new();
    for (f, value) in &checked {
        match value {
            None => store::clear_user(conn, mod_id, f),
            Some(v) => store::set_user(conn, mod_id, f, v),
        }
        .map_err(|e| e.to_string())?;
        if !fields.contains(&f.as_str()) {
            fields.push(f);
        }
    }
    refresh_cache(conn, mod_id).map_err(|e| e.to_string())?;
    let details = json!({ "key": "techEdited", "fields": fields }).to_string();
    crate::overlay::add_history(
        conn,
        mod_id,
        &chrono::Local::now().to_rfc3339(),
        "TECH_EDITED",
        &details,
    )
    .map_err(|e| e.to_string())
}

/// ABS and traction control of a car, as the session screen asks for them
/// (SESSION§3) — through the sheet, so a correction made there holds there too.
pub fn factory_assists(conn: &Connection, mod_id: &str) -> rusqlite::Result<(Option<bool>, Option<bool>)> {
    let sheet = effective(conn, mod_id)?;
    let aid = |f: &str| sheet.values.get(f).and_then(|r| r.value.as_bool());
    Ok((aid(field::ABS), aid(field::TC)))
}

#[cfg(test)]
mod tests;
