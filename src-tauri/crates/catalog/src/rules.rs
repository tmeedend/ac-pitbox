//! The list rules of the Rules screen, in two layers (REGLES§2, REGLES§4, REGLES§5).
//!
//! Brand fixes, name → tag, class fixes, tag merges, spec extractions: each
//! shipped rule carries a **stable id, written by hand** in the catalogue and
//! never derived from its content (REGLES§4) — so a rule can be corrected in a
//! later version and a user who disabled it keeps it disabled.
//!
//! The user's overlay holds decisions, never a copy of the catalogue:
//! - **disabled** — a flag. The rule keeps receiving improvements; it is
//!   simply off (REGLES§5: the frequent case, so it must not cost them);
//! - **forked** — a full copy, frozen, with the fingerprint of the version it
//!   was taken from, so a later catalogue can say "a new version exists";
//! - **own** — the user's rules.
//!
//! ## Precedence
//!
//! The user's rules run **before** the catalogue's: `brand_fix`, `class_fix`,
//! `tag_merge` and the spec extractions stop at the first rule that matches,
//! so "the user's rule wins" (REGLES§3) means it comes first. It is also where
//! the Rules screen has always inserted a new rule. A fork takes the place of
//! the rule it forks.

use std::collections::{BTreeMap, BTreeSet};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

