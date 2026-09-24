//! The list rules of the Rules screen as catalogue + overlay (REGLES§2), on the
//! application's `Rules`.
//!
//! The merge itself lives in `pitbox_catalog::rules` (shared with
//! `rules-tool`); this module maps it onto the sections of `Rules`, reads and
//! writes `rules-overlay.json`, and migrates, once, the `tag-rules.json` that
//! pre-layer versions copied whole.
//!
//! ## The frozen manifest
//!
//! The migration compares the user's copy to what the pre-layer versions
//! SHIPPED, never to the current catalogue (REGLES§13.1): otherwise a user
//! skipping versions would see improvements he never received recorded as his
//! own decisions, and frozen. Measured on the history: the list rules are
//! identical from the first commit (63e44ba) to v0.7.0 - only `remove`, a
//! blacklist the engine no longer read, went away in v0.4, and the allowlist
//! appeared in v0.1 - so one frozen file stands for every version:
//! `rules/manifests/pre-layer-rules.json`. It carries the catalogue's ids, and
//! that is how a copied rule is tied back to the shipped rule it was.

use std::path::Path;

use pitbox_catalog::rules::{
    BrandFix, ClassFix, ListOverlay, NameToTag, OrderOverlay, Row, Rule, RulesOverlay, SetRule, TagMerge, FORMAT,
};
use serde::Serialize;

use crate::harmonize::Effects;
use crate::rules::Rules;

const PRE_LAYER_RULES: &str = include_str!("../rules/manifests/pre-layer-rules.json");

/// The list rules every pre-layer version shipped — frozen, with the ids.
pub fn pre_layer_rules() -> Rules {
    serde_json::from_str(PRE_LAYER_RULES).expect("the pre-layer rules manifest must be valid")
}

/// The catalogue with its list rules emptied: what applies when the global
/// switch is off (REGLES§7). The user's rules go on, and his forks with them -
/// against an empty catalogue they stand as his own, like the fork of a
/// retired rule. The taxonomy tables are not rules (REGLES§11) and stay.
fn switched_off(catalog: &Rules) -> Rules {
    let mut r = catalog.clone();
    let c = &mut r.car;
    c.extraction_specs.drivetrain.clear();
    c.extraction_specs.aspiration.clear();
    c.extraction_specs.engine_config.clear();
    c.extraction_specs.engine_pos.clear();
    c.extraction_specs.gearbox.clear();
    c.brand_fix.clear();
    c.name_to_tag.clear();
    c.class_fix.clear();
    c.tag_merge.clear();
    r.track.tag_merge.clear();
    r.track.category_allowlist.clear();
    r
}

/// Writes the effective list rules into `rules`: catalogue, then overlay.
pub fn apply(rules: &mut Rules, catalog: &Rules, o: &RulesOverlay) {
    let off;
    let catalog = if o.catalog_off {
        off = switched_off(catalog);
        &off
    } else {
        catalog
    };
    let (c, t) = (&catalog.car, &catalog.track);
    let (cs, s) = (&c.extraction_specs, &mut rules.car.extraction_specs);
    rules.car.brand_fix = o.brand_fix.apply(&c.brand_fix);
    rules.car.name_to_tag = o.name_to_tag.apply(&c.name_to_tag);
    rules.car.class_fix = o.class_fix.apply(&c.class_fix);
    rules.car.tag_merge = o.car_tag_merge.apply(&c.tag_merge);
    s.drivetrain = o.drivetrain.apply(&cs.drivetrain);
    s.aspiration = o.aspiration.apply(&cs.aspiration);
    s.engine_config = o.engine_config.apply(&cs.engine_config);
    s.engine_pos = o.engine_pos.apply(&cs.engine_pos);
    s.gearbox = o.gearbox.apply(&cs.gearbox);
    rules.track.tag_merge = o.track_tag_merge.apply(&t.tag_merge);
    rules.track.category_allowlist = o.track_categories.apply(&t.category_allowlist);
}

