//! Content found in `content/` (§8.1): the game's own cars and tracks, and
//! those installed outside the app.

use super::*;

/// Indexe une voiture/circuit vivant dans `content/` : ligne minimale
/// `is_stock=1` (lecture seule). Ne touche pas un mod déjà présent (un vrai mod
/// géré n'est jamais « stock »).
///
/// `unmanaged` distingue le mod installé hors Pit Box du contenu de base
/// (§8.2). C'est le **seul** champ réécrit sur une ligne déjà indexée :
/// c'est ce qui reclasse les bases d'avant cette distinction — où tout ce qui
/// traînait dans `content/` passait pour du Kunos — sans perdre ce que
/// l'utilisateur y a saisi (nom repris à la main, description, tags manuels,
/// favori). Le `WHERE is_stock` protège d'une reclassification accidentelle
/// d'un mod géré qui porterait le même id.
pub fn upsert_stock_mod(
    conn: &Connection,
    id: &str,
    kind: &str,
    brand: Option<&str>,
    name: Option<&str>,
    created_at: &str,
    unmanaged: bool,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO mods (id_interne, kind, brand, display_name, is_stock, is_unmanaged, created_at)
           VALUES (?1, ?2, ?3, ?4, 1, ?6, ?5)
           ON CONFLICT(id_interne) DO UPDATE SET is_unmanaged = ?6 WHERE is_stock = 1"#,
        params![id, kind, brand, name, created_at, unmanaged as i64],
    )?;
    Ok(())
}

/// Efface les **versions** synthétiques du contenu de base, en gardant les
/// lignes `mods` (§8.1). C'est ce qui permet de réindexer sans détruire ce
/// que l'utilisateur a mis dans l'overlay — nom repris à la main, description,
/// tags manuels, favori, catégorie.
///
/// Les versions, elles, se refabriquent à chaque passage avec un nouvel UUID :
/// sans cet effacement elles s'accumuleraient à chaque réindexation.
pub fn clear_stock_versions(conn: &Connection) -> rusqlite::Result<usize> {
    // `active_version_id` d'abord : la contrainte ne l'impose pas, mais laisser
    // un mod pointer une version qui vient d'être supprimée le rendrait
    // brièvement incohérent si l'indexation s'interrompait ici.
    conn.execute("UPDATE mods SET active_version_id = NULL WHERE is_stock = 1", [])?;
    conn.execute(
        "DELETE FROM versions WHERE mod_id IN (SELECT id_interne FROM mods WHERE is_stock = 1)",
        [],
    )
}

/// Supprime les entrées de contenu de base **absentes de la liste** — celles
/// dont le dossier n'est plus dans `content/`. Complément de
/// `clear_stock_versions` : une réindexation qui ne détruit plus tout doit
/// quand même faire disparaître ce qui a été désinstallé du jeu.
pub fn delete_stock_absent(conn: &Connection, present: &[String]) -> rusqlite::Result<usize> {
    // Liste vide = plus rien sur disque : `NOT IN ()` étant invalide en SQL,
    // le cas se traite à part plutôt que de construire une requête bancale.
    if present.is_empty() {
        return delete_all_stock(conn);
    }
    let placeholders = std::iter::repeat_n("?", present.len()).collect::<Vec<_>>().join(",");
    let params = rusqlite::params_from_iter(present.iter());
    conn.execute(
        &format!("DELETE FROM history WHERE mod_id IN (SELECT id_interne FROM mods WHERE is_stock = 1 AND id_interne NOT IN ({placeholders}))"),
        rusqlite::params_from_iter(present.iter()),
    )?;
    conn.execute(
        &format!("DELETE FROM mods WHERE is_stock = 1 AND id_interne NOT IN ({placeholders})"),
        params,
    )
}

pub(super) fn delete_all_stock(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM history WHERE mod_id IN (SELECT id_interne FROM mods WHERE is_stock = 1)",
        [],
    )?;
    conn.execute("DELETE FROM mods WHERE is_stock = 1", [])
}

/// Supprime toutes les entrées de contenu de base (ré-indexation depuis zéro).
/// Les versions associées tombent par CASCADE. Les vrais mods ne sont pas touchés.
///
/// **Détruit aussi ce que l'utilisateur a saisi** sur ce contenu (nom,
/// description, tags manuels, favori) : réservé à la réinitialisation
/// explicitement demandée, jamais au réindex ordinaire (§5bis.3).
pub fn clear_stock(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM history WHERE mod_id IN (SELECT id_interne FROM mods WHERE is_stock = 1)",
        [],
    )?;
    conn.execute(
        "DELETE FROM tech_user WHERE mod_id IN (SELECT id_interne FROM mods WHERE is_stock = 1)",
        [],
    )?;
    let n = conn.execute("DELETE FROM mods WHERE is_stock = 1", [])?;
    Ok(n)
}

/// Nombre d'entrées de contenu de base déjà indexées — sert à déclencher un
/// scan automatique au premier démarrage (§8.1).
pub fn count_stock(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("SELECT COUNT(*) FROM mods WHERE is_stock = 1", [], |r| r.get(0))
}

/// Ids indexés depuis `content/`, avec leur type — de quoi rejuger leur
/// classement sans relire le disque ([`crate::stock::reclassify_indexed_content`]).
pub fn list_stock_ids(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT id_interne, kind FROM mods WHERE is_stock = 1")?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    rows.collect()
}

/// Repositionne le seul drapeau `is_unmanaged` d'une entrée déjà indexée
/// (§8.2). N'écrit rien d'autre : c'est ce qui permet de reclasser une
/// base existante sans toucher aux saisies de l'utilisateur.
pub fn set_unmanaged(conn: &Connection, id: &str, unmanaged: bool) -> rusqlite::Result<usize> {
    conn.execute(
        // `is_unmanaged != ?2` : le nombre de lignes touchées devient le nombre de
        // reclassements réels, pas le nombre d'entrées examinées.
        "UPDATE mods SET is_unmanaged = ?2 WHERE id_interne = ?1 AND is_stock = 1 AND is_unmanaged != ?2",
        params![id, unmanaged as i64],
    )
}
