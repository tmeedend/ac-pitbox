//! Appariements livrés avec l'application (WIKI§10).
//!
//! The spec prepares for sharing matches between installations and makes
//! `source = import` the mechanism. This is that mechanism's first use, and the
//! most useful one: the maintainer corrects a mod once, the correction ships in
//! the binary, and nobody else has to do the work again.
//!
//! Three properties come free from the precedence of WIKI§3.1, and they are the
//! reason this is safe:
//!
//! - **A shipped match never overwrites a local correction.** `manual` outranks
//!   `import`, so a user who picked a different article keeps it — including
//!   across updates that change this file.
//! - **It does outrank the automatic matching.** A curated entry is a human
//!   decision; `auto` gives way to it.
//! - **It is reversible.** Removing the entry from the file, or correcting it
//!   locally, is all it takes.
//!
//! **On WIKI§10's "import toujours explicite, jamais automatique".** That rule
//! guards against a *third party's* file being trusted silently, and its
//! companion demand — strict validation, URL allowlists against homographs —
//! exists for the same reason. A table compiled into the application is not
//! that: it is application data, like `default-tag-rules.json`, and it carries
//! exactly the trust the binary already carries. The Q-id format check below
//! stays anyway, because it costs nothing and this file is edited by hand.

use std::collections::BTreeMap;

use rusqlite::Connection;
use serde::Deserialize;

use super::api::is_entity_id;
use super::store::{self, LinkSource};

const SEED: &str = include_str!("../../rules/wiki-links.json");

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CuratedLinks {
    /// Mod folder name to Wikidata id.
    #[serde(default)]
    pub links: BTreeMap<String, String>,
    /// Mods known to have no real-world counterpart. Calibration only — this
    /// one changes nothing about how the application behaves.
    #[serde(default)]
    pub no_counterpart: Vec<String>,
}

/// The shipped table. Read-only: a user's own corrections live in the overlay
/// as `manual`, not in a copy of this file that a later release would leave
/// stale.
pub fn shipped() -> CuratedLinks {
    serde_json::from_str(SEED).expect("le fichier wiki-links.json embarqué doit être valide")
}

/// Writes the shipped matches into the overlay, respecting precedence.
///
/// Returns how many rows were actually written — entries refused because a
/// local `manual` correction already stands are counted as skipped, which is
/// the normal and wanted outcome.
pub fn seed_links(conn: &Connection, curated: &CuratedLinks) -> (usize, usize) {
    let mut written = 0;
    let mut skipped = 0;
    for (mod_key, entity_id) in &curated.links {
        if !is_entity_id(entity_id) {
            log::warn!("wiki: {mod_key} → {entity_id:?} n'est pas un identifiant Wikidata, ignoré");
            skipped += 1;
            continue;
        }
        match store::set_link(conn, mod_key, entity_id, LinkSource::Import) {
            Ok(true) => written += 1,
            // Refused by precedence: the user corrected this one by hand.
            Ok(false) => skipped += 1,
            Err(e) => {
                log::warn!("wiki: appariement livré {mod_key} non écrit — {e}");
                skipped += 1;
            }
        }
    }
    (written, skipped)
}

/// Le fichier livré, tel qu'il doit être recollé dans le dépôt, avec les
/// corrections locales fondues dedans (WIKI§10).
///
/// **Seules les corrections `manual` sont exportées.** Jamais les `auto`, et ce
/// n'est pas une question de propreté : la précédence du WIKI§3.1 fait qu'un
/// `import` l'emporte sur un `auto`, donc exporter les verdicts de
/// l'algorithme d'aujourd'hui les figerait dans le binaire — et une version
/// future, au moteur meilleur, se ferait écraser par ses propres vieilles
/// réponses. Le piège est silencieux et durable.
///
/// Les entrées déjà livrées sont conservées ; une correction locale portant sur
/// la même clé l'emporte, parce qu'elle est plus récente et faite à la main.
pub fn export_links(conn: &Connection, curated: &CuratedLinks) -> rusqlite::Result<String> {
    let mut links = curated.links.clone();
    let mut stmt = conn.prepare("SELECT mod_key, entity_id FROM wiki_link WHERE source = 'manual' ORDER BY mod_key")?;
    let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
    let mut added = 0;
    for row in rows {
        let (mod_key, entity_id) = row?;
        if !is_entity_id(&entity_id) {
            continue;
        }
        if links.insert(mod_key, entity_id).is_none() {
            added += 1;
        }
    }
    log::debug!("wiki: export de {} appariements ({added} nouveaux)", links.len());

    // Écrit à la main plutôt que par `serde_json` : le fichier du dépôt porte
    // des commentaires `_comment_*` que l'utilisateur relit, et un export qui
    // les effacerait rendrait la table illisible au bout de deux tours.
    let mut out = String::from("{\n");
    out.push_str(
        "  \"_comment\": \"Appariements Wikipédia livrés avec l'application (WIKI§10). \
Généré par Réglages › Wikipédia › Exporter mes corrections, puis recollé ici. \
Ne contient que des corrections faites à la main : les verdicts automatiques n'y ont pas leur place, \
ils écraseraient un futur moteur meilleur.\",\n\n",
    );
    out.push_str("  \"links\": {\n");
    let total = links.len();
    for (index, (mod_key, entity_id)) in links.iter().enumerate() {
        let comma = if index + 1 < total { "," } else { "" };
        out.push_str(&format!(
            "    {}: {}{comma}\n",
            json_string(mod_key),
            json_string(entity_id)
        ));
    }
    out.push_str("  },\n\n");

    out.push_str("  \"no_counterpart\": [\n");
    let total = curated.no_counterpart.len();
    for (index, key) in curated.no_counterpart.iter().enumerate() {
        let comma = if index + 1 < total { "," } else { "" };
        out.push_str(&format!("    {}{comma}\n", json_string(key)));
    }
    out.push_str("  ]\n}\n");
    Ok(out)
}