/// The decisions that turn `catalog` into `edited` — what the Rules screen
/// saves (it still edits whole lists; the ids riding on each rule are what
/// tell a disabled or forked shipped rule from a rule of the user's).
/// Every rule of the user given an id, forks given their origin, forks that
/// restate their catalogue rule dropped (`ListOverlay::normalized`). Done on
/// every load and every save: the ids are what the switches and the effect
/// counters hang on, and they are assigned deterministically, so an overlay
/// written before they existed gets the same ones every time.
pub fn normalized(mut o: RulesOverlay, catalog: &Rules) -> RulesOverlay {
    let (c, cs) = (&catalog.car, &catalog.car.extraction_specs);
    o.brand_fix = o.brand_fix.normalized(&c.brand_fix);
    o.name_to_tag = o.name_to_tag.normalized(&c.name_to_tag);
    o.class_fix = o.class_fix.normalized(&c.class_fix);
    o.car_tag_merge = o.car_tag_merge.normalized(&c.tag_merge);
    o.drivetrain = o.drivetrain.normalized(&cs.drivetrain);
    o.aspiration = o.aspiration.normalized(&cs.aspiration);
    o.engine_config = o.engine_config.normalized(&cs.engine_config);
    o.engine_pos = o.engine_pos.normalized(&cs.engine_pos);
    o.gearbox = o.gearbox.normalized(&cs.gearbox);
    o.track_tag_merge = o.track_tag_merge.normalized(&catalog.track.tag_merge);
    o.format = FORMAT;
    o
}

/// Switches one rule off or back on, named as a catalogue update report names
/// it (`catalog_update::Change`): its list and its key - the id, or the name of
/// a track category. How "Disable" works from the report (REGLES§6.3): an
/// ordinary switch (REGLES§5), the rule keeps receiving improvements.
pub fn set_enabled(o: &mut RulesOverlay, list: &str, key: &str, on: bool) -> Result<(), String> {
    fn flip(d: &mut std::collections::BTreeSet<String>, key: &str, on: bool) {
        if on {
            d.remove(key);
        } else {
            d.insert(key.to_string());
        }
    }
    match list {
        "brand_fix" => flip(&mut o.brand_fix.disabled, key, on),
        "name_to_tag" => flip(&mut o.name_to_tag.disabled, key, on),
        "class_fix" => flip(&mut o.class_fix.disabled, key, on),
        "tag_merge" | "car_tag_merge" => flip(&mut o.car_tag_merge.disabled, key, on),
        "drivetrain" => flip(&mut o.drivetrain.disabled, key, on),
        "aspiration" => flip(&mut o.aspiration.disabled, key, on),
        "engine_config" => flip(&mut o.engine_config.disabled, key, on),
        "engine_pos" => flip(&mut o.engine_pos.disabled, key, on),
        "gearbox" => flip(&mut o.gearbox.disabled, key, on),
        "track_tag_merge" => flip(&mut o.track_tag_merge.disabled, key, on),
        "track_category" => flip(&mut o.track_categories.removed, key, on),
        _ => return Err(format!("not a list of rules: {list}")),
    }
    Ok(())
}

/// Everything switched off, as the report names it: rule ids, and the names
/// of removed track categories (`#…`, which no id can be).
pub fn disabled_keys(o: &RulesOverlay) -> Vec<String> {
    [
        &o.brand_fix.disabled,
        &o.name_to_tag.disabled,
        &o.class_fix.disabled,
        &o.car_tag_merge.disabled,
        &o.drivetrain.disabled,
        &o.aspiration.disabled,
        &o.engine_config.disabled,
        &o.engine_pos.disabled,
        &o.gearbox.disabled,
        &o.track_tag_merge.disabled,
        &o.track_categories.removed,
    ]
    .into_iter()
    .flatten()
    .cloned()
    .collect()
}

// --- The Rules screen (REGLES§8) ---------------------------------------------

