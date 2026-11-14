//! What each table of the base becomes in an export, and how it comes back
//! (EXPORT§4).
//!
//! [`TABLES`] is a gate (EXPORT§8.3): a test lists the tables of a fresh base
//! and fails when one is missing here. A table added later without a thought
//! for the export would otherwise leave with all its content, or be lost in
//! silence — the same reasoning as the check on spec references.

use std::path::Path;

use rusqlite::{params, Connection};

use super::fusion::{self, Named, Outcome, SKIP_MODS};
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

/// On the way in, into a base that may already hold a library (EXPORT§7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum In {
    /// Managed mods inserted; game content only lends its user fields.
    Mods,
    /// The export's row wins: a decision or an entry of the user's.
    Replace,
    /// This machine's row wins: something it computed from its own files.
    Ignore,
    /// Nothing comes in.
    Skip,
    /// Merged by [`super::fusion`]: decisions keyed by mod, settled by group
    /// when both sides hold one (EXPORT§7.4).
    Decide,
    /// Merged by [`super::fusion::usage`]: launches add up.
    Usage,
    /// Merged by [`super::fusion::journals`]: a mod's own, only if it has none here.
    Journal,
    /// Merged by [`super::fusion::profiles`]: renamed rather than replaced.
    Profiles,
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
    ("usage", Out::Keep, In::Usage),
    ("history", Out::Keep, In::Journal),
    // The tech sheet: what the physics says is computed again here for the
    // game's content; what the user decided comes, settled by its group when
    // both sides decided (FICHE R6, EXPORT§7.4).
    ("tech_facts", Out::Keep, In::Ignore),
    ("tech_user", Out::Keep, In::Decide),
    // The user's category decisions (REGLES§3 level 4), by mod id: no file of
    // either machine could compute them again.
    ("category_user", Out::Keep, In::Decide),
    ("wiki_link", Out::Keep, In::Decide),
    ("wiki_no_match", Out::Keep, In::Ignore),
    ("wiki_cache", Out::Keep, In::Ignore),
    ("import_decisions", Out::Keep, In::Journal),
    ("forced_extras", Out::Keep, In::Ignore),
    // Track categories chosen on the Online page, by track id: a decision of
    // the user's, which no file of either machine could compute again.
    ("online_track_categories", Out::Keep, In::Decide),
    ("profiles", Out::Profiles, In::Profiles),
    ("profile_entries", Out::Profiles, In::Profiles),
    ("profile_extra_entries", Out::Profiles, In::Profiles),
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

/// The tables whose rows have a folder of their own: one already here, or
/// hung on a mod kept here, does not come (`fusion::prepare`).
const OWN_FOLDER: &[&str] = &["layers", "sub_mods", "apps", "other_mods"];

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
          OR {alias}brand_user IS NOT NULL
          OR {alias}notes_user IS NOT NULL OR {alias}tags_manual <> '[]' OR {alias}is_favorite = 1)"
    )
}

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

/// What [`merge`] did (EXPORT§7.4).
#[derive(Debug, Default)]
pub(super) struct Merged {
    /// Mods of the export brought in, in the showcase.
    pub added: Vec<Named>,
    pub layers: usize,
    pub apps: usize,
    pub others: usize,
    /// Apps and "other" mods brought in.
    pub added_addons: Vec<Named>,
    /// Mods, apps and "other" mods already here: their files, versions and
    /// rows stay this machine's.
    pub kept: Vec<Named>,
    /// Mods whose folder already sits in the library: not brought in.
    pub blocked: Vec<Named>,
    /// Layers, skins and sounds hung on what is kept here, or already here.
    pub attached_skipped: usize,
    /// What became of the user's entries on both sides.
    pub outcome: Outcome,
    pub stock_missing: Vec<String>,
    pub profiles_renamed: Vec<String>,
    /// The library folders of what was not brought in, whose skeletons stay
    /// in the zip.
    pub skipped_folders: Vec<String>,
    /// Showcase cars here taking the export's frozen thumbnails.
    pub thumb_pairs: Vec<(String, String)>,
}

pub(super) fn columns(conn: &Connection, schema: &str, table: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA {schema}.table_info({table})"))?;
    let cols = stmt.query_map([], |r| r.get::<_, String>(1))?;
    cols.collect()
}

