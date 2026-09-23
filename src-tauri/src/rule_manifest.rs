//! What the pre-layer versions shipped as list rules, and how a user's
//! `tag-rules.json` compares to it (REGLES§13.1, REGLES§13.2).
//!
//! The list rules — brand fixes, name → tag, class fixes, tag merges, spec
//! extraction, track category allowlist — are still copied into the user's
//! file (the taxonomy tables are not any more, see `taxonomy.rs`). Moving them
//! to a catalogue + overlay needs to know, for every rule found in a user's
//! file, whether it is a shipped rule left intact, one the user deleted, or
//! one of his own. Without that, a migration either freezes the user on his
//! copy or resurrects what he deleted.
//!
//! ## One manifest, measured
//!
//! REGLES§13.1 asks for one manifest per published version. Measured on the
//! history: the list rules are **identical** from the first commit (63e44ba)
//! to v0.7.0 — only `remove` (a blacklist the engine no longer reads) went
//! away in v0.4, and `category_allowlist` appeared in v0.1. One frozen file
//! therefore stands for every version: `rules/manifests/pre-layer-rules.json`.
//!
//! ## No ids yet
//!
//! The rules carry no stable identifier, so matching is by content
//! (REGLES§13.3): intact rules and deletions are recognised, a rule the user
//! EDITED is not — it reads as one deletion plus one rule of his own. Benign
//! by construction: his version is kept, and it takes precedence (below).
//!
//! ## Precedence is not the same for every section
//!
//! `brand_fix`, `class_fix`, `tag_merge` and the spec extractions stop at the
//! FIRST rule that matches; `name_to_tag` adds up every match. "The user's rule
//! wins" (REGLES§3) therefore means, for a first-match section, that it runs
//! before the catalogue's — which is also where the Rules screen has always
//! put a new rule (`unshift`). The migration to an overlay must keep that
//! order, and `harmonize::snapshot` is how it proves it did (REGLES§13.5).

// Used by the tests and the diff-nul bench today, and by the migration of the
// list rules to an overlay (lot 3 of the rules catalogue, docs/CHANTIERS.md).
// Remove this line when that migration calls `classify`.
#![cfg_attr(not(test), allow(dead_code))]

use serde::Serialize;

use crate::rules::Rules;

const PRE_LAYER_RULES: &str = include_str!("../rules/manifests/pre-layer-rules.json");

/// The list rules every pre-layer version shipped — frozen.
pub fn pre_layer_rules() -> Rules {
    serde_json::from_str(PRE_LAYER_RULES).expect("the pre-layer rules manifest must be valid")
}

/// The sections holding list rules, in the order the engine reads them.
pub const SECTIONS: &[&str] = &[
    "car.brand_fix",
    "car.name_to_tag",
    "car.class_fix",
    "car.tag_merge",
    "car.extraction_specs.drivetrain",
    "car.extraction_specs.aspiration",
    "car.extraction_specs.engine_config",
    "car.extraction_specs.engine_pos",
    "car.extraction_specs.gearbox",
    "track.tag_merge",
    "track.category_allowlist",
];

fn canon<T: Serialize>(items: &[T]) -> Vec<String> {
    items
        .iter()
        .map(|i| serde_json::to_string(i).expect("rule serialises"))
        .collect()
}

/// The entries of one section, each as its canonical JSON - the content a
/// rule is recognised by.
pub fn entries(r: &Rules, section: &str) -> Vec<String> {
    let s = &r.car.extraction_specs;
    match section {
        "car.brand_fix" => canon(&r.car.brand_fix),
        "car.name_to_tag" => canon(&r.car.name_to_tag),
        "car.class_fix" => canon(&r.car.class_fix),
        "car.tag_merge" => canon(&r.car.tag_merge),
        "car.extraction_specs.drivetrain" => canon(&s.drivetrain),
        "car.extraction_specs.aspiration" => canon(&s.aspiration),
        "car.extraction_specs.engine_config" => canon(&s.engine_config),
        "car.extraction_specs.engine_pos" => canon(&s.engine_pos),
        "car.extraction_specs.gearbox" => canon(&s.gearbox),
        "track.tag_merge" => canon(&r.track.tag_merge),
        "track.category_allowlist" => canon(&r.track.category_allowlist),
        _ => Vec::new(),
    }
}

