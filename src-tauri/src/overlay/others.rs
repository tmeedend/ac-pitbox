//! « Other » mods (§7.3).

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtherModRow {
    pub id: String,
    pub library_path: String,
    pub source_archive: Option<String>,
    pub imported_at: String,
    pub is_priority: bool,
    pub is_active: bool,
    /// Chemins absolus des jonctions créées lors de la dernière activation.
    pub junctions: Vec<String>,
    /// Nom repris à la main (REFONTE§6.1) : ces mods-là portent des noms
    /// d'archive (`policeman__ext_config.ini`), c'est-à-dire rien de lisible.
    pub display_name_user: Option<String>,
    /// Note libre (REFONTE§9).
    pub notes_user: Option<String>,
    /// Rattachement corrigé à la main (REFONTE§2.3) : id de l'entité visée. Seule la
    /// **correction** est stockée — la déduction se recalcule à chaque lecture,
    /// `attach.rs` dit pourquoi.
    pub attachment_user: Option<String>,
    /// [`CONTENT_FULL`] or [`CONTENT_SKELETON`]: an "other" mod can exist
    /// without its files, its folder reduced to the showcase manifest
    /// (ESPACE§5.6).
    pub content_state: String,
}

impl OtherModRow {
    pub fn is_skeleton(&self) -> bool {
        self.content_state == CONTENT_SKELETON
    }
}

pub fn insert_other_mod(
    conn: &Connection,
    id: &str,
    library_path: &str,
    source_archive: Option<&str>,
    imported_at: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO other_mods (id, library_path, source_archive, imported_at)
           VALUES (?1, ?2, ?3, ?4)"#,
        params![id, library_path, source_archive, imported_at],
    )?;
    Ok(())
}

pub(super) fn map_other(row: &rusqlite::Row) -> rusqlite::Result<OtherModRow> {
    let junctions: String = row.get(6)?;
    Ok(OtherModRow {
        id: row.get(0)?,
        library_path: row.get(1)?,
        source_archive: row.get(2)?,
        imported_at: row.get(3)?,
        is_priority: row.get::<_, i64>(4)? != 0,
        is_active: row.get::<_, i64>(5)? != 0,
        junctions: json_arr(&junctions),
        display_name_user: row.get(7)?,
        notes_user: row.get(8)?,
        attachment_user: row.get(9)?,
        content_state: row.get(10)?,
    })
}

pub(super) const OTHER_SELECT: &str =
    "SELECT id, library_path, source_archive, imported_at, is_priority, is_active, junctions, display_name_user, notes_user, attachment_user, content_state FROM other_mods";

pub fn list_other_mods(conn: &Connection) -> rusqlite::Result<Vec<OtherModRow>> {
    let mut stmt = conn.prepare(&format!("{OTHER_SELECT} ORDER BY id COLLATE NOCASE"))?;
    let rows = stmt.query_map([], map_other)?;
    rows.collect()
}

pub fn get_other_mod(conn: &Connection, id: &str) -> rusqlite::Result<Option<OtherModRow>> {
    let mut stmt = conn.prepare(&format!("{OTHER_SELECT} WHERE id = ?1"))?;
    let mut rows = stmt.query_map([id], map_other)?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

/// Only the tests ask: the import reads the row itself, which says whether it
/// has its files (ESPACE§5.6).
#[cfg(test)]
pub fn other_exists(conn: &Connection, id: &str) -> rusqlite::Result<bool> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM other_mods WHERE id = ?1", [id], |r| r.get(0))?;
    Ok(n > 0)
}

pub fn set_other_priority(conn: &Connection, id: &str, priority: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE other_mods SET is_priority = ?2 WHERE id = ?1",
        params![id, priority as i64],
    )?;
    Ok(())
}

/// Bascule active/inactive + mémorise les jonctions créées (pour une
/// désactivation exacte plus tard).
pub fn set_other_active(conn: &Connection, id: &str, active: bool, junctions: &[String]) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE other_mods SET is_active = ?2, junctions = ?3 WHERE id = ?1",
        params![
            id,
            active as i64,
            serde_json::to_string(junctions).unwrap_or_else(|_| "[]".into())
        ],
    )?;
    Ok(())
}

/// An "other" mod in the showcase got its files back (ESPACE§5.6): same row,
/// same id, name, note and priority.
pub fn mark_other_refilled(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE other_mods SET content_state = ?2, freed_at = NULL WHERE id = ?1",
        params![id, CONTENT_FULL],
    )?;
    Ok(())
}

pub fn delete_other_mod(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM other_mods WHERE id = ?1", [id])?;
    clear_forced_extras(conn, id)?;
    Ok(())
}