/// The columns of `table` both bases know: the export was brought up to this
/// schema by `overlay::open`, but a column it has and this one does not would
/// have no meaning here.
pub(super) fn shared_columns(conn: &Connection, table: &str) -> rusqlite::Result<Vec<String>> {
    let theirs = columns(conn, "src", table)?;
    Ok(columns(conn, "main", table)?
        .into_iter()
        .filter(|c| theirs.contains(c))
        .collect())
}

fn count(conn: &Connection, sql: &str) -> rusqlite::Result<usize> {
    conn.query_row(sql, [], |r| r.get::<_, i64>(0)).map(|n| n as usize)
}

/// What the import would do, counted on the attached export base before
/// anything is written: the import screen shows it (EXPORT§7.3, step 3).
pub(super) fn plan(conn: &Connection, library: &Path) -> Result<super::Plan, String> {
    let prepared = fusion::prepare(conn, library)?;
    let counted = (|| -> rusqlite::Result<super::Plan> {
        let outcome = fusion::user_fields(conn, &fusion::Choices::default(), false)?;
        let addons = count(
            conn,
            &format!(
                "SELECT (SELECT COUNT(*) FROM src.apps WHERE id NOT IN {})
                      + (SELECT COUNT(*) FROM src.other_mods WHERE id NOT IN {})",
                fusion::skipped_rows("apps"),
                fusion::skipped_rows("other_mods")
            ),
        )?;
        Ok(super::Plan {
            merging: held(conn)? > 0,
            added: count(
                conn,
                &format!("SELECT COUNT(*) FROM src.mods WHERE is_stock = 0 AND id_interne NOT IN {SKIP_MODS}"),
            )? + addons,
            kept: kept(conn)?.len(),
            blocked: blocked(conn, &prepared)?,
            attached_skipped: attached_skipped(conn)?,
            filled: outcome.filled.len(),
            notes_joined: outcome.notes_joined.len(),
            conflicts: outcome.conflicts.iter().map(|(g, s)| (*g, s.len())).collect(),
        })
    })();
    fusion::finish(conn);
    counted.map_err(|e| e.to_string())
}

fn kept(conn: &Connection) -> rusqlite::Result<Vec<Named>> {
    let mut out = fusion::named_list(
        conn,
        "SELECT s.id_interne FROM src.mods s
         WHERE s.is_stock = 0 AND s.id_interne IN (SELECT id FROM temp.transfer_both)",
        "main",
        "mods",
    )?;
    for table in ["apps", "other_mods"] {
        out.extend(fusion::named_list(
            conn,
            &format!("SELECT id FROM src.{table} WHERE id IN (SELECT id FROM main.{table})"),
            "main",
            table,
        )?);
    }
    Ok(out)
}

fn blocked(conn: &Connection, prepared: &fusion::Prepared) -> rusqlite::Result<Vec<Named>> {
    if prepared.blocked.is_empty() {
        return Ok(Vec::new());
    }
    fusion::named_list(conn, "SELECT id FROM temp.transfer_blocked", "src", "mods")
}

fn attached_skipped(conn: &Connection) -> rusqlite::Result<usize> {
    count(
        conn,
        "SELECT COUNT(*) FROM temp.transfer_skip_rows WHERE tbl IN ('layers', 'sub_mods')",
    )
}

/// Brings the attached export base (`src`) into the live one (`main`), inside
/// the caller's transaction (EXPORT§7.3, steps 3 and 4; EXPORT§7.4): what the
/// export alone has comes, what is on both sides keeps this machine's rows
/// and merges the user's entries along `choices`.
pub(super) fn merge(
    conn: &Connection,
    library: &Path,
    profiles: bool,
    wiki: bool,
    choices: &fusion::Choices,
    now: &str,
) -> Result<Merged, String> {
    let prepared = fusion::prepare(conn, library)?;
    let merged = merge_prepared(conn, &prepared, profiles, wiki, choices, now);
    fusion::finish(conn);
    merged.map_err(|e| e.to_string())
}