/// Échappe une chaîne en littéral JSON. `serde_json` le fait très bien, et
/// c'est lui qu'on appelle — la fonction existe pour que le rendu à la main
/// ci-dessus reste lisible.
fn json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rule (WIKI§10): the shipped file parses, and every match in it is a Q-id.
    /// It is embedded with `include_str!`, so a typo would otherwise only
    /// surface at runtime — on a user's machine.
    #[test]
    fn the_shipped_table_is_valid() {
        let curated = shipped();
        assert!(!curated.links.is_empty(), "des appariements livrés");
        for (mod_key, entity_id) in &curated.links {
            assert!(is_entity_id(entity_id), "{mod_key} → {entity_id} n'est pas un Q-id");
        }
        for key in &curated.no_counterpart {
            assert!(!key.trim().is_empty(), "pas de ligne vide dans no_counterpart");
        }
    }

    /// Rule (WIKI§3.1, WIKI§10): **a shipped match never overwrites a local
    /// correction**, and does replace an automatic one.
    ///
    /// This is the property that makes shipping the table safe at all: an
    /// update may change any entry here, and a user who chose a different
    /// article keeps theirs.
    #[test]
    fn a_shipped_match_yields_to_a_local_correction() {
        let base = crate::testutil::temp_dir("wiki-curated");
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).expect("overlay");

        // Une correction locale, et un appariement automatique.
        store::set_link(&conn, "corrected_by_hand", "Q1", LinkSource::Manual).unwrap();
        store::set_link(&conn, "found_by_the_engine", "Q2", LinkSource::Auto).unwrap();

        let curated = CuratedLinks {
            links: BTreeMap::from([
                ("corrected_by_hand".into(), "Q999".into()),
                ("found_by_the_engine".into(), "Q888".into()),
                ("brand_new".into(), "Q777".into()),
            ]),
            no_counterpart: Vec::new(),
        };
        let (written, skipped) = seed_links(&conn, &curated);

        assert_eq!(written, 2, "l'automatique et le nouveau");
        assert_eq!(skipped, 1, "la correction locale n'est pas touchée");
        assert_eq!(
            store::get_link(&conn, "corrected_by_hand").unwrap().unwrap().entity_id,
            "Q1",
            "la correction locale survit à la table livrée"
        );
        assert_eq!(
            store::get_link(&conn, "found_by_the_engine")
                .unwrap()
                .unwrap()
                .entity_id,
            "Q888",
            "une décision humaine l'emporte sur l'appariement automatique"
        );
    }

    /// Rule (WIKI§10): **the export carries the hand-made corrections and nothing
    /// else.**
    ///
    /// Exporting the automatic verdicts would be the quiet kind of mistake: an
    /// `import` outranks an `auto` (WIKI§3.1), so today's algorithm would be frozen
    /// into the binary and would overrule a future, better one with its own old
    /// answers. Nobody would notice for a release or two.
    #[test]
    fn the_export_carries_manual_corrections_only() {
        let base = crate::testutil::temp_dir("wiki-export");
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).expect("overlay");

        store::set_link(&conn, "chosen_by_hand", "Q111", LinkSource::Manual).unwrap();
        store::set_link(&conn, "found_by_the_engine", "Q222", LinkSource::Auto).unwrap();
        // Une correction locale qui contredit la table livrée : c'est la plus
        // récente et elle est humaine, elle gagne.
        store::set_link(&conn, "shipped_but_corrected", "Q333", LinkSource::Manual).unwrap();

        let curated = CuratedLinks {
            links: BTreeMap::from([
                ("shipped_but_corrected".into(), "Q999".into()),
                ("shipped_elsewhere".into(), "Q444".into()),
            ]),
            no_counterpart: vec!["rss_formula_2013".into()],
        };
        let json = export_links(&conn, &curated).unwrap();

        assert!(json.contains("\"chosen_by_hand\": \"Q111\""), "la correction manuelle");
        assert!(
            !json.contains("found_by_the_engine"),
            "un verdict automatique n'est jamais exporté"
        );
        assert!(
            json.contains("\"shipped_but_corrected\": \"Q333\""),
            "la correction locale l'emporte sur l'entrée livrée"
        );
        assert!(
            json.contains("\"shipped_elsewhere\": \"Q444\""),
            "le reste de la table survit"
        );
        assert!(json.contains("rss_formula_2013"), "les absences attendues aussi");

        // Et le résultat doit être relisible par le chargeur qui le consommera.
        let back: CuratedLinks = serde_json::from_str(&json).expect("export relisible");
        assert_eq!(back.links.len(), 3);
        assert_eq!(back.no_counterpart, vec!["rss_formula_2013".to_string()]);
    }

    /// Rule (WIKI§1, WIKI§10): a malformed entry is skipped, never written. The file is
    /// hand-edited, and a URL pasted where a Q-id belongs is the likely slip.
    #[test]
    fn a_malformed_entry_is_skipped_not_written() {
        let base = crate::testutil::temp_dir("wiki-curated-bad");
        let conn = crate::overlay::open(&base.join("overlay.sqlite")).expect("overlay");

        let curated = CuratedLinks {
            links: BTreeMap::from([("pasted_a_url".into(), "https://fr.wikipedia.org/wiki/Abarth_500".into())]),
            no_counterpart: Vec::new(),
        };
        let (written, skipped) = seed_links(&conn, &curated);

        assert_eq!((written, skipped), (0, 1));
        assert!(
            store::get_link(&conn, "pasted_a_url").unwrap().is_none(),
            "rien d'écrit"
        );
    }
}