/// A row and the number of mods its rule acts on (REGLES§8.3).
#[derive(Debug, Serialize)]
pub struct RowView<T> {
    #[serde(flatten)]
    pub row: Row<T>,
    pub effect: usize,
}

/// A track category of the allowlist: an ordered name, not a rule with an id,
/// so a row of its own. `on: false` is a shipped category the user removed.
#[derive(Debug, Serialize)]
pub struct CategoryRow {
    pub name: String,
    pub shipped: bool,
    pub on: bool,
    pub effect: usize,
}

/// Everything the Rules screen shows: the overlay it edits, and each section
/// as rows in execution order with their counters.
#[derive(Debug, Serialize)]
pub struct RulesView {
    pub catalog_on: bool,
    /// The catalogue in force, named after the application that shipped it.
    pub catalog_version: String,
    /// Shipped list rules, all sections together.
    pub catalog_count: usize,
    pub overlay: RulesOverlay,
    pub brand_fix: Vec<RowView<BrandFix>>,
    pub name_to_tag: Vec<RowView<NameToTag>>,
    pub class_fix: Vec<RowView<ClassFix>>,
    pub car_tag_merge: Vec<RowView<TagMerge>>,
    pub drivetrain: Vec<RowView<SetRule>>,
    pub aspiration: Vec<RowView<SetRule>>,
    pub engine_config: Vec<RowView<SetRule>>,
    pub engine_pos: Vec<RowView<SetRule>>,
    pub gearbox: Vec<RowView<SetRule>>,
    pub track_tag_merge: Vec<RowView<TagMerge>>,
    pub track_categories: Vec<CategoryRow>,
}

/// Key of an allowlist entry in `Effects` (`rules::apply_track`).
pub fn category_key(name: &str) -> String {
    format!("track-category:{name}")
}

fn rows<T: Rule>(o: &ListOverlay<T>, catalog: &[T], effects: &Effects) -> Vec<RowView<T>> {
    o.rows(catalog)
        .into_iter()
        .map(|row| RowView {
            effect: row.rule.id().and_then(|id| effects.get(id)).copied().unwrap_or(0),
            row,
        })
        .collect()
}

/// The screen's view. The overlay must be `normalized`: the ids are what the
/// counters are looked up by. Catalogue switched off, its rows are not shown -
/// the user's rules, forks included, are what applies.
pub fn view(o: &RulesOverlay, catalog: &Rules, effects: &Effects, version: String) -> RulesView {
    let off = switched_off(catalog);
    let shown = if o.catalog_off { &off } else { catalog };
    let (c, cs) = (&shown.car, &shown.car.extraction_specs);
    let allow = &shown.track.category_allowlist;
    let mut track_categories: Vec<CategoryRow> = o
        .track_categories
        .apply(allow)
        .into_iter()
        .map(|name| CategoryRow {
            shipped: allow.contains(&name),
            on: true,
            effect: effects.get(&category_key(&name)).copied().unwrap_or(0),
            name,
        })
        .collect();
    track_categories.extend(
        allow
            .iter()
            .filter(|n| o.track_categories.removed.contains(*n))
            .map(|name| CategoryRow {
                name: name.clone(),
                shipped: true,
                on: false,
                effect: 0,
            }),
    );
    let (k, ks) = (&catalog.car, &catalog.car.extraction_specs);
    RulesView {
        catalog_on: !o.catalog_off,
        catalog_version: version,
        catalog_count: k.brand_fix.len()
            + k.name_to_tag.len()
            + k.class_fix.len()
            + k.tag_merge.len()
            + ks.drivetrain.len()
            + ks.aspiration.len()
            + ks.engine_config.len()
            + ks.engine_pos.len()
            + ks.gearbox.len()
            + catalog.track.tag_merge.len(),
        overlay: o.clone(),
        brand_fix: rows(&o.brand_fix, &c.brand_fix, effects),
        name_to_tag: rows(&o.name_to_tag, &c.name_to_tag, effects),
        class_fix: rows(&o.class_fix, &c.class_fix, effects),
        car_tag_merge: rows(&o.car_tag_merge, &c.tag_merge, effects),
        drivetrain: rows(&o.drivetrain, &cs.drivetrain, effects),
        aspiration: rows(&o.aspiration, &cs.aspiration, effects),
        engine_config: rows(&o.engine_config, &cs.engine_config, effects),
        engine_pos: rows(&o.engine_pos, &cs.engine_pos, effects),
        gearbox: rows(&o.gearbox, &cs.gearbox, effects),
        track_tag_merge: rows(&o.track_tag_merge, &shown.track.tag_merge, effects),
        track_categories,
    }
}

