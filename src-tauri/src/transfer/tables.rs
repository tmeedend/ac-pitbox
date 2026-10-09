//! What each table of the base becomes in an export, and how it comes back
//! (EXPORT§4).
//!
//! [`TABLES`] is a gate (EXPORT§8.3): a test lists the tables of a fresh base
//! and fails when one is missing here. A table added later without a thought
//! for the export would otherwise leave with all its content, or be lost in
//! silence — the same reasoning as the check on spec references.

use std::path::Path;

use rusqlite::{params, Connection};

use super::{Counts, Part};
use crate::config::AppConfig;
use crate::modscan::ModKind;

/// On the way out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Out {
    /// Leaves, reshaped by [`finish`] where it holds machine state.
    Keep,
    /// Leaves with the Profiles part only.
    Profiles,
    /// Never leaves: the machine's state, or work in progress (R2).
    Exclude,
}

/// On the way in, into a base that only holds this machine's game content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum In {
    /// Managed mods inserted; game content only lends its user fields.
    Mods,
    /// The export's row wins: a decision or an entry of the user's.
    Replace,
    /// This machine's row wins: something it computed from its own files.
    Ignore,
    /// Every row added, its autoincrement `id` left to this base.
    Append,
    /// Nothing comes in.
    Skip,
}

/// Every table, in the order they are merged: a parent before the rows that
/// name it (foreign keys are on in the live base).
pub(super) const TABLES: &[(&str, Out, In)] = &[
    ("mods", Out::Keep, In::Mods),
    ("versions", Out::Keep, In::Replace),
    ("layers", Out::Keep, In::Replace),
    ("sub_mods", Out::Keep, In::Replace),
    ("apps", Out::Keep, In::Replace),
    ("other_mods", Out::Keep, In::Replace),
    ("usage", Out::Keep, In::Replace),
    ("history", Out::Keep, In::Append),
    // The tech sheet: what the physics says is computed again here for the
    // game's content; what the user decided always wins (FICHE R6).
    ("tech_facts", Out::Keep, In::Ignore),
    ("tech_user", Out::Keep, In::Replace),
    ("wiki_link", Out::Keep, In::Replace),
    ("wiki_no_match", Out::Keep, In::Ignore),
    ("wiki_cache", Out::Keep, In::Ignore),
    ("import_decisions", Out::Keep, In::Append),
    ("forced_extras", Out::Keep, In::Ignore),
    ("profiles", Out::Profiles, In::Replace),
    ("profile_entries", Out::Profiles, In::Append),
    ("profile_extra_entries", Out::Profiles, In::Append),
    // Captures and replays attached by path to files of the other machine.
    ("media_links", Out::Exclude, In::Skip),
    // The other machine's game folder (R2).
    ("extra_links", Out::Exclude, In::Skip),
    ("game_backups", Out::Exclude, In::Skip),
    // Imports waiting for an answer.
    ("pending_folders", Out::Exclude, In::Skip),
    ("pending_answers", Out::Exclude, In::Skip),
    // Engine versions: their absence is what re-harmonizes at arrival.
    ("meta", Out::Exclude, In::Skip),
];

/// The tables whose foreign key names a mod. A row about game content this
/// machine does not have (a DLC) has nowhere to go and would fail the whole
/// import: [`merge`] brings only the rows whose mod is here — what the user
/// wrote on the others is counted in `stock_missing`. Checked against the
/// schema's real foreign keys by a test, like [`TABLES`].
pub(super) const NAME_A_MOD: &[&str] = &["versions", "tech_facts"];

/// The tables whose `parent_id` names the mod a row is laid on: a layer, a
/// skin or a sound of a mod kept as it is here ([`keep_local`]) does not come.
const NAME_A_HOST: &[&str] = &["layers", "sub_mods"];

/// The tables that hold something of a library: an installation with any
/// of them filled is not empty (EXPORT R3). Game content does not count — a
/// fresh installation indexes it at its first start.
pub(super) fn held(conn: &Connection) -> rusqlite::Result<usize> {
    conn.query_row(
        "SELECT (SELECT COUNT(*) FROM mods WHERE is_stock = 0)
              + (SELECT COUNT(*) FROM apps)
              + (SELECT COUNT(*) FROM other_mods)
              + (SELECT COUNT(*) FROM layers)",
        [],
        |r| r.get::<_, i64>(0),
    )
    .map(|n| n as usize)
}

