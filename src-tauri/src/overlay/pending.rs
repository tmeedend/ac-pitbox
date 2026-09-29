//! Proposed folders (§4.6ter): what an import set aside for the user to place.

use super::*;

#[derive(Debug, Clone)]
pub struct PendingFolderRow {
    pub id: String,
    pub archive: String,
    pub rel_path: String,
    pub library_path: String,
    pub owner_id: Option<String>,
    pub owner_kind: Option<String>,
    pub shape: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub readme: Option<String>,
    pub skin_target: Option<String>,
    pub replaced: usize,
    pub found_at: String,
}

pub(super) const PENDING_SELECT: &str =
    "SELECT id, archive, rel_path, library_path, owner_id, owner_kind, shape, title, \
     description, readme, skin_target, replaced, found_at FROM pending_folders";

pub(super) fn map_pending(row: &rusqlite::Row) -> rusqlite::Result<PendingFolderRow> {
    Ok(PendingFolderRow {
        id: row.get(0)?,
        archive: row.get(1)?,
        rel_path: row.get(2)?,
        library_path: row.get(3)?,
        owner_id: row.get(4)?,
        owner_kind: row.get(5)?,
        shape: row.get(6)?,
        title: row.get(7)?,
        description: row.get(8)?,
        readme: row.get(9)?,
        skin_target: row.get(10)?,
        replaced: row.get::<_, i64>(11)?.max(0) as usize,
        found_at: row.get(12)?,
    })
}

/// `INSERT OR REPLACE` : reimporter la meme archive represente les memes
/// dossiers, et c'est voulu — la source est fraiche, la question se repose.
#[allow(clippy::too_many_arguments)]
pub fn insert_pending_folder(conn: &Connection, r: &PendingFolderRow) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT OR REPLACE INTO pending_folders
           (id, archive, rel_path, library_path, owner_id, owner_kind, shape, title,
            description, readme, skin_target, replaced, found_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"#,
        params![
            r.id,
            r.archive,
            r.rel_path,
            r.library_path,
            r.owner_id,
            r.owner_kind,
            r.shape,
            r.title,
            r.description,
            r.readme,
            r.skin_target,
            r.replaced as i64,
            r.found_at,
        ],
    )?;
    Ok(())
}

/// Les plus recents d'abord : ce qui vient d'etre importe est ce qu'on veut
/// trancher, et une vieille ligne encore en attente ne doit pas passer devant.
pub fn list_pending_folders(conn: &Connection) -> rusqlite::Result<Vec<PendingFolderRow>> {
    let mut stmt = conn.prepare(&format!(
        "{PENDING_SELECT} ORDER BY found_at DESC, rel_path COLLATE NOCASE"
    ))?;
    let rows = stmt.query_map([], map_pending)?;
    rows.collect()
}

pub fn get_pending_folder(conn: &Connection, id: &str) -> rusqlite::Result<Option<PendingFolderRow>> {
    let mut stmt = conn.prepare(&format!("{PENDING_SELECT} WHERE id = ?1"))?;
    let mut rows = stmt.query_map([id], map_pending)?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

pub fn delete_pending_folder(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM pending_folders WHERE id = ?1", [id])?;
    Ok(())
}

/// Remembers the answer given to a proposed folder (§4.6ter). `owner_kind` is
/// the category (`cars`, `tracks`…); the name is stored lower-case, like the
/// folder names Windows compares.
pub fn remember_pending_answer(
    conn: &Connection,
    owner_id: &str,
    owner_kind: &str,
    name: &str,
    action: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO pending_answers (owner_id, owner_kind, name, action, answered_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            owner_id,
            owner_kind,
            name.to_lowercase(),
            action,
            chrono::Local::now().to_rfc3339()
        ],
    )?;
    Ok(())
}

pub fn pending_answer(
    conn: &Connection,
    owner_id: &str,
    owner_kind: &str,
    name: &str,
) -> rusqlite::Result<Option<String>> {
    use rusqlite::OptionalExtension;
    conn.query_row(
        "SELECT action FROM pending_answers WHERE owner_id = ?1 AND owner_kind = ?2 AND name = ?3",
        params![owner_id, owner_kind, name.to_lowercase()],
        |r| r.get(0),
    )
    .optional()
}
