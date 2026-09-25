//! Taxonomy tables of Pit Box in two layers (REGLES§2): the **catalogue** the
//! application ships, and the user's **overlay** of decisions on top of it.
//!
//! Three tables: the category families, the country aliases (a spelling →
//! the game's name) and the country tags (a tag → the country it gives a mod
//! declaring none).
//!
//! ## Why two layers
//!
//! The shipped rules used to be COPIED into the user's `tag-rules.json` on
//! first launch, and that copy became the only truth: no later version could
//! reach it. Measured on a real install: a file seeded on June 27 still carried
//! the `remove` blacklist that v0.4 deleted. Now the catalogue is never copied,
//! and the overlay holds only DECISIONS, keyed by the natural key of each entry
//! (the tag, the spelling, the family id) — which is what makes them survive an
//! update: a catalogue that adds `lmgt3` to Race reaches a user who moved
//! `gt3` to Classic, and his move still wins.
//!
//! ## Precedence
//!
//! Overlay over catalogue, entry by entry, silently (REGLES§3). An overlay
//! entry pointing at what the catalogue no longer has is kept, without effect
//! (REGLES§4).
//!
//! ## Why a crate
//!
//! The application merges the layers to classify the library; `rules-tool`
//! merges them to promote a developer's decisions into the catalogue. Two
//! implementations would end up disagreeing - the country aliases once had a
//! TypeScript copy, and it is the copy nobody saw that won. No I/O here: the
//! callers read and write the files.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// One family of car categories.
///
/// A tag belongs to **one** family at most: attached elsewhere it moves,
/// otherwise the index counters stop meaning anything (TAXO§7.3). A car, on
/// the other hand, belongs to as many families as its tags reach — a 250 GTO
/// is Classic, Sportscars *and* Race.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategoryFamily {
    /// Stable slug. The shipped families are translated from it; it is also
    /// the value a filter chip stores, so it must never be renamed.
    pub id: String,
    /// Displayed name of a family the user made up. Absent on the shipped ones,
    /// which are translated instead (TAXO§12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Silhouette picked from the embedded set; the neutral glyph otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Tags compared lowercased and without their leading `#`: the same tag
    /// reaches a car as `#rally` through a rule and as `rally` from its file.
    #[serde(default)]
    pub tags: Vec<String>,
}

/// The tables, as a catalogue ships them or as they apply.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TaxonomyTables {
    #[serde(default)]
    pub families: Vec<CategoryFamily>,
    #[serde(default)]
    pub country_aliases: BTreeMap<String, String>,
    #[serde(default)]
    pub country_tags: BTreeMap<String, String>,
    /// How a brand is spelled (lowercased) → the brand it is filed under
    /// (TAXO§7). Empty in the catalogue so far: a brand merge is a decision
    /// the user makes from a proposal, never a silent one (TAXO§7.2).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub brand_aliases: BTreeMap<String, String>,
    /// Values of the brand field that are no brand - a pack, a series, a
    /// modder (`AER`, `WSC Legends`, `traffic`), lowercased: the real brand is
    /// read from the car's name instead (`brands::from_name` in the app).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub not_brands: BTreeSet<String>,
}

/// Overlay of a `key → value` table (country aliases, country tags).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MapOverlay {
    /// Entries added, or catalogue entries given another value.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub set: BTreeMap<String, String>,
    /// Catalogue entries the user removed. A tombstone, not an absence: an
    /// absence would let the next catalogue bring the entry back.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub removed: BTreeSet<String>,
}

/// Overlay of a set of names (the brands that are no brand): entries added,
/// catalogue entries removed - a removal being a tombstone, as everywhere.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SetOverlay {
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub added: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub removed: BTreeSet<String>,
}

impl SetOverlay {
    pub fn apply(&self, catalog: &BTreeSet<String>) -> BTreeSet<String> {
        catalog
            .iter()
            .filter(|k| !self.removed.contains(*k))
            .chain(&self.added)
            .cloned()
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }

    /// Entries in their compared form (lowercased, trimmed); adding what the
    /// catalogue has, or removing what it has not, is no decision.
    fn normalized(self, catalog: &BTreeSet<String>) -> Self {
        let key = |k: String| k.trim().to_lowercase();
        SetOverlay {
            added: self
                .added
                .into_iter()
                .map(key)
                .filter(|k| !k.is_empty() && !catalog.contains(k))
                .collect(),
            removed: self
                .removed
                .into_iter()
                .map(key)
                .filter(|k| catalog.contains(k))
                .collect(),
        }
    }
}

