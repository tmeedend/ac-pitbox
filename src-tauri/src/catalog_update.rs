//! A new catalogue of rules, and what it did (REGLES§6).
//!
//! The catalogue is embedded in the application, so it changes with an
//! update. At startup the embedded catalogue is compared to the one seen last
//! time (`catalog-state.json`): when it differs, the new one is **applied
//! automatically**, the library is re-classified, and a report says what
//! changed — rules added, corrected, retired, and how many mods it
//! reclassified. Never an assistant asking beforehand (REGLES§6.2): the user
//! is told after, with the real effect in front of him, and undoing costs one
//! click.
//!
//! **Undoing** ("go back to the previous catalogue", REGLES§6.4) pins the
//! previous catalogue, kept whole in the state file, until the next update.
//! One previous version only: a full history would be storage for a case that
//! does not happen. The user's overlays are never touched either way.
//!
//! **Why re-classify here at all.** The classification is computed at import
//! and STORED; before this module, only a change of `ENGINE_VERSION` triggered
//! a re-classification at startup — so an improved catalogue reached new
//! imports and never the mods already in the library.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::rules::Rules;

/// A catalogue as the application embeds it: the two JSON texts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogTexts {
    pub rules: String,
    pub taxonomy: String,
}

impl CatalogTexts {
    /// FNV-1a 64 over both texts, line endings ignored (a checkout with
    /// `autocrlf` must not look like a new catalogue).
    pub fn fingerprint(&self) -> String {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in self.rules.bytes().chain([0u8]).chain(self.taxonomy.bytes()) {
            if b == b'\r' {
                continue;
            }
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        format!("{h:016x}")
    }
}

/// The catalogue embedded in this build.
pub fn embedded() -> CatalogTexts {
    CatalogTexts {
        rules: crate::rules::DEFAULT_RULES.to_string(),
        taxonomy: crate::taxonomy::CATALOG.to_string(),
    }
}

/// The previous catalogue, when the user went back to it. Process-wide because
/// every reader of the catalogue (`rules::default_rules`, `taxonomy::catalog`)
/// must see the same one, and threading it through would reach every caller.
static PINNED: RwLock<Option<CatalogTexts>> = RwLock::new(None);

/// The catalogue in force: the pinned previous one, or the embedded one.
pub fn active() -> CatalogTexts {
    match PINNED.read() {
        Ok(p) => p.clone().unwrap_or_else(embedded),
        Err(_) => embedded(),
    }
}

fn pin(texts: Option<CatalogTexts>) {
    match PINNED.write() {
        Ok(mut p) => *p = texts,
        Err(e) => log::warn!("catalogue pin not set: {e}"),
    }
}

// --- What changed ---------------------------------------------------------------

/// One line of the report: which list, which entry, what it says now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Change {
    /// The list: `brand_fix`, `tag_merge`, `family`, `country_alias`…
    pub list: String,
    /// The entry's id, or its natural key for the taxonomy tables.
    pub key: String,
    /// What it does, in the rules' own words (tags, names) - data, not prose.
    pub label: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Changes {
    pub added: Vec<Change>,
    pub corrected: Vec<Change>,
    pub retired: Vec<Change>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.corrected.is_empty() && self.retired.is_empty()
    }
}

fn list_changes<T: pitbox_catalog::rules::Rule>(
    list: &str,
    old: &[T],
    new: &[T],
    label: impl Fn(&T) -> String,
    out: &mut Changes,
) {
    use pitbox_catalog::rules::content;
    let key = |r: &T| r.id().unwrap_or("").to_string();
    for n in new {
        match old.iter().find(|o| o.id().is_some() && o.id() == n.id()) {
            None => out.added.push(Change {
                list: list.into(),
                key: key(n),
                label: label(n),
            }),
            Some(o) if content(o) != content(n) => out.corrected.push(Change {
                list: list.into(),
                key: key(n),
                label: label(n),
            }),
            _ => {}
        }
    }
    for o in old.iter().filter(|o| !new.iter().any(|n| n.id() == o.id())) {
        out.retired.push(Change {
            list: list.into(),
            key: key(o),
            label: label(o),
        });
    }
}

