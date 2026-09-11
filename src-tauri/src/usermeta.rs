//! User-entered metadata, for **every** kind of entity (refonte §6.1 and §9).
//!
//! Two fields, one mechanism: a display name and a free-form note. Both used to
//! belong to cars and tracks only, which left the objects that need them most
//! without any — a layer is called `spa2022-release_V1-03.rar`, an "other" mod
//! is called `policeman__ext_config.ini`.
//!
//! Two properties the callers rely on:
//!
//! - **The entry survives a mod update and a reindex.** Like `mods`'
//!   `display_name_user`, these columns sit *beside* the file-derived fields,
//!   never in their place: re-importing a mod rewrites the derived name and
//!   leaves the user's alone.
//! - **A write on an unknown id fails.** Losing what someone just typed because
//!   the row was deleted meanwhile is worse than an error message: the UI can
//!   keep the text and say so.
//!
//! A note is **not** a description (§9.2). A description overrides what the mod
//! file says, so clearing it means "go back to the file"; a note has no
//! original value, so clearing it means empty. Blank input is therefore stored
//! as `NULL` — otherwise the "has a note" facet would count empty notes.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// Which table a piece of user input belongs to. Spelled out rather than taken
/// as a table name from the frontend: the SQL below is built by `format!`, and
/// only a closed set keeps that safe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EntityKind {
    /// A car or a track (`mods`).
    Mod,
    /// A skin, a sound, a track skin (`sub_mods`).
    SubMod,
    App,
    /// An "other" mod (§7.3), including everything Pit Box could not identify.
    Other,
    /// A layer (§4.4).
    Layer,
}

impl EntityKind {
    /// Table and primary-key column. Both are `&'static str` from this closed
    /// enum, never user input.
    fn table(self) -> (&'static str, &'static str) {
        match self {
            EntityKind::Mod => ("mods", "id_interne"),
            EntityKind::SubMod => ("sub_mods", "id"),
            EntityKind::App => ("apps", "id"),
            EntityKind::Other => ("other_mods", "id"),
            EntityKind::Layer => ("layers", "id"),
        }
    }

    /// The i18n key to raise when the id is unknown — each kind already has
    /// one. "Other" mods share the mods' key, as `others.rs` already does.
    fn not_found(self) -> &'static str {
        match self {
            EntityKind::Mod | EntityKind::Other => crate::errors::MOD_NOT_FOUND,
            EntityKind::SubMod => crate::errors::SUB_MOD_NOT_FOUND,
            EntityKind::App => crate::errors::APP_NOT_FOUND,
            EntityKind::Layer => crate::errors::LAYER_NOT_FOUND,
        }
    }
}

/// Blank (or whitespace-only) input means "cleared", i.e. `NULL`.
fn cleaned(text: Option<&str>) -> Option<&str> {
    text.map(str::trim).filter(|s| !s.is_empty())
}

fn set_column(conn: &Connection, kind: EntityKind, id: &str, column: &str, value: Option<&str>) -> Result<(), String> {
    let (table, pk) = kind.table();
    let n = conn
        .execute(
            &format!("UPDATE {table} SET {column} = ?2 WHERE {pk} = ?1"),
            rusqlite::params![id, cleaned(value)],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err(kind.not_found().to_string());
    }
    Ok(())
}

/// Stores the free-form note (§9). `None` or blank clears it.
pub fn set_note(conn: &Connection, kind: EntityKind, id: &str, text: Option<&str>) -> Result<(), String> {
    set_column(conn, kind, id, "notes_user", text)
}

