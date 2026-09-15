//! Name cleaning and scoring weights (WIKI§4.3), driven by a configuration file.
//!
//! The spec is explicit that the list of things to strip "doit vivre dans un
//! fichier de configuration, pas dans le code": modders invent a new suffix
//! every season, and a list in the source means a release to add one. The seed
//! is embedded (`rules/wiki-matching.json`) and copied into the config
//! directory on first use, exactly like `tag-rules.json`.
//!
//! No `AppHandle` here, on purpose: this module takes a directory path. Keeping
//! `tauri` out of the business modules is a rule of the project, and it costs
//! one argument.

use std::path::Path;

use regex::Regex;
use serde::{Deserialize, Serialize};

const SEED: &str = include_str!("../../rules/wiki-matching.json");

/// File name in the config directory, once seeded.
const FILE_NAME: &str = "wiki-matching.json";

/// Weights of the car score (§4.1.4). They sum to 1 in the seed; nothing
/// enforces it, because a user editing the file to make the brand decisive is
/// doing something legitimate.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(default)]
pub struct Weights {
    pub name: f64,
    pub brand: f64,
    pub year: f64,
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            name: 0.6,
            brand: 0.3,
            year: 0.1,
        }
    }
}

/// **No `#[serde(default)]` on the struct itself**, and that is not a style
/// choice: the container attribute makes deserialization start from
/// `MatchingConfig::default()`, which parses the embedded seed, which
/// deserializes a `MatchingConfig`… The recursion has no base case and shows up
/// as a stack overflow in a test with no mention of serde in it. Per-field
/// defaults call `Default` on the *field's* type, so they are safe.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MatchingConfig {
    #[serde(default = "enabled")]
    pub strip_bracketed: bool,
    #[serde(default)]
    pub version_patterns: Vec<String>,
    #[serde(default)]
    pub remove_phrases: Vec<String>,
    #[serde(default)]
    pub remove_suffix_words: Vec<String>,
    /// Patterns removed **only** when the mod carries that category.
    ///
    /// The general list cannot hold everything: "traffic" belongs in a car
    /// name (the Renault Trafic is a real van) and is noise on a mod whose
    /// category *is* `#traffic`. What separates the two is not the word, it is
    /// what the library already knows about the mod — which is on disk, so the
    /// app can decide rather than guess.
    #[serde(default)]
    pub remove_patterns_by_category: std::collections::BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub weights: Weights,
}

fn enabled() -> bool {
    true
}

impl Default for MatchingConfig {
    /// The embedded seed. `expect` because a malformed seed is a build
    /// mistake, not a runtime condition — same stance as `rules::default_rules`.
    fn default() -> Self {
        serde_json::from_str(SEED).expect("le fichier wiki-matching.json embarqué doit être valide")
    }
}

/// Reads the editable copy, seeding it on first use. Any failure — unwritable
/// directory, hand-edited file that no longer parses — falls back to the
/// embedded seed: a broken config file must not take a decorative feature's
/// matching down with it.
pub fn load(config_dir: &Path) -> MatchingConfig {
    let path = config_dir.join(FILE_NAME);
    if !path.exists() {
        let _ = std::fs::create_dir_all(config_dir);
        if let Err(e) = std::fs::write(&path, SEED) {
            log::warn!("wiki: semis de {} impossible — {e}", path.display());
            return MatchingConfig::default();
        }
    }
    let mut cfg: MatchingConfig = match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            log::warn!("wiki: {} illisible, jeu embarqué utilisé — {e}", path.display());
            MatchingConfig::default()
        }),
        Err(e) => {
            log::warn!("wiki: {} illisible — {e}", path.display());
            MatchingConfig::default()
        }
    };

    // **Backfill.** The copy is seeded once and never rewritten, so a key added
    // to the embedded seed afterwards is missing from every file already out
    // there — and the rule it carries silently does nothing. That is exactly
    // how the `#traffic` rule looked broken on a machine whose file predated
    // it. Same remedy as `rules::load`, and the same limit: a user who empties
    // the list on purpose gets it back. Never rewrites the file — the user's
    // edits stay theirs.
    if cfg.remove_patterns_by_category.is_empty() {
        cfg.remove_patterns_by_category = MatchingConfig::default().remove_patterns_by_category;
    }
    cfg
}

/// Compiled once, applied to every name of a run.
pub struct Cleaner {
    /// Normalised category name (no leading `#`, lowercase) to its patterns.
    by_category: Vec<(String, Vec<Regex>)>,
    bracketed: Option<Regex>,
    versions: Vec<Regex>,
    phrases: Vec<Regex>,
    suffix_words: Vec<String>,
    punctuation: Regex,
    whitespace: Regex,
}

/// A category compares without its leading `#` and without case: the overlay
/// writes `#traffic`, a hand-edited config file may well write `traffic`.
fn normalise_category(category: &str) -> String {
    category.trim().trim_start_matches('#').to_lowercase()
}

