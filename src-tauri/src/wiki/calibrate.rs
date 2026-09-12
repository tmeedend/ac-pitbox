//! Calibration run: the whole library through the matching, **persisting
//! nothing** (§13).
//!
//! The thresholds of §4.1.5 and §4.2.3 cannot be chosen from a desk. The spec
//! says as much — its numbers are "des points de départ, pas des cibles" — so
//! this module exists to produce the evidence: every mod, the candidate that
//! was retained, its score, and the margin over the runner-up.
//!
//! Two properties matter and are load-bearing:
//!
//! - **Nothing is written.** No `wiki_link`, no cache, no negative cache. A
//!   calibration run over three hundred mods with thresholds that turn out to
//!   be wrong must leave no trace to undo.
//! - **The rejections are the interesting half.** A report of what matched says
//!   nothing about where the threshold should sit; what says it is the list of
//!   mods rejected for ambiguity, with the two candidates that tied. That is
//!   the section to read first.
//!
//! It is driven by the ignored test at the bottom — the project's convention
//! for anything that needs the real library (`acd.rs`, `driver.rs`), and the
//! only way to run this without an interface. Exposing it as a Tauri command
//! later costs one façade.

use std::path::Path;

use rusqlite::Connection;

use super::api::WikiClient;
use super::clean::{Cleaner, Weights};
use super::matchcar::{self, CarSubject};
use super::matching::{MatchOutcome, Thresholds};
use super::matchtrack::{self, TrackSubject};

/// Courtesy pause between two mods (§6.2: sequential, never a burst). Three
/// hundred mods at two requests each is already the largest thing this feature
/// will ever ask of Wikimedia, and it only ever happens on demand.
const PAUSE_BETWEEN_MODS: std::time::Duration = std::time::Duration::from_millis(100);

/// One line of the report.
pub struct Row {
    pub mod_key: String,
    /// `"Car"` or `"Track"`, as the overlay spells it.
    pub kind: String,
    pub display_name: String,
    /// What was actually searched — the cleaned name, or the coordinates and
    /// where they came from. Half the value of the report is seeing this.
    pub query: String,
    pub outcome: MatchOutcome,
}

pub struct Report {
    pub rows: Vec<Row>,
    pub thresholds: Thresholds,
    /// Mods known to have **no real-world counterpart** — fictional cars and
    /// invented circuits, from the shipped table.
    pub expected_absences: Vec<String>,
    /// True when the run covered only part of the library (`PITBOX_WIKI_LIMIT`).
    /// A partial run cannot tell a typo from a mod it simply did not reach, so
    /// the "clés sans mod" check stays quiet.
    pub partial: bool,
}

/// Everything a run needs, so the loop takes one argument instead of six.
pub struct Calibration<'a> {
    pub net: &'a WikiClient,
    pub cleaner: &'a Cleaner,
    pub weights: &'a Weights,
    pub thresholds: Thresholds,
    /// Assetto Corsa install, for the track coordinates (`sun::track_location`).
    pub ac_install_path: Option<&'a Path>,
    /// The reader's language: searched before English (`lang::search_order`).
    pub locale: &'a str,
    /// Mods known to have no real-world counterpart.
    pub expected_absences: Vec<String>,
}

