//! Importing someone's decisions into one's own (REGLES§9): a MERGE that
//! never takes anything away.
//!
//! Replacing would need a question ("replace or merge?") whose answer the
//! user rarely knows before seeing the result, and would lose his decisions on
//! a wrong click. Merging without loss needs none: what the import brings is
//! added, and where both overlays decided the same entry differently, **his
//! decision stays** and is counted as such. The report says how many of each.
//!
//! Decisions that name a rule this catalogue does not have (retired, or from
//! a newer catalogue) are merged all the same and counted apart: a
//! switched-off unknown rule is inert, a fork of one stands as a rule of his
//! own - exactly what an update that retires a rule does (REGLES§4).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::rules::{content, ListOverlay, OrderOverlay, Rule, RulesOverlay};
use crate::taxonomy::{FamilyOverlay, MapOverlay, SetOverlay, TaxonomyOverlay};

/// What an import did, counted by decision.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MergeStats {
    /// Decisions taken from the import.
    pub added: usize,
    /// Decisions the import made differently, where his were kept.
    pub kept_yours: usize,
    /// Decisions he had already made the same way.
    pub already: usize,
    /// Decisions naming a rule this catalogue does not have.
    pub unknown: usize,
}

fn next_own_id(taken: &BTreeSet<String>) -> String {
    (1..)
        .map(|n| format!("own-{n}"))
        .find(|id| !taken.contains(id))
        .expect("a free id")
}

/// One list of rules. `catalog` is this user's catalogue: an id absent from it
/// is counted as unknown.
pub fn merge_list<T: Rule>(mine: &mut ListOverlay<T>, theirs: &ListOverlay<T>, catalog: &[T], st: &mut MergeStats) {
    let shipped = |id: &str| catalog.iter().any(|c| c.id() == Some(id));
    let their_own: BTreeSet<&str> = theirs.own.iter().filter_map(|r| r.id()).collect();

    // Their own rules are his own rules now, under ids of his. Identical
    // content is the same rule, not a second copy.
    let mut taken: BTreeSet<String> = mine
        .own
        .iter()
        .filter_map(|r| r.id().map(str::to_string))
        .chain(catalog.iter().filter_map(|c| c.id().map(str::to_string)))
        .collect();
    for r in &theirs.own {
        if mine.own.iter().any(|m| content(m) == content(r)) {
            st.already += 1;
            continue;
        }
        let id = next_own_id(&taken);
        taken.insert(id.clone());
        if r.id().is_some_and(|i| theirs.disabled.contains(i)) {
            mine.disabled.insert(id.clone());
        }
        mine.own.push(r.clone().with_id(Some(id)));
        st.added += 1;
    }

    for id in theirs.disabled.iter().filter(|id| !their_own.contains(id.as_str())) {
        if !shipped(id) {
            st.unknown += 1;
        }
        if mine.disabled.contains(id) {
            st.already += 1;
        } else if mine.forks.contains_key(id) {
            st.kept_yours += 1;
        } else {
            mine.disabled.insert(id.clone());
            st.added += 1;
        }
    }

    for (id, f) in &theirs.forks {
        if !shipped(id) {
            st.unknown += 1;
        }
        match mine.forks.get(id) {
            Some(m) if content(&m.rule) == content(&f.rule) => st.already += 1,
            Some(_) => st.kept_yours += 1,
            None if mine.disabled.contains(id) => st.kept_yours += 1,
            None => {
                mine.forks.insert(id.clone(), f.clone());
                st.added += 1;
            }
        }
    }
}

/// The allowlist: removals and additions are unions; the order is his if he
/// has one, theirs otherwise.
pub fn merge_order(mine: &mut OrderOverlay, theirs: &OrderOverlay, st: &mut MergeStats) {
    for r in &theirs.removed {
        if mine.removed.insert(r.clone()) {
            st.added += 1;
        } else {
            st.already += 1;
        }
    }
    for a in &theirs.added {
        if mine.added.contains(a) {
            st.already += 1;
        } else {
            mine.added.push(a.clone());
            st.added += 1;
        }
    }
    match (&mine.order, &theirs.order) {
        (None, Some(o)) => {
            mine.order = Some(o.clone());
            st.added += 1;
        }
        (Some(m), Some(o)) if m != o => st.kept_yours += 1,
        (Some(_), Some(_)) => st.already += 1,
        _ => {}
    }
}

