//! Merging an export into a library that already exists (EXPORT§7.4).
//!
//! `tables.rs` copies the rows the export brings; this module holds everything
//! that has two sides: which rows are the same here and in the export, which
//! of them must not come at all, and how what the user entered on each side
//! becomes one value.
//!
//! **One code path for the screen and the import.** The inspection runs the
//! same decisions without writing ([`user_fields`] with `write: false`), so
//! that the counts the import screen announces are the ones the import then
//! acts on — two implementations would drift, and the screen would promise
//! one thing while the import did another.
//!
//! On an empty installation nothing is on both sides, and every function here
//! reduces to "take what the export has": the import of a whole library is
//! the special case of a merge, not a second mechanism.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Which side wins when both hold a different value (EXPORT§7.4).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    /// The value of this machine — the default: nothing is replaced unless
    /// the user asks for it.
    #[default]
    Here,
    Export,
}

/// The groups of entries the user decides about, one choice each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    /// Name given by hand, description.
    Names,
    /// Brand chosen, category decided, track categories chosen online.
    Classification,
    /// Tech sheet values corrected by hand.
    Tech,
    /// Preferred livery and layout, Wikipedia article chosen.
    Prefs,
}

/// The user's choice for each group.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Choices {
    pub names: Side,
    pub classification: Side,
    pub tech: Side,
    pub prefs: Side,
}

impl Choices {
    pub fn of(&self, group: Group) -> Side {
        match group {
            Group::Names => self.names,
            Group::Classification => self.classification,
            Group::Tech => self.tech,
            Group::Prefs => self.prefs,
        }
    }
}

/// An entry as the report names it: its id, and the name the user sees.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Named {
    pub id: String,
    pub name: String,
}

/// What merging the user's entries did, or would do.
#[derive(Debug, Clone, Default)]
pub(super) struct Outcome {
    /// Entries that got a value they had none of here.
    pub filled: BTreeSet<Named>,
    /// Entries holding two different values, by group.
    pub conflicts: BTreeMap<Group, BTreeSet<Named>>,
    /// Entries whose two notes are now one, the export's under this one's.
    pub notes_joined: BTreeSet<Named>,
    /// Game content rows that got something (EXPORT§4.2).
    pub stock_changed: BTreeSet<String>,
}

impl Outcome {
    fn conflict(&mut self, group: Group, who: &Named) {
        self.conflicts.entry(group).or_default().insert(who.clone());
    }
}

// --- What is on both sides ---------------------------------------------------

/// Mods of the export not brought in: already here, or blocked by a folder.
pub(super) const SKIP_MODS: &str = "(SELECT id FROM temp.transfer_both UNION SELECT id FROM temp.transfer_blocked)";
/// Mods of the export whose own rows stay those of this machine: what is
/// attached to them in the export (layers, skins, sounds) does not come.
/// Game content is not among them — a layer on a Kunos car still comes.
pub(super) const KEPT_HOSTS: &str = "(SELECT id_interne FROM src.mods WHERE is_stock = 0 AND id_interne IN \
     (SELECT id FROM temp.transfer_both UNION SELECT id FROM temp.transfer_blocked))";

/// The rows of `table` (layers, sub_mods, apps, other_mods) that do not come.
pub(super) fn skipped_rows(table: &str) -> String {
    format!("(SELECT id FROM temp.transfer_skip_rows WHERE tbl = '{table}')")
}

/// What [`prepare`] found before anything is written.
#[derive(Debug, Default)]
pub(super) struct Prepared {
    /// Mods of the export whose folder already sits in the library, without a
    /// row of this base for it: never written over, so not brought in (R3).
    pub blocked: Vec<String>,
}

