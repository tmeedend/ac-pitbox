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

use std::collections::{BTreeMap, BTreeSet};
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

/// Whether a brand is no brand - a pack, a series, a modder the user marked
/// as such (`not_brands`, TAXO§7).
pub fn is_not_brand(brand: &str, not_brands: &BTreeSet<String>) -> bool {
    let key = fold(brand);
    not_brands.iter().any(|n| fold(n) == key)
}

/// The brand a car's NAME gives, for a car filed under something that is no
/// brand (`AER`: "Ferrari 488 GTE"; `WSC Legends`: "WSC60 Ford GT40 MkII").
/// The words sought are the brands this library knows, every brand a merge
/// leads to, and every spelling merged into one (`chevy` → Chevrolet catches
/// "Chevy Camaro") - never a brand marked as none.
pub fn from_name(name: &str, aliases: &BTreeMap<String, String>, not_brands: &BTreeSet<String>) -> Option<String> {
    let known = KNOWN.read().map(|k| k.clone()).unwrap_or_default();
    from_name_in(name, aliases, not_brands, &known)
}

/// Words, folded: what a name is matched on. Whole words only - "ford" must
/// not be found in "Oxford", nor "seat" in "2-seater".
fn words(s: &str) -> Vec<String> {
    fold(s)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

pub fn from_name_in(
    name: &str,
    aliases: &BTreeMap<String, String>,
    not_brands: &BTreeSet<String>,
    known: &BTreeMap<String, String>,
) -> Option<String> {
    let name = words(name);
    let vocabulary = known
        .values()
        .map(|b| (b.clone(), b.clone()))
        .chain(aliases.iter().map(|(spelling, b)| (spelling.clone(), b.clone())))
        .chain(aliases.values().map(|b| (b.clone(), b.clone())))
        .filter(|(_, b)| !is_not_brand(b, not_brands));
    // The EARLIEST brand in the name wins, then the longest: "Jordan-Ford
    // 191" is a Jordan with a Ford engine, "Alfa Romeo" beats "Alfa".
    let mut best: Option<(usize, usize, String)> = None;
    for (spelling, brand) in vocabulary {
        let w = words(&spelling);
        if w.is_empty() || w.len() > name.len() {
            continue;
        }
        if let Some(at) = (0..=name.len() - w.len()).find(|&i| name[i..i + w.len()] == w[..]) {
            let better = best
                .as_ref()
                .is_none_or(|(a, l, _)| at < *a || (at == *a && w.len() > *l));
            if better {
                best = Some((at, w.len(), brand));
            }
        }
    }
    best.map(|(_, _, b)| b)
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

    /// A pack is no brand: the car's name says its brand - whole words only,
    /// the earliest in the name, the longest on a tie; a merged spelling
    /// counts; nothing found, the pack stays.
    #[test]
    fn a_pack_gives_way_to_the_brand_in_the_name() {
        let known = elect(["Ferrari", "Ford", "Jordan", "Alfa Romeo", "Alfa", "AER", "Chevrolet"]);
        let aliases = BTreeMap::from([
            ("chevy".to_string(), "Chevrolet".to_string()),
            ("oreca technology".to_string(), "Oreca".to_string()),
        ]);
        let not: BTreeSet<String> = ["aer".to_string()].into();
        let find = |n: &str| from_name_in(n, &aliases, &not, &known);
        assert_eq!(find("Ferrari 488 GTE").as_deref(), Some("Ferrari"));
        assert_eq!(find("Jordan-Ford 191").as_deref(), Some("Jordan"), "the earliest");
        assert_eq!(find("Alfa Romeo 155 TI").as_deref(), Some("Alfa Romeo"), "the longest");
        assert_eq!(
            find("Chevy_Camaro_1968").as_deref(),
            Some("Chevrolet"),
            "a merged spelling"
        );
        assert_eq!(
            find("Oreca 07").as_deref(),
            Some("Oreca"),
            "a merge's target is a brand"
        );
        assert_eq!(find("Oxford Special"), None, "a whole word, not a part of one");
        assert_eq!(find("AER Benetton B191"), None, "never the pack itself");
        assert!(is_not_brand("  aer ", &not));
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