impl Cleaner {
    /// A pattern that does not compile is dropped with a warning rather than
    /// failing the run: the file is hand-editable, and one bad regex should
    /// cost that one rule, not the feature.
    pub fn new(cfg: &MatchingConfig) -> Self {
        let compile = |pattern: &str| match Regex::new(pattern) {
            Ok(re) => Some(re),
            Err(e) => {
                log::warn!("wiki: motif de nettoyage ignoré ({pattern}) — {e}");
                None
            }
        };
        Self {
            by_category: cfg
                .remove_patterns_by_category
                .iter()
                .map(|(category, patterns)| {
                    let compiled = patterns.iter().filter_map(|p| compile(&format!("(?i){p}"))).collect();
                    (normalise_category(category), compiled)
                })
                .collect(),
            bracketed: cfg
                .strip_bracketed
                .then(|| Regex::new(r"[\[\(\{][^\]\)\}]*[\]\)\}]").expect("motif de crochets valide")),
            versions: cfg
                .version_patterns
                .iter()
                .filter_map(|p| compile(&format!("(?i){p}")))
                .collect(),
            phrases: cfg
                .remove_phrases
                .iter()
                .filter_map(|p| compile(&format!(r"(?i)\b{}\b", regex::escape(p))))
                .collect(),
            suffix_words: cfg.remove_suffix_words.iter().map(|w| w.to_lowercase()).collect(),
            punctuation: Regex::new(r"^[\s\-_,.:;|/]+|[\s\-_,.:;|/]+$").expect("motif de ponctuation valide"),
            whitespace: Regex::new(r"\s+").expect("motif d'espaces valide"),
        }
    }

    /// The cleaned name, ready to be searched.
    ///
    /// Order matters: brackets go first (they hide versions and quality tags
    /// inside them), versions next, then phrases, then trailing words. Running
    /// the suffix pass last is what lets `Fixed 1.05` lose both halves.
    pub fn clean(&self, raw: &str) -> String {
        self.clean_in_category(raw, None)
    }

    /// The cleaned name, knowing which category the mod belongs to.
    ///
    /// The category patterns run **first**: they strip pack prefixes that sit
    /// in front of the real model name, and everything after them assumes it is
    /// looking at a model name.
    pub fn clean_in_category(&self, raw: &str, category: Option<&str>) -> String {
        let mut raw = raw.to_string();
        if let Some(category) = category.map(normalise_category) {
            for (name, patterns) in &self.by_category {
                if *name != category {
                    continue;
                }
                for re in patterns {
                    raw = re.replace_all(&raw, " ").into_owned();
                }
            }
        }
        self.clean_name(&raw)
    }