fn map_changes(list: &str, old: &BTreeMap<String, String>, new: &BTreeMap<String, String>, out: &mut Changes) {
    for (k, v) in new {
        let c = Change {
            list: list.into(),
            key: k.clone(),
            label: format!("{k} → {v}"),
        };
        match old.get(k) {
            None => out.added.push(c),
            Some(o) if o != v => out.corrected.push(c),
            _ => {}
        }
    }
    for (k, v) in old.iter().filter(|(k, _)| !new.contains_key(*k)) {
        out.retired.push(Change {
            list: list.into(),
            key: k.clone(),
            label: format!("{k} → {v}"),
        });
    }
}

/// Everything that differs between two catalogues, entry by entry - by id for
/// the list rules, by natural key for the taxonomy tables (a family's tags
/// are reported one by one: `race: + lmgt3`).
pub fn changes(old: &Rules, new: &Rules) -> Changes {
    let mut out = Changes::default();
    let (o, n) = (&old.car, &new.car);
    let join = |v: &[String]| v.join(", ");
    list_changes(
        "brand_fix",
        &o.brand_fix,
        &n.brand_fix,
        |r| format!("{} → {}", r.name_contains, r.set_brand),
        &mut out,
    );
    list_changes(
        "name_to_tag",
        &o.name_to_tag,
        &n.name_to_tag,
        |r| format!("{} → {}", r.name_contains, join(&r.add)),
        &mut out,
    );
    list_changes(
        "class_fix",
        &o.class_fix,
        &n.class_fix,
        |r| format!("{} → {}", join(&r.from), r.set_class.clone().unwrap_or_default()),
        &mut out,
    );
    list_changes(
        "tag_merge",
        &o.tag_merge,
        &n.tag_merge,
        |r| format!("{} → {}", join(&r.from), join(&r.to)),
        &mut out,
    );
    let (os, ns) = (&o.extraction_specs, &n.extraction_specs);
    for (list, a, b) in [
        ("drivetrain", &os.drivetrain, &ns.drivetrain),
        ("aspiration", &os.aspiration, &ns.aspiration),
        ("engine_config", &os.engine_config, &ns.engine_config),
        ("engine_pos", &os.engine_pos, &ns.engine_pos),
        ("gearbox", &os.gearbox, &ns.gearbox),
    ] {
        list_changes(list, a, b, |r| format!("{} → {}", join(&r.from), r.set), &mut out);
    }
    list_changes(
        "track_tag_merge",
        &old.track.tag_merge,
        &new.track.tag_merge,
        |r| format!("{} → {}", join(&r.from), join(&r.to)),
        &mut out,
    );
    let (oa, na) = (&old.track.category_allowlist, &new.track.category_allowlist);
    for c in na.iter().filter(|c| !oa.contains(c)) {
        out.added.push(Change {
            list: "track_category".into(),
            key: c.clone(),
            label: c.clone(),
        });
    }
    for c in oa.iter().filter(|c| !na.contains(c)) {
        out.retired.push(Change {
            list: "track_category".into(),
            key: c.clone(),
            label: c.clone(),
        });
    }
    // Families: tag by tag, the natural key of that table.
    let (of, nf) = (
        pitbox_catalog::taxonomy::lookup(&o.category_families),
        pitbox_catalog::taxonomy::lookup(&n.category_families),
    );
    let tags: BTreeSet<&String> = of.keys().chain(nf.keys()).collect();
    for t in tags {
        match (of.get(t), nf.get(t)) {
            (None, Some(f)) => out.added.push(Change {
                list: "family".into(),
                key: t.clone(),
                label: format!("{f}: + {t}"),
            }),
            (Some(f), None) => out.retired.push(Change {
                list: "family".into(),
                key: t.clone(),
                label: format!("{f}: − {t}"),
            }),
            (Some(a), Some(b)) if a != b => out.corrected.push(Change {
                list: "family".into(),
                key: t.clone(),
                label: format!("{a} → {b}: {t}"),
            }),
            _ => {}
        }
    }
    map_changes(
        "country_alias",
        &old.country_aliases.map,
        &new.country_aliases.map,
        &mut out,
    );
    map_changes(
        "country_tag",
        &o.extraction_country.map,
        &n.extraction_country.map,
        &mut out,
    );
    out
}

// --- The state file ---------------------------------------------------------------

