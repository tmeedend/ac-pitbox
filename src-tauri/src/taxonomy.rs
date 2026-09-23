//! Two layers for the taxonomy tables (REGLES§2): the
//! **catalogue** Pit Box ships, and the user's **overlay** on top of it.
//!
//! Three tables so far, the ones the Categories and Countries tabs edit: the
//! category families, the country aliases, and the country tags (a tag that
//! gives a country to a mod declaring none — `extraction_country`).
//!
//! ## Why two layers
//!
//! The shipped rules used to be COPIED into `tag-rules.json` on first launch,
//! and that copy became the only truth: no later version of Pit Box could
//! reach it, except by refilling a section left entirely empty. Measured on a
//! real install: a file seeded on June 27 still carried the `remove` blacklist
//! that v0.4 deleted. An improvement to the shipped tables never arrived — and
//! the first edit in the Categories tab froze the whole family table for good.
//!
//! Now the catalogue is **never copied**: it is read from the embedded seed on
//! every load, so an update replaces it by construction. The overlay holds only
//! the user's DECISIONS, keyed by the natural key of each entry (the tag, the
//! spelling, the family id) — which is what makes them survive an update: a
//! catalogue that adds `lmgt3` to Race reaches a user who moved `gt3` to
//! Classic, and his move still wins.
//!
//! ## Precedence
//!
//! Overlay over catalogue, entry by entry, silently (REGLES§3): there is no
//! conflict to arbitrate, so no dialog. An overlay entry pointing at something
//! the catalogue no longer has is kept, and simply has no effect (REGLES§4).
//!
//! Pure on purpose — paths in, values out, no Tauri: the file is handed in by
//! `rules::load`, and everything here is testable on its own.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::rules::{CategoryFamily, Rules};

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

    /// The overlay that turns `catalog` into `user` — the migration of a table
    /// that was stored whole.
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

fn family_tag(tag: &str) -> String {
    tag.trim().to_lowercase().trim_start_matches('#').trim().to_string()
}

/// Tag → family id of a table, the first family winning a tag listed twice.
fn lookup(families: &[CategoryFamily]) -> BTreeMap<String, String> {
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
        let mut owner = lookup(catalog);
        for (tag, id) in &self.tags {
            owner.insert(family_tag(tag), id.clone());
        }
        for (tag, id) in owner {
            // A tag sent to a family that no longer exists - retired from the
            // catalogue, deleted by the user - attaches nothing (REGLES§4).
            if let Some(f) = families.iter_mut().find(|f| f.id == id) {
                f.tags.push(tag);
            }
        }
        families
    }

    /// The overlay that turns `catalog` into `user` — the migration of a family
    /// table written whole by the first version of the Categories tab.
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

/// The three tables as every pre-layer version copied them into
/// `tag-rules.json` (REGLES§13.1) — **frozen**, never updated with the
/// catalogue.
///
/// The migration diffs a user's file against THIS, not against the current
/// catalogue: a user skipping versions would otherwise arrive with an old
/// copy, compared to a newer catalogue, and every difference - improvements
/// he never saw - would be recorded as HIS decision and frozen for good.
const PRE_LAYER: &str = include_str!("../rules/manifests/pre-layer-tables.json");

pub fn pre_layer() -> Rules {
    serde_json::from_str(PRE_LAYER).expect("the pre-layer manifest must be valid")
}

/// Current format of `taxonomy.json`. Bumped if its shape changes.
pub const FORMAT: u32 = 1;

/// Everything the user decided on the taxonomy tables. `taxonomy.json` in the
/// config directory, written whole and synchronously (CLAUDE.md rule 6).
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
}

impl TaxonomyOverlay {
    /// Cleans the overlay against the catalogue before it is written: compared
    /// keys, and no entry restating what the catalogue already says — so that
    /// a user who undoes a change by hand is back on the catalogue, and keeps
    /// receiving its improvements.
    pub fn normalized(self, catalog: &Rules) -> Self {
        let mut ignored: Vec<String> = self
            .ignored_countries
            .into_iter()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .collect();
        ignored.sort();
        ignored.dedup();
        TaxonomyOverlay {
            format: FORMAT,
            families: self.families.normalized(&catalog.car.category_families),
            country_aliases: self.country_aliases.normalized(&catalog.country_aliases.map),
            country_tags: self.country_tags.normalized(&catalog.car.extraction_country.map),
            ignored_countries: ignored,
        }
    }