/// Lists, in temporary tables, what is on both sides and what cannot come
/// (EXPORT§7.4) — the base read, the library's folders checked for presence,
/// nothing written. [`finish`] drops them.
pub(super) fn prepare(conn: &Connection, library: &Path) -> Result<Prepared, String> {
    let sql = |e: rusqlite::Error| e.to_string();
    conn.execute_batch(
        "CREATE TEMP TABLE IF NOT EXISTS transfer_both (id TEXT PRIMARY KEY);
         CREATE TEMP TABLE IF NOT EXISTS transfer_blocked (id TEXT PRIMARY KEY);
         CREATE TEMP TABLE IF NOT EXISTS transfer_skip_rows (tbl TEXT NOT NULL, id TEXT NOT NULL);
         CREATE TEMP TABLE IF NOT EXISTS transfer_had_history (mod_id TEXT PRIMARY KEY);
         CREATE TEMP TABLE IF NOT EXISTS transfer_had_decisions (mod_id TEXT PRIMARY KEY);
         DELETE FROM temp.transfer_both;
         DELETE FROM temp.transfer_blocked;
         DELETE FROM temp.transfer_skip_rows;
         DELETE FROM temp.transfer_had_history;
         DELETE FROM temp.transfer_had_decisions;
         INSERT INTO temp.transfer_both
           SELECT s.id_interne FROM src.mods s WHERE s.id_interne IN (SELECT id_interne FROM main.mods);
         INSERT OR IGNORE INTO temp.transfer_had_history SELECT DISTINCT mod_id FROM main.history;
         INSERT OR IGNORE INTO temp.transfer_had_decisions
           SELECT DISTINCT mod_id FROM main.import_decisions WHERE mod_id IS NOT NULL;",
    )
    .map_err(sql)?;

    // A new mod one of whose folders is already in the library: a folder
    // this base does not know of, which the import must not write into.
    let versions: Vec<(String, String)> = query_pairs(
        conn,
        "SELECT v.mod_id, v.library_path FROM src.versions v JOIN src.mods s ON s.id_interne = v.mod_id
         WHERE s.is_stock = 0 AND s.id_interne NOT IN (SELECT id FROM temp.transfer_both)",
    )
    .map_err(sql)?;
    let mut blocked: BTreeSet<String> = BTreeSet::new();
    for (id, rel) in versions {
        if super::unpack::occupied(library, &rel) {
            blocked.insert(id);
        }
    }
    for id in &blocked {
        conn.execute("INSERT OR IGNORE INTO temp.transfer_blocked (id) VALUES (?1)", [id])
            .map_err(sql)?;
    }

    // Rows with a folder of their own: one already here by id or by folder
    // stays this machine's, and so does whatever hangs on a mod kept here.
    for table in ["layers", "sub_mods"] {
        let mut skip: BTreeSet<String> = query_ids(
            conn,
            &format!(
                "SELECT id FROM src.{table}
                 WHERE id IN (SELECT id FROM main.{table}) OR parent_id IN {KEPT_HOSTS}"
            ),
        )
        .map_err(sql)?
        .into_iter()
        .collect();
        for (id, rel) in query_pairs(conn, &format!("SELECT id, library_path FROM src.{table}")).map_err(sql)? {
            if super::unpack::occupied(library, &rel) {
                skip.insert(id);
            }
        }
        insert_skipped(conn, table, &skip).map_err(sql)?;
    }
    for table in ["apps", "other_mods"] {
        let mut skip: BTreeSet<String> = BTreeSet::new();
        for (id, rel) in query_pairs(conn, &format!("SELECT id, library_path FROM src.{table}")).map_err(sql)? {
            let here: bool = conn
                .query_row(
                    &format!("SELECT EXISTS (SELECT 1 FROM main.{table} WHERE id = ?1)"),
                    [&id],
                    |r| r.get(0),
                )
                .map_err(sql)?;
            if here || super::unpack::occupied(library, &rel) {
                skip.insert(id);
            }
        }
        insert_skipped(conn, table, &skip).map_err(sql)?;
    }
    Ok(Prepared {
        blocked: blocked.into_iter().collect(),
    })
}

fn insert_skipped(conn: &Connection, table: &str, ids: &BTreeSet<String>) -> rusqlite::Result<()> {
    for id in ids {
        conn.execute(
            "INSERT INTO temp.transfer_skip_rows (tbl, id) VALUES (?1, ?2)",
            params![table, id],
        )?;
    }
    Ok(())
}

fn query_pairs(conn: &Connection, sql: &str) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    rows.collect()
}

/// Drops what [`prepare`] made.
pub(super) fn finish(conn: &Connection) {
    let dropped = conn.execute_batch(
        "DROP TABLE IF EXISTS temp.transfer_both;
         DROP TABLE IF EXISTS temp.transfer_blocked;
         DROP TABLE IF EXISTS temp.transfer_skip_rows;
         DROP TABLE IF EXISTS temp.transfer_had_history;
         DROP TABLE IF EXISTS temp.transfer_had_decisions;",
    );
    if let Err(e) = dropped {
        log::warn!("transfer: temporary tables not dropped: {e}");
    }
}

// --- Names ---------------------------------------------------------------------

/// A mod, an app or an "other" mod as the report names it, read in `schema`.
fn named(conn: &Connection, schema: &str, table: &str, id: &str) -> Named {
    let (pk, name) = match table {
        "mods" => ("id_interne", "COALESCE(display_name_user, display_name, id_interne)"),
        _ => ("id", "COALESCE(display_name_user, id)"),
    };
    let name = conn
        .query_row(
            &format!("SELECT {name} FROM {schema}.{table} WHERE {pk} = ?1"),
            [id],
            |r| r.get::<_, String>(0),
        )
        .unwrap_or_else(|_| id.to_string());
    Named {
        id: id.to_string(),
        name,
    }
}

/// The ids `sql` selects, named from `schema`.`table`.
pub(super) fn named_list(conn: &Connection, sql: &str, schema: &str, table: &str) -> rusqlite::Result<Vec<Named>> {
    let mut stmt = conn.prepare(sql)?;
    let ids: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut out: Vec<Named> = ids.iter().map(|id| named(conn, schema, table, id)).collect();
    out.sort_by_key(|n| n.name.to_lowercase());
    Ok(out)
}

/// One entry present on both sides: its id, whether it is game content, and
/// the merged columns as this base and the export hold them.
struct BothSides {
    id: String,
    is_stock: bool,
    ours: Vec<Option<String>>,
    theirs: Vec<Option<String>>,
}

