//! The survey (the "relevé", REGLES§14 and TAXO): the real engine run on a
//! real library, read-only, written to one JSON file the user can send.
//!
//! What it is for: improving the rules catalogue shipped with Pit Box from
//! libraries larger than the developer's - which tags no rule knows, which
//! cars no family takes, which countries have no flag, how brands are
//! spelled, what the logos look like (the baked-background threshold is
//! checked on it) - and the user's own curation, which is exactly what
//! `rules-tool promote` turns into catalogue.
//!
//! **Anonymous by construction**, because it is meant to be sent - by the
//! user, or by contributors, and the repository is public: mod ids and the
//! metadata mods publish (name, brand, class, country, tags), never a path,
//! never an author, never a note or a tag typed by the user. The file is
//! written where the user chooses, and sent by him; nothing leaves on its own.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;

use crate::config::AppConfig;
use crate::rules::Rules;

pub const FORMAT: u32 = 1;

/// What the mod's own file says.
#[derive(Debug, Serialize)]
pub struct FromFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brand: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    pub tags: Vec<String>,
}

/// One mod: its file, and what the rules made of it.
#[derive(Debug, Serialize)]
pub struct ModLine {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub file: FromFile,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brand: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    /// A car's `#` category, or a track's categories.
    pub categories: Vec<String>,
    /// Cars only: the families its tags reach (INDEX§6.1).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub families: Vec<String>,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub specs: BTreeMap<&'static str, String>,
    /// Raw tags no rule recognised.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unrecognized: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Count {
    pub value: String,
    pub mods: usize,
}

#[derive(Debug, Serialize)]
pub struct BrandLine {
    pub name: String,
    pub cars: usize,
    /// Every spelling its cars' files use, when more than the name itself.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub spellings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct LogoVariantLine {
    pub background: crate::logos::Background,
    pub width: u32,
    pub height: u32,
    pub cars: usize,
}

#[derive(Debug, Serialize)]
pub struct LogoLine {
    pub brand: String,
    pub variants: Vec<LogoVariantLine>,
}

/// What to look at first.
#[derive(Debug, Serialize)]
pub struct Summary {
    pub cars: usize,
    pub tracks: usize,
    /// Cars no family takes (INDEX§6.1).
    pub unclassified_cars: Vec<String>,
    /// Raw tags no rule recognises, by number of mods, most seen first.
    pub unrecognized_tags: Vec<Count>,
    /// Countries the game has no flag for; `None` when the game's table could
    /// not be read (no install configured), rather than every country.
    pub countries_without_flag: Option<Vec<Count>>,
    pub brands: Vec<BrandLine>,
}

#[derive(Debug, Serialize)]
pub struct Survey {
    pub pitbox_survey: u32,
    pub app_version: String,
    pub catalog_version: String,
    pub summary: Summary,
    pub cars: Vec<ModLine>,
    pub tracks: Vec<ModLine>,
    pub logos: Vec<LogoLine>,
    /// The user's decisions on the rules - his curation, what the catalogue
    /// can take in (`rules_share::export`, the same content as an export).
    pub decisions: crate::rules_share::RulesExport,
}

fn counts(map: BTreeMap<String, usize>) -> Vec<Count> {
    let mut v: Vec<Count> = map.into_iter().map(|(value, mods)| Count { value, mods }).collect();
    v.sort_by(|a, b| b.mods.cmp(&a.mods).then_with(|| a.value.cmp(&b.value)));
    v
}

