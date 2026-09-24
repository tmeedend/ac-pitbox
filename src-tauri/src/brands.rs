//! The brand as a TERM (TAXO§2): the name a car is filed under, whatever
//! spelling its `ui_car.json` uses.
//!
//! Two steps, decided at write time like the country (`harmonize::store`):
//!
//! 1. **The user's merges** - `brand_aliases`, spelling (lowercased) → brand.
//!    Only he decides them, from the proposals of the Brands tab: `Alfa` and
//!    `Alfa Romeo`, `Nissan` and `Nismo` may be distinctions he wants to keep
//!    (TAXO§7.2), so nothing merges them silently.
//! 2. **Case, spaces and accents**, which are not a decision (TAXO§7.1):
//!    `PORSCHE`, `Porsche` and `porsche ` are one brand. They fold onto the
//!    spelling the library uses most - elected over the stored brands at
//!    startup - so the choice follows the collection rather than an arbitrary
//!    rule, and two launches give the same result. Once elected, a spelling
//!    stays: the stored brands are then all elected ones, and a car imported
//!    later with another case joins the brand already there.

use std::collections::BTreeMap;
use std::sync::RwLock;

/// Folded spelling → the spelling elected for it.
static KNOWN: RwLock<BTreeMap<String, String>> = RwLock::new(BTreeMap::new());

/// The key two spellings of one brand share: lowercase, no accents, single
/// spaces (TAXO§7.1). The same folding as the countries'.
pub fn fold(s: &str) -> String {
    crate::rules::fold(&clean(s))
}

/// Trimmed, inner whitespace collapsed.
fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Elects one spelling per folded key: the most used; on a tie, the one with
/// the most capitals short of all capitals (`Porsche` over `porsche` and over
/// `PORSCHE`, but `BMW` stays `BMW` when it is the only form), then the
/// smallest - deterministic whatever the order of the library.
pub fn elect<'a>(spellings: impl IntoIterator<Item = &'a str>) -> BTreeMap<String, String> {
    let mut counts: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for s in spellings {
        let c = clean(s);
        if c.is_empty() {
            continue;
        }
        *counts.entry(fold(&c)).or_default().entry(c).or_default() += 1;
    }
    let casing = |s: &str| {
        let letters = s.chars().filter(|c| c.is_alphabetic()).count();
        let upper = s.chars().filter(|c| c.is_uppercase()).count();
        if letters > 0 && upper == letters {
            0 // ALL CAPS: the least likely intended form when others exist
        } else {
            upper + 1
        }
    };
    counts
        .into_iter()
        .map(|(key, forms)| {
            let best = forms
                .into_iter()
                .max_by(|(a, na), (b, nb)| {
                    na.cmp(nb)
                        .then_with(|| casing(a).cmp(&casing(b)))
                        .then_with(|| b.cmp(a))
                })
                .map(|(s, _)| s)
                .expect("a key has at least one form");
            (key, best)
        })
        .collect()
}

/// Replaces the elected spellings (`refresh_from`, at startup).
pub fn set_known(map: BTreeMap<String, String>) {
    match KNOWN.write() {
        Ok(mut k) => *k = map,
        Err(e) => log::warn!("brand spellings not updated: {e}"),
    }
}

/// Elects from the brands the library stores (cars only).
pub fn refresh_from(conn: &rusqlite::Connection) {
    match crate::overlay::list_mods(conn) {
        Ok(mods) => set_known(elect(
            mods.iter()
                .filter(|m| m.kind == "Car")
                .filter_map(|m| m.brand.as_deref()),
        )),
        Err(e) => log::warn!("brand spellings not elected: {e}"),
    }
}

/// The brand a car is filed under, from what its file (or a rule) says.
pub fn canonical(raw: &str, aliases: &BTreeMap<String, String>) -> Option<String> {
    let known = KNOWN.read().map(|k| k.clone()).unwrap_or_default();
    canonical_in(raw, aliases, &known)
}

pub fn canonical_in(raw: &str, aliases: &BTreeMap<String, String>, known: &BTreeMap<String, String>) -> Option<String> {
    let cleaned = clean(raw);
    if cleaned.is_empty() {
        return None;
    }
    if let Some(to) = aliases.get(&cleaned.to_lowercase()) {
        return Some(to.clone());
    }
    let key = fold(&cleaned);
    // A merge written for one spelling holds for its case and accent variants.
    if let Some(to) = aliases.iter().find(|(k, _)| fold(k) == key).map(|(_, v)| v) {
        return Some(to.clone());
    }
    Some(known.get(&key).cloned().unwrap_or(cleaned))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TAXO§7.1: case, spaces and accents are one brand, filed under the
    /// spelling the library uses most - `Porsche` over `PORSCHE`.
    #[test]
    fn spellings_fold_onto_the_most_used_one() {
        let known = elect(["Porsche", "PORSCHE", "porsche ", "Porsche", "BMW", "Citroën", "citroen"]);
        assert_eq!(known["porsche"], "Porsche");
        assert_eq!(known["bmw"], "BMW", "a single all-caps form stays as it is");
        assert_eq!(known["citroen"], "Citroën", "a tie goes to the form with capitals");
        let none = BTreeMap::new();
        assert_eq!(canonical_in("  PORSCHE ", &none, &known).as_deref(), Some("Porsche"));
        assert_eq!(
            canonical_in("Nismo", &none, &known).as_deref(),
            Some("Nismo"),
            "unknown stays"
        );
        assert_eq!(canonical_in("  ", &none, &known), None);
    }

    /// TAXO§7.2: a merge is the user's decision, and it wins over the
    /// folding - also for the case variants of the spelling he merged.
    #[test]
    fn a_merge_wins_and_covers_its_case_variants() {
        let known = elect(["Alfa", "Alfa Romeo"]);
        let aliases = BTreeMap::from([("alfa".to_string(), "Alfa Romeo".to_string())]);
        assert_eq!(canonical_in("Alfa", &aliases, &known).as_deref(), Some("Alfa Romeo"));
        assert_eq!(canonical_in("ALFA", &aliases, &known).as_deref(), Some("Alfa Romeo"));
        assert_eq!(
            canonical_in("Alfa Romeo", &aliases, &known).as_deref(),
            Some("Alfa Romeo")
        );
    }
}