/// How one section of a user's file compares to the manifest (REGLES§13.2).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SectionClass {
    /// Shipped rules found unchanged: a reference to the catalogue, no overlay.
    pub intact: Vec<String>,
    /// Shipped rules absent from the file: the user deleted them. The most
    /// important line of REGLES§13.2 - forgetting it resurrects them.
    pub removed: Vec<String>,
    /// Rules matching nothing shipped: his own, or a shipped rule he edited.
    pub user: Vec<String>,
    /// The shipped rules are all there, but not in their order. Only matters
    /// for a first-match section, and for the allowlist, whose order IS the
    /// priority of a track's categories.
    pub reordered: bool,
}

/// Compares every section of `file` to `manifest`, as a multiset of contents.
///
/// A section empty in the file is skipped: it was refilled from the embedded
/// rules on each load (the allowlist of a pre-v0.1 file), so it holds no
/// decision.
pub fn classify(file: &Rules, manifest: &Rules) -> Vec<(&'static str, SectionClass)> {
    SECTIONS
        .iter()
        .map(|&sec| {
            let mine = entries(file, sec);
            let theirs = entries(manifest, sec);
            if mine.is_empty() {
                return (
                    sec,
                    SectionClass {
                        intact: theirs,
                        ..Default::default()
                    },
                );
            }
            let mut left = theirs.clone();
            let mut c = SectionClass::default();
            for e in &mine {
                match left.iter().position(|t| t == e) {
                    Some(i) => {
                        left.remove(i);
                        c.intact.push(e.clone());
                    }
                    None => c.user.push(e.clone()),
                }
            }
            c.removed = left;
            let shipped_in_file: Vec<&String> = mine.iter().filter(|e| theirs.contains(e)).collect();
            let shipped_order: Vec<&String> = theirs.iter().filter(|e| mine.contains(e)).collect();
            c.reordered = shipped_in_file != shipped_order;
            (sec, c)
        })
        .collect()
}

/// Whether the file holds any decision at all on the list rules.
pub fn has_decisions(classes: &[(&str, SectionClass)]) -> bool {
    classes
        .iter()
        .any(|(_, c)| !c.removed.is_empty() || !c.user.is_empty() || c.reordered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::BrandFix;

    fn fix(n: &str, b: &str) -> BrandFix {
        BrandFix {
            name_contains: n.into(),
            set_brand: b.into(),
        }
    }

    fn section<'a>(c: &'a [(&str, SectionClass)], name: &str) -> &'a SectionClass {
        &c.iter().find(|(s, _)| *s == name).unwrap().1
    }

    /// REGLES§13.2: intact, deleted, the user's own - each recognised.
    #[test]
    fn a_file_is_told_apart_into_intact_deleted_and_own_rules() {
        let manifest = pre_layer_rules();
        let mut file = manifest.clone();
        let shipped = file.car.brand_fix.remove(0); // deleted by the user
        file.car.brand_fix.insert(0, fix("lanzo", "RSS")); // his own
        let c = classify(&file, &manifest);
        let b = section(&c, "car.brand_fix");
        assert_eq!(
            b.removed,
            vec![serde_json::to_string(&shipped).unwrap()],
            "the deletion is seen"
        );
        assert_eq!(b.user.len(), 1, "his rule is his");
        assert_eq!(b.intact.len(), manifest.car.brand_fix.len() - 1);
        assert!(!b.reordered, "a rule of his in front is not a reordering");
        assert!(has_decisions(&c));
    }

    /// An untouched file - the case of every install measured so far - holds
    /// no decision: migrating it must produce an empty overlay.
    #[test]
    fn an_untouched_copy_holds_no_decision() {
        let c = classify(&pre_layer_rules(), &pre_layer_rules());
        assert!(!has_decisions(&c), "{c:?}");
    }

    /// A pre-v0.1 file has no allowlist: it was refilled on load, not deleted.
    #[test]
    fn an_empty_section_is_not_a_deletion() {
        let mut file = pre_layer_rules();
        file.track.category_allowlist.clear();
        let c = classify(&file, &pre_layer_rules());
        assert!(section(&c, "track.category_allowlist").removed.is_empty());
    }

    /// The allowlist's order is the priority of a track's categories: moving
    /// one up is a decision.
    #[test]
    fn reordering_the_allowlist_is_a_decision() {
        let mut file = pre_layer_rules();
        file.track.category_allowlist.swap(0, 1);
        let c = classify(&file, &pre_layer_rules());
        assert!(section(&c, "track.category_allowlist").reordered);
        assert!(has_decisions(&c));
    }
}
