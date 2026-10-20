//! Forcing a category (REGLES§3 level 4): the user's decision on one mod,
//! which wins over the rules and over every recomputation.
//!
//! Two shapes, one table (`overlay::category_user`):
//!
//! - **A car** has one category. The user forces a value; choosing the one the
//!   rules compute already stores nothing and forgets a forced one - the car is
//!   followed by the rules again.
//! - **A track** has an ordered list. The user's decision is deltas on the
//!   computed list (`added`, `removed`), because a list is not a value to pin:
//!   a rule that later finds another category must still reach the track.
//!
//! What the screens read (`mods.category`, `mods.categories`) is the computed
//! result with the decision laid on it, written by [`shown`] at the two moments
//! either side can change: the harmonisation (`harmonize::store`) and the
//! decision itself ([`apply`]). The computed result is kept beside it
//! (`category_auto`, `categories_auto`), for "back to computed" and for the
//! Rules screen's counters.

use std::collections::BTreeSet;

use rusqlite::Connection;
use serde::Deserialize;

use crate::modscan::ModKind;
use crate::overlay::{self, CategoryDecision};
use crate::rules::Rules;

/// One gesture on the category of one or several mods.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", content = "tag", rename_all = "snake_case")]
pub enum CategoryOp {
    /// A car: file it under this category.
    Force(String),
    /// A track: add this category to its list.
    Add(String),
    /// A track: take this category off its list.
    Remove(String),
    /// Forget the decision: back to what the rules compute.
    Reset,
}

/// `#slug`: lower case, accents dropped, `_` as the separator - the way the
/// creation of a track category writes a tag (`tagForName`, TAXO§6.4).
/// `None` for a text with nothing in it.
pub fn slug_tag(raw: &str) -> Option<String> {
    let folded = crate::rules::fold(raw);
    let mut slug = String::new();
    let mut gap = false;
    for c in folded.chars() {
        if c.is_ascii_alphanumeric() {
            if gap && !slug.is_empty() {
                slug.push('_');
            }
            slug.push(c);
            gap = false;
        } else {
            gap = true;
        }
    }
    (!slug.is_empty()).then(|| format!("#{slug}"))
}

/// How two spellings of a category compare: with a `#`, in lower case.
pub fn key(tag: &str) -> String {
    format!("#{}", tag.trim().trim_start_matches('#').to_lowercase())
}

/// The category a typed or picked text means. A tag the rules know, or one
/// already decided somewhere, keeps its own spelling (`#gt-3` is not
/// `#gt_3`); anything else is slugged like a new category.
fn canonical(raw: &str, known: &[String]) -> Option<String> {
    let k = key(raw);
    known
        .iter()
        .find(|t| key(t) == k)
        .map(|t| key(t))
        .or_else(|| slug_tag(raw))
}

/// A car's category: the forced one, else the computed one.
fn shown_car(auto: Option<&str>, d: Option<&CategoryDecision>) -> Option<String> {
    d.and_then(|d| d.forced.clone()).or_else(|| auto.map(str::to_string))
}

/// A track's categories: the computed list less what was taken off, plus what
/// was added, in the order of Familles › Circuits (`order`, the rules'
/// allowlist) - the first stays the main category. A category the list does
/// not know goes after the others, by name.
fn shown_track(auto: &[String], d: Option<&CategoryDecision>, order: &[String]) -> Vec<String> {
    let mut set: BTreeSet<String> = auto.iter().map(|c| key(c)).collect();
    if let Some(d) = d {
        for r in &d.removed {
            set.remove(&key(r));
        }
        set.extend(d.added.iter().map(|c| key(c)));
    }
    let mut out: Vec<String> = order.iter().map(|c| key(c)).filter(|c| set.remove(c)).collect();
    out.extend(set);
    out
}

/// What the screens show for a mod: the computed category with its decision
/// laid on. `(category, categories)`; a car has no `categories`.
///
/// **Nothing decided, nothing touched**: the computed result goes through as it
/// is, spelling and order included - a library with no decision is classified
/// exactly as before (REGLES§13.5).
pub fn shown(
    is_car: bool,
    auto_category: Option<&str>,
    auto_categories: &[String],
    d: Option<&CategoryDecision>,
    order: &[String],
) -> (Option<String>, Vec<String>) {
    if d.is_none() {
        let categories = if is_car { Vec::new() } else { auto_categories.to_vec() };
        return (auto_category.map(str::to_string), categories);
    }
    if is_car {
        (shown_car(auto_category, d), Vec::new())
    } else {
        let list = shown_track(auto_categories, d, order);
        (list.first().cloned(), list)
    }
}

/// Lays one gesture on a decision. `auto` is the computed list (a car's
/// category as a list of one), `known` what the pickers offered. Pure.
fn decide(
    is_car: bool,
    auto: &[String],
    mut d: CategoryDecision,
    op: &CategoryOp,
    known: &[String],
) -> Result<CategoryDecision, String> {
    let computed = |t: &str| auto.iter().any(|a| key(a) == key(t));
    let tag = |raw: &str| canonical(raw, known).ok_or_else(|| format!("categories: nothing to file under in {raw:?}"));
    match op {
        CategoryOp::Reset => d = CategoryDecision::new(&d.mod_id),
        CategoryOp::Force(raw) if is_car => {
            let t = tag(raw)?;
            // The category the rules compute is no decision: the car follows them.
            d.forced = (!computed(&t)).then_some(t);
        }
        CategoryOp::Add(raw) if !is_car => {
            let t = tag(raw)?;
            if computed(&t) {
                // Putting back a computed category one took off cancels it.
                d.removed.retain(|r| key(r) != key(&t));
            } else if !d.added.iter().any(|a| key(a) == key(&t)) {
                d.added.push(t);
            }
        }
        CategoryOp::Remove(raw) if !is_car => {
            let t = tag(raw)?;
            d.added.retain(|a| key(a) != key(&t));
            if computed(&t) && !d.removed.iter().any(|r| key(r) == key(&t)) {
                d.removed.push(t);
            }
        }
        _ => {
            return Err(format!(
                "categories: {op:?} does not apply to a {}",
                if is_car { "car" } else { "track" }
            ))
        }
    }
    Ok(d)
}

