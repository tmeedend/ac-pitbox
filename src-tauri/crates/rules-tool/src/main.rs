//! `rules-tool` — the developer's side of the rules catalogue (REGLES§2).
//! Never shipped to users.
//!
//! The workflow it serves: curate in the application, with its screens
//! (Workshop › Categories, Countries), then turn those decisions into the
//! catalogue every user receives — instead of hand-editing JSON.
//!
//! ```text
//! cargo run -p rules-tool -- diff      what my decisions would change in the catalogue
//! cargo run -p rules-tool -- promote   write them into the catalogue, empty my overlay
//! ```
//!
//! Options: `--catalog <file>` (default: `src-tauri/rules/taxonomy-catalog.json`
//! of this repository), `--overlay <file>` (default: the application's
//! `%APPDATA%\com.pitbox.app\taxonomy.json`).
//!
//! The merge is the application's own (`pitbox-taxonomy`): what `promote`
//! writes is exactly what the application was showing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use pitbox_taxonomy::{TaxonomyOverlay, TaxonomyTables, FORMAT};
use serde::de::DeserializeOwned;

const USAGE: &str = "usage: rules-tool <diff|promote> [--catalog <file>] [--overlay <file>]";

fn default_catalog() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rules/taxonomy-catalog.json")
}

fn default_overlay() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| Path::new(&d).join("com.pitbox.app").join("taxonomy.json"))
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
    let mut catalog_path = default_catalog();
    let mut overlay_path = default_overlay();
    let mut rest = args[1..].iter();
    while let Some(a) = rest.next() {
        match a.as_str() {
            "--catalog" => catalog_path = rest.next().ok_or(USAGE)?.into(),
            "--overlay" => overlay_path = Some(rest.next().ok_or(USAGE)?.into()),
            _ => return Err(USAGE.into()),
        }
    }
    let overlay_path = overlay_path.ok_or("no --overlay given and APPDATA is not set")?;
    let catalog: TaxonomyTables = read(&catalog_path)?;
    let overlay: TaxonomyOverlay = read(&overlay_path)?;
    let promoted = catalog.apply(&overlay);
    let changes = describe(&catalog, &promoted);

    match cmd.as_str() {
        "diff" => {
            if changes.is_empty() {
                println!("no decision to promote: the overlay changes nothing in the catalogue");
            }
            for c in &changes {
                println!("{c}");
            }
            Ok(())
        }
        "promote" => {
            if changes.is_empty() {
                println!("nothing to promote");
                return Ok(());
            }
            // The overlay is kept aside before being emptied: a promotion is
            // one command away from losing a curation session.
            let backup = overlay_path.with_extension("json.bak");
            std::fs::copy(&overlay_path, &backup).map_err(|e| format!("{}: {e}", backup.display()))?;
            std::fs::write(&catalog_path, render(&promoted)).map_err(|e| format!("{}: {e}", catalog_path.display()))?;
            // The decisions now ARE the catalogue. Left in the overlay they
            // would restate it - harmless, but every one of them would keep a
            // ⚑ on a line that no longer differs from anything. What is not a
            // table entry (the ignored countries) stays.
            let emptied = TaxonomyOverlay {
                format: FORMAT,
                ignored_countries: overlay.ignored_countries.clone(),
                ..Default::default()
            };
            let json = serde_json::to_string_pretty(&emptied).expect("overlay serialises");
            std::fs::write(&overlay_path, json).map_err(|e| format!("{}: {e}", overlay_path.display()))?;
            for c in &changes {
                println!("{c}");
            }
            println!("\n{} change(s) written to {}", changes.len(), catalog_path.display());
            println!("overlay emptied, previous one kept as {}", backup.display());
            for f in promoted.families.iter().filter(|f| f.name.is_some()) {
                println!(
                    "note: family `{}` carries a name, shown untranslated - add `families.{}` to fr.json and en.json, then drop the name",
                    f.id, f.id
                );
            }
            println!("rebuild the application: until then it still embeds the previous catalogue");
            Ok(())
        }
        _ => Err(USAGE.into()),
    }
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
