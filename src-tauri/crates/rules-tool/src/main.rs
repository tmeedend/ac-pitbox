//! `rules-tool` — the developer's side of the rules catalogue (REGLES§2).
//! Never shipped to users.
//!
//! The workflow it serves: curate in the application, with its screens
//! (Workshop › Rules, Categories, Countries), then turn those decisions into
//! the catalogue every user receives — instead of hand-editing JSON. A fix a
//! user proposed (the Rules screen links to GitHub) goes the same way: made
//! in the application, then promoted.
//!
//! ```text
//! cargo run -p rules-tool -- diff      what my decisions would change in the catalogue
//! cargo run -p rules-tool -- promote   write them into the catalogue, empty my overlays
//! ```
//!
//! Two catalogues, two overlays: `taxonomy-catalog.json` + `taxonomy.json`
//! (families, countries) and `default-tag-rules.json` + `rules-overlay.json`
//! (the list rules, `lists.rs`). Options: `--catalog-dir <dir>` (default:
//! `src-tauri/rules` of this repository), `--config <dir>` (default: the
//! application's `%APPDATA%\com.pitbox.app`).
//!
//! The merge is the application's own (`pitbox-catalog`): what `promote`
//! writes is exactly what the application was showing.

mod lists;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pitbox_catalog::rules::RulesOverlay;
use pitbox_catalog::taxonomy::{TaxonomyOverlay, TaxonomyTables, FORMAT};
use serde::de::DeserializeOwned;
use serde::Serialize;

const USAGE: &str = "usage: rules-tool <diff|promote> [--catalog-dir <dir>] [--config <dir>]";

fn default_catalog_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rules")
}

#[cfg(test)]
fn default_catalog() -> PathBuf {
    default_catalog_dir().join("taxonomy-catalog.json")
}

fn default_config() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| Path::new(&d).join("com.pitbox.app"))
}