/// Writes what a mod shows from its computed category and its decision.
fn lay(conn: &Connection, rules: &Rules, id: &str) -> Result<(), String> {
    let Some(m) = overlay::get_mod(conn, id).map_err(|e| e.to_string())? else {
        return Ok(());
    };
    let d = overlay::get_category_decision(conn, id).map_err(|e| e.to_string())?;
    let (category, categories) = shown(
        ModKind::from_column(&m.kind) == ModKind::Car,
        m.category_auto.as_deref(),
        &m.categories_auto,
        d.as_ref(),
        &rules.track.category_allowlist,
    );
    overlay::set_category_shown(conn, id, category.as_deref(), &categories).map_err(|e| e.to_string())
}

/// Lays `op` on every mod of `ids`, all or nothing, and rewrites what each
/// shows. `rules` gives the order of a track's categories.
pub fn apply(conn: &Connection, rules: &Rules, ids: &[String], op: &CategoryOp) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    // What the pickers offered, read once per kind and not per mod: a group of
    // a few thousand tracks must not read the whole table of decisions each time.
    let mut offered: [Option<Vec<String>>; 2] = [None, None];
    for id in ids {
        // The shown category is the computed one until this decision: keep it so.
        overlay::ensure_category_auto(&tx, id).map_err(|e| e.to_string())?;
        let m = overlay::get_mod(&tx, id)
            .map_err(|e| e.to_string())?
            .ok_or(crate::errors::MOD_NOT_FOUND)?;
        let kind = ModKind::from_column(&m.kind);
        let is_car = kind == ModKind::Car;
        let auto: Vec<String> = if is_car {
            m.category_auto.iter().cloned().collect()
        } else {
            m.categories_auto.clone()
        };
        let slot = &mut offered[usize::from(!is_car)];
        if slot.is_none() {
            *slot = Some(vocabulary(&tx, rules, kind).map_err(|e| e.to_string())?);
        }
        let known = slot.as_deref().unwrap_or_default();
        let current = overlay::get_category_decision(&tx, id)
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| CategoryDecision::new(id));
        let d = decide(is_car, &auto, current, op, known)?;
        overlay::set_category_decision(&tx, &d).map_err(|e| e.to_string())?;
        lay(&tx, rules, id)?;
    }
    tx.commit().map_err(|e| e.to_string())
}

/// A decision read from an export, in the form the base keeps: `#tag` in lower
/// case, nothing empty. The spelling of a known tag (`#formula 1`) is kept -
/// it is already the exporter's stored form, not a typed text.
fn sanitized(d: &CategoryDecision) -> CategoryDecision {
    let tags = |list: &[String]| -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for t in list.iter().map(|t| key(t)).filter(|t| t.len() > 1) {
            if !out.contains(&t) {
                out.push(t);
            }
        }
        out
    };
    CategoryDecision {
        mod_id: d.mod_id.clone(),
        forced: d.forced.as_deref().map(key).filter(|t| t.len() > 1),
        added: tags(&d.added),
        removed: tags(&d.removed),
    }
}

/// Takes the decisions of an import: a mod the base already decided keeps its
/// own, the others are written (a mod this library lacks keeps its decision
/// for the day it is imported). `(added, kept)`.
///
/// Each decision is laid on its mod right here, not left to the re-application
/// that follows: that one skips a mod whose files it cannot read, and the
/// decision would then wait unseen for a later pass.
pub fn merge_decisions(
    conn: &Connection,
    rules: &Rules,
    incoming: &[CategoryDecision],
) -> Result<(usize, usize), String> {
    let (mut added, mut kept) = (0, 0);
    for d in incoming.iter().map(sanitized).filter(|d| !d.is_empty()) {
        if overlay::get_category_decision(conn, &d.mod_id)
            .map_err(|e| e.to_string())?
            .is_some()
        {
            kept += 1;
            continue;
        }
        overlay::ensure_category_auto(conn, &d.mod_id).map_err(|e| e.to_string())?;
        overlay::set_category_decision(conn, &d).map_err(|e| e.to_string())?;
        lay(conn, rules, &d.mod_id)?;
        added += 1;
    }
    Ok((added, kept))
}

/// The categories a picker offers for `kind`: what the rules can produce, then
/// what the user decided on other mods. A track's come in the order of
/// Familles › Circuits.
pub fn vocabulary(conn: &Connection, rules: &Rules, kind: ModKind) -> rusqlite::Result<Vec<String>> {
    let decided = overlay::decided_categories(conn)?;
    Ok(match kind {
        ModKind::Car => {
            let mut set: BTreeSet<String> = crate::rules::car_category_vocabulary(rules).into_iter().collect();
            set.extend(decided.forced);
            set.into_iter().collect()
        }
        ModKind::Track => {
            let mut out: Vec<String> = Vec::new();
            for c in rules
                .track
                .category_allowlist
                .iter()
                .map(|c| key(c))
                .chain(decided.added)
            {
                if !out.contains(&c) {
                    out.push(c);
                }
            }
            out
        }
    })
}

#[cfg(test)]
mod tests;
