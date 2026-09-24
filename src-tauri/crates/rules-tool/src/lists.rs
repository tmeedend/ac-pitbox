//! The list rules' half of `promote` (REGLES§4): the decisions of
//! `rules-overlay.json` written into `default-tag-rules.json`.
//!
//! | Decision in the overlay        | Becomes in the catalogue                      |
//! |--------------------------------|-----------------------------------------------|
//! | a fork of a shipped rule       | that rule's new content, **same id**          |
//! | a rule of the user's own       | a shipped rule with a **new id**, run first   |
//! | a shipped rule switched off    | removed, its id listed in `retired`           |
//! | a user rule switched off       | not promoted, left in the overlay             |
//!
//! Why each: a corrected rule keeps its id, so a user who had it disabled
//! keeps it disabled (REGLES§4); the user's rules ran before the catalogue's,
//! so they go first to classify exactly as before; a retired id is never
//! handed out again, because some user's overlay may still name it.
//!
//! The file is read into typed structs, not a `serde_json::Value`: without
//! `preserve_order` a `Value` sorts its keys, and turning that feature on here
//! would turn it on for the whole workspace build - where `content()`, hence
//! every fork's fingerprint, depends on key order. `_meta` passes through as
//! raw text.

use std::collections::BTreeSet;

use pitbox_catalog::rules::{
    content, BrandFix, ClassFix, ListOverlay, NameToTag, Rule, RulesOverlay, SetRule, TagMerge, FORMAT,
};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesFile {
    #[serde(rename = "_meta")]
    meta: Box<RawValue>,
    car: CarLists,
    track: TrackLists,
    /// Ids that left the catalogue (REGLES§4). Kept so no later rule is given
    /// one of them: an overlay somewhere may still switch it off.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    retired: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CarLists {
    brand_fix: Vec<BrandFix>,
    name_to_tag: Vec<NameToTag>,
    class_fix: Vec<ClassFix>,
    tag_merge: Vec<TagMerge>,
    extraction_specs: Specs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Specs {
    #[serde(rename = "_note", default, skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    drivetrain: Vec<SetRule>,
    aspiration: Vec<SetRule>,
    engine_config: Vec<SetRule>,
    engine_pos: Vec<SetRule>,
    gearbox: Vec<SetRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrackLists {
    tag_merge: Vec<TagMerge>,
    category_allowlist: Vec<String>,
}

/// Reads the catalogue file. Line endings are made `\n` first: a Windows
/// checkout turns them into `\r\n`, and `_meta`, passed through as raw text,
/// would keep them - a promotion then wrote a file with mixed endings, and
/// the no-change test caught it on the dev machine.
pub fn read(path: &std::path::Path) -> Result<RulesFile, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text.replace("\r\n", "\n")).map_err(|e| format!("{}: {e}", path.display()))
}

/// The catalogue as committed: pretty JSON, final newline.
pub fn render(f: &RulesFile) -> String {
    format!("{}\n", serde_json::to_string_pretty(f).expect("rules serialise"))
}

/// A promotion: the new catalogue, what is left in the overlay (the user's
/// rules he switched off), and the changes in words.
pub struct Promotion {
    pub file: RulesFile,
    pub left: RulesOverlay,
    pub changes: Vec<String>,
}

/// `pitbox.brand.bayro` → `pitbox.brand.`; the section's convention, read off
/// its first id.
fn prefix_of<T: Rule>(list: &[T], fallback: &str) -> String {
    list.iter()
        .find_map(|r| r.id().and_then(|id| id.rsplit_once('.')).map(|(p, _)| format!("{p}.")))
        .unwrap_or_else(|| format!("pitbox.{fallback}."))
}

/// What a new id is made of: what the rule looks for (`name_contains`, or its
/// first `from` tag). The id is then written in the catalogue once and never
/// recomputed - it only has to be readable the day it is born.
fn slug<T: Rule>(r: &T) -> String {
    let v = serde_json::to_value(r).expect("rule serialises");
    let source = v
        .get("name_contains")
        .and_then(|s| s.as_str())
        .or_else(|| v.get("from").and_then(|f| f.get(0)).and_then(|s| s.as_str()))
        .unwrap_or("rule");
    let mut out = String::new();
    for c in source.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "rule".into()
    } else {
        out
    }
}