// --- The rule types -----------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrandFix {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name_contains: String,
    pub set_brand: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NameToTag {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name_contains: String,
    pub add: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassFix {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub from: Vec<String>,
    pub set_class: Option<String>,
    #[serde(default)]
    pub add: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TagMerge {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub from: Vec<String>,
    pub to: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub from: Vec<String>,
    pub set: String,
}

/// A rule that can sit in a catalogue: it has an optional id.
pub trait Rule: Clone + Serialize + DeserializeOwned {
    fn id(&self) -> Option<&str>;
    fn with_id(self, id: Option<String>) -> Self;
}

macro_rules! rule {
    ($t:ty) => {
        impl Rule for $t {
            fn id(&self) -> Option<&str> {
                self.id.as_deref()
            }
            fn with_id(mut self, id: Option<String>) -> Self {
                self.id = id;
                self
            }
        }
    };
}
rule!(BrandFix);
rule!(NameToTag);
rule!(ClassFix);
rule!(TagMerge);
rule!(SetRule);

/// What a rule DOES, as canonical JSON: its content without its id. Two rules
/// doing the same thing compare equal whatever they are called.
pub fn content<T: Serialize>(r: &T) -> String {
    let mut v = serde_json::to_value(r).expect("rule serialises");
    if let Some(o) = v.as_object_mut() {
        o.remove("id");
    }
    serde_json::to_string(&v).expect("value serialises")
}

/// FNV-1a 64 of a rule's content. Not `DefaultHasher`, whose output is not
/// stable across Rust versions: this one is stored in the user's file, and must
/// still mean the same thing after the application is rebuilt.
pub fn fingerprint<T: Serialize>(r: &T) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in content(r).bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

// --- Overlay of one list ------------------------------------------------------

/// A shipped rule the user edited: his copy, and what it was copied from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fork<T> {
    pub rule: T,
    /// `fingerprint` of the catalogue rule at the time of the fork: when the
    /// catalogue's version no longer matches, a newer one exists (REGLES§6.1,
    /// case 4) - the fork is kept, the user is told.
    pub forked_from: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(bound(serialize = "T: Serialize", deserialize = "T: DeserializeOwned"))]
pub struct ListOverlay<T> {
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub disabled: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub forks: BTreeMap<String, Fork<T>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub own: Vec<T>,
}

impl<T> Default for ListOverlay<T> {
    fn default() -> Self {
        ListOverlay {
            disabled: BTreeSet::new(),
            forks: BTreeMap::new(),
            own: Vec::new(),
        }
    }
}

impl<T: Rule> ListOverlay<T> {
    pub fn is_empty(&self) -> bool {
        self.disabled.is_empty() && self.forks.is_empty() && self.own.is_empty()
    }

    /// The list as it applies: the user's rules first, then the catalogue in
    /// its order, disabled rules left out and forked ones replaced. A decision
    /// about a rule the catalogue no longer has is kept, without effect
    /// (REGLES§4) - except a fork, which then stands as a rule of the user's
    /// (REGLES§6.1, case 5: nothing he wrote is lost).
    pub fn apply(&self, catalog: &[T]) -> Vec<T> {
        let mut out: Vec<T> = self.own.iter().map(|r| r.clone().with_id(None)).collect();
        for (id, f) in &self.forks {
            if !catalog.iter().any(|c| c.id() == Some(id.as_str())) {
                out.push(f.rule.clone().with_id(None));
            }
        }
        for c in catalog {
            let id = c.id().map(str::to_string);
            match id {
                Some(ref id) if self.disabled.contains(id) => {}
                Some(ref id) if self.forks.contains_key(id) => {
                    out.push(self.forks[id].rule.clone().with_id(Some(id.clone())));
                }
                _ => out.push(c.clone()),
            }
        }
        out
    }

    /// The decisions that turn `catalog` into `edited` - what the Rules screen
    /// saves. A rule carrying a catalogue id is that rule: same content, no
    /// decision; other content, a fork. A rule without a known id is the
    /// user's. A catalogue rule missing from `edited` was taken off: disabled,
    /// not deleted, so it keeps receiving improvements (REGLES§5).
    pub fn diff(edited: &[T], catalog: &[T]) -> Self {
        let mut o = ListOverlay::default();
        let mut seen = BTreeSet::new();
        for r in edited {
            let shipped = r
                .id()
                .and_then(|id| catalog.iter().find(|c| c.id() == Some(id)))
                .filter(|c| seen.insert(c.id().unwrap_or_default().to_string()));
            match shipped {
                Some(c) if content(c) == content(r) => {}
                Some(c) => {
                    let id = c.id().unwrap_or_default().to_string();
                    o.forks.insert(
                        id,
                        Fork {
                            rule: r.clone().with_id(None),
                            forked_from: fingerprint(c),
                        },
                    );
                }
                None => o.own.push(r.clone().with_id(None)),
            }
        }
        for c in catalog {
            if let Some(id) = c.id() {
                if !seen.contains(id) {
                    o.disabled.insert(id.to_string());
                }
            }
        }
        o
    }

    /// The decisions a pre-layer file held (REGLES§13.2), against the frozen
    /// `manifest` of what those versions copied - which carries the same ids as
    /// the catalogue. A rule of the file is the shipped rule whose CONTENT it
    /// has (the file has no ids): no decision, even if the catalogue has since
    /// improved that rule - it is the improvement the user must receive. What
    /// matches nothing is his; what he left out, he removed.
    ///
    /// An edited shipped rule reads as one removal plus one rule of his own
    /// (REGLES§13.3): nothing is lost, and his version wins.
    pub fn migrate(file: &[T], manifest: &[T]) -> Self {
        let mut o = ListOverlay::default();
        let mut left: Vec<&T> = manifest.iter().collect();
        for r in file {
            match left.iter().position(|m| content(*m) == content(r)) {
                Some(i) => {
                    left.remove(i);
                }
                None => o.own.push(r.clone().with_id(None)),
            }
        }
        for m in left {
            if let Some(id) = m.id() {
                o.disabled.insert(id.to_string());
            }
        }
        o
    }
}

// --- Overlay of an ordered list of names --------------------------------------

/// Overlay of an ordered list whose entries are their own key - the track
/// category allowlist, where the order IS the priority of a track's
/// categories.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrderOverlay {
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub removed: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added: Vec<String>,
    /// The user's order, when he changed it. Catalogue entries it does not
    /// mention - added by a later version - follow it, in catalogue order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<String>>,
}

impl OrderOverlay {
    pub fn is_empty(&self) -> bool {
        self == &OrderOverlay::default()
    }

    fn base(&self, catalog: &[String]) -> Vec<String> {
        let mut out: Vec<String> = catalog.iter().filter(|c| !self.removed.contains(*c)).cloned().collect();
        for a in &self.added {
            if !out.contains(a) {
                out.push(a.clone());
            }
        }
        out
    }