fn merge_prepared(
    conn: &Connection,
    prepared: &fusion::Prepared,
    profiles: bool,
    wiki: bool,
    choices: &fusion::Choices,
    now: &str,
) -> rusqlite::Result<Merged> {
    let mut merged = Merged {
        added: fusion::named_list(
            conn,
            &format!("SELECT id_interne FROM src.mods WHERE is_stock = 0 AND id_interne NOT IN {SKIP_MODS}"),
            "src",
            "mods",
        )?,
        layers: count(
            conn,
            &format!(
                "SELECT COUNT(*) FROM src.layers WHERE id NOT IN {}",
                fusion::skipped_rows("layers")
            ),
        )?,
        kept: kept(conn)?,
        blocked: blocked(conn, prepared)?,
        attached_skipped: attached_skipped(conn)?,
        ..Default::default()
    };
    for table in ["apps", "other_mods"] {
        let added = fusion::named_list(
            conn,
            &format!(
                "SELECT id FROM src.{table} WHERE id NOT IN {}",
                fusion::skipped_rows(table)
            ),
            "src",
            table,
        )?;
        match table {
            "apps" => merged.apps = added.len(),
            _ => merged.others = added.len(),
        }
        merged.added_addons.extend(added);
    }
    // The entries on both sides first, while `main` holds only this
    // machine's rows.
    merged.outcome = fusion::user_fields(conn, choices, true)?;
    copy_tables(conn, wiki)?;
    fusion::usage(conn)?;
    fusion::journals(conn)?;
    if profiles {
        merged.profiles_renamed = fusion::profiles(conn)?;
    }
    merged.stock_missing = stock_missing(conn)?;
    merged.skipped_folders = skipped_folders(conn)?;
    merged.thumb_pairs = fusion::thumb_pairs(conn)?;

    // One line in each mod's timeline: where it came from.
    conn.execute(
        &format!(
            "INSERT INTO main.history (mod_id, timestamp, event, details)
             SELECT id_interne, ?1, 'TRANSFER', '{{\"key\":\"transferred\"}}' FROM src.mods
             WHERE is_stock = 0 AND id_interne NOT IN {SKIP_MODS}"
        ),
        [now],
    )?;
    // Harmonized by another engine maybe: the next start runs it again
    // (EXPORT§7.3, step 8).
    conn.execute(
        "DELETE FROM main.meta WHERE key = ?1",
        [crate::overlay::META_ENGINE_VERSION],
    )?;
    Ok(merged)
}

/// The library folders of what was not brought in: their skeletons stay in
/// the zip.
fn skipped_folders(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT library_path FROM src.versions WHERE mod_id IN {SKIP_MODS}
         UNION ALL SELECT library_path FROM src.layers WHERE id IN {}
         UNION ALL SELECT library_path FROM src.sub_mods WHERE id IN {}
         UNION ALL SELECT library_path FROM src.apps WHERE id IN {}
         UNION ALL SELECT library_path FROM src.other_mods WHERE id IN {}",
        fusion::skipped_rows("layers"),
        fusion::skipped_rows("sub_mods"),
        fusion::skipped_rows("apps"),
        fusion::skipped_rows("other_mods"),
    ))?;
    let paths = stmt.query_map([], |r| r.get::<_, String>(0))?;
    paths.collect()
}

/// Game content the user wrote on that this machine does not have (a DLC?):
/// named, never dropped in silence (EXPORT§4.2).
fn stock_missing(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT s.id_interne FROM src.mods AS s
         WHERE s.is_stock = 1 AND {}
           AND NOT EXISTS (SELECT 1 FROM main.mods m WHERE m.id_interne = s.id_interne)
         ORDER BY s.id_interne",
        user_data("s.")
    ))?;
    let ids = stmt.query_map([], |r| r.get::<_, String>(0))?;
    ids.collect()
}

/// Every table that comes in by copy, table by table (EXPORT§4.1); those with
/// two sides to settle are [`fusion`]'s. Only the columns both bases know.
fn copy_tables(conn: &Connection, wiki: bool) -> rusqlite::Result<()> {
    for (table, out, into) in TABLES {
        let handled = matches!(into, In::Skip | In::Decide | In::Usage | In::Journal | In::Profiles);
        if *out == Out::Exclude || handled || (*table == "wiki_cache" && !wiki) {
            continue;
        }
        let cols = shared_columns(conn, table)?;
        if cols.is_empty() {
            continue;
        }
        let list = cols.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
        let verb = match into {
            In::Mods | In::Replace => "INSERT OR REPLACE",
            _ => "INSERT OR IGNORE",
        };
        let filter = if *into == In::Mods {
            format!(" WHERE is_stock = 0 AND id_interne NOT IN {SKIP_MODS}")
        } else if NAME_A_MOD.contains(table) {
            // Only the mods brought in: game content computes its own here,
            // and a mod kept here keeps its versions.
            format!(
                " WHERE mod_id IN (SELECT id_interne FROM src.mods WHERE is_stock = 0 AND id_interne NOT IN {SKIP_MODS})"
            )
        } else if OWN_FOLDER.contains(table) {
            format!(" WHERE id NOT IN {}", fusion::skipped_rows(table))
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