    /// The overlay reproducing the tables a pre-layer `tag-rules.json` stored
    /// whole — **so that the library is classified exactly as before**
    /// (REGLES§13.5).
    ///
    /// `baseline` is the frozen `pre_layer()` manifest, measured on the release
    /// history: the country tags are identical in every published version
    /// (v0.1.0 to v0.7.0), and the aliases and families were never published;
    /// they only exist in files written by this branch, from these very
    /// tables. So the copy a user's file holds came from the manifest, and
    /// every difference is his. A section missing or empty in the file was
    /// refilled from the catalogue on each load: no decision, no overlay.
    pub fn migrate(file: &Rules, baseline: &Rules) -> Self {
        let mut o = TaxonomyOverlay {
            format: FORMAT,
            ..Default::default()
        };
        if !file.car.category_families.is_empty() {
            o.families = FamilyOverlay::diff(&file.car.category_families, &baseline.car.category_families);
        }
        if !file.country_aliases.map.is_empty() {
            o.country_aliases = MapOverlay::diff(&file.country_aliases.map, &baseline.country_aliases.map);
        }
        if !file.car.extraction_country.map.is_empty() {
            o.country_tags = MapOverlay::diff(&file.car.extraction_country.map, &baseline.car.extraction_country.map);
        }
        o.ignored_countries = file.country_aliases.ignored.clone();
        o
    }
}

/// Writes the effective tables into `rules`: catalogue, then overlay.
pub fn apply(rules: &mut Rules, catalog: &Rules, o: &TaxonomyOverlay) {
    rules.car.category_families = o.families.apply(&catalog.car.category_families);
    rules.country_aliases.map = o.country_aliases.apply(&catalog.country_aliases.map);
    rules.country_aliases.ignored = o.ignored_countries.clone();
    rules.car.extraction_country.map = o.country_tags.apply(&catalog.car.extraction_country.map);
}

/// Empties the three tables before `tag-rules.json` is written: they belong to
/// the catalogue and to `taxonomy.json` now, and a copy left in the rules file
/// is how the catalogue got frozen in the first place.
pub fn strip(rules: &mut Rules) {
    rules.car.category_families.clear();
    rules.country_aliases.map.clear();
    rules.country_aliases.ignored.clear();
    rules.car.extraction_country.map.clear();
}

/// Reads `taxonomy.json`, or builds it from the tables of the rules file the
/// first time (then writes it, so the migration happens once).
///
/// **A file that exists but does not parse is never overwritten**: an empty
/// overlay is used for this session and the failure is logged. Rewriting it
/// would erase the user's decisions for the sake of a clean start.
pub fn load_or_migrate(path: &Path, file: &Rules) -> TaxonomyOverlay {
    match std::fs::read_to_string(path) {
        Ok(s) => match serde_json::from_str::<TaxonomyOverlay>(&s) {
            Ok(o) => o,
            Err(e) => {
                log::warn!(
                    "{} unreadable, taxonomy overlay ignored this session: {e}",
                    path.display()
                );
                TaxonomyOverlay::default()
            }
        },
        Err(_) => {
            let o = TaxonomyOverlay::migrate(file, &pre_layer());
            if let Err(e) = save(path, &o) {
                log::warn!("taxonomy overlay migration not written, will run again: {e}");
            }
            o
        }
    }
}