/// The report of the last catalogue update, until the user closes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// Application versions: the catalogue is embedded, so that is its name.
    pub from_version: String,
    pub to_version: String,
    pub changes: Changes,
    /// Mods whose classification the update changed (REGLES§6.3: the measured
    /// effect, not the rule alone).
    pub reclassified: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Seen {
    pub app_version: String,
    pub texts: CatalogTexts,
}

/// `catalog-state.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CatalogState {
    /// The embedded catalogue last seen at startup.
    #[serde(default)]
    pub current: Option<Seen>,
    /// The one before it - what "go back" restores.
    #[serde(default)]
    pub previous: Option<Seen>,
    /// The user went back to `previous`.
    #[serde(default)]
    pub reverted: bool,
    #[serde(default)]
    pub report: Option<Report>,
    /// The report was closed. Kept rather than deleted: "go back" stays
    /// available after the banner is gone (REGLES§6.4: until the next update).
    #[serde(default)]
    pub report_dismissed: bool,
}

pub fn load_state(dir: &Path) -> CatalogState {
    let path = dir.join("catalog-state.json");
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
            log::warn!("{} unreadable, catalogue history ignored: {e}", path.display());
            CatalogState::default()
        }),
        Err(_) => CatalogState::default(),
    }
}

pub fn save_state(dir: &Path, s: &CatalogState) -> Result<(), String> {
    let json = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("catalog-state.json"), json).map_err(|e| e.to_string())
}

/// What startup has to do, decided from the state alone - the testable half.
#[derive(Debug, PartialEq)]
pub enum Startup {
    /// First launch with a catalogue history: remember it, nothing to report.
    FirstSeen,
    /// Same catalogue as last time.
    Unchanged,
    /// A new embedded catalogue; the classification before it is computed
    /// under this one - the pinned previous one if the user had gone back.
    Updated { before: CatalogTexts },
}

pub fn decide(state: &CatalogState, embedded: &CatalogTexts) -> Startup {
    match &state.current {
        None => Startup::FirstSeen,
        Some(c) if c.texts.fingerprint() == embedded.fingerprint() => Startup::Unchanged,
        Some(c) => Startup::Updated {
            before: match (&state.previous, state.reverted) {
                (Some(p), true) => p.texts.clone(),
                _ => c.texts.clone(),
            },
        },
    }
}

/// The rules a catalogue gives once the user's overlays are applied - built
/// from texts, so a previous catalogue can be classified with. `None` when the
/// texts no longer parse (a catalogue from a build whose rule shapes differed).
pub fn rules_with(dir: &Path, texts: &CatalogTexts) -> Option<Rules> {
    crate::rules::catalog_from(texts).map(|c| crate::rules::load_from_dir_on(dir, &c))
}

/// Startup: detect a new catalogue, re-classify the library under it, write
/// the report. Also re-installs the pin of a user who had gone back. Runs
/// before anything reads the rules.
///
/// Without a reachable library (an external disk not mounted) an update is
/// left for the next start rather than recorded: measured on an empty
/// library, it would report nothing and re-classify nothing, and be marked
/// done for good.
pub fn on_startup(dir: &Path, conn: &rusqlite::Connection, cfg: &crate::config::AppConfig, library_ready: bool) {
    let mut state = load_state(dir);
    let embedded = embedded();
    let version = env!("CARGO_PKG_VERSION").to_string();
    let decision = decide(&state, &embedded);
    if !library_ready && matches!(decision, Startup::Updated { .. }) {
        log::warn!("catalogue update postponed: library not reachable");
        return;
    }
    match decision {
        Startup::FirstSeen => {
            state.current = Some(Seen {
                app_version: version,
                texts: embedded,
            });
        }
        Startup::Unchanged => {
            if state.reverted {
                if let Some(p) = &state.previous {
                    pin(Some(p.texts.clone()));
                }
            }
            return;
        }
        Startup::Updated { before } => {
            let from_version = if state.reverted {
                state.previous.as_ref().map(|p| p.app_version.clone())
            } else {
                state.current.as_ref().map(|c| c.app_version.clone())
            }
            .unwrap_or_default();
            let old_rules = rules_with(dir, &before);
            pin(None);
            let new_rules = crate::rules::load_from_dir(dir);
            let (changes, reclassified) = match &old_rules {
                Some(old) => (
                    changes(old, &new_rules),
                    reclassify(conn, cfg, old, &new_rules).unwrap_or_else(|e| {
                        log::warn!("catalogue update: classification not compared: {e}");
                        Vec::new()
                    }),
                ),
                None => {
                    log::warn!("catalogue update: previous catalogue unreadable, no report");
                    (Changes::default(), Vec::new())
                }
            };
            if let Err(e) = crate::harmonize::harmonize_all(conn, cfg, &new_rules) {
                log::warn!("catalogue update: re-classification failed: {e}");
            }
            state.previous = Some(Seen {
                app_version: from_version.clone(),
                texts: before,
            });
            state.current = Some(Seen {
                app_version: version.clone(),
                texts: embedded,
            });
            state.reverted = false;
            state.report_dismissed = false;
            state.report = (!changes.is_empty() || !reclassified.is_empty()).then_some(Report {
                from_version,
                to_version: version,
                changes,
                reclassified,
            });
        }
    }
    if let Err(e) = save_state(dir, &state) {
        log::warn!("catalogue history not written: {e}");
    }
}

