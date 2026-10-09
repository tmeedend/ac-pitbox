//! Apps (§8.4).

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppRow {
    pub id: String,
    pub library_path: String,
    pub source_archive: Option<String>,
    pub imported_at: String,
    /// Nom repris à la main (REFONTE§6.1) : le titre d'une fiche d'app n'est
    /// plus son nom de dossier.
    pub display_name_user: Option<String>,
    /// Note libre (REFONTE§9).
    pub notes_user: Option<String>,
    /// [`CONTENT_FULL`] or [`CONTENT_SKELETON`]: an app can exist without its
    /// files, its folder reduced to the showcase manifest (ESPACE§5.6).
    pub content_state: String,
}

impl AppRow {
    pub fn is_skeleton(&self) -> bool {
        self.content_state == CONTENT_SKELETON
    }
}

/// An imported app, or a reimported one: an existing row keeps its id, name
/// and note, and gets its files back — which is how an app in the showcase is
/// recovered (ESPACE§5.6).
pub fn insert_app(
    conn: &Connection,
    id: &str,
    library_path: &str,
    source_archive: Option<&str>,
    imported_at: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO apps (id, library_path, source_archive, imported_at)
           VALUES (?1, ?2, ?3, ?4)
           ON CONFLICT(id) DO UPDATE SET library_path = excluded.library_path,
                                         content_state = 'full', freed_at = NULL"#,
        params![id, library_path, source_archive, imported_at],
    )?;
    Ok(())
}

pub(super) fn map_app(row: &rusqlite::Row) -> rusqlite::Result<AppRow> {
    Ok(AppRow {
        id: row.get(0)?,
        library_path: row.get(1)?,
        source_archive: row.get(2)?,
        imported_at: row.get(3)?,
        display_name_user: row.get(4)?,
        notes_user: row.get(5)?,
        content_state: row.get(6)?,
    })
}

pub(super) const APP_SELECT: &str =
    "SELECT id, library_path, source_archive, imported_at, display_name_user, notes_user, content_state FROM apps";

pub fn list_apps(conn: &Connection) -> rusqlite::Result<Vec<AppRow>> {
    let mut stmt = conn.prepare(&format!("{APP_SELECT} ORDER BY id COLLATE NOCASE"))?;
    let rows = stmt.query_map([], map_app)?;
    rows.collect()
}

pub fn get_app(conn: &Connection, id: &str) -> rusqlite::Result<Option<AppRow>> {
    let mut stmt = conn.prepare(&format!("{APP_SELECT} WHERE id = ?1"))?;
    let mut rows = stmt.query_map([id], map_app)?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

#[allow(dead_code)] // utilisé par les tests d'apps
pub fn app_exists(conn: &Connection, id: &str) -> rusqlite::Result<bool> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM apps WHERE id = ?1", [id], |r| r.get(0))?;
    Ok(n > 0)
}

pub fn delete_app(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM apps WHERE id = ?1", [id])?;
    conn.execute("DELETE FROM extra_links WHERE mod_id = ?1", [id])?;
    clear_forced_extras(conn, id)?;
    Ok(())
}