fn fresh_id(prefix: &str, slug: &str, taken: &mut BTreeSet<String>) -> String {
    let id = std::iter::once(format!("{prefix}{slug}"))
        .chain((2..).map(|n| format!("{prefix}{slug}-{n}")))
        .find(|id| !taken.contains(id))
        .expect("a free id");
    taken.insert(id.clone());
    id
}

struct Ctx<'a> {
    taken: &'a mut BTreeSet<String>,
    retired: &'a mut Vec<String>,
    changes: &'a mut Vec<String>,
}

/// One section: returns the promoted list and what stays in the overlay.
fn promote_list<T: Rule>(section: &str, list: &[T], o: &ListOverlay<T>, cx: &mut Ctx) -> (Vec<T>, ListOverlay<T>) {
    let prefix = prefix_of(list, section);
    let shipped = |id: &str| list.iter().any(|c| c.id() == Some(id));
    let mut left = ListOverlay::default();
    let mut out = Vec::new();

    // The user's rules - and his forks of rules the catalogue no longer has,
    // which were his rules in effect - first, as they ran.
    let mine = o.own.iter().cloned().chain(
        o.forks
            .iter()
            .filter(|(id, _)| !shipped(id))
            .map(|(id, f)| f.rule.clone().with_id(Some(id.clone()))),
    );
    for r in mine {
        let own_id = r.id().map(str::to_string);
        if own_id.as_ref().is_some_and(|id| o.disabled.contains(id)) {
            // Switched off: nothing to ship, but still his decision.
            left.disabled.extend(own_id);
            left.own.push(r);
            continue;
        }
        let id = fresh_id(&prefix, &slug(&r), cx.taken);
        cx.changes.push(format!("+ {section} {id}: {}", content(&r)));
        out.push(r.with_id(Some(id)));
    }

    for c in list {
        let Some(id) = c.id() else {
            out.push(c.clone());
            continue;
        };
        if o.disabled.contains(id) {
            cx.changes.push(format!("- {section} {id} (retired): {}", content(c)));
            cx.retired.push(id.to_string());
        } else if let Some(f) = o.forks.get(id) {
            cx.changes
                .push(format!("~ {section} {id}: {} (was {})", content(&f.rule), content(c)));
            out.push(f.rule.clone().with_id(Some(id.to_string())));
        } else {
            out.push(c.clone());
        }
    }
    (out, left)
}