/// The mods two rule sets classify differently - measured on the library,
/// without writing anything (`harmonize::snapshot`).
fn reclassify(
    conn: &rusqlite::Connection,
    cfg: &crate::config::AppConfig,
    old: &Rules,
    new: &Rules,
) -> rusqlite::Result<Vec<String>> {
    let a = crate::harmonize::snapshot(conn, cfg, old)?;
    let b = crate::harmonize::snapshot(conn, cfg, new)?;
    Ok(crate::harmonize::snapshot_diff(&a, &b))
}

/// "Go back to the previous catalogue" (`true`), or return to the current one
/// (`false`). Re-classifies the library under the catalogue now in force.
/// Returns the number of mods processed.
pub fn set_reverted(
    dir: &Path,
    conn: &rusqlite::Connection,
    cfg: &crate::config::AppConfig,
    reverted: bool,
) -> Result<usize, String> {
    let mut state = load_state(dir);
    let previous = state.previous.clone().ok_or(crate::errors::NO_PREVIOUS_CATALOG)?;
    pin(reverted.then_some(previous.texts));
    state.reverted = reverted;
    save_state(dir, &state)?;
    let rules = crate::rules::load_from_dir(dir);
    crate::harmonize::harmonize_all(conn, cfg, &rules).map_err(|e| e.to_string())
}