impl MapOverlay {
    pub fn apply(&self, catalog: &BTreeMap<String, String>) -> BTreeMap<String, String> {
        let mut out: BTreeMap<String, String> = catalog
            .iter()
            .filter(|(k, _)| !self.removed.contains(*k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        out.extend(self.set.iter().map(|(k, v)| (k.clone(), v.clone())));
        out
    }

    /// The overlay that turns `catalog` into `user`.
    pub fn diff(user: &BTreeMap<String, String>, catalog: &BTreeMap<String, String>) -> Self {
        MapOverlay {
            set: user
                .iter()
                .filter(|(k, v)| catalog.get(*k) != Some(*v))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            removed: catalog.keys().filter(|k| !user.contains_key(*k)).cloned().collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.set.is_empty() && self.removed.is_empty()
    }

    /// Keys in their compared form (lowercased, trimmed); an entry set to what
    /// the catalogue already says is dead weight, and dropped.
    fn normalized(self, catalog: &BTreeMap<String, String>) -> Self {
        let set = self
            .set
            .into_iter()
            .map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string()))
            .filter(|(k, v)| !k.is_empty() && !v.is_empty() && catalog.get(k) != Some(v))
            .collect();
        let removed = self
            .removed
            .into_iter()
            .map(|k| k.trim().to_lowercase())
            .filter(|k| catalog.contains_key(k))
            .collect();
        MapOverlay { set, removed }
    }
}

/// Name or icon given to a SHIPPED family.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FamilyMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

/// Overlay of the family table.
///
/// **Tag by tag, not family by family.** Forking a whole family to move one
/// tag would freeze it: the next catalogue could no longer add a tag to it.
/// The natural key of a family table is the tag (TAXO§7.3, one tag, one
/// family), so that is what the overlay records.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FamilyOverlay {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub meta: BTreeMap<String, FamilyMeta>,
    /// Families the user made — their tags come from `tags`, like any other.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub created: Vec<CategoryFamily>,
    /// Shipped families the user deleted.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub removed: BTreeSet<String>,
    /// `tag → family id`, overriding the catalogue; `""` detaches the tag from
    /// every family.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tags: BTreeMap<String, String>,
}

/// A tag as families compare it: lowercased, trimmed, without its leading `#`.
pub fn family_tag(tag: &str) -> String {
    tag.trim().to_lowercase().trim_start_matches('#').trim().to_string()
}

/// Tag → family id of a table, the first family winning a tag listed twice.
pub fn lookup(families: &[CategoryFamily]) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    for f in families {
        for t in &f.tags {
            m.entry(family_tag(t)).or_insert_with(|| f.id.clone());
        }
    }
    m
}

impl FamilyOverlay {
    pub fn apply(&self, catalog: &[CategoryFamily]) -> Vec<CategoryFamily> {
        let mut families: Vec<CategoryFamily> = catalog
            .iter()
            .filter(|f| !self.removed.contains(&f.id))
            .map(|f| {
                let meta = self.meta.get(&f.id);
                CategoryFamily {
                    id: f.id.clone(),
                    name: meta.and_then(|m| m.name.clone()).or_else(|| f.name.clone()),
                    icon: meta.and_then(|m| m.icon.clone()).or_else(|| f.icon.clone()),
                    tags: Vec::new(),
                }
            })
            .collect();
        for c in &self.created {
            if !families.iter().any(|f| f.id == c.id) {
                families.push(CategoryFamily {
                    tags: Vec::new(),
                    ..c.clone()
                });
            }
        }
        // Each family keeps ITS catalogue tags in their order, and receives the
        // tags moved to it after them: a promoted catalogue then diffs line by
        // line instead of being reshuffled.
        let mut owner = lookup(catalog);
        for (tag, id) in &self.tags {
            owner.insert(family_tag(tag), id.clone());
        }
        let mut placed = BTreeSet::new();
        let own = catalog
            .iter()
            .flat_map(|f| f.tags.iter().map(move |t| (family_tag(t), &f.id)));
        let kept = own.filter(|(t, id)| owner.get(t) == Some(*id)).map(|(t, _)| t);
        let moved = owner.keys().cloned();
        for tag in kept.chain(moved) {
            if !placed.insert(tag.clone()) {
                continue;
            }
            // A tag sent to a family that no longer exists - retired from the
            // catalogue, deleted by the user - attaches nothing (REGLES§4).
            if let Some(f) = owner.get(&tag).and_then(|id| families.iter_mut().find(|f| &f.id == id)) {
                f.tags.push(tag);
            }
        }
        families
    }