impl Calibration<'_> {
    /// Runs the whole library. `progress` is called before each mod, so a
    /// caller can print or emit — a closure rather than an `AppHandle`, for the
    /// reason `bulk.rs` documents.
    pub fn run(&self, conn: &Connection, limit: Option<usize>, progress: &mut dyn FnMut(usize, usize, &str)) -> Report {
        let mods = crate::overlay::list_mods(conn).unwrap_or_default();
        let mods: Vec<_> = match limit {
            Some(n) => mods.into_iter().take(n).collect(),
            None => mods,
        };
        let total = mods.len();
        let mut rows = Vec::with_capacity(total);

        for (index, m) in mods.iter().enumerate() {
            progress(index + 1, total, &m.id_interne);
            let display_name = m.display_name.clone().unwrap_or_else(|| m.id_interne.clone());

            let (query, outcome) = if m.kind == "Track" {
                self.one_track(&m.id_interne, &display_name)
            } else {
                self.one_car(m.brand.clone(), &display_name, m.year, m.category.clone())
            };

            rows.push(Row {
                mod_key: m.id_interne.clone(),
                kind: m.kind.clone(),
                display_name,
                query,
                outcome,
            });
            std::thread::sleep(PAUSE_BETWEEN_MODS);
        }

        Report {
            rows,
            thresholds: self.thresholds,
            expected_absences: self.expected_absences.clone(),
            partial: limit.is_some(),
        }
    }

    fn one_car(
        &self,
        brand: Option<String>,
        display_name: &str,
        year: Option<i64>,
        category: Option<String>,
    ) -> (String, MatchOutcome) {
        let subject = CarSubject {
            brand,
            name: Some(display_name.to_string()),
            year,
            category,
        };
        let query = matchcar::search_query(self.cleaner, &subject);
        let outcome = matchcar::match_car(
            self.net,
            self.cleaner,
            self.weights,
            &self.thresholds,
            &subject,
            self.locale,
        );
        (query, outcome)
    }

    fn one_track(&self, track_id: &str, display_name: &str) -> (String, MatchOutcome) {
        // CSP's table first, the mod's geotags second — `sun.rs` already knows
        // how, and knows that Kunos geotags are a placeholder.
        let location = self
            .ac_install_path
            .and_then(|ac| crate::sun::track_location(ac, track_id, None))
            .map(|l| (l.latitude, l.longitude));
        let subject = TrackSubject {
            name: Some(display_name.to_string()),
            location,
        };
        let query = match location {
            Some((lat, lon)) => format!("{lat:.5}, {lon:.5}"),
            None => format!("nom : {}", self.cleaner.clean(display_name)),
        };
        let outcome = matchtrack::match_track(self.net, self.cleaner, &self.thresholds, &subject, self.locale);
        (query, outcome)
    }
}

impl Report {
    fn count(&self, pick: impl Fn(&MatchOutcome) -> bool) -> usize {
        self.rows.iter().filter(|r| pick(&r.outcome)).count()
    }

    /// The report as Markdown.
    ///
    /// Matched rows are sorted by **ascending margin**: the most fragile match
    /// first. Reading the top of that table is how the floor gets chosen —
    /// everything above the first wrong answer is what the threshold should
    /// keep.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        let t = &self.thresholds;
        out.push_str("# Calibration de l'appariement Wikipédia\n\n");
        out.push_str(&format!(
            "Seuils : score min **{}** · marge min **{}** · rayon **{} m** · égalité **{} m**\n\n",
            t.min_score, t.min_margin, t.track_radius_m, t.track_tie_margin_m
        ));
        let matched = self.count(|o| matches!(o, MatchOutcome::Matched { .. }));
        let ambiguous = self.count(|o| matches!(o, MatchOutcome::Ambiguous { .. }));
        let none = self.count(|o| matches!(o, MatchOutcome::NoCandidate));
        let unavailable = self.count(|o| matches!(o, MatchOutcome::Unavailable));
        out.push_str(&format!(
            "{} mods — **{matched} retenus**, {ambiguous} ambigus, {none} sans candidat, {unavailable} réseau indisponible\n\n",
            self.rows.len()
        ));

        out.push_str("## Retenus — marge croissante, les plus fragiles en tête\n\n");
        out.push_str("| mod | type | recherche | entité retenue | score | marge | second |\n");
        out.push_str("|---|---|---|---|---:|---:|---|\n");
        let mut matched_rows: Vec<(&Row, f64)> = self
            .rows
            .iter()
            .filter_map(|r| match &r.outcome {
                // No runner-up means nothing to be wrong about: sorted last.
                MatchOutcome::Matched { margin, .. } => Some((r, margin.unwrap_or(f64::MAX))),
                _ => None,
            })
            .collect();
        matched_rows.sort_by(|a, b| a.1.total_cmp(&b.1));
        for (row, _) in matched_rows {
            let MatchOutcome::Matched {
                candidate,
                runner_up,
                margin,
            } = &row.outcome
            else {
                continue;
            };
            out.push_str(&format!(
                "| `{}` | {} | {} | {} ({}) | {:.3} | {} | {} |\n",
                row.mod_key,
                row.kind,
                row.query,
                candidate.name(),
                candidate.entity_id,
                candidate.score,
                margin.map(|m| format!("{m:.3}")).unwrap_or_else(|| "seul".into()),
                runner_up.as_ref().map(|c| c.name().to_string()).unwrap_or_default(),
            ));
        }

