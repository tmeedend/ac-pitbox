//! Usage tracking (§6): which mods were launched, and how often.

use super::*;

/// Pose/incrémente le marqueur « essayé » d'un mod au lancement d'une session.
pub fn mark_launched(conn: &Connection, mod_id: &str, ts: &str) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO usage (mod_id, launched, launch_count, last_launched)
           VALUES (?1, 1, 1, ?2)
           ON CONFLICT(mod_id) DO UPDATE SET
               launched = 1,
               launch_count = usage.launch_count + 1,
               last_launched = ?2"#,
        params![mod_id, ts],
    )?;
    Ok(())
}

/// Ids des mods déjà lancés au moins une fois par l'app.
pub fn launched_ids(conn: &Connection) -> rusqlite::Result<std::collections::HashSet<String>> {
    let mut stmt = conn.prepare("SELECT mod_id FROM usage WHERE launched = 1")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    rows.collect()
}
