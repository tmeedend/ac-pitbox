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

/// Trimmed, inner whitespace collapsed, invisible characters dropped
/// (`uijson::strip_invisible`): a brand also comes from a rule or from the
/// base, not only from a file just read.
fn clean(s: &str) -> String {
    crate::uijson::strip_invisible(s)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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

/// Files a car under the brand the user typed in its header (`brand_user`).
///
/// The text goes through `canonical` like any brand does: `porsche` typed by
/// hand joins the `Porsche` the library already has instead of opening a second
/// one (TAXO§7.1). A brand equal to the computed one is no decision, so it
/// leaves the car unmarked rather than pinning today's result against the
/// rules' next one; blank goes back to the computed brand too.
///
/// Cars only: a track has no brand, and a write on an unknown id fails rather
/// than loses what was typed (the caller keeps the text).
pub fn set_user_brand(
    conn: &rusqlite::Connection,
    id: &str,
    typed: Option<&str>,
    canonical: impl Fn(&str) -> Option<String>,
) -> Result<(), String> {
    let row = crate::overlay::get_mod(conn, id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::MOD_NOT_FOUND)?;
    if row.kind != "Car" {
        return Err(format!("brand: {id} is not a car"));
    }
    let filed = typed.and_then(canonical);
    let decision = filed
        .as_deref()
        .filter(|b| row.brand_auto.as_deref().is_none_or(|auto| fold(auto) != fold(b)));
    crate::overlay::set_brand_user(conn, id, decision).map_err(|e| e.to_string())?;
    Ok(())
}

/// Whether a car whose file says `raw` names, itself, the brand it is filed
/// under (TAXO§4): the same brand once folded or merged, or the brand's words
/// inside what it says ("Mercedes Benz AMG" filed under Mercedes-Benz).
pub fn declares(raw: &str, filed: &str, aliases: &BTreeMap<String, String>) -> bool {
    let known = KNOWN.read().map(|k| k.clone()).unwrap_or_default();
    declares_in(raw, filed, aliases, &known)
}

/// [`declares`], the elected spellings given. Not "VRC" filed under Peugeot by
/// a rule: the VRC team may not write the real brand, so its file says "VRC"
/// and its badge is the team's - which then won Peugeot's logo, being
/// transparent and sharp.
pub fn declares_in(
    raw: &str,
    filed: &str,
    aliases: &BTreeMap<String, String>,
    known: &BTreeMap<String, String>,
) -> bool {
    if canonical_in(raw, aliases, known).is_some_and(|c| fold(&c) == fold(filed)) {
        return true;
    }
    let (raw, filed) = (words(raw), words(filed));
    !filed.is_empty() && raw.windows(filed.len()).any(|w| w == filed.as_slice())
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

    /// TAXO§7.1: an invisible character is no difference either. Real case:
    /// the 14 cars of the "No Hesi Traffic" pack start their brand (and name)
    /// with U+1D17A, a musical formatting character nothing draws - "BMW"
    /// and "BMW" then showed as two brands in the Brands tab, each with its
    /// own logo.
    #[test]
    fn an_invisible_character_does_not_make_a_second_brand() {
        let known = elect(["BMW", "BMW", "\u{1d17a}BMW", "\u{200b}Ford\u{feff}", "Ford"]);
        assert_eq!(known.len(), 2, "one brand per folded key: {known:?}");
        assert_eq!(known["bmw"], "BMW");
        assert_eq!(known["ford"], "Ford", "zero-width space and BOM are invisible too");
        let none = BTreeMap::new();
        assert_eq!(
            canonical_in("\u{1d17a}BMW", &none, &known).as_deref(),
            Some("BMW"),
            "filed under the brand already there"
        );
        assert_eq!(
            canonical_in("\u{1d17a}Unknown", &none, &known).as_deref(),
            Some("Unknown"),
            "a brand of its own is stored without the invisible character"
        );
        assert_eq!(
            canonical_in("\u{1d17a}", &none, &known),
            None,
            "nothing visible, no brand"
        );
    }

    /// TAXO§4: a car declares the brand it is filed under when its file says
    /// it - in another case, through a merge, or within a longer brand - and
    /// not when a rule moved it there from another name (the VRC case).
    #[test]
    fn a_car_declares_its_brand_only_when_its_file_says_it() {
        let known = elect(["Volkswagen", "Peugeot", "Mercedes-Benz"]);
        let aliases = BTreeMap::from([("vw".to_string(), "Volkswagen".to_string())]);
        let says = |raw: &str, filed: &str| declares_in(raw, filed, &aliases, &known);
        assert!(says("Volkswagen", "Volkswagen"));
        assert!(says("VOLKSWAGEN ", "Volkswagen"), "case and spaces");
        assert!(says("VW", "Volkswagen"), "a merge of the user's");
        assert!(
            says("Mercedes Benz AMG", "Mercedes-Benz"),
            "the brand within a longer name"
        );
        assert!(
            !says("VRC", "Peugeot"),
            "moved by a rule: the team's badge, not Peugeot's"
        );
        assert!(!says("Pageau", "Peugeot"), "a near spelling is no declaration");
        assert!(!says("Volkswagenwerk", "Volkswagen"), "whole words only");
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

    fn car_in_base(conn: &rusqlite::Connection, id: &str, kind: &str, brand: &str) {
        crate::overlay::upsert_mod(conn, id, kind, Some(brand), Some(id), "h", None, "2026-10-20").unwrap();
    }

    /// A brand typed in the header is stored beside the computed one: it wins
    /// in every listing, survives the next harmonisation and the next reindex,
    /// and "back to original" lands on what the rules compute now.
    #[test]
    fn a_brand_typed_by_hand_wins_survives_and_goes_back() {
        let base = crate::testutil::temp_dir("brand-user");
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        car_in_base(&conn, "car", "Car", "Ferrari");
        let known = elect(["Ferrari", "Porsche", "Porsche"]);
        let none = BTreeMap::new();
        let canon = |b: &str| canonical_in(b, &none, &known);
        let brand = |c: &rusqlite::Connection| crate::overlay::get_mod(c, "car").unwrap().unwrap();

        set_user_brand(&conn, "car", Some("  porsche "), canon).unwrap();
        let row = brand(&conn);
        assert_eq!(
            row.brand.as_deref(),
            Some("Porsche"),
            "the decision is what the list shows"
        );
        assert_eq!(
            row.brand_user.as_deref(),
            Some("Porsche"),
            "typed lower case, filed as the library spells it"
        );
        assert_eq!(
            row.brand_auto.as_deref(),
            Some("Ferrari"),
            "the computed brand stays known"
        );

        crate::overlay::update_harmonization(&conn, "car", Some("Lamborghini"), None, None, &[], None, &[]).unwrap();
        car_in_base(&conn, "car", "Car", "Maserati");
        let row = brand(&conn);
        assert_eq!(
            row.brand.as_deref(),
            Some("Porsche"),
            "harmonisation and reindex leave the decision alone"
        );
        assert_eq!(
            row.brand_auto.as_deref(),
            Some("Maserati"),
            "they do move the computed one"
        );

        set_user_brand(&conn, "car", Some("MASERATI"), canon).unwrap();
        assert_eq!(
            brand(&conn).brand_user,
            None,
            "the computed brand typed again is no decision"
        );
        set_user_brand(&conn, "car", Some("Porsche"), canon).unwrap();
        set_user_brand(&conn, "car", None, canon).unwrap();
        let row = brand(&conn);
        assert_eq!(row.brand.as_deref(), Some("Maserati"), "back to original");
        assert_eq!(row.brand_user, None);
    }

    /// The suggestions are the brands cars are filed under now: the user's
    /// choices count, tracks and blanks do not, each brand appears once.
    #[test]
    fn the_suggested_brands_are_the_ones_cars_are_filed_under() {
        let base = crate::testutil::temp_dir("brand-suggestions");
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        car_in_base(&conn, "a", "Car", "Ferrari");
        car_in_base(&conn, "b", "Car", "Ferrari");
        car_in_base(&conn, "c", "Car", "Mazda");
        car_in_base(&conn, "d", "Car", "  ");
        car_in_base(&conn, "t", "Track", "Spa Ltd");
        set_user_brand(&conn, "c", Some("Abarth"), |b| Some(b.to_string())).unwrap();
        assert_eq!(
            crate::overlay::list_car_brands(&conn).unwrap(),
            ["Abarth", "Ferrari"],
            "once each, sorted, the user's choice replacing the computed brand"
        );
    }

    /// A track has no brand, and an unknown id fails instead of losing the
    /// typed text silently.
    #[test]
    fn a_brand_is_refused_for_a_track_and_for_an_unknown_mod() {
        let base = crate::testutil::temp_dir("brand-user-refused");
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        car_in_base(&conn, "spa", "Track", "");
        let canon = |b: &str| Some(b.to_string());
        assert!(
            set_user_brand(&conn, "spa", Some("Ferrari"), canon).is_err(),
            "tracks have no brand"
        );
        assert!(
            set_user_brand(&conn, "ghost", Some("Ferrari"), canon).is_err(),
            "unknown id"
        );
    }
}