// --- The user's entries on both sides -------------------------------------------

/// How one column of an entry merges.
#[derive(Clone, Copy)]
enum Rule {
    /// One value: present on one side, it stays or comes; different on both,
    /// the group decides.
    Pick(Group),
    /// A free text: two different ones are both kept, this one first.
    Join,
    /// A JSON list of strings: the two side by side, nothing lost.
    Union,
    /// A flag: set on one side, set at arrival.
    Or,
}

/// The columns of the user's that a mod, an app or an "other" mod carries.
/// Only those both bases know are merged: an app has no brand.
const FIELDS: &[(&str, Rule)] = &[
    ("display_name_user", Rule::Pick(Group::Names)),
    ("description_user", Rule::Pick(Group::Names)),
    ("brand_user", Rule::Pick(Group::Classification)),
    ("notes_user", Rule::Join),
    ("tags_manual", Rule::Union),
    ("is_favorite", Rule::Or),
];

/// What separates the note of this machine from the export's: no word, so
/// that it reads the same in every language.
pub(super) const NOTE_SEPARATOR: &str = "\n\n---\n\n";

fn blank(s: &Option<String>) -> bool {
    s.as_deref().is_none_or(|s| s.trim().is_empty() || s.trim() == "[]")
}

/// The merged value of one column, or `None` when this side keeps its own.
/// Records in `out` what happened to `who`.
fn merge_value(
    rule: Rule,
    ours: Option<String>,
    theirs: Option<String>,
    choices: &Choices,
    who: &Named,
    out: &mut Outcome,
) -> Option<String> {
    if blank(&theirs) {
        return None;
    }
    let theirs = theirs.unwrap_or_default();
    if blank(&ours) {
        out.filled.insert(who.clone());
        return Some(theirs);
    }
    let ours = ours.unwrap_or_default();
    match rule {
        Rule::Pick(group) => {
            if ours.trim() == theirs.trim() {
                return None;
            }
            out.conflict(group, who);
            (choices.of(group) == Side::Export).then_some(theirs)
        }
        Rule::Join => {
            // Already joined: the same file imported twice. Joining again
            // would double it at every import. Only a joined note counts —
            // "ok" is not already in "not ok on wet tracks".
            let theirs_trimmed = theirs.trim();
            if ours.trim() == theirs_trimmed || ours.contains(&format!("{NOTE_SEPARATOR}{theirs_trimmed}")) {
                return None;
            }
            out.notes_joined.insert(who.clone());
            Some(format!("{}{NOTE_SEPARATOR}{}", ours.trim_end(), theirs.trim()))
        }
        Rule::Union => {
            let mut list: Vec<String> = serde_json::from_str(&ours).unwrap_or_default();
            let theirs: Vec<String> = serde_json::from_str(&theirs).unwrap_or_default();
            let before = list.len();
            for tag in theirs {
                if !list.contains(&tag) {
                    list.push(tag);
                }
            }
            if list.len() == before {
                return None;
            }
            out.filled.insert(who.clone());
            serde_json::to_string(&list).ok()
        }
        Rule::Or => {
            if ours != "0" || theirs == "0" {
                return None;
            }
            out.filled.insert(who.clone());
            Some(theirs)
        }
    }
}