    /// The overlay that turns `catalog` into `user`.
    pub fn diff(user: &[CategoryFamily], catalog: &[CategoryFamily]) -> Self {
        let mut o = FamilyOverlay::default();
        for c in catalog {
            match user.iter().find(|u| u.id == c.id) {
                None => {
                    o.removed.insert(c.id.clone());
                }
                Some(u) => {
                    let meta = FamilyMeta {
                        name: u.name.clone().filter(|n| Some(n) != c.name.as_ref()),
                        icon: u.icon.clone().filter(|i| Some(i) != c.icon.as_ref()),
                    };
                    if meta != FamilyMeta::default() {
                        o.meta.insert(c.id.clone(), meta);
                    }
                }
            }
        }
        o.created = user
            .iter()
            .filter(|u| !catalog.iter().any(|c| c.id == u.id))
            .map(|u| CategoryFamily {
                tags: Vec::new(),
                ..u.clone()
            })
            .collect();
        let (mine, theirs) = (lookup(user), lookup(catalog));
        for tag in mine.keys().chain(theirs.keys()) {
            let (m, t) = (mine.get(tag), theirs.get(tag));
            if m != t {
                o.tags.insert(tag.clone(), m.cloned().unwrap_or_default());
            }
        }
        o
    }

    pub fn is_empty(&self) -> bool {
        self == &FamilyOverlay::default()
    }

    fn normalized(mut self, catalog: &[CategoryFamily]) -> Self {
        let theirs = lookup(catalog);
        self.tags = self
            .tags
            .into_iter()
            .map(|(t, id)| (family_tag(&t), id.trim().to_string()))
            .filter(|(t, id)| !t.is_empty() && theirs.get(t).map(String::as_str).unwrap_or("") != id)
            .collect();
        self.created
            .retain(|c| !c.id.trim().is_empty() && !catalog.iter().any(|f| f.id == c.id));
        for c in &mut self.created {
            c.tags.clear();
        }
        self
    }
}

/// Current format of the overlay file. Bumped if its shape changes.
pub const FORMAT: u32 = 1;

/// Everything the user decided on the taxonomy tables.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TaxonomyOverlay {
    #[serde(default)]
    pub format: u32,
    #[serde(default)]
    pub families: FamilyOverlay,
    #[serde(default)]
    pub country_aliases: MapOverlay,
    #[serde(default)]
    pub country_tags: MapOverlay,
    /// Values unknown to the game left as they are (TAXO§7.2, "Ignore").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignored_countries: Vec<String>,
    #[serde(default, skip_serializing_if = "MapOverlay::is_empty")]
    pub brand_aliases: MapOverlay,
    /// Brand merges the user answered "Ignore" to, as `from → to` (TAXO§7.2:
    /// definitive and remembered - the proposal does not come back).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignored_brand_merges: Vec<String>,
    #[serde(default, skip_serializing_if = "SetOverlay::is_empty")]
    pub not_brands: SetOverlay,
}

impl TaxonomyOverlay {
    /// Cleans the overlay against the catalogue before it is written: compared
    /// keys, and no entry restating what the catalogue already says — so that
    /// a user who undoes a change by hand is back on the catalogue, and keeps
    /// receiving its improvements.
    pub fn normalized(self, catalog: &TaxonomyTables) -> Self {
        let mut ignored: Vec<String> = self
            .ignored_countries
            .into_iter()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .collect();
        ignored.sort();
        ignored.dedup();
        let mut ignored_merges = self.ignored_brand_merges;
        ignored_merges.sort();
        ignored_merges.dedup();
        TaxonomyOverlay {
            format: FORMAT,
            families: self.families.normalized(&catalog.families),
            country_aliases: self.country_aliases.normalized(&catalog.country_aliases),
            country_tags: self.country_tags.normalized(&catalog.country_tags),
            ignored_countries: ignored,
            brand_aliases: self.brand_aliases.normalized(&catalog.brand_aliases),
            ignored_brand_merges: ignored_merges,
            not_brands: self.not_brands.normalized(&catalog.not_brands),
        }
    }

    /// Whether any decision is recorded on the three tables (the ignored
    /// countries are a preference of the Countries tab, not a table entry).
    pub fn has_decisions(&self) -> bool {
        !(self.families.is_empty()
            && self.country_aliases.is_empty()
            && self.country_tags.is_empty()
            && self.brand_aliases.is_empty()
            && self.not_brands.is_empty())
    }
}