/// Builds the overlay from a whole edited rule set - how the pre-overlay
/// screen saved, kept for the migration tests that prove the round trip.
#[cfg(test)]
pub fn diff(edited: &Rules, catalog: &Rules) -> RulesOverlay {
    let (e, c) = (&edited.car, &catalog.car);
    let (es, cs) = (&e.extraction_specs, &c.extraction_specs);
    RulesOverlay {
        format: FORMAT,
        catalog_off: false,
        brand_fix: ListOverlay::diff(&e.brand_fix, &c.brand_fix),
        name_to_tag: ListOverlay::diff(&e.name_to_tag, &c.name_to_tag),
        class_fix: ListOverlay::diff(&e.class_fix, &c.class_fix),
        car_tag_merge: ListOverlay::diff(&e.tag_merge, &c.tag_merge),
        drivetrain: ListOverlay::diff(&es.drivetrain, &cs.drivetrain),
        aspiration: ListOverlay::diff(&es.aspiration, &cs.aspiration),
        engine_config: ListOverlay::diff(&es.engine_config, &cs.engine_config),
        engine_pos: ListOverlay::diff(&es.engine_pos, &cs.engine_pos),
        gearbox: ListOverlay::diff(&es.gearbox, &cs.gearbox),
        track_tag_merge: ListOverlay::diff(&edited.track.tag_merge, &catalog.track.tag_merge),
        track_categories: OrderOverlay::diff(&edited.track.category_allowlist, &catalog.track.category_allowlist),
    }
}

/// `ListOverlay::migrate`, except that a section the file does not have - or
/// left empty - holds no decision: it was refilled from the embedded rules on
/// each load (the allowlist of a pre-v0.1 file).
fn migrate_list<T: pitbox_catalog::rules::Rule>(file: &[T], manifest: &[T]) -> ListOverlay<T> {
    if file.is_empty() {
        ListOverlay::default()
    } else {
        ListOverlay::migrate(file, manifest)
    }
}

/// The overlay reproducing a pre-layer `tag-rules.json` (REGLES§13.2),
/// against the frozen manifest.
pub fn migrate(file: &Rules, manifest: &Rules) -> RulesOverlay {
    let (f, m) = (&file.car, &manifest.car);
    let (fs, ms) = (&f.extraction_specs, &m.extraction_specs);
    let allowlist = &file.track.category_allowlist;
    RulesOverlay {
        format: FORMAT,
        catalog_off: false,
        brand_fix: migrate_list(&f.brand_fix, &m.brand_fix),
        name_to_tag: migrate_list(&f.name_to_tag, &m.name_to_tag),
        class_fix: migrate_list(&f.class_fix, &m.class_fix),
        car_tag_merge: migrate_list(&f.tag_merge, &m.tag_merge),
        drivetrain: migrate_list(&fs.drivetrain, &ms.drivetrain),
        aspiration: migrate_list(&fs.aspiration, &ms.aspiration),
        engine_config: migrate_list(&fs.engine_config, &ms.engine_config),
        engine_pos: migrate_list(&fs.engine_pos, &ms.engine_pos),
        gearbox: migrate_list(&fs.gearbox, &ms.gearbox),
        track_tag_merge: migrate_list(&file.track.tag_merge, &manifest.track.tag_merge),
        track_categories: if allowlist.is_empty() {
            OrderOverlay::default()
        } else {
            OrderOverlay::diff(allowlist, &manifest.track.category_allowlist)
        },
    }
}

