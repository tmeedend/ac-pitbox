//! The two tables of the sheet (FICHE§6.1), created with the rest of the
//! schema in `overlay::init`. Only this module writes them.

use std::collections::BTreeMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

use super::{field, Fact, Facts, Source};

/// The key the files of this mod are filed under (see `super::version_key`).
pub(super) fn version_key_of(conn: &Connection, mod_id: &str) -> rusqlite::Result<String> {
    let row: Option<(i64, Option<String>)> = conn
        .query_row(
            "SELECT is_stock, active_version_id FROM mods WHERE id_interne = ?1",
            [mod_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(row
        .map(|(stock, v)| super::version_key(stock != 0, v.as_deref()))
        .unwrap_or_default())
}

fn insert(conn: &Connection, mod_id: &str, version: &str, facts: &[Fact]) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(
        "INSERT OR REPLACE INTO tech_facts (mod_id, version_id, field, source, value) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for f in facts {
        stmt.execute(params![
            mod_id,
            version,
            f.field,
            f.source.as_str(),
            f.value.to_string()
        ])?;
    }
    Ok(())
}

/// Runs `work` all or nothing. A savepoint and not a transaction: the
/// harmonisation calls in from inside its own (`harmonize_all_counting`), and
/// SQLite refuses a transaction within a transaction.
fn atomically(conn: &Connection, work: impl FnOnce() -> rusqlite::Result<()>) -> rusqlite::Result<()> {
    conn.execute_batch("SAVEPOINT techsheet")?;
    match work() {
        Ok(()) => conn.execute_batch("RELEASE techsheet"),
        Err(e) => {
            if let Err(undo) = conn.execute_batch("ROLLBACK TO techsheet; RELEASE techsheet") {
                log::warn!("techsheet: rollback failed — {undo}");
            }
            Err(e)
        }
    }
}

/// What the files of one version said, replaced whole. The country is the
/// harmonisation's (see [`replace_harmonized_facts`]), never the files'.
pub(super) fn replace_file_facts(
    conn: &Connection,
    mod_id: &str,
    version: &str,
    facts: &[Fact],
) -> rusqlite::Result<()> {
    atomically(conn, || {
        conn.execute(
            "DELETE FROM tech_facts WHERE mod_id = ?1 AND version_id = ?2
                 AND source IN ('physics', 'ui', 'table') AND field <> ?3",
            params![mod_id, version, field::COUNTRY],
        )?;
        insert(conn, mod_id, version, facts)
    })
}

/// What the harmonisation settled, replaced whole. Filed under `''`: the rules
/// describe the mod as last harmonised, whatever version that read.
pub(super) fn replace_harmonized_facts(conn: &Connection, mod_id: &str, facts: &[Fact]) -> rusqlite::Result<()> {
    atomically(conn, || {
        conn.execute(
            "DELETE FROM tech_facts WHERE mod_id = ?1 AND version_id = ''
                 AND (source = 'rules' OR field = ?2)",
            params![mod_id, field::COUNTRY],
        )?;
        insert(conn, mod_id, "", facts)
    })
}

/// The facts that apply to the mod: its version's, and the mod-wide ones.
pub(super) fn facts(conn: &Connection, mod_id: &str, version: &str) -> rusqlite::Result<Facts> {
    let mut stmt =
        conn.prepare("SELECT field, source, value FROM tech_facts WHERE mod_id = ?1 AND version_id IN ('', ?2)")?;
    let rows = stmt.query_map(params![mod_id, version], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
    })?;
    let mut out = Facts::new();
    for row in rows {
        let (f, s, v) = row?;
        let (Some(source), Ok(value)) = (Source::parse(&s), serde_json::from_str(&v)) else {
            log::warn!("techsheet: unreadable fact {mod_id}/{f}/{s}");
            continue;
        };
        out.insert((f, source), value);
    }
    Ok(out)
}

/// The user's decisions: `Some(value)`, or `None` for a forced "unknown".
pub(super) fn user(conn: &Connection, mod_id: &str) -> rusqlite::Result<BTreeMap<String, Option<Value>>> {
    let mut stmt = conn.prepare("SELECT field, value FROM tech_user WHERE mod_id = ?1")?;
    let rows = stmt.query_map([mod_id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
    })?;
    let mut out = BTreeMap::new();
    for row in rows {
        let (f, v) = row?;
        let value = match v {
            None => None,
            Some(text) => match serde_json::from_str(&text) {
                Ok(v) => Some(v),
                Err(e) => {
                    log::warn!("techsheet: unreadable decision {mod_id}/{f} — {e}");
                    continue;
                }
            },
        };
        out.insert(f, value);
    }
    Ok(out)
}