pub fn merge_rules(mine: &mut RulesOverlay, theirs: &RulesOverlay, catalog: &RulesCatalog, st: &mut MergeStats) {
    merge_list(&mut mine.brand_fix, &theirs.brand_fix, catalog.brand_fix, st);
    merge_list(&mut mine.name_to_tag, &theirs.name_to_tag, catalog.name_to_tag, st);
    merge_list(&mut mine.class_fix, &theirs.class_fix, catalog.class_fix, st);
    merge_list(
        &mut mine.car_tag_merge,
        &theirs.car_tag_merge,
        catalog.car_tag_merge,
        st,
    );
    merge_list(&mut mine.drivetrain, &theirs.drivetrain, catalog.drivetrain, st);
    merge_list(&mut mine.aspiration, &theirs.aspiration, catalog.aspiration, st);
    merge_list(
        &mut mine.engine_config,
        &theirs.engine_config,
        catalog.engine_config,
        st,
    );
    merge_list(&mut mine.engine_pos, &theirs.engine_pos, catalog.engine_pos, st);
    merge_list(&mut mine.gearbox, &theirs.gearbox, catalog.gearbox, st);
    merge_list(
        &mut mine.track_tag_merge,
        &theirs.track_tag_merge,
        catalog.track_tag_merge,
        st,
    );
    merge_order(&mut mine.track_categories, &theirs.track_categories, st);
    // `catalog_off` is a preference of the machine it was set on, not a rule:
    // never imported.
}

/// The list rules of this user's catalogue, section by section - borrowed,
/// the application keeps them in its own `Rules`.
pub struct RulesCatalog<'a> {
    pub brand_fix: &'a [crate::rules::BrandFix],
    pub name_to_tag: &'a [crate::rules::NameToTag],
    pub class_fix: &'a [crate::rules::ClassFix],
    pub car_tag_merge: &'a [crate::rules::TagMerge],
    pub drivetrain: &'a [crate::rules::SetRule],
    pub aspiration: &'a [crate::rules::SetRule],
    pub engine_config: &'a [crate::rules::SetRule],
    pub engine_pos: &'a [crate::rules::SetRule],
    pub gearbox: &'a [crate::rules::SetRule],
    pub track_tag_merge: &'a [crate::rules::TagMerge],
}

/// A `key → value` table: his entry wins wherever both decided.
pub fn merge_map(mine: &mut MapOverlay, theirs: &MapOverlay, st: &mut MergeStats) {
    for (k, v) in &theirs.set {
        match (mine.set.get(k), mine.removed.contains(k)) {
            (Some(m), _) if m == v => st.already += 1,
            (Some(_), _) | (None, true) => st.kept_yours += 1,
            (None, false) => {
                mine.set.insert(k.clone(), v.clone());
                st.added += 1;
            }
        }
    }
    for k in &theirs.removed {
        if mine.removed.contains(k) {
            st.already += 1;
        } else if mine.set.contains_key(k) {
            st.kept_yours += 1;
        } else {
            mine.removed.insert(k.clone());
            st.added += 1;
        }
    }
}

/// A set: what one side added the other removed is a conflict, his stays.
fn merge_set(mine: &mut SetOverlay, theirs: &SetOverlay, st: &mut MergeStats) {
    for k in &theirs.added {
        if mine.added.contains(k) {
            st.already += 1;
        } else if mine.removed.contains(k) {
            st.kept_yours += 1;
        } else {
            mine.added.insert(k.clone());
            st.added += 1;
        }
    }
    for k in &theirs.removed {
        if mine.removed.contains(k) {
            st.already += 1;
        } else if mine.added.contains(k) {
            st.kept_yours += 1;
        } else {
            mine.removed.insert(k.clone());
            st.added += 1;
        }
    }
}

fn merge_families(mine: &mut FamilyOverlay, theirs: &FamilyOverlay, st: &mut MergeStats) {
    for (id, m) in &theirs.meta {
        match mine.meta.get(id) {
            Some(x) if x == m => st.already += 1,
            Some(_) => st.kept_yours += 1,
            None => {
                mine.meta.insert(id.clone(), m.clone());
                st.added += 1;
            }
        }
    }
    for f in &theirs.created {
        match mine.created.iter().find(|c| c.id == f.id) {
            Some(c) if c == f => st.already += 1,
            Some(_) => st.kept_yours += 1,
            None => {
                mine.created.push(f.clone());
                st.added += 1;
            }
        }
    }
    for id in &theirs.removed {
        if mine.removed.insert(id.clone()) {
            st.added += 1;
        } else {
            st.already += 1;
        }
    }
    let tags: BTreeMap<&String, &String> = theirs.tags.iter().collect();
    for (tag, family) in tags {
        match mine.tags.get(tag) {
            Some(f) if f == family => st.already += 1,
            Some(_) => st.kept_yours += 1,
            None => {
                mine.tags.insert(tag.clone(), family.clone());
                st.added += 1;
            }
        }
    }
}