/// An overlay file, or no decision when there is none: a developer who never
/// touched the Rules screen has no `rules-overlay.json`.
fn read_overlay<T: DeserializeOwned + Default>(path: &Path) -> Result<T, String> {
    if path.is_file() {
        read(path)
    } else {
        Ok(T::default())
    }
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Keeps the overlay aside, then writes what is left of it: a promotion is
/// one command away from losing a curation session.
fn replace_overlay<T: Serialize>(path: &Path, left: &T) -> Result<PathBuf, String> {
    let backup = path.with_extension("json.bak");
    std::fs::copy(path, &backup).map_err(|e| format!("{}: {e}", backup.display()))?;
    write(path, &serde_json::to_string_pretty(left).expect("overlay serialises"))?;
    Ok(backup)
}

fn read<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The catalogue as a file: pretty JSON and a final newline, the form it is
/// committed in — so a promotion with nothing to promote leaves it byte for
/// byte unchanged, and a real one diffs line by line.
fn render(t: &TaxonomyTables) -> String {
    format!("{}\n", serde_json::to_string_pretty(t).expect("tables serialise"))
}

/// What changes between two versions of the catalogue, in words.
fn describe(before: &TaxonomyTables, after: &TaxonomyTables) -> Vec<String> {
    let mut out = Vec::new();
    for f in &after.families {
        match before.families.iter().find(|b| b.id == f.id) {
            None => out.push(format!("+ family {} ({} tags)", f.id, f.tags.len())),
            Some(b) => {
                for t in f.tags.iter().filter(|t| !b.tags.contains(t)) {
                    out.push(format!("  family {}: + {t}", f.id));
                }
                for t in b.tags.iter().filter(|t| !f.tags.contains(t)) {
                    out.push(format!("  family {}: - {t}", f.id));
                }
                if b.name != f.name || b.icon != f.icon {
                    out.push(format!("  family {}: name {:?} icon {:?}", f.id, f.name, f.icon));
                }
            }
        }
    }
    for b in before
        .families
        .iter()
        .filter(|b| !after.families.iter().any(|f| f.id == b.id))
    {
        out.push(format!("- family {}", b.id));
    }
    let maps = |label: &str, b: &BTreeMap<String, String>, a: &BTreeMap<String, String>, out: &mut Vec<String>| {
        for (k, v) in a {
            match b.get(k) {
                None => out.push(format!("+ {label} {k} → {v}")),
                Some(old) if old != v => out.push(format!("~ {label} {k} → {v} (was {old})")),
                _ => {}
            }
        }
        for k in b.keys().filter(|k| !a.contains_key(*k)) {
            out.push(format!("- {label} {k}"));
        }
    };
    maps(
        "country alias",
        &before.country_aliases,
        &after.country_aliases,
        &mut out,
    );
    maps("country tag", &before.country_tags, &after.country_tags, &mut out);
    out
}

fn run(args: &[String]) -> Result<(), String> {
    let cmd = args.first().ok_or(USAGE)?;
    if cmd != "diff" && cmd != "promote" {
        return Err(USAGE.into());
    }
    let mut catalog_dir = default_catalog_dir();
    let mut config = default_config();
    let mut rest = args[1..].iter();
    while let Some(a) = rest.next() {
        match a.as_str() {
            "--catalog-dir" => catalog_dir = rest.next().ok_or(USAGE)?.into(),
            "--config" => config = Some(rest.next().ok_or(USAGE)?.into()),
            _ => return Err(USAGE.into()),
        }
    }
    let config = config.ok_or("no --config given and APPDATA is not set")?;

    let tax_catalog_path = catalog_dir.join("taxonomy-catalog.json");
    let tax_overlay_path = config.join("taxonomy.json");
    let tax_catalog: TaxonomyTables = read(&tax_catalog_path)?;
    let tax_overlay: TaxonomyOverlay = read_overlay(&tax_overlay_path)?;
    let tax_promoted = tax_catalog.apply(&tax_overlay);
    let tax_changes = describe(&tax_catalog, &tax_promoted);

    let rules_catalog_path = catalog_dir.join("default-tag-rules.json");
    let rules_overlay_path = config.join("rules-overlay.json");
    let rules_catalog: lists::RulesFile = read(&rules_catalog_path)?;
    let rules_overlay: RulesOverlay = read_overlay(&rules_overlay_path)?;
    let rules = lists::promote(&rules_catalog, &rules_overlay);

    for (title, changes) in [("taxonomy", &tax_changes), ("list rules", &rules.changes)] {
        if !changes.is_empty() {
            println!("{title}:");
            for c in changes {
                println!("{c}");
            }
        }
    }
    if rules_overlay.catalog_off {
        println!("note: the catalogue is switched off in this configuration - a preference, not promoted");
    }
    if tax_changes.is_empty() && rules.changes.is_empty() {
        println!("no decision to promote: the overlays change nothing in the catalogue");
        return Ok(());
    }
    if cmd == "diff" {
        return Ok(());
    }

    if !tax_changes.is_empty() {
        write(&tax_catalog_path, &render(&tax_promoted))?;
        // The decisions now ARE the catalogue. Left in the overlay they would
        // restate it - harmless, but each would keep its mark on a line that
        // no longer differs from anything. What is not a table entry (the
        // ignored countries) stays.
        let emptied = TaxonomyOverlay {
            format: FORMAT,
            ignored_countries: tax_overlay.ignored_countries.clone(),
            ..Default::default()
        };
        let backup = replace_overlay(&tax_overlay_path, &emptied)?;
        println!(
            "\n{} change(s) written to {}",
            tax_changes.len(),
            tax_catalog_path.display()
        );
        println!("taxonomy overlay emptied, previous one kept as {}", backup.display());
        for f in tax_promoted.families.iter().filter(|f| f.name.is_some()) {
            println!(
                "note: family `{}` carries a name, shown untranslated - add `families.{}` to fr.json and en.json, then drop the name",
                f.id, f.id
            );
        }
    }
    if !rules.changes.is_empty() {
        write(&rules_catalog_path, &lists::render(&rules.file))?;
        let backup = replace_overlay(&rules_overlay_path, &rules.left)?;
        println!(
            "\n{} change(s) written to {}",
            rules.changes.len(),
            rules_catalog_path.display()
        );
        println!("rules overlay emptied, previous one kept as {}", backup.display());
        println!("the new ids are written once and for all - rename one now if it reads badly, never later");
    }
    println!("rebuild the application: until then it still embeds the previous catalogue");
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A promotion with nothing to promote must leave the committed catalogue
    /// byte for byte unchanged - otherwise the first real promotion drowns its
    /// change in a reformatting of the whole file.
    #[test]
    fn rendering_the_committed_catalogue_changes_nothing() {
        let text = std::fs::read_to_string(default_catalog()).expect("catalogue in the repository");
        let t: TaxonomyTables = serde_json::from_str(&text).expect("valid catalogue");
        assert_eq!(
            render(&t.apply(&TaxonomyOverlay::default())),
            text.replace("\r\n", "\n")
        );
    }

    #[test]
    fn a_moved_tag_is_described_on_both_families() {
        let before: TaxonomyTables = serde_json::from_str(
            r#"{"families":[{"id":"race","tags":["race","gt3"]},{"id":"classic","tags":["vintage"]}],
                "country_aliases":{"usa":"United States"},"country_tags":{}}"#,
        )
        .unwrap();
        let mut o = TaxonomyOverlay::default();
        o.families.tags.insert("gt3".into(), "classic".into());
        o.country_aliases.set.insert("nippon".into(), "Japan".into());
        let lines = describe(&before, &before.apply(&o));
        assert!(lines.contains(&"  family classic: + gt3".to_string()), "{lines:?}");
        assert!(lines.contains(&"  family race: - gt3".to_string()), "{lines:?}");
        assert!(
            lines.contains(&"+ country alias nippon → Japan".to_string()),
            "{lines:?}"
        );
    }
}