/// What is in the game now: mods, apps and "other" mods (EXPORT§6).
pub(super) fn active_ids(conn: &Connection, cfg: &AppConfig) -> rusqlite::Result<Vec<String>> {
    let mut out = Vec::new();
    for m in crate::overlay::list_mods(conn)?.into_iter().filter(|m| !m.is_stock) {
        if crate::activation::is_mod_active(cfg, ModKind::from_column(&m.kind), &m.id_interne) {
            out.push(m.id_interne);
        }
    }
    for a in crate::overlay::list_apps(conn)? {
        if crate::apps::is_app_active(cfg, &a.id) {
            out.push(a.id);
        }
    }
    out.extend(
        crate::overlay::list_other_mods(conn)?
            .into_iter()
            .filter(|o| o.is_active)
            .map(|o| o.id),
    );
    Ok(out)
}

/// The game's content leaves without its versions: they point into the
/// `content/` of this machine, and the other one indexes its own (EXPORT§4.2).
/// Called once the mods installed outside Pit Box have become managed.
pub(super) fn drop_stock_versions(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "UPDATE mods SET active_version_id = NULL WHERE is_stock = 1;
         DELETE FROM versions WHERE mod_id IN (SELECT id_interne FROM mods WHERE is_stock = 1);",
    )
}

/// A stored folder relative to the library, as the zip names it; `None` for
/// one outside the library, which describes nothing that travels.
///
/// Windows paths compare without case, `Path::strip_prefix` with it: a row
/// written as `d:\ac-library\…` under a library set as `D:\AC-Library` is in
/// the library all the same, and dropping it would send its mod without a
/// version.
pub(super) fn relative(library: &Path, stored: &str) -> Option<String> {
    let p = Path::new(stored);
    let rel = if p.is_absolute() {
        match p.strip_prefix(library) {
            Ok(rel) => rel.to_path_buf(),
            Err(_) => {
                let lib = library.to_string_lossy().trim_end_matches(['\\', '/']).to_lowercase();
                let head = stored.get(..lib.len())?;
                let rest = stored.get(lib.len()..)?;
                if head.to_lowercase() != lib || !rest.starts_with(['\\', '/']) {
                    return None;
                }
                Path::new(rest.trim_start_matches(['\\', '/'])).to_path_buf()
            }
        }
    } else {
        p.to_path_buf()
    };
    let s = rel.to_string_lossy().replace('\\', "/");
    (!s.is_empty() && !rel.components().any(|c| matches!(c, std::path::Component::ParentDir))).then_some(s)
}

