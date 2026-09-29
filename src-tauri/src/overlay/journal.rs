//! The import journal: decisions the app took alone during an import (§4.6).

use super::*;

/// Une décision prise seule par l'**app** pendant un import (§4.6).
///
/// À ne pas confondre avec `importer::ImportDecision`, qui est la décision de
/// l'**utilisateur** sur un cas ambigu (§4.4). Les deux existent parce que la
/// ligne entre elles est le vrai choix de conception : l'app tranche tout ce
/// qui est déterminable depuis le disque et en **rend compte** ici ; elle ne
/// demande que ce dont la réponse est dans la tête de l'utilisateur.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ImportJournalEntry {
    /// Clé i18n courte (`pathNormalized`, `pathRefused`, `leftoverAttached`…) —
    /// jamais une phrase : le libellé appartient au frontend.
    pub kind: String,
    /// Ce sur quoi la décision a porté : le chemin du reste, tel qu'il était
    /// dans l'archive.
    pub subject: String,
    /// Ce qui en a été fait — destination, mod de rattachement. `None` quand la
    /// nature de la décision se suffit à elle-même.
    pub detail: Option<String>,
    pub archive: String,
    pub decided_at: String,
}

/// Enregistre une décision. **Best-effort assumé** : le journal explique
/// l'import, il ne le conditionne pas — un échec d'écriture ne doit jamais
/// faire échouer un rangement qui, lui, a réussi.
pub fn record_decision(
    conn: &Connection,
    mod_id: Option<&str>,
    archive: &str,
    kind: &str,
    subject: &str,
    detail: Option<&str>,
) {
    let now = chrono::Local::now().to_rfc3339();
    if let Err(e) = conn.execute(
        "INSERT INTO import_decisions (mod_id, archive, kind, subject, detail, decided_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![mod_id, archive, kind, subject, detail, now],
    ) {
        log::warn!("record_decision {kind} {subject}: {e}");
    }
}

/// Décisions rattachées à un mod, les plus récentes d'abord — c'est le dernier
/// import qui intéresse, pas le premier.
pub fn decisions_for_mod(conn: &Connection, mod_id: &str) -> rusqlite::Result<Vec<ImportJournalEntry>> {
    let mut stmt = conn.prepare(
        "SELECT kind, subject, detail, archive, decided_at FROM import_decisions
         WHERE mod_id = ?1 ORDER BY id DESC",
    )?;
    let rows = stmt.query_map([mod_id], |r| {
        Ok(ImportJournalEntry {
            kind: r.get(0)?,
            subject: r.get(1)?,
            detail: r.get(2)?,
            archive: r.get(3)?,
            decided_at: r.get(4)?,
        })
    })?;
    rows.collect()
}

/// Efface les décisions d'un mod avant de réenregistrer celles d'un nouvel
/// import : sans ça, réimporter un mod corrigé laisserait à l'écran
/// l'explication de l'import fautif, indéfiniment.
pub fn clear_decisions(conn: &Connection, mod_id: &str) {
    if let Err(e) = conn.execute("DELETE FROM import_decisions WHERE mod_id = ?1", [mod_id]) {
        log::warn!("clear_decisions {mod_id}: {e}");
    }
}

/// Efface les décisions d'une archive avant de rejouer son balayage.
///
/// Le nettoyage par mod ne suffit pas : réimporter une archive **à l'identique**
/// classe ses mods en doublons, ce qui court-circuite leur écriture overlay —
/// mais le balayage des restes, lui, tourne quand même et réenregistre tout.
/// Sans ce second nettoyage, chaque réimport empilait un exemplaire de plus de
/// la même décision. C'est aussi le seul moyen d'oublier les décisions qui ne
/// se rattachent à aucun mod (`mod_id` nul).
pub fn clear_decisions_for_archive(conn: &Connection, archive: &str) {
    if let Err(e) = conn.execute("DELETE FROM import_decisions WHERE archive = ?1", [archive]) {
        log::warn!("clear_decisions_for_archive {archive}: {e}");
    }
}
