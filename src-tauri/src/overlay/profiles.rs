//! Profiles (L3): a named set of mod versions.

use super::*;

#[derive(Debug, Clone, Serialize)]
pub struct ProfileRow {
    pub id: String,
    pub name: String,
    pub entry_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileEntry {
    pub mod_id: String,
    pub version_id: String,
}

/// Entrée de profil sans notion de version — Autre mod ou App (§7.3/§8.4),
/// simplement actif ou non. `kind` vaut "other" ou "app", `entry_id` est l'id
/// dans la table correspondante.
#[derive(Debug, Clone, Serialize)]
pub struct ProfileExtraEntry {
    pub kind: String,
    pub entry_id: String,
}

pub fn create_profile(conn: &Connection, id: &str, name: &str, created_at: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO profiles (id, name, created_at) VALUES (?1, ?2, ?3)",
        params![id, name, created_at],
    )?;
    Ok(())
}

pub fn add_profile_entry(conn: &Connection, profile_id: &str, mod_id: &str, version_id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO profile_entries (profile_id, mod_id, version_id) VALUES (?1, ?2, ?3)",
        params![profile_id, mod_id, version_id],
    )?;
    Ok(())
}

pub fn add_profile_extra_entry(
    conn: &Connection,
    profile_id: &str,
    kind: &str,
    entry_id: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO profile_extra_entries (profile_id, kind, entry_id) VALUES (?1, ?2, ?3)",
        params![profile_id, kind, entry_id],
    )?;
    Ok(())
}

pub fn list_profiles(conn: &Connection) -> rusqlite::Result<Vec<ProfileRow>> {
    let mut stmt = conn.prepare(
        r#"SELECT p.id, p.name,
                  (SELECT COUNT(*) FROM profile_entries e WHERE e.profile_id = p.id)
                  + (SELECT COUNT(*) FROM profile_extra_entries x WHERE x.profile_id = p.id) AS entry_count
           FROM profiles p ORDER BY p.name COLLATE NOCASE"#,
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ProfileRow {
            id: r.get(0)?,
            name: r.get(1)?,
            entry_count: r.get(2)?,
        })
    })?;
    rows.collect()
}

pub fn get_profile_entries(conn: &Connection, profile_id: &str) -> rusqlite::Result<Vec<ProfileEntry>> {
    let mut stmt = conn.prepare("SELECT mod_id, version_id FROM profile_entries WHERE profile_id = ?1")?;
    let rows = stmt.query_map([profile_id], |r| {
        Ok(ProfileEntry {
            mod_id: r.get(0)?,
            version_id: r.get(1)?,
        })
    })?;
    rows.collect()
}

pub fn get_profile_extra_entries(conn: &Connection, profile_id: &str) -> rusqlite::Result<Vec<ProfileExtraEntry>> {
    let mut stmt = conn.prepare("SELECT kind, entry_id FROM profile_extra_entries WHERE profile_id = ?1")?;
    let rows = stmt.query_map([profile_id], |r| {
        Ok(ProfileExtraEntry {
            kind: r.get(0)?,
            entry_id: r.get(1)?,
        })
    })?;
    rows.collect()
}

pub fn delete_profile(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM profiles WHERE id = ?1", [id])?;
    Ok(())
}

/// Ids des mods partageant un même pack d'origine (§4.4).
pub fn list_pack_ids(conn: &Connection, pack: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT id_interne FROM mods WHERE source_pack = ?1")?;
    let rows = stmt.query_map([pack], |r| r.get::<_, String>(0))?;
    rows.collect()
}

pub fn mod_exists(conn: &Connection, id: &str) -> rusqlite::Result<bool> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM mods WHERE id_interne = ?1", [id], |r| r.get(0))?;
    Ok(n > 0)
}

/// Tous les id_interne d'un type (voiture/circuit, mod comme stock) — sert à
/// `media.rs` pour retrouver le « contrepartie » (circuit dans un nom de
/// screenshot de voiture, et inversement, §6.1).
pub fn list_mod_ids_by_kind(conn: &Connection, kind: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT id_interne FROM mods WHERE kind = ?1")?;
    let rows = stmt.query_map([kind], |r| r.get::<_, String>(0))?;
    rows.collect()
}