/// Stores the display name taken over by hand (§6.1). `None` or blank clears
/// it, which brings back the derived name — same semantics as §5bis.3.
pub fn set_display_name(conn: &Connection, kind: EntityKind, id: &str, name: Option<&str>) -> Result<(), String> {
    set_column(conn, kind, id, "display_name_user", name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overlay;

    fn db() -> (crate::testutil::TempDir, Connection) {
        let base = crate::testutil::temp_dir("usermeta");
        let conn = overlay::open(&base.join("overlay.sqlite")).unwrap();
        (base, conn)
    }

    fn now() -> String {
        chrono::Local::now().to_rfc3339()
    }

    /// Rule (§9.4, §6.1): every kind of entity can carry a note and a name,
    /// and both come back as they were typed.
    #[test]
    fn note_and_display_name_round_trip_on_every_entity_kind() {
        let (base, conn) = db();
        let now = now();
        overlay::upsert_mod(&conn, "spa", "Track", None, Some("Spa"), "h", None, &now).unwrap();
        overlay::insert_sub_mod(&conn, "S1", "SKIN", "spa", "skin_01", "lib/s1", None, &now).unwrap();
        overlay::insert_app(&conn, "camtool", "lib/apps/camtool", None, &now).unwrap();
        overlay::insert_other_mod(&conn, "policeman", "lib/others/policeman", None, &now).unwrap();
        overlay::insert_layer(
            &conn,
            "L1",
            "spa",
            "Track",
            "spa2022.rar",
            "lib/l1",
            None,
            1,
            0,
            0,
            &now,
        )
        .unwrap();

        let cases = [
            (EntityKind::Mod, "spa"),
            (EntityKind::SubMod, "S1"),
            (EntityKind::App, "camtool"),
            (EntityKind::Other, "policeman"),
            (EntityKind::Layer, "L1"),
        ];
        for (kind, id) in cases {
            set_note(&conn, kind, id, Some("pneus tendres seulement")).unwrap();
            set_display_name(&conn, kind, id, Some("Spa 2022")).unwrap();
        }

        assert_eq!(
            overlay::get_mod(&conn, "spa").unwrap().unwrap().notes_user.as_deref(),
            Some("pneus tendres seulement"),
            "a mod's note reads back"
        );
        let sub = overlay::get_sub_mod(&conn, "S1").unwrap().unwrap();
        assert_eq!(sub.display_name_user.as_deref(), Some("Spa 2022"), "skin renamed");
        assert_eq!(sub.notes_user.as_deref(), Some("pneus tendres seulement"), "skin note");
        let app = overlay::get_app(&conn, "camtool").unwrap().unwrap();
        assert_eq!(app.display_name_user.as_deref(), Some("Spa 2022"), "app renamed");
        let other = overlay::get_other_mod(&conn, "policeman").unwrap().unwrap();
        assert_eq!(
            other.notes_user.as_deref(),
            Some("pneus tendres seulement"),
            "other mod note"
        );
        let layers = overlay::list_layers(&conn, "spa", crate::layers::HostKind::Track).unwrap();
        assert_eq!(
            layers[0].display_name_user.as_deref(),
            Some("Spa 2022"),
            "layer renamed"
        );
        assert_eq!(
            layers[0].notes_user.as_deref(),
            Some("pneus tendres seulement"),
            "layer note"
        );
        drop(base);
    }

    /// Rule (§9.4): the entry survives a reimport of the entity — that is the
    /// whole point of storing it beside the derived fields instead of in them.
    #[test]
    fn note_survives_a_reimport_of_the_entity() {
        let (base, conn) = db();
        let now = now();
        overlay::upsert_mod(&conn, "spa", "Track", None, Some("Spa"), "h", None, &now).unwrap();
        overlay::insert_app(&conn, "camtool", "lib/apps/camtool", None, &now).unwrap();
        set_note(&conn, EntityKind::Mod, "spa", Some("virage 12 refait")).unwrap();
        set_display_name(&conn, EntityKind::App, "camtool", Some("CamTool")).unwrap();

        // Reimport: the mod comes back with a fresh display name, the app with
        // another library path.
        overlay::upsert_mod(
            &conn,
            "spa",
            "Track",
            None,
            Some("Spa-Francorchamps v2"),
            "h2",
            None,
            &now,
        )
        .unwrap();
        overlay::insert_app(&conn, "camtool", "lib/apps/camtool_v3", None, &now).unwrap();

        let m = overlay::get_mod(&conn, "spa").unwrap().unwrap();
        assert_eq!(
            m.notes_user.as_deref(),
            Some("virage 12 refait"),
            "the note survives the reimport"
        );
        assert_eq!(
            m.display_name.as_deref(),
            Some("Spa-Francorchamps v2"),
            "the derived name, on the other hand, is refreshed"
        );
        let a = overlay::get_app(&conn, "camtool").unwrap().unwrap();
        assert_eq!(
            a.display_name_user.as_deref(),
            Some("CamTool"),
            "the name taken over survives"
        );
        assert_eq!(
            a.library_path, "lib/apps/camtool_v3",
            "the path, on the other hand, is refreshed"
        );
        drop(base);
    }

    /// Rule (§9.2): clearing stores `NULL`, never an empty string — otherwise
    /// the "has a note" facet would count empty notes.
    #[test]
    fn blank_input_clears_instead_of_storing_an_empty_string() {
        let (base, conn) = db();
        let now = now();
        overlay::insert_other_mod(&conn, "sol", "lib/others/sol", None, &now).unwrap();
        set_note(&conn, EntityKind::Other, "sol", Some("à surveiller")).unwrap();

        set_note(&conn, EntityKind::Other, "sol", Some("   ")).unwrap();
        assert_eq!(
            overlay::get_other_mod(&conn, "sol").unwrap().unwrap().notes_user,
            None,
            "whitespace only clears the note"
        );

        set_note(&conn, EntityKind::Other, "sol", Some("  revenue  ")).unwrap();
        assert_eq!(
            overlay::get_other_mod(&conn, "sol")
                .unwrap()
                .unwrap()
                .notes_user
                .as_deref(),
            Some("revenue"),
            "the text is trimmed, not the entry"
        );
        drop(base);
    }

    /// Rule: a write on an unknown id fails instead of vanishing silently, so
    /// the UI can keep the typed text and say so.
    #[test]
    fn writing_on_an_unknown_id_fails_loudly() {
        let (base, conn) = db();
        let err = set_note(&conn, EntityKind::Layer, "jamais-vue", Some("x")).unwrap_err();
        assert_eq!(
            err,
            crate::errors::LAYER_NOT_FOUND,
            "a translatable error, not a silent success"
        );
        let err = set_display_name(&conn, EntityKind::App, "jamais-vue", Some("x")).unwrap_err();
        assert_eq!(err, crate::errors::APP_NOT_FOUND, "every kind already has its own key");
        drop(base);
    }
}
