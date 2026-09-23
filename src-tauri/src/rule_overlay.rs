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

use pitbox_catalog::rules::{ListOverlay, OrderOverlay, RulesOverlay, FORMAT};

use crate::rules::Rules;

const PRE_LAYER_RULES: &str = include_str!("../rules/manifests/pre-layer-rules.json");

/// The list rules every pre-layer version shipped — frozen, with the ids.
pub fn pre_layer_rules() -> Rules {
    serde_json::from_str(PRE_LAYER_RULES).expect("the pre-layer rules manifest must be valid")
}

/// Writes the effective list rules into `rules`: catalogue, then overlay.
pub fn apply(rules: &mut Rules, catalog: &Rules, o: &RulesOverlay) {
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
pub fn diff(edited: &Rules, catalog: &Rules) -> RulesOverlay {
    let (e, c) = (&edited.car, &catalog.car);
    let (es, cs) = (&e.extraction_specs, &c.extraction_specs);
    RulesOverlay {
        format: FORMAT,
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

    /// A corrupt overlay is never overwritten by a migration.
    #[test]
    fn an_unreadable_overlay_is_left_alone() {
        let dir = crate::testutil::temp_dir("rule-overlay");
        let path = dir.join("rules-overlay.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert!(!load_or_migrate(&path, Some(&pre_layer_rules())).has_decisions());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
    }
}