        out.push_str("\n## Ambigus — rejetés par le §1, et la raison du seuil\n\n");
        out.push_str("| mod | type | recherche | premier | second | marge |\n");
        out.push_str("|---|---|---|---|---|---:|\n");
        for row in &self.rows {
            let MatchOutcome::Ambiguous {
                best,
                runner_up,
                margin,
            } = &row.outcome
            else {
                continue;
            };
            out.push_str(&format!(
                "| `{}` | {} | {} | {} ({}, {:.3}) | {} ({}, {:.3}) | {margin:.3} |\n",
                row.mod_key,
                row.kind,
                row.query,
                best.name(),
                best.entity_id,
                best.score,
                runner_up.name(),
                runner_up.entity_id,
                runner_up.score,
            ));
        }

        // **The two populations that must never be added together.** A
        // fictional car finding nothing is a success; a real one finding
        // nothing is a miss. Merged, they made one number that said nothing —
        // and hid the Ferrari SF15-T, whose article existed all along.
        let expected = |key: &str| self.expected_absences.iter().any(|k| k == key);
        let absent: Vec<&Row> = self
            .rows
            .iter()
            .filter(|r| matches!(r.outcome, MatchOutcome::NoCandidate))
            .collect();
        let (known, missed): (Vec<&Row>, Vec<&Row>) = absent.into_iter().partition(|r| expected(&r.mod_key));

        out.push_str(&format!(
            "\n## Manques — un article existe peut-être, on ne l'a pas trouvé ({})\n\n",
            missed.len()
        ));
        out.push_str("| mod | type | recherche |\n|---|---|---|\n");
        for row in &missed {
            out.push_str(&format!("| `{}` | {} | {} |\n", row.mod_key, row.kind, row.query));
        }

        if !self.expected_absences.is_empty() {
            out.push_str(&format!(
                "\n## Absences attendues — rien à trouver, et rien trouvé ({})\n\n",
                known.len()
            ));
            for row in &known {
                out.push_str(&format!("- `{}` — {}\n", row.mod_key, row.display_name));
            }
            // An expected absence that came back matched is a certain false
            // positive: no opinion needed, the answer is known to be wrong.
            let contradicted: Vec<&Row> = self
                .rows
                .iter()
                .filter(|r| expected(&r.mod_key) && matches!(r.outcome, MatchOutcome::Matched { .. }))
                .collect();
            if !contradicted.is_empty() {
                out.push_str(&format!(
                    "\n### Attendus absents, pourtant appariés ({}) — faux positifs certains\n\n",
                    contradicted.len()
                ));
                for row in &contradicted {
                    if let MatchOutcome::Matched { candidate, .. } = &row.outcome {
                        out.push_str(&format!("- `{}` → {}\n", row.mod_key, candidate.name()));
                    }
                }
            }
        }

        // **A curated key that matches no mod does nothing, silently.** A typo
        // in `wiki-links.json` is invisible otherwise: no error, no row, just
        // an appariement that never appears. The run has the whole library in
        // hand, so it is the only place that can say so.
        let curated = if self.partial {
            crate::wiki::curated::CuratedLinks::default()
        } else {
            crate::wiki::curated::shipped()
        };
        let unknown: Vec<&String> = curated
            .links
            .keys()
            .chain(curated.no_counterpart.iter())
            .filter(|key| !self.rows.iter().any(|r| r.mod_key == **key))
            .collect();
        if !unknown.is_empty() {
            out.push_str(&format!(
                "\n## Clés livrées sans mod correspondant ({}) — coquille probable\n\n",
                unknown.len()
            ));
            for key in unknown {
                out.push_str(&format!("- `{key}`\n"));
            }
        }