pub fn build(conn: &Connection, cfg: &AppConfig, rules: &Rules, dir: &Path) -> rusqlite::Result<Survey> {
    let owner = pitbox_catalog::taxonomy::lookup(&rules.car.category_families);
    let flags: Option<BTreeSet<String>> =
        crate::nationalities::with_known(|k| (!k.is_empty()).then(|| k.iter().map(|n| n.name.clone()).collect()));
    let (mut cars, mut tracks) = (Vec::new(), Vec::new());
    let mut unrecognized: BTreeMap<String, usize> = BTreeMap::new();
    let mut flagless: BTreeMap<String, usize> = BTreeMap::new();
    let mut brands: BTreeMap<String, (usize, BTreeSet<String>)> = BTreeMap::new();
    let mut unclassified = Vec::new();
    // Tags the engine drops ON PURPOSE, not for want of a rule: the two class
    // values (the `class` field carries them) and the country tags, which only
    // speak when the file declares no country. Counted, they buried the tags
    // the catalogue could learn - measured on the dev library: `street` 167,
    // `race` 107, `japan` 52 at the top of the list.
    let dropped: BTreeSet<String> = ["street", "race"]
        .into_iter()
        .map(str::to_string)
        .chain(rules.car.extraction_country.map.keys().cloned())
        .collect();

    for m in crate::overlay::list_mods(conn)? {
        let Some((ui, h)) = crate::harmonize::read_and_compute(conn, cfg, rules, &m) else {
            continue;
        };
        let is_car = m.kind != "Track";
        let country = crate::harmonize::final_country(rules, &h, ui.country.as_deref());
        let unknown: BTreeSet<String> = h
            .unrecognized
            .iter()
            .filter(|t| !dropped.contains(*t))
            .cloned()
            .collect();
        for t in &unknown {
            *unrecognized.entry(t.clone()).or_default() += 1;
        }
        if let (Some(c), Some(known)) = (&country, &flags) {
            if !known.contains(c) {
                *flagless.entry(c.clone()).or_default() += 1;
            }
        }
        // Families from the tags the library merges (`modTags`) - the file's
        // and the rules', never the ones the user typed: those are his.
        let families: Vec<String> = if is_car {
            let set: BTreeSet<String> = ui
                .tags
                .iter()
                .chain(&h.tags_from_rule)
                .filter_map(|t| owner.get(&pitbox_catalog::taxonomy::family_tag(t)).cloned())
                .collect();
            set.into_iter().collect()
        } else {
            Vec::new()
        };
        if is_car {
            if families.is_empty() {
                unclassified.push(m.id_interne.clone());
            }
            if let Some(b) = &h.brand {
                let e = brands.entry(b.clone()).or_default();
                e.0 += 1;
                if let Some(raw) = ui.brand.as_deref().map(str::trim).filter(|r| !r.is_empty() && *r != b) {
                    e.1.insert(raw.to_string());
                }
            }
        }
        let specs: BTreeMap<&'static str, String> = [
            ("drivetrain", &h.drivetrain),
            ("aspiration", &h.aspiration),
            ("engine_config", &h.engine_config),
            ("engine_pos", &h.engine_pos),
            ("gearbox", &h.gearbox),
        ]
        .into_iter()
        .filter_map(|(k, v)| v.clone().map(|v| (k, v)))
        .collect();
        let mut tags = h.tags_from_rule.clone();
        tags.sort();
        let line = ModLine {
            id: m.id_interne.clone(),
            name: ui.name.clone(),
            file: FromFile {
                brand: ui.brand.clone(),
                class: ui.class.clone(),
                country: ui.country.clone(),
                tags: ui.tags.clone(),
            },
            brand: if is_car { h.brand.clone() } else { None },
            country,
            categories: if is_car {
                h.category.iter().cloned().collect()
            } else {
                h.categories.clone()
            },
            families,
            tags,
            specs,
            unrecognized: unknown.into_iter().collect(),
        };
        if is_car {
            cars.push(line);
        } else {
            tracks.push(line);
        }
    }

    let badges = crate::library::car_badges(conn, cfg)?;
    let logos = crate::logos::elect(&badges, &crate::logos::Prefs::new(), dir)
        .into_values()
        .filter(|b| !b.variants.is_empty())
        .map(|b| LogoLine {
            brand: b.brand,
            variants: b
                .variants
                .iter()
                .map(|v| LogoVariantLine {
                    background: v.background,
                    width: v.width,
                    height: v.height,
                    cars: v.cars,
                })
                .collect(),
        })
        .collect();

    let mut brand_lines: Vec<BrandLine> = brands
        .into_iter()
        .map(|(name, (cars, spellings))| BrandLine {
            name,
            cars,
            spellings: spellings.into_iter().collect(),
        })
        .collect();
    brand_lines.sort_by(|a, b| b.cars.cmp(&a.cars).then_with(|| a.name.cmp(&b.name)));

    Ok(Survey {
        pitbox_survey: FORMAT,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        catalog_version: crate::catalog_update::version_in_force(dir),
        summary: Summary {
            cars: cars.len(),
            tracks: tracks.len(),
            unclassified_cars: unclassified,
            unrecognized_tags: counts(unrecognized),
            countries_without_flag: flags.map(|_| counts(flagless)),
            brands: brand_lines,
        },
        cars,
        tracks,
        logos,
        decisions: crate::rules_share::export(dir, &crate::rules::default_rules()),
    })
}