pub fn dismiss_report(dir: &Path) -> Result<(), String> {
    let mut state = load_state(dir);
    state.report_dismissed = true;
    save_state(dir, &state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::default_rules;

    fn texts(rules: &str) -> CatalogTexts {
        CatalogTexts {
            rules: rules.into(),
            taxonomy: "{}".into(),
        }
    }

    /// REGLES§6.1: an added rule, a corrected one, a retired one - each told
    /// apart by its id; the taxonomy tables by their natural key.
    #[test]
    fn a_new_catalogue_is_described_entry_by_entry() {
        let old = default_rules();
        let mut new = old.clone();
        new.car.brand_fix[0].set_brand = "BMW M".into(); // corrected, same id
        let retired = new.car.brand_fix.remove(1);
        new.car.tag_merge.push(pitbox_catalog::rules::TagMerge {
            id: Some("pitbox.car-merge.lmgt3".into()),
            from: vec!["lmgt3".into()],
            to: vec!["#gt3".into()],
        });
        new.car.category_families[2].tags.push("lmgt3".into());
        new.country_aliases.map.insert("nippon".into(), "Japan".into());
        let c = changes(&old, &new);
        assert_eq!(c.corrected.iter().filter(|x| x.list == "brand_fix").count(), 1);
        assert_eq!(c.retired[0].key, retired.id.unwrap(), "retired by its id");
        assert!(c.added.iter().any(|x| x.key == "pitbox.car-merge.lmgt3"));
        assert!(c.added.iter().any(|x| x.list == "family" && x.key == "lmgt3"));
        assert!(c.added.iter().any(|x| x.list == "country_alias" && x.key == "nippon"));
        assert!(changes(&old, &old).is_empty(), "same catalogue, nothing to say");
    }

    #[test]
    fn startup_tells_a_first_run_from_an_update_and_an_unchanged_catalogue() {
        let (a, b) = (texts("{\"a\":1}"), texts("{\"a\":2}"));
        let mut s = CatalogState::default();
        assert_eq!(decide(&s, &a), Startup::FirstSeen);
        s.current = Some(Seen {
            app_version: "0.7.0".into(),
            texts: a.clone(),
        });
        assert_eq!(decide(&s, &a), Startup::Unchanged);
        assert_eq!(decide(&s, &b), Startup::Updated { before: a.clone() });
        // A user who had gone back is compared from where he actually is.
        s.previous = Some(Seen {
            app_version: "0.6.0".into(),
            texts: b.clone(),
        });
        s.reverted = true;
        let c = texts("{\"a\":3}");
        assert_eq!(decide(&s, &c), Startup::Updated { before: b });
    }

    /// The whole startup path on a real library: the previous catalogue had no
    /// drivetrain extraction, the new one has - the car is re-classified, the
    /// report names the rules and the mod, and the previous catalogue is kept
    /// for "go back".
    #[test]
    fn an_update_is_applied_measured_and_reported() {
        let base = crate::testutil::temp_dir("catalog-update");
        let lib = base.join("lib");
        let dir = lib.join("cars").join("rss_car").join("v");
        std::fs::create_dir_all(dir.join("ui")).unwrap();
        std::fs::write(
            dir.join("ui").join("ui_car.json"),
            r#"{"name":"RSS Formula","class":"race","tags":["singleseater","rwd"]}"#,
        )
        .unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        crate::overlay::upsert_mod(&conn, "rss_car", "Car", None, Some("RSS Formula"), "h", None, &now).unwrap();
        crate::overlay::insert_version(
            &conn,
            "rss_car_v",
            "rss_car",
            Some("1.0"),
            None,
            &now,
            &dir.to_string_lossy(),
            None,
            "sig",
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        crate::overlay::set_active_version(&conn, "rss_car", "rss_car_v").unwrap();
        let cfg = crate::config::AppConfig {
            library_path: Some(lib.clone()),
            ..Default::default()
        };
        let config = base.join("config");
        std::fs::create_dir_all(&config).unwrap();

        let mut old: Rules = serde_json::from_str(crate::rules::DEFAULT_RULES).unwrap();
        old.car.extraction_specs.drivetrain.clear();
        let old_texts = CatalogTexts {
            rules: serde_json::to_string(&old).unwrap(),
            taxonomy: crate::taxonomy::CATALOG.to_string(),
        };
        let state = CatalogState {
            current: Some(Seen {
                app_version: "0.6.0".into(),
                texts: old_texts.clone(),
            }),
            ..Default::default()
        };
        save_state(&config, &state).unwrap();

        on_startup(&config, &conn, &cfg, true);

        let s = load_state(&config);
        let report = s.report.expect("a report");
        assert_eq!(report.reclassified, vec!["rss_car".to_string()], "the measured effect");
        assert!(
            report.changes.added.iter().any(|c| c.list == "drivetrain"),
            "the rules that did it"
        );
        assert_eq!(report.from_version, "0.6.0");
        assert_eq!(s.previous.map(|p| p.texts), Some(old_texts), "kept for going back");
        assert!(!s.reverted);

        on_startup(&config, &conn, &cfg, true);
        assert_eq!(
            load_state(&config).report.map(|r| r.reclassified.len()),
            Some(1),
            "reported once, kept until closed"
        );
    }

    /// An update seen while the library is not reachable is not recorded:
    /// measured on nothing, it would be marked done for good.
    #[test]
    fn an_update_waits_for_the_library() {
        let base = crate::testutil::temp_dir("catalog-update-later");
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let state = CatalogState {
            current: Some(Seen {
                app_version: "0.6.0".into(),
                texts: texts("{}"),
            }),
            ..Default::default()
        };
        save_state(&base, &state).unwrap();
        on_startup(&base, &conn, &crate::config::AppConfig::default(), false);
        assert_eq!(load_state(&base), state, "nothing recorded");
    }

    /// A checkout converting line endings must not look like a new catalogue.
    #[test]
    fn line_endings_do_not_make_a_new_catalogue() {
        assert_eq!(
            texts("{\n\"a\": 1\n}").fingerprint(),
            texts("{\r\n\"a\": 1\r\n}").fingerprint()
        );
    }
}