pub fn save(path: &Path, o: &TaxonomyOverlay) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(o).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::default_rules;

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

    fn sorted(mut f: Vec<CategoryFamily>) -> Vec<(String, Vec<String>)> {
        for x in &mut f {
            x.tags.sort();
        }
        f.into_iter().map(|x| (x.id, x.tags)).collect()
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
        let out = sorted(overlay.apply(&v2));
        assert_eq!(out[0].1, vec!["lmgt3", "race"], "the new catalogue tag arrived");
        assert_eq!(out[1].1, vec!["gt3", "vintage"], "the user's move still wins");
    }

    /// REGLES§13.5: migrating must not reclassify anything. The effective table
    /// rebuilt from catalogue + migrated overlay is the table the file held.
    #[test]
    fn migrating_a_whole_table_reproduces_it_exactly() {
        let catalog = default_rules();
        let mut file = catalog.clone();
        file.car.category_families.retain(|f| f.id != "drift");
        file.car.category_families[0].tags.push("sport".into());
        file.car.category_families[1].name = Some("Sport".into());
        file.car.category_families.push(CategoryFamily {
            id: "endurance".into(),
            name: Some("Endurance".into()),
            icon: None,
            tags: vec!["wec".into()],
        });
        file.country_aliases.map.remove("usa");
        file.country_aliases.map.insert("nippon".into(), "Japan".into());
        file.car.extraction_country.map.insert("nihon".into(), "Japan".into());

        let o = TaxonomyOverlay::migrate(&file, &catalog);
        let mut rebuilt = file.clone();
        apply(&mut rebuilt, &catalog, &o);

        let want = |f: &Rules| {
            (
                sorted(f.car.category_families.clone()),
                f.country_aliases.map.clone(),
                f.car.extraction_country.map.clone(),
            )
        };
        assert_eq!(want(&rebuilt), want(&file), "diff nul: same tables before and after");
        assert!(
            o.country_aliases.removed.contains("usa"),
            "a removed shipped alias is a tombstone"
        );
    }

    /// A section a pre-layer file did not have was refilled from the catalogue:
    /// not a decision, so nothing to record - and the next catalogue reaches it.
    #[test]
    fn a_missing_section_migrates_to_no_overlay() {
        let catalog = default_rules();
        let mut file = catalog.clone();
        file.car.category_families.clear();
        file.country_aliases.map.clear();
        assert_eq!(
            TaxonomyOverlay::migrate(&file, &catalog).families,
            FamilyOverlay::default()
        );
        assert_eq!(
            TaxonomyOverlay::migrate(&file, &catalog).country_aliases,
            MapOverlay::default()
        );
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
        let catalog = default_rules();
        let mut o = TaxonomyOverlay::default();
        o.country_aliases.set.insert(" USA ".into(), "United States".into());
        o.families.tags.insert("#GT3".into(), "race".into());
        let n = o.normalized(&catalog);
        assert!(n.country_aliases.set.is_empty());
        assert!(n.families.tags.is_empty());
    }

    /// REGLES§13.1: a user who skipped versions must not see catalogue
    /// improvements he never received recorded as his own decisions.
    #[test]
    fn migration_compares_to_the_frozen_manifest_not_the_current_catalogue() {
        let old_copy = pre_layer();
        let mut newer_catalog = pre_layer();
        newer_catalog
            .car
            .extraction_country
            .map
            .insert("nihon".into(), "Japan".into());
        let o = TaxonomyOverlay::migrate(&old_copy, &pre_layer());
        assert_eq!(
            o,
            TaxonomyOverlay {
                format: FORMAT,
                ..Default::default()
            },
            "an untouched copy is no decision"
        );
        let mut rules = old_copy.clone();
        apply(&mut rules, &newer_catalog, &o);
        assert_eq!(
            rules.car.extraction_country.map.get("nihon").map(String::as_str),
            Some("Japan"),
            "the improvement arrives"
        );
    }

    /// A corrupt file is never overwritten by a migration.
    #[test]
    fn an_unreadable_overlay_is_left_alone() {
        let dir = crate::testutil::temp_dir("taxonomy");
        let path = dir.join("taxonomy.json");
        std::fs::write(&path, "{ not json").unwrap();
        let o = load_or_migrate(&path, &default_rules());
        assert_eq!(o, TaxonomyOverlay::default());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{ not json",
            "the user's file is untouched"
        );
    }
}
