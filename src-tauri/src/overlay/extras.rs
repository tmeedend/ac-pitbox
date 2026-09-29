//! What a mod puts outside `content/<type>/<id>`: replaced game files (§4.5.4),
//! additions to the game (§4.5.3) and the placements explicitly allowed (§4.6ter).

use super::*;

// --- Fichiers du jeu remplacés (§4.5.4) ---------------------------------------

/// Enregistre la sauvegarde de l'original. `INSERT OR IGNORE` : la **première**
/// sauvegarde fait foi, un second mod visant le même chemin ne l'écrase pas.
pub fn add_game_backup(conn: &Connection, ac_path: &str, backup_path: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO game_backups (ac_path, backup_path, created_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![ac_path, backup_path, chrono::Local::now().to_rfc3339()],
    )?;
    Ok(())
}

pub fn game_backup_of(conn: &Connection, ac_path: &str) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT backup_path FROM game_backups WHERE ac_path = ?1")?;
    let mut rows = stmt.query_map([ac_path], |r| r.get::<_, String>(0))?;
    rows.next().transpose()
}

pub fn remove_game_backup(conn: &Connection, ac_path: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM game_backups WHERE ac_path = ?1", [ac_path])?;
    Ok(())
}

pub fn list_game_backups(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT ac_path, backup_path FROM game_backups")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    rows.collect()
}

// --- Ajouts au jeu posés dans AC (§4.5.3) ----------------------------------

/// Remplace la liste des ajouts posés pour un mod (liste vide = plus rien
/// de posé). Réécriture complète : c'est l'état du disque après l'opération qui
/// est mémorisé, jamais un cumul. `(chemin, est_un_dossier_créé)`.
pub fn set_extra_links(
    conn: &Connection,
    mod_id: &str,
    kind: &str,
    entries: &[(String, bool)],
) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM extra_links WHERE mod_id = ?1", [mod_id])?;
    let now = chrono::Local::now().to_rfc3339();
    for (p, is_dir) in entries {
        conn.execute(
            "INSERT OR IGNORE INTO extra_links (mod_id, ac_path, is_dir, kind, claimed_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![mod_id, p, *is_dir as i64, kind, now],
        )?;
    }
    Ok(())
}

/// Ce qu'un mod réclame — `(ac_path, is_dir, kind)`. Le `kind` est celui qu'a
/// écrit [`set_extra_links`], donc la forme `content_folder()` ("cars"/"tracks")
/// : il faut le rendre avec le reste, parce que le retrait doit retrouver
/// l'exemplaire en bibliothèque **avant** d'effacer la réclamation — après, plus
/// rien en base ne dit de quel arbre il venait.
pub fn get_extra_links(conn: &Connection, mod_id: &str) -> rusqlite::Result<Vec<(String, bool, String)>> {
    let mut stmt = conn.prepare("SELECT ac_path, is_dir, kind FROM extra_links WHERE mod_id = ?1")?;
    let rows = stmt.query_map([mod_id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? != 0, r.get::<_, String>(2)?))
    })?;
    rows.collect()
}

/// One claim on an AC path, as `extra_links` stores it (§4.5.3).
#[derive(Debug, Clone)]
pub struct ExtraLinkRow {
    pub mod_id: String,
    pub ac_path: String,
    pub is_dir: bool,
    /// [`crate::extras::OwnerKind::category`], as written by [`set_extra_links`].
    pub kind: String,
    /// This claim provides the copy laid in AC (at most one per path).
    pub provided: bool,
}

/// Every claim of every mod, in one read - the game folder scan (DOSSIER§5.1)
/// crosses them with the disk instead of asking path by path.
pub fn list_all_extra_links(conn: &Connection) -> rusqlite::Result<Vec<ExtraLinkRow>> {
    let mut stmt = conn.prepare("SELECT mod_id, ac_path, is_dir, kind, provided FROM extra_links")?;
    let rows = stmt.query_map([], |r| {
        Ok(ExtraLinkRow {
            mod_id: r.get(0)?,
            ac_path: r.get(1)?,
            is_dir: r.get::<_, i64>(2)? != 0,
            kind: r.get(3)?,
            provided: r.get::<_, i64>(4)? != 0,
        })
    })?;
    rows.collect()
}

/// Mods qui réclament ce fichier d'AC — `(mod_id, kind, claimed_at)`. C'est le
/// compteur de références des fichiers partagés (§4.5.4) : tant qu'il reste au
/// moins une ligne, le fichier est encore réclamé et ne doit pas être retiré
/// d'AC. `claimed_at` départage deux exemplaires de même date de modification :
/// le dernier mod installé gagne.
pub fn extra_claimants(conn: &Connection, ac_path: &str) -> rusqlite::Result<Vec<(String, String, String)>> {
    let mut stmt =
        conn.prepare("SELECT mod_id, kind, claimed_at FROM extra_links WHERE ac_path = ?1 AND is_dir = 0")?;
    let rows = stmt.query_map([ac_path], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
    })?;
    rows.collect()
}

/// Mod dont l'exemplaire est actuellement posé dans AC à ce chemin.
pub fn extra_provider(conn: &Connection, ac_path: &str) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT mod_id FROM extra_links WHERE ac_path = ?1 AND provided = 1")?;
    let mut rows = stmt.query_map([ac_path], |r| r.get::<_, String>(0))?;
    rows.next().transpose()
}

/// Désigne le mod qui fournit désormais ce chemin — au plus un à la fois.
pub fn set_extra_provider(conn: &Connection, ac_path: &str, mod_id: &str) -> rusqlite::Result<()> {
    conn.execute("UPDATE extra_links SET provided = 0 WHERE ac_path = ?1", [ac_path])?;
    conn.execute(
        "UPDATE extra_links SET provided = 1 WHERE ac_path = ?1 AND mod_id = ?2",
        [ac_path, mod_id],
    )?;
    Ok(())
}

// --- Poses explicitement autorisees (§4.6ter) -------------------------------

/// Enregistre qu'un mod a l'autorisation de poser ce chemin quoi qu'il occupe
/// deja. Idempotent.
pub fn mark_forced_extra(conn: &Connection, mod_id: &str, ac_path: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO forced_extras (mod_id, ac_path) VALUES (?1, ?2)",
        params![mod_id, ac_path],
    )?;
    Ok(())
}

pub fn is_forced_extra(conn: &Connection, mod_id: &str, ac_path: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM forced_extras WHERE mod_id = ?1 AND ac_path = ?2",
        params![mod_id, ac_path],
        |r| r.get::<_, i64>(0),
    )
    .map(|n| n > 0)
    .unwrap_or(false)
}

/// Every explicit authorisation (§4.6ter): `(mod_id, ac_path)`.
pub fn list_forced_extras(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT mod_id, ac_path FROM forced_extras")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    rows.collect()
}

/// Retire les autorisations d'un mod — a la suppression du mod, jamais a sa
/// desactivation : desactiver puis reactiver ne doit pas reposer la question.
pub fn clear_forced_extras(conn: &Connection, mod_id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM forced_extras WHERE mod_id = ?1", [mod_id])?;
    Ok(())
}