/// Merges what the user entered on the mods, apps and "other" mods present on
/// both sides (EXPORT§7.4) — or, `write` false, only says what it would do.
/// Game content counts among the mods: a note on a Kunos car is the user's.
pub(super) fn user_fields(conn: &Connection, choices: &Choices, write: bool) -> rusqlite::Result<Outcome> {
    let mut out = Outcome::default();
    for (table, pk) in [("mods", "id_interne"), ("apps", "id"), ("other_mods", "id")] {
        let theirs = super::tables::columns(conn, "src", table)?;
        let ours = super::tables::columns(conn, "main", table)?;
        let fields: Vec<(&str, Rule)> = FIELDS
            .iter()
            .copied()
            .filter(|(c, _)| theirs.iter().any(|t| t == c) && ours.iter().any(|o| o == c))
            .collect();
        if fields.is_empty() {
            continue;
        }
        let read = |alias: &str| {
            fields
                .iter()
                .map(|(c, _)| format!("CAST({alias}.\"{c}\" AS TEXT)"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let stock = if table == "mods" { "m.is_stock" } else { "0" };
        let mut stmt = conn.prepare(&format!(
            "SELECT s.{pk}, {stock}, {}, {} FROM src.{table} s JOIN main.{table} m ON m.{pk} = s.{pk}",
            read("m"),
            read("s"),
        ))?;
        let n = fields.len();
        let rows: Vec<BothSides> = stmt
            .query_map([], |r| {
                Ok(BothSides {
                    id: r.get(0)?,
                    is_stock: r.get(1)?,
                    ours: (0..n).map(|i| r.get(2 + i)).collect::<rusqlite::Result<_>>()?,
                    theirs: (0..n).map(|i| r.get(2 + n + i)).collect::<rusqlite::Result<_>>()?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        drop(stmt);
        for BothSides {
            id,
            is_stock,
            ours,
            theirs,
        } in rows
        {
            let who = named(conn, "main", table, &id);
            for ((column, rule), (o, t)) in fields.iter().zip(ours.into_iter().zip(theirs)) {
                let Some(value) = merge_value(*rule, o, t, choices, &who, &mut out) else {
                    continue;
                };
                if is_stock {
                    out.stock_changed.insert(id.clone());
                }
                if write {
                    conn.execute(
                        &format!("UPDATE main.{table} SET \"{column}\" = ?2 WHERE {pk} = ?1"),
                        params![id, value],
                    )?;
                }
            }
        }
    }
    decisions(conn, choices, write, &mut out)?;
    wiki_links(conn, choices, write, &mut out)?;
    Ok(out)
}

/// A table of decisions keyed by mod (EXPORT§7.4): its key, the columns that
/// are the decision, and the group that settles a disagreement.
struct Decided {
    table: &'static str,
    keys: &'static [&'static str],
    values: &'static [&'static str],
    group: Group,
}

const DECIDED: &[Decided] = &[
    Decided {
        table: "tech_user",
        keys: &["mod_id", "field"],
        values: &["value"],
        group: Group::Tech,
    },
    Decided {
        table: "category_user",
        keys: &["mod_id"],
        values: &["forced", "added", "removed"],
        group: Group::Classification,
    },
    Decided {
        table: "online_track_categories",
        keys: &["track_id"],
        values: &["categories"],
        group: Group::Classification,
    },
];

/// The decision tables: a decision taken on one side only comes (or stays);
/// two different ones on the same key are settled by their group.
fn decisions(conn: &Connection, choices: &Choices, write: bool, out: &mut Outcome) -> rusqlite::Result<()> {
    for d in DECIDED {
        let join = d
            .keys
            .iter()
            .map(|k| format!("m.\"{k}\" = s.\"{k}\""))
            .collect::<Vec<_>>()
            .join(" AND ");
        let differ = d
            .values
            .iter()
            .map(|v| format!("m.\"{v}\" IS NOT s.\"{v}\""))
            .collect::<Vec<_>>()
            .join(" OR ");
        let owner = d.keys[0];
        let both = format!(
            "SELECT s.\"{owner}\" FROM src.{} s JOIN main.{} m ON {join}",
            d.table, d.table
        );
        let conflicting = query_ids(conn, &format!("{both} WHERE {differ}"))?;
        // Filled: a decision on a mod already here that had none — not one
        // on a mod that arrives, which is simply part of it.
        let filled = query_ids(
            conn,
            &format!(
                "SELECT s.\"{owner}\" FROM src.{t} s
                 WHERE lower(s.\"{owner}\") IN (SELECT lower(id) FROM temp.transfer_both)
                   AND NOT EXISTS (SELECT 1 FROM main.{t} m WHERE {join})",
                t = d.table
            ),
        )?;
        for id in &conflicting {
            out.conflict(d.group, &mod_named(conn, id));
        }
        for id in &filled {
            out.filled.insert(mod_named(conn, id));
        }
        if !write {
            continue;
        }
        let cols = super::tables::shared_columns(conn, d.table)?;
        let list = cols.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
        // Not those of a mod blocked by its folder: it does not come, and its
        // decisions would wait for an unrelated mod of the same id.
        conn.execute(
            &format!(
                "INSERT OR IGNORE INTO main.{t} ({list}) SELECT {list} FROM src.{t}
                 WHERE lower(\"{owner}\") NOT IN (SELECT lower(id) FROM temp.transfer_blocked)",
                t = d.table
            ),
            [],
        )?;
        if choices.of(d.group) == Side::Export {
            conn.execute(
                &format!(
                    "INSERT OR REPLACE INTO main.{t} ({list}) SELECT {list} FROM src.{t} s
                     WHERE EXISTS (SELECT 1 FROM main.{t} m WHERE {join} AND ({differ}))",
                    t = d.table
                ),
                [],
            )?;
        }
    }
    Ok(())
}

/// The Wikipedia article of a mod (WIKI§3.1): a link chosen by hand outranks
/// one the app matched, on either side, without a question; two links chosen
/// by hand that differ are settled by the group.
fn wiki_links(conn: &Connection, choices: &Choices, write: bool, out: &mut Outcome) -> rusqlite::Result<()> {
    const RANK: &str = "CASE source WHEN 'manual' THEN 2 WHEN 'import' THEN 1 ELSE 0 END";
    let rank = |alias: &str| RANK.replace("source", &format!("{alias}.source"));
    let (rs, rm) = (rank("s"), rank("m"));
    let base = "FROM src.wiki_link s JOIN main.wiki_link m ON m.mod_key = s.mod_key";
    let conflicting = query_ids(
        conn,
        &format!("SELECT s.mod_key {base} WHERE {rs} = 2 AND {rm} = 2 AND s.entity_id <> m.entity_id"),
    )?;
    let upgraded = query_ids(conn, &format!("SELECT s.mod_key {base} WHERE {rs} > {rm}"))?;
    for id in &conflicting {
        out.conflict(Group::Prefs, &mod_named(conn, id));
    }
    for id in &upgraded {
        out.filled.insert(mod_named(conn, id));
    }
    if !write {
        return Ok(());
    }
    let cols = super::tables::shared_columns(conn, "wiki_link")?;
    let list = cols.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
    conn.execute(
        &format!(
            "INSERT OR IGNORE INTO main.wiki_link ({list}) SELECT {list} FROM src.wiki_link
             WHERE mod_key NOT IN (SELECT id FROM temp.transfer_blocked)"
        ),
        [],
    )?;
    let export_wins = if choices.prefs == Side::Export {
        format!(" OR ({rs} = 2 AND {rm} = 2 AND s.entity_id <> m.entity_id)")
    } else {
        String::new()
    };
    conn.execute(
        &format!(
            "INSERT OR REPLACE INTO main.wiki_link ({list}) SELECT {list} FROM src.wiki_link s
             WHERE EXISTS (SELECT 1 FROM main.wiki_link m WHERE m.mod_key = s.mod_key AND ({rs} > {rm}{export_wins}))"
        ),
        [],
    )?;
    Ok(())
}

fn query_ids(conn: &Connection, sql: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(sql)?;
    let ids = stmt.query_map([], |r| r.get::<_, String>(0))?;
    ids.collect()
}

/// A mod named from this base — or, when only the export knows it (a track
/// category chosen online for a track not installed), by its id.
fn mod_named(conn: &Connection, id: &str) -> Named {
    let here: Option<String> = conn
        .query_row(
            "SELECT id_interne FROM main.mods WHERE lower(id_interne) = lower(?1)",
            [id],
            |r| r.get(0),
        )
        .ok();
    match here {
        Some(found) => named(conn, "main", "mods", &found),
        None => named(conn, "src", "mods", id),
    }
}

/// What was played (EXPORT§7.4): tried on one side is tried, the launches
/// add up, the most recent date stays.
pub(super) fn usage(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO main.usage (mod_id, launched, launch_count, last_launched)
         SELECT mod_id, launched, launch_count, last_launched FROM src.usage WHERE true
         ON CONFLICT(mod_id) DO UPDATE SET
           launched = max(launched, excluded.launched),
           launch_count = launch_count + excluded.launch_count,
           last_launched = NULLIF(max(COALESCE(last_launched, ''), COALESCE(excluded.last_launched, '')), '')",
        [],
    )?;
    Ok(())
}

/// The sessions driven (RESULTAT§12): those of the export not already here.
/// The same session is the same car on the same layout, ended within a second
/// — the end of a session is the one time both machines agree on. The export
/// left without launches or CM file names (`tables::finish`).
pub(super) fn sessions(conn: &Connection) -> rusqlite::Result<()> {
    let cols: Vec<String> = super::tables::shared_columns(conn, "session_results")?
        .into_iter()
        .filter(|c| c != "id")
        .collect();
    let list = cols.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
    let picked = cols.iter().map(|c| format!("s.\"{c}\"")).collect::<Vec<_>>().join(", ");
    conn.execute(
        &format!(
            "INSERT INTO main.session_results ({list})
             SELECT {picked} FROM src.session_results s
             WHERE NOT EXISTS (
               SELECT 1 FROM main.session_results m
               WHERE m.car_id = s.car_id AND m.track_id = s.track_id
                 AND COALESCE(m.track_layout, '') = COALESCE(s.track_layout, '')
                 AND abs(m.ended_at - s.ended_at) <= 1000)"
        ),
        [],
    )?;
    Ok(())
}

/// A mod's history and its journal of import decisions come only if it has
/// none here: otherwise every import event would be there twice (EXPORT§7.4).
pub(super) fn journals(conn: &Connection) -> rusqlite::Result<()> {
    for (table, had) in [
        ("history", "transfer_had_history"),
        ("import_decisions", "transfer_had_decisions"),
    ] {
        let cols: Vec<String> = super::tables::shared_columns(conn, table)?
            .into_iter()
            .filter(|c| c != "id")
            .collect();
        let list = cols.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
        conn.execute(
            &format!(
                "INSERT INTO main.{table} ({list}) SELECT {list} FROM src.{table}
                 WHERE COALESCE(mod_id, '') NOT IN (SELECT mod_id FROM temp.{had})
                   AND COALESCE(mod_id, '') NOT IN (SELECT id FROM temp.transfer_blocked)"
            ),
            [],
        )?;
    }
    Ok(())
}

/// The profiles (EXPORT§7.4): every one comes, a name already taken here
/// gets a ` (2)`, an id already taken a new one — but one already here under
/// the same id and name, the same file imported again, is left as it is. An
/// entry naming a version this machine has not got names the mod's active
/// one here. Returns the names given to the renamed.
pub(super) fn profiles(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut taken: BTreeSet<String> = query_ids(conn, "SELECT name FROM main.profiles")?.into_iter().collect();
    let theirs: Vec<(String, String, String)> = {
        let mut stmt = conn.prepare("SELECT id, name, created_at FROM src.profiles ORDER BY created_at, id")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let mut renamed = Vec::new();
    for (id, name, created_at) in theirs {
        let same: bool = conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM main.profiles WHERE id = ?1 AND name = ?2)",
            params![id, name],
            |r| r.get(0),
        )?;
        if same {
            continue;
        }
        let free = free_name(&taken, &name);
        if free != name {
            renamed.push(free.clone());
        }
        taken.insert(free.clone());
        let id_taken: bool = conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM main.profiles WHERE id = ?1)",
            [&id],
            |r| r.get(0),
        )?;
        let new_id = if id_taken {
            uuid::Uuid::new_v4().to_string()
        } else {
            id.clone()
        };
        conn.execute(
            "INSERT INTO main.profiles (id, name, created_at) VALUES (?1, ?2, ?3)",
            params![new_id, free, created_at],
        )?;
        conn.execute(
            "INSERT INTO main.profile_entries (profile_id, mod_id, version_id)
             SELECT ?2, e.mod_id,
                    CASE WHEN EXISTS (SELECT 1 FROM main.versions v WHERE v.id = e.version_id) THEN e.version_id
                         ELSE (SELECT m.active_version_id FROM main.mods m WHERE m.id_interne = e.mod_id) END
             FROM src.profile_entries e
             WHERE e.profile_id = ?1 AND e.mod_id IN (SELECT id_interne FROM main.mods)
               AND COALESCE(
                    CASE WHEN EXISTS (SELECT 1 FROM main.versions v WHERE v.id = e.version_id) THEN e.version_id
                         ELSE (SELECT m.active_version_id FROM main.mods m WHERE m.id_interne = e.mod_id) END,
                    '') <> ''",
            params![id, new_id],
        )?;
        conn.execute(
            "INSERT INTO main.profile_extra_entries (profile_id, kind, entry_id)
             SELECT ?2, kind, entry_id FROM src.profile_extra_entries WHERE profile_id = ?1",
            params![id, new_id],
        )?;
    }
    Ok(renamed)
}

/// `name`, or `name (2)`, `(3)`… when taken. No word in the suffix: the
/// backend does not know the user's language.
pub(super) fn free_name(taken: &BTreeSet<String>, name: &str) -> String {
    if !taken.contains(name) {
        return name.to_string();
    }
    (2..)
        .map(|n| format!("{name} ({n})"))
        .find(|candidate| !taken.contains(candidate))
        .unwrap_or_else(|| name.to_string())
}

/// The cars in the showcase here, also in the export, whose skeleton holds no
/// frozen thumbnail: they take the export's (EXPORT§7.4). Pairs of the
/// export's version folder and this one's, both as stored.
pub(super) fn thumb_pairs(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    query_pairs(
        conn,
        "SELECT sv.library_path, mv.library_path
         FROM src.mods s JOIN src.versions sv ON sv.id = s.active_version_id
         JOIN main.mods m ON m.id_interne = s.id_interne
         JOIN main.versions mv ON mv.id = m.active_version_id
         WHERE s.is_stock = 0 AND m.is_stock = 0 AND s.kind = 'Car' AND mv.content_state = 'skeleton'",
    )
}

// --- Files of settings ------------------------------------------------------------

/// Whether a file of classification holds a decision of the user's: one key
/// besides `format` with something in it. Without, it is the empty file a
/// first start writes, and the export's replaces it (EXPORT§7.4).
pub(super) fn has_decisions(bytes: &[u8]) -> bool {
    fn filled(v: &Value) -> bool {
        match v {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Number(n) => n.as_f64() != Some(0.0),
            Value::String(s) => !s.is_empty(),
            Value::Array(a) => a.iter().any(filled),
            Value::Object(o) => o.values().any(filled),
        }
    }
    match serde_json::from_slice::<Value>(bytes) {
        Ok(Value::Object(o)) => o.iter().any(|(k, v)| k != "format" && filled(v)),
        // Unreadable or of another shape: kept, never replaced on a guess.
        _ => true,
    }
}

/// Two objects keyed by name (saved grids): the export's entries all come, a
/// name already here with another content under ` (2)`. Returns the merged
/// object and the names given to the renamed.
pub(super) fn merge_named(ours: &Value, theirs: &Value) -> (Value, Vec<String>) {
    let mut merged = ours.as_object().cloned().unwrap_or_default();
    let mut renamed = Vec::new();
    if let Some(theirs) = theirs.as_object() {
        for (name, entry) in theirs {
            match merged.get(name) {
                None => {
                    merged.insert(name.clone(), entry.clone());
                }
                Some(here) if here == entry => {}
                Some(_) => {
                    let taken: BTreeSet<String> = merged.keys().cloned().collect();
                    let free = free_name(&taken, name);
                    renamed.push(free.clone());
                    merged.insert(free, entry.clone());
                }
            }
        }
    }
    (Value::Object(merged), renamed)
}

/// What merging `ui_prefs.json` did.
#[derive(Debug, Default)]
pub(super) struct PrefsMerge {
    pub value: Value,
    /// Mods whose preferred livery or layout came, having none here.
    pub filled: Vec<String>,
    /// Mods with another preferred livery or layout on each side.
    pub conflicts: Vec<String>,
}

const PRESETS_KEY: &str = "pitbox.gridThumbs.presets";

/// The mod a per-mod preference names: `pitbox.skin.<id>` or
/// `pitbox.layout.<id>` (`preferred.ts`).
fn per_mod(key: &str) -> Option<&str> {
    key.strip_prefix("pitbox.skin.")
        .or_else(|| key.strip_prefix("pitbox.layout."))
}

/// What a per-mod preference chooses: the `id` of the object serialized in it.
fn chosen(value: &Value) -> Option<String> {
    let inner: Value = serde_json::from_str(value.as_str()?).ok()?;
    inner.get("id")?.as_str().map(str::to_string)
}

/// Merges `ui_prefs.json` (EXPORT§7.4). `library`: the per-mod preferences
/// and the grid presets come, merged by mod and by preset id. `preferences`:
/// the application's own preferences come, the export's replacing these.
/// A value the export lost to R2 (a path become `null`) never replaces one
/// that is here.
pub(super) fn merge_ui_prefs(ours: &Value, theirs: &Value, library: bool, preferences: bool, side: Side) -> PrefsMerge {
    let mut merged = ours.as_object().cloned().unwrap_or_default();
    let mut out = PrefsMerge::default();
    let Some(theirs) = theirs.as_object() else {
        out.value = Value::Object(merged);
        return out;
    };
    for (key, value) in theirs {
        if value.is_null() {
            continue;
        }
        if let Some(id) = per_mod(key) {
            if !library {
                continue;
            }
            match merged.get(key) {
                None => {
                    merged.insert(key.clone(), value.clone());
                    out.filled.push(id.to_string());
                }
                Some(here) if chosen(here) == chosen(value) => {}
                Some(_) => {
                    out.conflicts.push(id.to_string());
                    if side == Side::Export {
                        merged.insert(key.clone(), value.clone());
                    }
                }
            }
        } else if key == PRESETS_KEY {
            if library {
                let joined = union_presets(merged.get(key), value);
                merged.insert(key.clone(), joined);
            }
        } else if preferences {
            merged.insert(key.clone(), value.clone());
        }
    }
    out.value = Value::Object(merged);
    out
}

/// The user's grid presets of both sides (GRILLE§5.6), by id: one here keeps
/// its settings, one only in the export comes — its frozen thumbnails need it
/// to be shown (ESPACE§3.3). Stored as a JSON list inside a string.
fn union_presets(ours: Option<&Value>, theirs: &Value) -> Value {
    let parse = |v: Option<&Value>| -> Vec<Value> {
        v.and_then(Value::as_str)
            .and_then(|s| serde_json::from_str::<Vec<Value>>(s).ok())
            .unwrap_or_default()
    };
    let mut list = parse(ours);
    let id = |p: &Value| p.get("id").and_then(Value::as_str).map(str::to_string);
    let known: BTreeSet<String> = list.iter().filter_map(id).collect();
    for preset in parse(Some(theirs)) {
        if id(&preset).is_some_and(|i| !known.contains(&i)) {
            list.push(preset);
        }
    }
    Value::String(serde_json::to_string(&list).unwrap_or_else(|_| "[]".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn who() -> Named {
        Named {
            id: "lanzo".into(),
            name: "Lanzo".into(),
        }
    }

    fn some(s: &str) -> Option<String> {
        Some(s.to_string())
    }

    /// Rule (EXPORT§7.4): a value on one side only stays or comes; the same
    /// on both changes nothing; two different ones are the group's to settle.
    #[test]
    fn a_value_on_one_side_comes_and_a_disagreement_follows_the_choice() {
        let rule = Rule::Pick(Group::Names);
        let here = Choices::default();
        let export = Choices {
            names: Side::Export,
            ..Default::default()
        };
        let mut out = Outcome::default();
        assert_eq!(
            merge_value(rule, None, some("V10"), &here, &who(), &mut out),
            some("V10")
        );
        assert!(out.filled.contains(&who()), "counted as filled");
        assert_eq!(merge_value(rule, some("Mine"), None, &here, &who(), &mut out), None);
        assert_eq!(
            merge_value(rule, some("Same"), some("Same"), &here, &who(), &mut out),
            None
        );
        assert!(out.conflicts.is_empty(), "no disagreement so far");
        assert_eq!(
            merge_value(rule, some("Mine"), some("Theirs"), &here, &who(), &mut out),
            None,
            "kept"
        );
        assert_eq!(
            merge_value(rule, some("Mine"), some("Theirs"), &export, &who(), &mut out),
            some("Theirs"),
            "taken when asked"
        );
        assert_eq!(out.conflicts[&Group::Names].len(), 1, "one entry in disagreement");
    }

    /// Rule (EXPORT§7.4): two notes are both kept, this machine's first — and
    /// importing the same file again does not double the export's.
    #[test]
    fn two_notes_are_both_kept_and_never_twice() {
        let mut out = Outcome::default();
        let joined = merge_value(
            Rule::Join,
            some("brakes early"),
            some("great on Spa"),
            &Choices::default(),
            &who(),
            &mut out,
        )
        .unwrap();
        assert_eq!(joined, format!("brakes early{NOTE_SEPARATOR}great on Spa"));
        assert!(out.notes_joined.contains(&who()));
        assert_eq!(
            merge_value(
                Rule::Join,
                some(&joined),
                some("great on Spa"),
                &Choices::default(),
                &who(),
                &mut out
            ),
            None,
            "already joined"
        );
        assert_eq!(
            merge_value(
                Rule::Join,
                some("not ok on wet tracks"),
                some("ok"),
                &Choices::default(),
                &who(),
                &mut out
            ),
            some(&format!("not ok on wet tracks{NOTE_SEPARATOR}ok")),
            "a note found inside the other one by chance is still kept"
        );
    }

    /// Rule (EXPORT§7.4): tags set by hand join, a favourite on one side is one.
    #[test]
    fn tags_join_and_a_favourite_stays_one() {
        let mut out = Outcome::default();
        let c = Choices::default();
        assert_eq!(
            merge_value(
                Rule::Union,
                some(r##"["#a"]"##),
                some(r##"["#b","#a"]"##),
                &c,
                &who(),
                &mut out
            ),
            some(r##"["#a","#b"]"##)
        );
        assert_eq!(
            merge_value(Rule::Or, some("0"), some("1"), &c, &who(), &mut out),
            some("1")
        );
        assert_eq!(merge_value(Rule::Or, some("1"), some("0"), &c, &who(), &mut out), None);
    }

    /// Rule (EXPORT§7.4): per-mod preferences come by mod and the grid presets
    /// by id, with the library; the application's own only with Preferences;
    /// and a path the export lost never replaces one here.
    #[test]
    fn ui_prefs_merge_by_mod_by_preset_and_by_part() {
        let skin = |id: &str| Value::String(format!(r#"{{"id":"{id}","preview":null}}"#));
        let ours = serde_json::json!({
            "pitbox.skin.lanzo": skin("red"),
            "pitbox.skin.only_here": skin("blue"),
            "pitbox.gridThumbs.presets": r#"[{"id":"u1","name":"Mine"}]"#,
            "pitbox.zoom": "1.25",
            "pitbox.music.folder": "D:/music",
        });
        let theirs = serde_json::json!({
            "pitbox.skin.lanzo": skin("green"),
            "pitbox.skin.new_car": skin("gold"),
            "pitbox.gridThumbs.presets": r#"[{"id":"u1","name":"Theirs"},{"id":"u2","name":"Other"}]"#,
            "pitbox.zoom": "1",
            "pitbox.music.folder": null,
        });
        let m = merge_ui_prefs(&ours, &theirs, true, false, Side::Here);
        assert_eq!(m.filled, vec!["new_car"]);
        assert_eq!(m.conflicts, vec!["lanzo"]);
        assert_eq!(m.value["pitbox.skin.lanzo"], skin("red"), "this machine's choice kept");
        assert_eq!(m.value["pitbox.zoom"], "1.25", "Preferences unchecked: the zoom stays");
        let presets: Vec<Value> = serde_json::from_str(m.value[PRESETS_KEY].as_str().unwrap()).unwrap();
        assert_eq!(presets.len(), 2, "the export's other preset joins");
        assert_eq!(presets[0]["name"], "Mine", "a preset here keeps its settings");

        let m = merge_ui_prefs(&ours, &theirs, false, true, Side::Export);
        assert_eq!(m.value["pitbox.zoom"], "1", "Preferences checked: the export's");
        assert_eq!(
            m.value["pitbox.music.folder"], "D:/music",
            "a path lost on the way never wins"
        );
        assert_eq!(
            m.value["pitbox.skin.lanzo"],
            skin("red"),
            "per-mod ones need the library"
        );
    }

    /// Rule (EXPORT§7.4): a grid of the same name and another content is
    /// renamed, never written over; the same one is not doubled.
    #[test]
    fn a_grid_of_the_same_name_is_renamed_not_replaced() {
        let ours = serde_json::json!({ "GT3": {"cars": ["a"]}, "Same": {"cars": ["s"]} });
        let theirs = serde_json::json!({ "GT3": {"cars": ["b"]}, "Same": {"cars": ["s"]}, "New": {} });
        let (merged, renamed) = merge_named(&ours, &theirs);
        assert_eq!(renamed, vec!["GT3 (2)"]);
        assert_eq!(merged["GT3"]["cars"][0], "a", "this machine's kept");
        assert_eq!(merged["GT3 (2)"]["cars"][0], "b");
        assert_eq!(merged.as_object().unwrap().len(), 4, "the same grid is not doubled");
    }

    /// Rule (EXPORT§7.4): a file of classification the first start wrote empty
    /// is replaced; one holding a decision is kept, and so is one not read.
    #[test]
    fn a_classification_file_is_kept_only_when_it_holds_a_decision() {
        assert!(!has_decisions(br#"{"format":2}"#));
        assert!(!has_decisions(br#"{"format":2,"brand_aliases":{"set":{}},"own":[]}"#));
        assert!(has_decisions(br#"{"format":2,"brand_aliases":{"set":{"Bmw":"BMW"}}}"#));
        assert!(has_decisions(b"not json"), "what cannot be read is kept");
    }
}
