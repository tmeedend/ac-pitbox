//! Manual media links (§6.1): a screenshot or replay attached by hand.

use super::*;

/// Associe manuellement un fichier (screenshot/replay) à une entité — repli
/// quand `media.rs` ne l'a pas trouvé automatiquement. Idempotent (clé
/// primaire `(file_path, entity_id)`).
pub fn add_media_link(conn: &Connection, file_path: &str, entity_id: &str, kind: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO media_links (file_path, entity_id, kind) VALUES (?1, ?2, ?3)",
        params![file_path, entity_id, kind],
    )?;
    Ok(())
}

/// Retire tout rattachement pointant sur ce fichier, quelle que soit l'entité —
/// appelé quand le fichier part à la corbeille (§6.1). Sans ça, la ligne
/// survivrait au fichier et `merge_*_links` referait apparaître le média
/// supprimé dans la galerie à la prochaine ouverture de l'onglet.
pub fn remove_media_links_for_path(conn: &Connection, file_path: &str) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM media_links WHERE file_path = ?1", params![file_path])
}

/// Fichiers rattachés manuellement à `entity_id` pour ce type de média.
pub fn list_media_links(conn: &Connection, entity_id: &str, kind: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT file_path FROM media_links WHERE entity_id = ?1 AND kind = ?2")?;
    let rows = stmt.query_map(params![entity_id, kind], |r| r.get::<_, String>(0))?;
    rows.collect()
}