    pub fn apply(&self, catalog: &[String]) -> Vec<String> {
        let base = self.base(catalog);
        let Some(order) = &self.order else {
            return base;
        };
        let mut out: Vec<String> = order.iter().filter(|e| base.contains(e)).cloned().collect();
        for e in base {
            if !out.contains(&e) {
                out.push(e);
            }
        }
        out
    }

    pub fn diff(edited: &[String], catalog: &[String]) -> Self {
        let mut o = OrderOverlay {
            removed: catalog.iter().filter(|c| !edited.contains(c)).cloned().collect(),
            added: edited.iter().filter(|e| !catalog.contains(e)).cloned().collect(),
            order: None,
        };
        if o.base(catalog) != edited {
            o.order = Some(edited.to_vec());
        }
        o
    }
}

// --- All the list rules -------------------------------------------------------

/// Current format of the list-rules overlay file.
pub const FORMAT: u32 = 1;

/// Every decision of the user on the list rules - `rules-overlay.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RulesOverlay {
    #[serde(default)]
    pub format: u32,
    #[serde(default, skip_serializing_if = "ListOverlay::is_empty")]
    pub brand_fix: ListOverlay<BrandFix>,
    #[serde(default, skip_serializing_if = "ListOverlay::is_empty")]
    pub name_to_tag: ListOverlay<NameToTag>,
    #[serde(default, skip_serializing_if = "ListOverlay::is_empty")]
    pub class_fix: ListOverlay<ClassFix>,
    #[serde(default, skip_serializing_if = "ListOverlay::is_empty")]
    pub car_tag_merge: ListOverlay<TagMerge>,
    #[serde(default, skip_serializing_if = "ListOverlay::is_empty")]
    pub drivetrain: ListOverlay<SetRule>,
    #[serde(default, skip_serializing_if = "ListOverlay::is_empty")]
    pub aspiration: ListOverlay<SetRule>,
    #[serde(default, skip_serializing_if = "ListOverlay::is_empty")]
    pub engine_config: ListOverlay<SetRule>,
    #[serde(default, skip_serializing_if = "ListOverlay::is_empty")]
    pub engine_pos: ListOverlay<SetRule>,
    #[serde(default, skip_serializing_if = "ListOverlay::is_empty")]
    pub gearbox: ListOverlay<SetRule>,
    #[serde(default, skip_serializing_if = "ListOverlay::is_empty")]
    pub track_tag_merge: ListOverlay<TagMerge>,
    #[serde(default, skip_serializing_if = "OrderOverlay::is_empty")]
    pub track_categories: OrderOverlay,
}