/// Every stored folder made relative to the library (R2: no absolute path
/// leaves). A row whose folder is outside it is dropped: a skin delivered with
/// the game, found in `content/`, which the other machine finds again.
pub(super) fn relativize(conn: &Connection, library: &Path) -> rusqlite::Result<usize> {
    let mut dropped = 0;
    for table in ["versions", "layers", "sub_mods", "apps", "other_mods"] {
        let rows: Vec<(String, String)> = {
            let mut stmt = conn.prepare(&format!("SELECT id, library_path FROM {table}"))?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        for (id, stored) in rows {
            match relative(library, &stored) {
                Some(rel) if rel != stored => {
                    conn.execute(
                        &format!("UPDATE {table} SET library_path = ?2 WHERE id = ?1"),
                        params![id, rel],
                    )?;
                }
                Some(_) => {}
                None => {
                    log::warn!("transfer: {table} {id} lives outside the library ({stored}), not exported");
                    conn.execute(&format!("DELETE FROM {table} WHERE id = ?1"), [&id])?;
                    dropped += 1;
                }
            }
        }
    }
    Ok(dropped)
}

/// The last reshaping (EXPORT§4.1): every row without its files, nothing in
/// the game, and nothing of the machine's state.
///
/// A layer keeps its switch (`is_active`): it says whether the layer is part
/// of the composition, a choice of the user's, not whether it is laid —
/// `active_layers` already leaves out a layer in the showcase. A sound or a
/// track skin's `is_active` does say it is laid, and goes to 0.
pub(super) fn finish(conn: &Connection, parts: &[Part], stamp: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE versions SET content_state = 'skeleton', freed_at = COALESCE(freed_at, ?1)
         WHERE content_state = 'full'",
        [stamp],
    )?;
    // A kept archive stays on the other machine.
    conn.execute("UPDATE versions SET kept_archive_path = NULL", [])?;
    conn.execute(
        "UPDATE layers SET content_state = 'skeleton', freed_at = COALESCE(freed_at, ?1)
         WHERE content_state = 'full'",
        [stamp],
    )?;
    // Skins delivered with a mod live in its version's folder and leave with
    // it, complete in the base as in the showcase (ESPACE§10.1).
    conn.execute(
        "UPDATE sub_mods SET content_state = 'skeleton', freed_at = COALESCE(freed_at, ?1)
         WHERE content_state = 'full' AND removable = 1",
        [stamp],
    )?;
    conn.execute("UPDATE sub_mods SET is_active = 0", [])?;
    for table in ["apps", "other_mods"] {
        conn.execute(
            &format!(
                "UPDATE {table} SET content_state = 'skeleton', freed_at = COALESCE(freed_at, ?1)
                 WHERE content_state = 'full'"
            ),
            [stamp],
        )?;
    }
    conn.execute("UPDATE other_mods SET is_active = 0, junctions = '[]'", [])?;
    for (table, out, _) in TABLES {
        let cleared = match out {
            Out::Exclude => true,
            Out::Profiles => !parts.contains(&Part::Profiles),
            Out::Keep => false,
        };
        if cleared {
            conn.execute(&format!("DELETE FROM {table}"), [])?;
        }
    }
    Ok(())
}

/// What makes a game content row worth carrying: something the user wrote.
/// `alias` qualifies the columns (`"s."`), or nothing (`""`).
fn user_data(alias: &str) -> String {
    format!(
        "({alias}display_name_user IS NOT NULL OR {alias}description_user IS NOT NULL
          OR {alias}notes_user IS NOT NULL OR {alias}tags_manual <> '[]' OR {alias}is_favorite = 1)"
    )
}

/// The fields of a game content row that are the user's, laid on the same id
/// at arrival (EXPORT§4.2). Category, class and country too: the screens
/// edit them, and harmonization runs again afterwards anyway.
const STOCK_USER_FIELDS: &[&str] = &[
    "display_name_user",
    "description_user",
    "notes_user",
    "tags_manual",
    "is_favorite",
    "category",
    "car_class",
    "country",
];

/// What a base holds, part by part. Read on the live base by the estimate,
/// and on the export's copy once reshaped: a mod installed by hand counts
/// among the cars and tracks on both — it leaves managed (EXPORT§4.2).
pub(super) fn counts(conn: &Connection) -> rusqlite::Result<Counts> {
    let n = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0)).map(|n| n as usize);
    const MODS: &str = "(is_stock = 0 OR (is_unmanaged = 1 AND active_version_id IS NOT NULL))";
    Ok(Counts {
        cars: n(&format!("SELECT COUNT(*) FROM mods WHERE {MODS} AND kind = 'Car'"))?,
        tracks: n(&format!("SELECT COUNT(*) FROM mods WHERE {MODS} AND kind = 'Track'"))?,
        layers: n("SELECT COUNT(*) FROM layers")?,
        skins: n("SELECT COUNT(*) FROM sub_mods WHERE sub_type <> 'SOUND' AND removable = 1")?,
        sounds: n("SELECT COUNT(*) FROM sub_mods WHERE sub_type = 'SOUND'")?,
        apps: n("SELECT COUNT(*) FROM apps")?,
        others: n("SELECT COUNT(*) FROM other_mods")?,
        stock_with_user_data: n(&format!(
            "SELECT COUNT(*) FROM mods WHERE is_stock = 1 AND is_unmanaged = 0 AND {}",
            user_data("")
        ))?,
        unmanaged: n(
            "SELECT COUNT(*) FROM mods WHERE is_stock = 1 AND is_unmanaged = 1 AND active_version_id IS NOT NULL",
        )?,
        profiles: n("SELECT COUNT(*) FROM profiles")?,
        sessions: 0,
        grids: 0,
    })
}