pub fn promote(file: &RulesFile, o: &RulesOverlay) -> Promotion {
    let mut f = file.clone();
    let mut changes = Vec::new();
    let mut retired = f.retired.clone();
    let mut taken: BTreeSet<String> = retired.iter().cloned().collect();
    let (c, s, t) = (&file.car, &file.car.extraction_specs, &file.track);
    for id in c
        .brand_fix
        .iter()
        .filter_map(Rule::id)
        .chain(c.name_to_tag.iter().filter_map(Rule::id))
        .chain(c.class_fix.iter().filter_map(Rule::id))
        .chain(c.tag_merge.iter().filter_map(Rule::id))
        .chain(
            [
                &s.drivetrain,
                &s.aspiration,
                &s.engine_config,
                &s.engine_pos,
                &s.gearbox,
            ]
            .into_iter()
            .flatten()
            .filter_map(Rule::id),
        )
        .chain(t.tag_merge.iter().filter_map(Rule::id))
    {
        taken.insert(id.to_string());
    }
    let mut left = RulesOverlay {
        format: FORMAT,
        catalog_off: o.catalog_off,
        ..Default::default()
    };
    {
        let mut cx = Ctx {
            taken: &mut taken,
            retired: &mut retired,
            changes: &mut changes,
        };
        (f.car.brand_fix, left.brand_fix) = promote_list("brand_fix", &c.brand_fix, &o.brand_fix, &mut cx);
        (f.car.name_to_tag, left.name_to_tag) = promote_list("name_to_tag", &c.name_to_tag, &o.name_to_tag, &mut cx);
        (f.car.class_fix, left.class_fix) = promote_list("class_fix", &c.class_fix, &o.class_fix, &mut cx);
        (f.car.tag_merge, left.car_tag_merge) = promote_list("car_tag_merge", &c.tag_merge, &o.car_tag_merge, &mut cx);
        let fs = &mut f.car.extraction_specs;
        (fs.drivetrain, left.drivetrain) = promote_list("drivetrain", &s.drivetrain, &o.drivetrain, &mut cx);
        (fs.aspiration, left.aspiration) = promote_list("aspiration", &s.aspiration, &o.aspiration, &mut cx);
        (fs.engine_config, left.engine_config) =
            promote_list("engine_config", &s.engine_config, &o.engine_config, &mut cx);
        (fs.engine_pos, left.engine_pos) = promote_list("engine_pos", &s.engine_pos, &o.engine_pos, &mut cx);
        (fs.gearbox, left.gearbox) = promote_list("gearbox", &s.gearbox, &o.gearbox, &mut cx);
        (f.track.tag_merge, left.track_tag_merge) =
            promote_list("track_tag_merge", &t.tag_merge, &o.track_tag_merge, &mut cx);
    }
    let allow = o.track_categories.apply(&t.category_allowlist);
    if allow != t.category_allowlist {
        changes.push(format!(
            "~ track categories: {} (was {})",
            allow.join(" "),
            t.category_allowlist.join(" ")
        ));
        f.track.category_allowlist = allow;
    }
    f.retired = retired;
    Promotion { file: f, left, changes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pitbox_catalog::rules::Fork;

    fn committed() -> (String, RulesFile) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rules/default-tag-rules.json");
        let text = std::fs::read_to_string(&path).expect("catalogue in the repository");
        (text.replace("\r\n", "\n"), read(&path).expect("valid catalogue"))
    }

    /// Nothing to promote leaves the committed file byte for byte as it is -
    /// a real promotion then diffs line by line instead of drowning in a
    /// reformatting.
    #[test]
    fn promoting_no_decision_changes_nothing() {
        let (text, f) = committed();
        let p = promote(&f, &RulesOverlay::default());
        assert!(p.changes.is_empty(), "{:?}", p.changes);
        assert_eq!(render(&p.file), text, "the file must round-trip unchanged");
    }

    /// REGLES§4: a fork keeps its id, a switched-off rule is retired (and its
    /// id never reused), the user's rule is shipped first under a new id, and
    /// his switched-off rule stays his.
    #[test]
    fn each_decision_lands_where_the_spec_says() {
        let (_, f) = committed();
        let bayro = f.car.brand_fix[0].clone();
        let bar_id = f
            .car
            .brand_fix
            .iter()
            .find(|r| r.name_contains == "bar ")
            .unwrap()
            .id
            .clone()
            .unwrap();
        let mut o = RulesOverlay::default();
        o.brand_fix.forks.insert(
            bayro.id.clone().unwrap(),
            Fork {
                rule: BrandFix {
                    set_brand: "BMW M".into(),
                    ..bayro.clone()
                },
                forked_from: String::new(),
            },
        );
        o.brand_fix.disabled.insert(bar_id.clone());
        let mine = |id: &str, name: &str| BrandFix {
            id: Some(id.into()),
            name_contains: name.into(),
            set_brand: "RSS".into(),
        };
        o.brand_fix.own = vec![mine("own-1", "Lanzo V8"), mine("own-2", "bayro")];
        o.brand_fix.disabled.insert("own-2".into());

        let p = promote(&f, &o);
        let list = &p.file.car.brand_fix;
        assert_eq!(
            list[0].id.as_deref(),
            Some("pitbox.brand.lanzo-v8"),
            "his rule, first, new id"
        );
        assert!(
            list.iter().any(|r| r.id == bayro.id && r.set_brand == "BMW M"),
            "fork kept its id"
        );
        assert!(
            list.iter().all(|r| r.id.as_deref() != Some(bar_id.as_str())),
            "retired rule gone"
        );
        assert_eq!(p.file.retired, std::slice::from_ref(&bar_id), "and its id recorded");
        assert_eq!(
            p.left.brand_fix.own.len(),
            1,
            "his switched-off rule stays in the overlay"
        );
        assert!(p.left.brand_fix.disabled.contains("own-2"));

        // Promoting again a rule whose slug is the retired one's: never reused.
        let mut again = RulesOverlay::default();
        again.brand_fix.own = vec![mine("own-1", "bar")];
        let q = promote(&p.file, &again);
        assert_eq!(
            q.file.car.brand_fix[0].id.as_deref(),
            Some("pitbox.brand.bar-2"),
            "{:?}",
            q.changes
        );
    }
}
