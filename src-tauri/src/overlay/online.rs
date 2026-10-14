//! Track categories chosen on the Online page (SPEC-play-online.md, "Catégorie
//! de circuit"): by track id, for the tracks one does not own — and kept when
//! the track is installed, since a choice made by hand is the first source.

use super::*;

/// Every choice: `(track id, categories)`, the categories as stored
/// (`#drift #touge`).
pub fn list_chosen_track_categories(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT track_id, categories FROM online_track_categories")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    rows.collect()
}

/// Records the categories chosen for a track, replacing the previous choice.
pub fn set_chosen_track_categories(
    conn: &Connection,
    track_id: &str,
    categories: &str,
    at: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO online_track_categories (track_id, categories, chosen_at)
           VALUES (?1, ?2, ?3)
           ON CONFLICT(track_id) DO UPDATE SET categories = ?2, chosen_at = ?3"#,
        params![track_id, categories, at],
    )?;
    Ok(())
}

/// Forgets the choice: the track goes back to what the other sources say.
pub fn clear_chosen_track_categories(conn: &Connection, track_id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM online_track_categories WHERE track_id = ?1", [track_id])?;
    Ok(())
}