        if unavailable > 0 {
            out.push_str("\n## Réseau indisponible — à rejouer, ce ne sont pas des verdicts\n\n");
            for row in &self.rows {
                if matches!(row.outcome, MatchOutcome::Unavailable) {
                    out.push_str(&format!("- `{}`\n", row.mod_key));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wiki::matching::Candidate;

    fn thresholds() -> Thresholds {
        Thresholds {
            min_score: 0.55,
            min_margin: 0.12,
            track_radius_m: 5_000,
            track_tie_margin_m: 150.0,
        }
    }

    /// Rule (§13): the report shows what the threshold has to arbitrate — the
    /// retained candidate, its score, its margin, and the ambiguities that were
    /// refused. A report without the refusals cannot calibrate anything.
    #[test]
    fn the_report_shows_the_matches_and_the_refusals() {
        let candidate = |id: &str, score: f64| Candidate {
            entity_id: id.into(),
            label: Some(format!("Label {id}")),
            description: None,
            score,
        };
        let report = Report {
            thresholds: thresholds(),
            expected_absences: vec!["rss_formula_hybrid".into()],
            partial: true,
            rows: vec![
                Row {
                    mod_key: "ks_toyota_ae86".into(),
                    kind: "Car".into(),
                    display_name: "Toyota AE86".into(),
                    query: "Toyota AE86".into(),
                    outcome: MatchOutcome::Matched {
                        candidate: candidate("Q1377219", 0.91),
                        runner_up: Some(candidate("Q2", 0.40)),
                        margin: Some(0.51),
                    },
                },
                Row {
                    mod_key: "some_supra".into(),
                    kind: "Car".into(),
                    display_name: "Supra".into(),
                    query: "Toyota Supra".into(),
                    outcome: MatchOutcome::Ambiguous {
                        best: candidate("Q3", 0.80),
                        runner_up: candidate("Q4", 0.78),
                        margin: 0.02,
                    },
                },
            ],
        };

        let md = report.to_markdown();
        assert!(md.contains("**1 retenus**"), "le décompte des retenus");
        assert!(md.contains("1 ambigus"), "et celui des refus");
        assert!(md.contains("Q1377219"), "l'entité retenue");
        assert!(md.contains("0.510"), "sa marge");
        assert!(md.contains("Q4"), "le second candidat d'une ambiguïté");
    }

    /// **Calibration run, by hand.** Needs the real library and the network, so
    /// it is ignored — §11 forbids a test that depends on Wikipedia.
    ///
    /// ```text
    /// PITBOX_WIKI_CALIBRATE=1 cargo test --lib wiki -- --ignored --nocapture calibrate_the_library
    /// ```
    ///
    /// Reads the app's own `config.json` for the Assetto Corsa path, and the
    /// overlay next to it. `PITBOX_WIKI_LIMIT=20` shortens a first pass;
    /// `PITBOX_WIKI_REPORT` chooses where the Markdown lands (default: the
    /// config directory).
    #[test]
    #[ignore = "runs the whole library against Wikidata; calibration, not a check"]
    fn calibrate_the_library() {
        let config_dir = dirs::config_dir()
            .map(|d| d.join("com.pitbox.app"))
            .expect("dossier de config");
        let cfg: crate::config::AppConfig = std::fs::read_to_string(config_dir.join("config.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let conn = crate::overlay::open(&config_dir.join("overlay.sqlite")).expect("overlay");
        let matching = crate::wiki::clean::load(&config_dir);
        let cleaner = Cleaner::new(&matching);
        let net = WikiClient::new();
        let limit = std::env::var("PITBOX_WIKI_LIMIT").ok().and_then(|v| v.parse().ok());

        let calibration = Calibration {
            net: &net,
            cleaner: &cleaner,
            weights: &matching.weights,
            thresholds: Thresholds::from_prefs(&cfg.prefs),
            ac_install_path: cfg.ac_install_path.as_deref(),
            // La langue de lecture décide du wiki interrogé en premier
            // (`lang::search_order`). Celle des réglages, français par défaut.
            locale: cfg.prefs.language.as_deref().unwrap_or("fr"),
            // Livrés avec l'application (`rules/wiki-links.json`) : rien à
            // poser dans le dossier de config, et la liste se versionne avec
            // le code plutôt que de vivre chez un seul utilisateur.
            expected_absences: crate::wiki::curated::shipped().no_counterpart,
        };

        let report = calibration.run(&conn, limit, &mut |done, total, id| {
            eprintln!("[{done}/{total}] {id}");
        });

        let path = std::env::var("PITBOX_WIKI_REPORT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| config_dir.join("wiki-calibration.md"));
        std::fs::write(&path, report.to_markdown()).expect("écriture du rapport");
        eprintln!("\nRapport : {}", path.display());
    }
}