/// Reads `rules-overlay.json`, or builds it from the pre-layer file the first
/// time (then writes it, so the migration happens once). No file at all - a
/// fresh install - is no decision.
///
/// **A file that exists but does not parse is never overwritten**: an empty
/// overlay is used for this session and the failure is logged. Rewriting it
/// would erase the user's decisions for the sake of a clean start.
pub fn load_or_migrate(path: &Path, legacy: Option<&Rules>) -> RulesOverlay {
    match std::fs::read_to_string(path) {
        Ok(s) => match serde_json::from_str::<RulesOverlay>(&s) {
            Ok(o) => o,
            Err(e) => {
                log::warn!("{} unreadable, rules overlay ignored this session: {e}", path.display());
                RulesOverlay::default()
            }
        },
        Err(_) => {
            let o = match legacy {
                Some(file) => migrate(file, &pre_layer_rules()),
                None => RulesOverlay {
                    format: FORMAT,
                    ..Default::default()
                },
            };
            if let Err(e) = save(path, &o) {
                log::warn!("rules overlay migration not written, will run again: {e}");
            }
            o
        }
    }
}

pub fn save(path: &Path, o: &RulesOverlay) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(o).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{default_rules, BrandFix};
    use pitbox_catalog::rules::content;

    fn lists(r: &Rules) -> Vec<Vec<String>> {
        let (c, s) = (&r.car, &r.car.extraction_specs);
        let v = |x: Vec<String>| x;
        vec![
            v(c.brand_fix.iter().map(content).collect()),
            v(c.name_to_tag.iter().map(content).collect()),
            v(c.class_fix.iter().map(content).collect()),
            v(c.tag_merge.iter().map(content).collect()),
            v(s.drivetrain.iter().map(content).collect()),
            v(s.aspiration.iter().map(content).collect()),
            v(s.engine_config.iter().map(content).collect()),
            v(s.engine_pos.iter().map(content).collect()),
            v(s.gearbox.iter().map(content).collect()),
            v(r.track.tag_merge.iter().map(content).collect()),
            r.track.category_allowlist.clone(),
        ]
    }

    /// Stripped of their ids, as a pre-layer version copied them.
    fn as_copied(mut r: Rules) -> Rules {
        for x in &mut r.car.brand_fix {
            x.id = None;
        }
        for x in &mut r.car.name_to_tag {
            x.id = None;
        }
        for x in &mut r.car.class_fix {
            x.id = None;
        }
        for x in r.car.tag_merge.iter_mut().chain(r.track.tag_merge.iter_mut()) {
            x.id = None;
        }
        let s = &mut r.car.extraction_specs;
        for l in [
            &mut s.drivetrain,
            &mut s.aspiration,
            &mut s.engine_config,
            &mut s.engine_pos,
            &mut s.gearbox,
        ] {
            for x in l.iter_mut() {
                x.id = None;
            }
        }
        r
    }

    /// Every install measured so far holds an untouched copy: its migration
    /// must be no decision at all.
    #[test]
    fn an_untouched_copy_migrates_to_no_decision() {
        let o = migrate(&as_copied(pre_layer_rules()), &pre_layer_rules());
        assert!(!o.has_decisions(), "{o:?}");
    }

    /// The manifest and the catalogue name their rules the same way - that is
    /// the only link between a copied rule and the shipped one.
    #[test]
    fn the_manifest_carries_the_catalogue_ids() {
        let (m, c) = (pre_layer_rules(), default_rules());
        for r in &m.car.tag_merge {
            assert!(
                c.car.tag_merge.iter().any(|x| x.id == r.id),
                "manifest id {:?} unknown to the catalogue",
                r.id
            );
        }
        assert!(
            m.car.brand_fix.iter().all(|r| r.id.is_some()),
            "every manifest rule has an id"
        );
    }

    /// REGLES§13.5: the migration classifies exactly as the file did - with a
    /// deletion, a rule of the user's in front (where the screen puts it) and a
    /// reordered allowlist.
    #[test]
    fn migrating_a_file_with_decisions_reproduces_it() {
        let mut file = as_copied(pre_layer_rules());
        file.car.brand_fix.remove(1);
        file.car.brand_fix.insert(
            0,
            BrandFix {
                id: None,
                name_contains: "lanzo".into(),
                set_brand: "RSS".into(),
            },
        );
        file.car.tag_merge.remove(3);
        file.track.category_allowlist.swap(0, 2);
        let o = migrate(&file, &pre_layer_rules());
        let mut rebuilt = default_rules();
        apply(&mut rebuilt, &default_rules(), &o);
        assert_eq!(lists(&rebuilt), lists(&file), "diff nul");
    }

    /// What the Rules screen saves comes back as saved.
    #[test]
    fn a_saved_screen_applies_back_to_itself() {
        let mut edited = default_rules();
        edited.car.class_fix.remove(0);
        edited.car.brand_fix[0].set_brand = "Bayerische".into();
        edited.track.category_allowlist.push("#karting".into());
        let o = diff(&edited, &default_rules());
        assert_eq!(o.brand_fix.forks.len(), 1);
        assert_eq!(o.class_fix.disabled.len(), 1);
        let mut rebuilt = default_rules();
        apply(&mut rebuilt, &default_rules(), &o);
        assert_eq!(lists(&rebuilt), lists(&edited));
    }

    /// The legacy file is set aside once both overlays are written - and a copy
    /// that reappears later is set aside again, next to the first, never over
    /// it (seen on the dev machine, from an unknown writer).
    #[test]
    fn the_legacy_file_is_set_aside_and_never_over_an_earlier_one() {
        let dir = crate::testutil::temp_dir("legacy-retire");
        let text = serde_json::to_string(&as_copied(pre_layer_rules())).unwrap();
        std::fs::write(dir.join("tag-rules.json"), &text).unwrap();
        crate::rules::load_from_dir(&dir);
        assert!(dir.join("rules-overlay.json").is_file() && dir.join("taxonomy.json").is_file());
        assert!(!dir.join("tag-rules.json").exists(), "set aside");
        assert!(dir.join("tag-rules.pre-overlay.json").is_file());

        std::fs::write(dir.join("tag-rules.json"), "{\"car\":{}}").unwrap();
        crate::rules::load_from_dir(&dir);
        assert_eq!(
            std::fs::read_to_string(dir.join("tag-rules.pre-overlay.json")).unwrap(),
            text,
            "the first copy is untouched"
        );
        assert!(
            dir.join("tag-rules.pre-overlay-2.json").is_file(),
            "the second goes next to it"
        );
    }

    /// A corrupt overlay is never overwritten by a migration.
    #[test]
    fn an_unreadable_overlay_is_left_alone() {
        let dir = crate::testutil::temp_dir("rule-overlay");
        let path = dir.join("rules-overlay.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert!(!load_or_migrate(&path, Some(&pre_layer_rules())).has_decisions());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
    }

    /// REGLES§7: the global switch off, the catalogue stops applying - all of
    /// it - and the user's rules go on, his fork of a shipped rule included.
    /// The screen then shows his rules only.
    #[test]
    fn switching_the_catalogue_off_leaves_the_users_rules_alone() {
        let catalog = default_rules();
        let mut o = RulesOverlay::default();
        o.brand_fix.own.push(BrandFix {
            id: None,
            name_contains: "lanzo".into(),
            set_brand: "RSS".into(),
        });
        let mut fork = catalog.car.brand_fix[0].clone();
        fork.set_brand = "Bayro Motors".into();
        o.brand_fix.forks.insert(
            fork.id.clone().unwrap(),
            pitbox_catalog::rules::Fork {
                rule: fork,
                forked_from: String::new(),
            },
        );
        let o = normalized(o, &catalog);
        assert!(
            o.brand_fix.forks.values().all(|f| !f.forked_from.is_empty()),
            "a fork made by the screen is given its origin"
        );
        assert_eq!(o.brand_fix.own[0].id.as_deref(), Some("own-1"), "his rule has an id");

        let off = RulesOverlay {
            catalog_off: true,
            ..o.clone()
        };
        let mut rules = catalog.clone();
        apply(&mut rules, &catalog, &off);
        let brands: Vec<&str> = rules.car.brand_fix.iter().map(|r| r.set_brand.as_str()).collect();
        assert_eq!(
            brands,
            ["RSS", "Bayro Motors"],
            "his rule and his fork, nothing shipped"
        );
        assert!(rules.car.tag_merge.is_empty() && rules.track.category_allowlist.is_empty());
        assert!(
            !rules.car.category_families.is_empty(),
            "the taxonomy tables are not rules"
        );

        let v = view(&off, &catalog, &Effects::new(), "0.7.0".into());
        assert!(!v.catalog_on);
        assert_eq!(v.brand_fix.len(), 2, "no shipped row while the catalogue is off");
        assert!(v.catalog_count > 100, "the switch line still counts the catalogue");
    }

    /// REGLES§8.1, §8.3: one list per section in execution order - his rules
    /// first - each row with the mods its rule acted on, `0` when none.
    #[test]
    fn the_view_lists_rows_in_execution_order_with_their_effect() {
        let catalog = default_rules();
        let mut o = RulesOverlay::default();
        o.brand_fix.own.push(BrandFix {
            id: None,
            name_contains: "lanzo".into(),
            set_brand: "RSS".into(),
        });
        o.brand_fix.disabled.insert("pitbox.brand.auriel".into());
        o.track_categories.removed.insert("#rally".into());
        let o = normalized(o, &catalog);
        let mut effects = Effects::new();
        effects.insert("pitbox.brand.bayro".into(), 3);
        effects.insert(category_key("#hillclimb"), 2);
        let v = view(&o, &catalog, &effects, "0.7.0".into());
        assert_eq!(
            v.brand_fix[0].row.origin,
            pitbox_catalog::rules::Origin::Own,
            "his rule runs first"
        );
        assert_eq!(
            v.brand_fix.len(),
            catalog.car.brand_fix.len() + 1,
            "disabled rows are shown"
        );
        let row = |id: &str| {
            v.brand_fix
                .iter()
                .find(|r| r.row.rule.id.as_deref() == Some(id))
                .unwrap()
        };
        assert_eq!(row("pitbox.brand.bayro").effect, 3);
        assert!(row("pitbox.brand.auriel").row.disabled);
        let rally = v.track_categories.iter().find(|c| c.name == "#rally").unwrap();
        assert!(
            rally.shipped && !rally.on,
            "a removed shipped category stays, switched off"
        );
        let hill = v.track_categories.iter().find(|c| c.name == "#hillclimb").unwrap();
        assert_eq!(hill.effect, 2);
    }

    /// REGLES§6.3: "Disable" on a report line is the ordinary switch, found
    /// by the names the report uses - `tag_merge` is the cars' list there.
    #[test]
    fn a_report_line_switches_its_rule() {
        let mut o = RulesOverlay::default();
        set_enabled(&mut o, "tag_merge", "pitbox.car-merge.gt1", false).unwrap();
        set_enabled(&mut o, "track_category", "#rally", false).unwrap();
        assert!(o.car_tag_merge.disabled.contains("pitbox.car-merge.gt1"));
        assert_eq!(disabled_keys(&o), ["pitbox.car-merge.gt1", "#rally"]);
        set_enabled(&mut o, "tag_merge", "pitbox.car-merge.gt1", true).unwrap();
        assert!(o.car_tag_merge.disabled.is_empty(), "switched back on");
        assert!(
            set_enabled(&mut o, "family", "gt3", false).is_err(),
            "a family is not a rule"
        );
    }
}