pub fn merge_taxonomy(mine: &mut TaxonomyOverlay, theirs: &TaxonomyOverlay, st: &mut MergeStats) {
    merge_families(&mut mine.families, &theirs.families, st);
    merge_map(&mut mine.country_aliases, &theirs.country_aliases, st);
    merge_map(&mut mine.country_tags, &theirs.country_tags, st);
    merge_map(&mut mine.brand_aliases, &theirs.brand_aliases, st);
    merge_set(&mut mine.not_brands, &theirs.not_brands, st);
    for m in &theirs.ignored_brand_merges {
        if mine.ignored_brand_merges.contains(m) {
            st.already += 1;
        } else {
            mine.ignored_brand_merges.push(m.clone());
            st.added += 1;
        }
    }
    for c in &theirs.ignored_countries {
        if mine.ignored_countries.contains(c) {
            st.already += 1;
        } else {
            mine.ignored_countries.push(c.clone());
            st.added += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{BrandFix, Fork};

    fn fix(id: Option<&str>, name: &str, brand: &str) -> BrandFix {
        BrandFix {
            id: id.map(str::to_string),
            name_contains: name.into(),
            set_brand: brand.into(),
        }
    }

    fn catalog() -> Vec<BrandFix> {
        vec![
            fix(Some("pitbox.brand.bayro"), "bayro", "BMW"),
            fix(Some("pitbox.brand.bar"), "bar ", "Bar"),
        ]
    }

    /// REGLES§9: an import adds, never takes away - where both decided the
    /// same entry differently, his decision stays.
    #[test]
    fn an_import_adds_and_keeps_his_decisions() {
        let mut mine = ListOverlay::default();
        mine.own.push(fix(Some("own-1"), "lanzo", "RSS"));
        mine.forks.insert(
            "pitbox.brand.bayro".into(),
            Fork {
                rule: fix(Some("pitbox.brand.bayro"), "bayro", "BMW M"),
                forked_from: "x".into(),
            },
        );
        let mut theirs = ListOverlay {
            own: vec![fix(Some("own-1"), "lanzo", "RSS"), fix(Some("own-2"), "zeta", "Z")],
            ..Default::default()
        };
        theirs.disabled.insert("own-2".into());
        theirs.disabled.insert("pitbox.brand.bar".into());
        theirs.disabled.insert("pitbox.brand.gone".into());
        theirs.forks.insert(
            "pitbox.brand.bayro".into(),
            Fork {
                rule: fix(Some("pitbox.brand.bayro"), "bayro", "Bayro"),
                forked_from: "y".into(),
            },
        );

        let mut st = MergeStats::default();
        merge_list(&mut mine, &theirs, &catalog(), &mut st);

        assert_eq!(mine.own.len(), 2, "the same rule is not copied twice");
        assert_eq!(
            mine.own[1].id.as_deref(),
            Some("own-2"),
            "their rule under an id of his"
        );
        assert!(mine.disabled.contains("own-2"), "and switched off as they had it");
        assert!(mine.disabled.contains("pitbox.brand.bar"));
        assert_eq!(
            mine.forks["pitbox.brand.bayro"].rule.set_brand, "BMW M",
            "his fork stays"
        );
        assert_eq!(
            st,
            MergeStats {
                added: 3,
                kept_yours: 1,
                already: 1,
                unknown: 1
            }
        );
    }

    #[test]
    fn a_table_entry_he_decided_stays_his() {
        let mut mine = MapOverlay::default();
        mine.set.insert("nippon".into(), "Japan".into());
        mine.removed.insert("uk".into());
        let mut theirs = MapOverlay::default();
        theirs.set.insert("nippon".into(), "Japon".into());
        theirs.set.insert("uk".into(), "United Kingdom".into());
        theirs.set.insert("deutschland".into(), "Germany".into());
        let mut st = MergeStats::default();
        merge_map(&mut mine, &theirs, &mut st);
        assert_eq!(mine.set["nippon"], "Japan");
        assert!(mine.removed.contains("uk") && !mine.set.contains_key("uk"));
        assert_eq!(mine.set["deutschland"], "Germany");
        assert_eq!((st.added, st.kept_yours), (1, 2));
    }
}
