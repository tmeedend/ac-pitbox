//! The user's decisions on categories, and the two columns that keep what the
//! rules computed (REGLES§3 level 4). `categories.rs` decides what they mean;
//! this file only reads and writes the rows.

use super::*;

/// What the user decided on one mod's category.
///
/// A car carries one `forced` category; a track carries deltas on its computed
/// list. Both shapes live in one row because a mod has only one kind, and the
/// other fields stay empty.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryDecision {
    pub mod_id: String,
    /// A car: the category the user filed it under (`#tag`).
    #[serde(default)]
    pub forced: Option<String>,
    /// A track: categories added to the computed list.
    #[serde(default)]
    pub added: Vec<String>,
    /// A track: computed categories taken off.
    #[serde(default)]
    pub removed: Vec<String>,
}

impl CategoryDecision {
    pub fn new(mod_id: &str) -> Self {
        Self {
            mod_id: mod_id.to_string(),
            ..Default::default()
        }
    }

    /// Nothing decided: the row is not kept.
    pub fn is_empty(&self) -> bool {
        self.forced.is_none() && self.added.is_empty() && self.removed.is_empty()
    }
}

fn decision_row(row: &rusqlite::Row) -> rusqlite::Result<CategoryDecision> {
    let added: String = row.get(2)?;
    let removed: String = row.get(3)?;
    Ok(CategoryDecision {
        mod_id: row.get(0)?,
        forced: row.get(1)?,
        added: json_arr(&added),
        removed: json_arr(&removed),
    })
}

const DECISION_COLUMNS: &str = "mod_id, forced, added, removed";

pub fn get_category_decision(conn: &Connection, id: &str) -> rusqlite::Result<Option<CategoryDecision>> {
    use rusqlite::OptionalExtension;
    conn.query_row(
        &format!("SELECT {DECISION_COLUMNS} FROM category_user WHERE mod_id = ?1"),
        [id],
        decision_row,
    )
    .optional()
}

pub fn list_category_decisions(conn: &Connection) -> rusqlite::Result<Vec<CategoryDecision>> {
    let mut stmt = conn.prepare(&format!("SELECT {DECISION_COLUMNS} FROM category_user ORDER BY mod_id"))?;
    let rows = stmt.query_map([], decision_row)?;
    rows.collect()
}

/// Writes a decision, or forgets the mod's when there is nothing left in it.
pub fn set_category_decision(conn: &Connection, d: &CategoryDecision) -> rusqlite::Result<()> {
    if d.is_empty() {
        conn.execute("DELETE FROM category_user WHERE mod_id = ?1", [&d.mod_id])?;
        return Ok(());
    }
    conn.execute(
        "INSERT OR REPLACE INTO category_user (mod_id, forced, added, removed, edited_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            d.mod_id,
            d.forced,
            serde_json::to_string(&d.added).unwrap_or_else(|_| "[]".into()),
            serde_json::to_string(&d.removed).unwrap_or_else(|_| "[]".into()),
            chrono::Local::now().to_rfc3339(),
        ],
    )?;
    Ok(())
}

/// The categories user decisions name, which the pickers offer next to what
/// the rules can produce.
#[derive(Debug, Default)]
pub struct DecidedCategories {
    /// The categories cars were forced into.
    pub forced: std::collections::BTreeSet<String>,
    /// The categories added to tracks.
    pub added: std::collections::BTreeSet<String>,
}

pub fn decided_categories(conn: &Connection) -> rusqlite::Result<DecidedCategories> {
    let mut out = DecidedCategories::default();
    for d in list_category_decisions(conn)? {
        out.forced.extend(d.forced);
        out.added.extend(d.added);
    }
    Ok(out)
}

/// Keeps what is shown as the computed category of a mod whose computed one
/// was never written (a mod not harmonised yet, a base not backfilled): the
/// shown category IS the computed one until a decision is laid on it, so this
/// is called before the first decision, never after.
pub fn ensure_category_auto(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE mods SET category_auto = category, categories_auto = categories
         WHERE id_interne = ?1 AND categories_auto IS NULL",
        [id],
    )?;
    Ok(())
}

/// What the rules computed, as the harmonisation wrote it.
pub fn set_category_auto(
    conn: &Connection,
    id: &str,
    category: Option<&str>,
    categories: &[String],
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE mods SET category_auto = ?2, categories_auto = ?3 WHERE id_interne = ?1",
        params![
            id,
            category,
            serde_json::to_string(categories).unwrap_or_else(|_| "[]".into()),
        ],
    )?;
    Ok(())
}

/// What the screens show: the computed category with the user's decision laid
/// on it (`categories::resolve`).
pub fn set_category_shown(
    conn: &Connection,
    id: &str,
    category: Option<&str>,
    categories: &[String],
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE mods SET category = ?2, categories = ?3 WHERE id_interne = ?1",
        params![
            id,
            category,
            serde_json::to_string(categories).unwrap_or_else(|_| "[]".into()),
        ],
    )?;
    Ok(())
}