pub fn write(path: &Path, survey: &Survey) -> Result<(), String> {
    let json = serde_json::to_string_pretty(survey).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A library of one car, `rss_car`, whose `ui_car.json` is `ui_car`.
    fn one_car_library(base: &Path, ui_car: &str) -> (Connection, AppConfig) {
        let lib = base.join("lib");
        let dir = lib.join("cars").join("rss_car").join("v");
        std::fs::create_dir_all(dir.join("ui")).unwrap();
        std::fs::write(dir.join("ui").join("ui_car.json"), ui_car).unwrap();
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).unwrap();
        let now = chrono::Local::now().to_rfc3339();
        crate::overlay::upsert_mod(
            &conn,
            "rss_car",
            "Car",
            Some("RSS"),
            Some("RSS Formula"),
            "h",
            None,
            &now,
        )
        .unwrap();
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
        let cfg = AppConfig {
            library_path: Some(lib),
            ..Default::default()
        };
        (conn, cfg)
    }

    /// The survey says what the engine did and what it did not know - and it
    /// carries no path: it is meant to be sent, and the repository it feeds
    /// is public.
    #[test]
    fn a_survey_reports_the_unknown_and_never_a_path() {
        let base = crate::testutil::temp_dir("survey");
        let (conn, cfg) = one_car_library(
            &base,
            r#"{"name":"RSS Formula","brand":"RSS","class":"race","author":"Some One","tags":["rwd","weirdtag"]}"#,
        );

        let s = build(&conn, &cfg, &crate::rules::default_rules(), &base).unwrap();
        assert_eq!(s.summary.cars, 1);
        assert_eq!(
            s.summary.unrecognized_tags[0].value, "weirdtag",
            "the tag no rule knows"
        );
        assert_eq!(s.cars[0].specs.get("drivetrain").map(String::as_str), Some("RWD"));
        assert_eq!(s.summary.unclassified_cars, ["rss_car"], "no family takes it");

        let file = base.join("survey.json");
        write(&file, &s).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        let base_text = base.to_string_lossy().replace('\\', "\\\\");
        assert!(!text.contains(&*base_text), "no path in a survey");
        assert!(!text.contains("Some One"), "no author either");
    }

    /// The survey of this machine's library, without the application - how
    /// Claude Code on another machine produces one. Works on copies; writes
    /// to `PITBOX_SURVEY_OUT` (default: `survey.json` in the temp folder) and
    /// prints the summary.
    ///
    /// ```text
    /// cargo test --lib survey::tests::real_install_survey -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "reads the Pit Box configuration and library of this machine"]
    fn real_install_survey() {
        let src = std::env::var_os("PITBOX_CONFIG_DIR")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("APPDATA").map(|d| Path::new(&d).join("com.pitbox.app")))
            .expect("PITBOX_CONFIG_DIR or APPDATA");
        let work = crate::testutil::temp_dir("real-survey");
        for f in [
            "config.json",
            "overlay.sqlite",
            "taxonomy.json",
            "rules-overlay.json",
            "catalog-state.json",
        ] {
            if src.join(f).is_file() {
                std::fs::copy(src.join(f), work.join(f)).unwrap();
            }
        }
        let cfg: AppConfig = serde_json::from_str(&std::fs::read_to_string(work.join("config.json")).unwrap()).unwrap();
        if let Some(root) = cfg.ac_install_path.as_deref() {
            crate::nationalities::set_known(crate::nationalities::nationalities(Path::new(root)));
        }
        let conn = crate::overlay::open(&work.join("overlay.sqlite")).unwrap();
        crate::brands::refresh_from(&conn);
        let rules = crate::rules::load_from_dir(&work);
        let t = std::time::Instant::now();
        let s = build(&conn, &cfg, &rules, &work).unwrap();
        let out = std::env::var_os("PITBOX_SURVEY_OUT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("survey.json"));
        write(&out, &s).unwrap();
        println!(
            "{} cars, {} tracks in {:?} -> {}",
            s.summary.cars,
            s.summary.tracks,
            t.elapsed(),
            out.display()
        );
        println!("unclassified cars: {}", s.summary.unclassified_cars.len());
        for c in s.summary.unrecognized_tags.iter().take(15) {
            println!("  unknown tag {:<24} {}", c.value, c.mods);
        }
        if let Some(f) = &s.summary.countries_without_flag {
            for c in f {
                println!("  no flag {:<24} {}", c.value, c.mods);
            }
        }
    }
}