impl RulesOverlay {
    pub fn has_decisions(&self) -> bool {
        self != &RulesOverlay {
            format: self.format,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fix(id: Option<&str>, n: &str, b: &str) -> BrandFix {
        BrandFix {
            id: id.map(str::to_string),
            name_contains: n.into(),
            set_brand: b.into(),
        }
    }

    fn catalog() -> Vec<BrandFix> {
        vec![
            fix(Some("pitbox.brand.bayro"), "bayro", "BMW"),
            fix(Some("pitbox.brand.auriel"), "auriel", "Audi"),
            fix(Some("pitbox.brand.darche"), "darche", "Porsche"),
        ]
    }

    /// Whatever the Rules screen saves comes back as it was saved - provided
    /// the user's rules come first, which is where the screen puts them.
    #[test]
    fn a_saved_list_applies_back_to_itself() {
        let mut edited = catalog();
        edited.remove(1); // disabled
        edited[1].set_brand = "Porsche AG".into(); // forked
        edited.insert(0, fix(None, "lanzo", "RSS")); // his own
        let o = ListOverlay::diff(&edited, &catalog());
        assert_eq!(o.disabled, BTreeSet::from(["pitbox.brand.auriel".to_string()]));
        assert_eq!(o.forks.len(), 1);
        assert_eq!(o.own.len(), 1);
        let back: Vec<String> = o.apply(&catalog()).iter().map(content).collect();
        let want: Vec<String> = edited.iter().map(content).collect();
        assert_eq!(back, want, "round trip");
    }

    /// REGLES§5: a disabled rule keeps receiving improvements - it is only a
    /// flag. And a new catalogue rule reaches a user who decided things.
    #[test]
    fn a_disabled_rule_follows_the_catalogue_and_new_rules_arrive() {
        let mut edited = catalog();
        edited.remove(0);
        let o = ListOverlay::diff(&edited, &catalog());
        let mut next = catalog();
        next[0].set_brand = "BMW M".into(); // improved, still disabled
        next.push(fix(Some("pitbox.brand.minardi"), "minardi", "Minardi"));
        let out = o.apply(&next);
        assert!(out.iter().all(|r| r.name_contains != "bayro"), "stays disabled");
        assert!(out.iter().any(|r| r.name_contains == "minardi"), "the new rule arrived");
    }

    /// REGLES§6.1 case 4: a fork is frozen - the catalogue's improvement does
    /// not overwrite it, and its fingerprint says a newer version exists.
    #[test]
    fn a_fork_is_frozen_and_knows_when_it_is_outdated() {
        let mut edited = catalog();
        edited[0].set_brand = "Bayerische".into();
        let o = ListOverlay::diff(&edited, &catalog());
        let mut next = catalog();
        next[0].set_brand = "BMW M".into();
        assert_eq!(o.apply(&next)[0].set_brand, "Bayerische", "his version wins");
        assert_ne!(
            o.forks["pitbox.brand.bayro"].forked_from,
            fingerprint(&next[0]),
            "outdated"
        );
        assert_eq!(o.forks["pitbox.brand.bayro"].forked_from, fingerprint(&catalog()[0]));
    }

    /// REGLES§6.1 case 5: a fork of a rule the catalogue retired becomes an
    /// ordinary rule of the user's - nothing he wrote is lost.
    #[test]
    fn a_fork_of_a_retired_rule_stands_as_his_own() {
        let mut edited = catalog();
        edited[2].set_brand = "Porsche AG".into();
        let o = ListOverlay::diff(&edited, &catalog());
        let out = o.apply(&catalog()[..2]);
        assert!(out.iter().any(|r| r.set_brand == "Porsche AG" && r.id.is_none()));
    }

    /// REGLES§13.2 against a manifest without ids in the FILE: matched by
    /// content, and a shipped rule improved since is not frozen.
    #[test]
    fn migrating_a_file_recognises_shipped_rules_by_content() {
        let manifest = catalog();
        let file = vec![
            fix(None, "lanzo", "RSS"),
            fix(None, "bayro", "BMW"),
            fix(None, "darche", "Porsche"),
        ];
        let o = ListOverlay::migrate(&file, &manifest);
        assert_eq!(
            o.disabled,
            BTreeSet::from(["pitbox.brand.auriel".to_string()]),
            "removed by him"
        );
        assert_eq!(o.own, vec![fix(None, "lanzo", "RSS")]);
        assert!(o.forks.is_empty());
        let mut improved = catalog();
        improved[0].set_brand = "BMW M".into();
        assert_eq!(o.apply(&improved)[1].set_brand, "BMW M", "the improvement reaches him");
    }

    #[test]
    fn the_allowlist_keeps_the_users_order_and_receives_new_categories() {
        let cat: Vec<String> = ["#circuit", "#drift", "#rally"].iter().map(|s| s.to_string()).collect();
        let edited: Vec<String> = ["#rally", "#circuit", "#karting"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let o = OrderOverlay::diff(&edited, &cat);
        assert_eq!(o.apply(&cat), edited, "round trip");
        let mut next = cat.clone();
        next.push("#hillclimb".into());
        assert_eq!(
            o.apply(&next).last().map(String::as_str),
            Some("#hillclimb"),
            "new category arrives"
        );
        assert!(OrderOverlay::diff(&cat, &cat).is_empty(), "untouched list, no decision");
    }

    #[test]
    fn a_fingerprint_ignores_the_id() {
        assert_eq!(
            fingerprint(&fix(Some("a"), "x", "y")),
            fingerprint(&fix(None, "x", "y"))
        );
    }
}