    fn clean_name(&self, raw: &str) -> String {
        // Folder ids arrive as `rss_gtm_lanzo_v8`; a display name rarely has
        // underscores. Splitting them into words costs nothing on a real name
        // and makes an id searchable.
        let mut text = raw.replace('_', " ");

        if let Some(re) = &self.bracketed {
            text = re.replace_all(&text, " ").into_owned();
        }
        for re in &self.versions {
            text = re.replace_all(&text, " ").into_owned();
        }
        for re in &self.phrases {
            text = re.replace_all(&text, " ").into_owned();
        }
        text = self.whitespace.replace_all(&text, " ").trim().to_string();

        // Trailing words, repeatedly: `BMW E30 M3 AC Conversion` has two.
        loop {
            let trimmed = self.punctuation.replace_all(&text, "").into_owned();
            let Some((head, last)) = trimmed.rsplit_once(' ') else {
                break;
            };
            if !self.suffix_words.contains(&last.to_lowercase()) {
                text = trimmed;
                break;
            }
            text = head.to_string();
        }

        let text = self.punctuation.replace_all(&text, "").into_owned();
        self.whitespace.replace_all(&text, " ").trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cleaner() -> Cleaner {
        Cleaner::new(&MatchingConfig::default())
    }

    /// Rule (WIKI§4.3): the shipped list is valid JSON and parses into the config.
    /// It is embedded with `include_str!`, so a typo in it is only discovered
    /// at runtime — this test is what turns that into a build-time failure.
    #[test]
    fn the_embedded_list_parses() {
        let cfg = MatchingConfig::default();
        assert!(cfg.strip_bracketed);
        assert!(!cfg.version_patterns.is_empty(), "des motifs de version");
        assert!(!cfg.remove_phrases.is_empty(), "des mentions à retirer");
        assert!((cfg.weights.name + cfg.weights.brand + cfg.weights.year - 1.0).abs() < 1e-9);
    }

    /// Rule (WIKI§4.3): real mod names from the library, cleaned down to something
    /// worth searching.
    #[test]
    fn real_mod_names_lose_what_wikipedia_never_heard_of() {
        let c = cleaner();
        assert_eq!(c.clean("Toyota AE86 Trueno v1.2"), "Toyota AE86 Trueno");
        assert_eq!(c.clean("Nissan Silvia S15 [4K] Rocket Bunny"), "Nissan Silvia S15");
        assert_eq!(c.clean("Ferrari F40 HD Remaster"), "Ferrari F40");
        assert_eq!(c.clean("BMW E30 M3 AC Conversion"), "BMW E30 M3");
        assert_eq!(c.clean("Mazda RX-7 FD3S (Fixed) 1.05"), "Mazda RX-7 FD3S");
        assert_eq!(c.clean("Porsche 911 GT3 RS - WIP"), "Porsche 911 GT3 RS");
    }

    /// Rule (WIKI§4.3): **a `v` followed by digits is not a version number.**
    ///
    /// `rss_gtm_lanzo_v8` is the project's own example of a mod folder, and its
    /// `V8` is an engine. `MK4` is a generation. A pattern matching a bare
    /// `v\d+` would eat both and search Wikipedia for "RSS GTM Lanzo" — which
    /// is why the shipped patterns all require a separator.
    #[test]
    fn an_engine_is_not_a_version_number() {
        let c = cleaner();
        assert_eq!(c.clean("RSS GTM Lanzo V8"), "RSS GTM Lanzo V8", "V8 est un moteur");
        assert_eq!(
            c.clean("rss_gtm_lanzo_v8"),
            "rss gtm lanzo v8",
            "même nom, côté dossier"
        );
        assert_eq!(
            c.clean("Toyota Supra MK4"),
            "Toyota Supra MK4",
            "MK4 est une génération"
        );
        assert_eq!(c.clean("Audi R8 V10"), "Audi R8 V10");
    }

    /// Rule (WIKI§4.3): a suffix word is only stripped at the end. "AC" inside a
    /// name can be part of it — `AC Cobra` is a car, not a conversion.
    #[test]
    fn a_suffix_word_is_only_stripped_at_the_end() {
        let c = cleaner();
        assert_eq!(c.clean("AC Cobra 427"), "AC Cobra 427", "la marque AC survit");
        assert_eq!(
            c.clean("Shelby Cobra AC"),
            "Shelby Cobra",
            "en fin de nom, c'est le suffixe"
        );
    }

    /// Rule (WIKI§4.3): **"traffic" is only noise when the mod says it is.**
    ///
    /// The Renault Trafic is a real van, so the word cannot go in the general
    /// list — a mod of it would lose its own name. But the twenty-five cars
    /// carrying `category = "#traffic"` in the reference library hide a real
    /// model behind a pack prefix: `τraffic Japan | Mazda RX-8 SE3P`, with a
    /// **Greek tau** (U+03C4), which is also why a plain "traffic" would never
    /// have matched it. What separates the two cases is on disk, so the app
    /// reads it instead of guessing.
    #[test]
    fn traffic_is_noise_only_in_the_traffic_category() {
        let c = cleaner();
        let traffic = Some("#traffic");

        assert_eq!(
            c.clean_in_category("τraffic Japan | Mazda RX-8 SE3P", traffic),
            "Mazda RX-8 SE3P"
        );
        assert_eq!(
            c.clean_in_category("τraffic jp - Toyota Camry", traffic),
            "Toyota Camry"
        );
        assert_eq!(
            c.clean_in_category("traffic jp - Nissan Leaf", traffic),
            "Nissan Leaf",
            "la graphie latine aussi"
        );

        // Hors catégorie, le mot est un nom de modèle et doit survivre.
        assert_eq!(c.clean_in_category("Renault Trafic", None), "Renault Trafic");
        assert_eq!(
            c.clean_in_category("Renault Trafic", Some("#sportscars")),
            "Renault Trafic",
            "une autre catégorie ne déclenche pas la règle"
        );
    }

    /// Rule: the category is compared without its `#` and without case — the
    /// overlay writes `#traffic`, a hand-edited config may write `Traffic`.
    #[test]
    fn a_category_matches_with_or_without_its_hash() {
        let c = cleaner();
        let expected = "Toyota Camry";
        for spelling in ["#traffic", "traffic", "#Traffic", " #TRAFFIC "] {
            assert_eq!(
                c.clean_in_category("τraffic jp - Toyota Camry", Some(spelling)),
                expected,
                "orthographe {spelling:?}"
            );
        }
    }

    /// Rule (WIKI§1): cleaning never invents. A name made only of noise comes back
    /// empty, and an empty name is a non-result — not a search for "".
    #[test]
    fn a_name_made_of_noise_comes_back_empty() {
        let c = cleaner();
        assert_eq!(c.clean("[4K] HD Remaster v1.2"), "");
        assert_eq!(c.clean("   "), "");
    }
}