impl TaxonomyTables {
    /// The tables as they apply: catalogue, then overlay.
    pub fn apply(&self, o: &TaxonomyOverlay) -> TaxonomyTables {
        TaxonomyTables {
            families: o.families.apply(&self.families),
            country_aliases: o.country_aliases.apply(&self.country_aliases),
            country_tags: o.country_tags.apply(&self.country_tags),
            brand_aliases: o.brand_aliases.apply(&self.brand_aliases),
            not_brands: o.not_brands.apply(&self.not_brands),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fam(id: &str, tags: &[&str]) -> CategoryFamily {
        CategoryFamily {
            id: id.into(),
            name: None,
            icon: Some(id.into()),
            tags: tags.iter().map(|t| t.to_string()).collect(),
        }
    }

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    /// The whole point of the layers: a catalogue improvement reaches a user
    /// who edited the same table, and his edit still wins.
    #[test]
    fn a_new_catalogue_reaches_an_edited_table_and_the_edit_still_wins() {
        let v1 = vec![fam("race", &["race", "gt3"]), fam("classic", &["vintage"])];
        let user = vec![fam("race", &["race"]), fam("classic", &["vintage", "gt3"])];
        let overlay = FamilyOverlay::diff(&user, &v1);
        assert_eq!(
            overlay.tags,
            map(&[("gt3", "classic")]),
            "one decision, keyed by the tag"
        );

        let v2 = vec![fam("race", &["race", "gt3", "lmgt3"]), fam("classic", &["vintage"])];
        let out = overlay.apply(&v2);
        assert_eq!(out[0].tags, vec!["race", "lmgt3"], "the new catalogue tag arrived");
        assert_eq!(out[1].tags, vec!["vintage", "gt3"], "the user's move still wins");
    }

    /// `rules-tool promote` rewrites the catalogue from `apply`: the tags keep
    /// the catalogue's order, so the promoted file diffs line by line.
    #[test]
    fn applying_nothing_gives_the_catalogue_back_in_its_order() {
        let cat = vec![fam("race", &["race", "gt3", "dtm"]), fam("classic", &["vintage"])];
        assert_eq!(FamilyOverlay::default().apply(&cat), cat);
    }

    /// REGLES§4: an overlay entry pointing at what the catalogue no longer has
    /// is kept, without effect and without error.
    #[test]
    fn an_entry_on_a_retired_family_has_no_effect() {
        let o = FamilyOverlay {
            tags: map(&[("gt3", "gone")]),
            ..Default::default()
        };
        let out = o.apply(&[fam("race", &["race", "gt3"])]);
        assert_eq!(
            out[0].tags,
            vec!["race"],
            "gt3 sent to a family that no longer exists attaches nothing"
        );
    }

    /// Undoing a change by hand puts the entry back on the catalogue.
    #[test]
    fn an_entry_restating_the_catalogue_is_dropped() {
        let catalog = TaxonomyTables {
            families: vec![fam("race", &["race", "gt3"])],
            country_aliases: map(&[("usa", "United States")]),
            country_tags: BTreeMap::new(),
            brand_aliases: map(&[("alfa", "Alfa Romeo")]),
            not_brands: BTreeSet::new(),
        };
        let mut o = TaxonomyOverlay::default();
        o.brand_aliases.set.insert("ALFA".into(), "Alfa Romeo".into());
        o.country_aliases.set.insert(" USA ".into(), "United States".into());
        o.families.tags.insert("#GT3".into(), "race".into());
        let n = o.normalized(&catalog);
        assert!(n.country_aliases.set.is_empty());
        assert!(n.brand_aliases.set.is_empty(), "a brand merge the catalogue makes too");
        assert!(n.families.tags.is_empty());
        assert!(!n.has_decisions());
    }

    #[test]
    fn a_map_overlay_reproduces_the_table_it_was_diffed_from() {
        let catalog = map(&[("usa", "United States"), ("holland", "Netherlands")]);
        let user = map(&[("usa", "United States"), ("nippon", "Japan")]);
        let o = MapOverlay::diff(&user, &catalog);
        assert!(o.removed.contains("holland"), "a removed shipped entry is a tombstone");
        assert_eq!(o.apply(&catalog), user);
    }

    /// "Not a brand" is a set in two layers like the others: added by the
    /// user, removed as a tombstone over the catalogue, and a file written
    /// before it existed reads as no decision.
    #[test]
    fn not_a_brand_is_a_set_in_two_layers() {
        let catalog = TaxonomyTables {
            not_brands: ["traffic".to_string()].into(),
            ..Default::default()
        };
        let mut o = TaxonomyOverlay::default();
        o.not_brands.added.insert(" AER ".into());
        o.not_brands.added.insert("traffic".into());
        o.not_brands.removed.insert("TRAFFIC".into());
        let o = o.normalized(&catalog);
        assert_eq!(
            o.not_brands.added,
            ["aer".to_string()].into(),
            "compared form, no restating"
        );
        assert_eq!(
            catalog.apply(&o).not_brands,
            ["aer".to_string()].into(),
            "the tombstone holds"
        );
        let old: TaxonomyOverlay = serde_json::from_str(r#"{"format":1,"families":{}}"#).unwrap();
        assert!(old.not_brands.is_empty(), "an older taxonomy.json reads as no decision");
    }
}