/// What the library weighed with its files: the sizes the versions keep
/// (ESPACE§4.1), the showcase's included.
pub(super) fn library_bytes(conn: &Connection) -> rusqlite::Result<u64> {
    conn.query_row("SELECT COALESCE(SUM(size_bytes), 0) FROM versions", [], |r| {
        r.get::<_, i64>(0)
    })
    .map(|n| n.max(0) as u64)
}

/// What [`merge`] brought in.
#[derive(Debug, Default)]
pub(super) struct Merged {
    pub mods: usize,
    pub layers: usize,
    pub apps: usize,
    pub others: usize,
    pub stock_applied: usize,
    pub stock_missing: Vec<String>,
    /// Mods of the export this machine has as its own, installed by hand:
    /// kept as they are here (see [`keep_local`]).
    pub local_kept: Vec<String>,
    /// The library folders of what was not brought in for them — their
    /// versions, layers, skins and sounds —, whose skeletons stay in the zip.
    pub skipped_folders: Vec<String>,
}

fn columns(conn: &Connection, schema: &str, table: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA {schema}.table_info({table})"))?;
    let cols = stmt.query_map([], |r| r.get::<_, String>(1))?;
    cols.collect()
}

/// Brings the attached export base (`src`) into the live one (`main`), inside
/// the caller's transaction (EXPORT§7.3, steps 3 and 4).
pub(super) fn merge(conn: &Connection, profiles: bool, wiki: bool, now: &str) -> rusqlite::Result<Merged> {
    let local_kept = keep_local(conn)?;
    copy_tables(conn, profiles, wiki)?;

    let n = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0)).map(|n| n as usize);
    let (stock_applied, stock_missing) = lay_user_fields(conn)?;
    let mut merged = Merged {
        mods: n(&format!(
            "SELECT COUNT(*) FROM src.mods WHERE is_stock = 0 AND id_interne NOT IN {KEPT}"
        ))?,
        layers: n(&format!(
            "SELECT COUNT(*) FROM src.layers WHERE parent_id NOT IN {KEPT}"
        ))?,
        apps: n("SELECT COUNT(*) FROM src.apps")?,
        others: n("SELECT COUNT(*) FROM src.other_mods")?,
        stock_applied,
        stock_missing,
        local_kept,
        skipped_folders: Vec::new(),
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT library_path FROM src.versions WHERE mod_id IN {KEPT}
         UNION ALL SELECT library_path FROM src.layers WHERE parent_id IN {KEPT}
         UNION ALL SELECT library_path FROM src.sub_mods WHERE parent_id IN {KEPT}"
    ))?;
    merged.skipped_folders = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;
    drop(stmt);

    // One line in each mod's timeline: where it came from.
    conn.execute(
        &format!(
            "INSERT INTO main.history (mod_id, timestamp, event, details)
             SELECT id_interne, ?1, 'TRANSFER', '{{\"key\":\"transferred\"}}' FROM src.mods
             WHERE is_stock = 0 AND id_interne NOT IN {KEPT}"
        ),
        [now],
    )?;
    // Harmonized by another engine maybe: the next start runs it again
    // (EXPORT§7.3, step 8).
    conn.execute(
        "DELETE FROM main.meta WHERE key = ?1",
        [crate::overlay::META_ENGINE_VERSION],
    )?;
    conn.execute_batch("DROP TABLE temp.transfer_kept")?;
    Ok(merged)
}

/// The ids [`keep_local`] lists, for a query's `IN`.
const KEPT: &str = "(SELECT id FROM temp.transfer_kept)";