/// `NULL` stores "unknown" (FICHE§6.1).
pub(super) fn set_user(conn: &Connection, mod_id: &str, f: &str, value: &Value) -> rusqlite::Result<()> {
    let text = (!value.is_null()).then(|| value.to_string());
    conn.execute(
        "INSERT OR REPLACE INTO tech_user (mod_id, field, value, edited_at) VALUES (?1, ?2, ?3, ?4)",
        params![mod_id, f, text, chrono::Local::now().to_rfc3339()],
    )?;
    Ok(())
}

pub(super) fn clear_user(conn: &Connection, mod_id: &str, f: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM tech_user WHERE mod_id = ?1 AND field = ?2",
        params![mod_id, f],
    )?;
    Ok(())
}

/// Whether the mod has any fact or decision at all.
pub(super) fn has_anything(conn: &Connection, mod_id: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM tech_facts WHERE mod_id = ?1)
             OR EXISTS(SELECT 1 FROM tech_user WHERE mod_id = ?1)",
        [mod_id],
        |r| r.get(0),
    )
}

/// Whether the files of this version were read by the current readers
/// through this stack of layers (FICHE§9.3, §4.3): a car read by an older
/// reader, or through other layers, is to be read again.
pub(super) fn is_current(conn: &Connection, mod_id: &str, version: &str, layers: &[String]) -> rusqlite::Result<bool> {
    let marker: Option<String> = conn
        .query_row(
            "SELECT value FROM tech_facts WHERE mod_id = ?1 AND version_id = ?2 AND field = ?3",
            params![mod_id, version, field::RECORDED],
            |r| r.get(0),
        )
        .optional()?;
    let Some(marker) = marker.and_then(|m| serde_json::from_str::<Value>(&m).ok()) else {
        return Ok(false);
    };
    Ok(
        marker.get("reader").and_then(Value::as_u64) == Some(u64::from(super::READER_VERSION))
            && marker.get("stack") == Some(&serde_json::json!(layers)),
    )
}

/// The five cached columns as they are now, in the order of `super::CACHED` —
/// what the backfill compares before and after reading a car (FICHE§9.3).
pub(super) fn cached_columns(conn: &Connection, mod_id: &str) -> rusqlite::Result<Vec<Option<String>>> {
    conn.query_row(
        "SELECT drivetrain, aspiration, gearbox, engine_config, engine_pos FROM mods WHERE id_interne = ?1",
        [mod_id],
        |r| (0..5).map(|i| r.get(i)).collect(),
    )
}

/// The five cached columns, in the order of `super::CACHED`.
pub(super) fn write_cache(
    conn: &Connection,
    mod_id: &str,
    values: &[Option<String>],
    marks: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE mods SET drivetrain = ?2, aspiration = ?3, gearbox = ?4, engine_config = ?5,
             engine_pos = ?6, tech_marks = ?7
         WHERE id_interne = ?1",
        params![mod_id, values[0], values[1], values[2], values[3], values[4], marks],
    )?;
    Ok(())
}

/// A car whose files have not been read yet (FICHE§9.3).
#[derive(Debug, Clone)]
pub struct Pending {
    pub mod_id: String,
    pub version: String,
    pub stack: super::Stack,
    pub stock: bool,
}

/// Every car of the base whose active files have no facts from the current
/// readers, with where they are. A car whose folder cannot be found is skipped — and asked about again
/// at the next start, which is right: its disk may simply not be mounted.
pub fn pending_cars(conn: &Connection, cfg: &crate::config::AppConfig) -> rusqlite::Result<Vec<Pending>> {
    let mut out = Vec::new();
    for m in crate::overlay::list_mods(conn)? {
        if m.kind != "Car" {
            continue;
        }
        let version = super::version_key(m.is_stock, m.active_version_id.as_deref());
        let Some(stack) = super::active_stack(conn, cfg, &m) else {
            continue;
        };
        if is_current(conn, &m.id_interne, &version, &stack.layers)? {
            continue;
        }
        out.push(Pending {
            mod_id: m.id_interne,
            version,
            stack,
            stock: m.is_stock,
        });
    }
    Ok(out)
}