/// Mods of the export this machine already has as its own game content —
/// installed by hand in its `content/`, a real folder Pit Box only reads.
/// They stay what they are here, and get only what the user wrote on them:
/// the export's version, layers and skeleton would make them mods in the
/// showcase whose real folder sits in the game, and recovering one would run
/// into that folder. Listed in `temp.transfer_kept` for the queries after.
fn keep_local(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    conn.execute_batch(
        "CREATE TEMP TABLE IF NOT EXISTS transfer_kept (id TEXT PRIMARY KEY);
         DELETE FROM temp.transfer_kept;
         INSERT INTO temp.transfer_kept
           SELECT s.id_interne FROM src.mods AS s JOIN main.mods AS m ON m.id_interne = s.id_interne
           WHERE s.is_stock = 0 AND m.is_stock = 1;",
    )?;
    let mut stmt = conn.prepare("SELECT id FROM temp.transfer_kept ORDER BY id")?;
    let ids = stmt.query_map([], |r| r.get::<_, String>(0))?;
    ids.collect()
}

/// Every table that comes in, table by table (EXPORT§4.1). Only the columns
/// both bases know: the export was brought up to this schema by
/// `overlay::open`, but a column it has and this one does not would have no
/// meaning here.
fn copy_tables(conn: &Connection, profiles: bool, wiki: bool) -> rusqlite::Result<()> {
    for (table, out, into) in TABLES {
        let skipped = *out == Out::Exclude
            || (*out == Out::Profiles && !profiles)
            || (*table == "wiki_cache" && !wiki)
            || *into == In::Skip;
        if skipped {
            continue;
        }
        let theirs = columns(conn, "src", table)?;
        let cols: Vec<String> = columns(conn, "main", table)?
            .into_iter()
            .filter(|c| theirs.contains(c))
            .filter(|c| !(*into == In::Append && c == "id"))
            .collect();
        if cols.is_empty() {
            continue;
        }
        let list = cols.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
        let verb = match into {
            In::Mods | In::Replace => "INSERT OR REPLACE",
            In::Ignore => "INSERT OR IGNORE",
            In::Append | In::Skip => "INSERT",
        };
        let filter = if *into == In::Mods {
            format!(" WHERE is_stock = 0 AND id_interne NOT IN {KEPT}")
        } else if NAME_A_MOD.contains(table) {
            // Only the mods brought in: game content computes its own here.
            " WHERE mod_id IN (SELECT id_interne FROM main.mods WHERE is_stock = 0)".to_string()
        } else if NAME_A_HOST.contains(table) {
            format!(" WHERE parent_id NOT IN {KEPT}")
        } else {
            String::new()
        };
        conn.execute(
            &format!("{verb} INTO main.{table} ({list}) SELECT {list} FROM src.{table}{filter}"),
            [],
        )?;
    }
    Ok(())
}

/// What the user wrote on content this machine has as its own — the game's,
/// or a mod installed by hand ([`keep_local`]) —, laid on the same id
/// (EXPORT§4.2). Returns how many rows got it, and the game content written on
/// that this machine does not have.
fn lay_user_fields(conn: &Connection) -> rusqlite::Result<(usize, Vec<String>)> {
    let theirs = columns(conn, "src", "mods")?;
    let ours = columns(conn, "main", "mods")?;
    let set = STOCK_USER_FIELDS
        .iter()
        .filter(|c| theirs.iter().any(|t| t == *c) && ours.iter().any(|o| o == *c))
        .map(|c| format!("\"{c}\" = s.\"{c}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let user_data = user_data("s.");
    let applied = conn.execute(
        &format!(
            "UPDATE main.mods SET {set} FROM src.mods AS s
             WHERE main.mods.id_interne = s.id_interne AND main.mods.is_stock = 1
               AND (s.is_stock = 1 OR s.id_interne IN {KEPT}) AND {user_data}"
        ),
        [],
    )?;
    let mut stmt = conn.prepare(&format!(
        "SELECT s.id_interne FROM src.mods AS s
         WHERE s.is_stock = 1 AND {user_data}
           AND NOT EXISTS (SELECT 1 FROM main.mods m WHERE m.id_interne = s.id_interne)
         ORDER BY s.id_interne"
    ))?;
    let missing = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok((applied, missing))
}
