//! User-entered metadata commands: one note and one display name, for every
//! kind of entity (refonte §6.1 and §9).
//!
//! One pair of commands rather than one pair per type: the frontend says which
//! kind it is holding, and `usermeta` resolves the table. Adding an entity type
//! later costs one enum variant, not two more commands.

use super::prelude::*;

use crate::usermeta::{self, EntityKind};

/// Stores the free-form note of an entity. An empty string clears it.
#[tauri::command]
pub fn set_entity_note(db: State<Db>, kind: EntityKind, id: String, text: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    usermeta::set_note(&conn, kind, &id, Some(&text))
}

/// Stores the display name taken over by hand. An empty string clears it,
/// which brings back the derived name (§5bis.3).
#[tauri::command]
pub fn set_entity_display_name(db: State<Db>, kind: EntityKind, id: String, name: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    usermeta::set_display_name(&conn, kind, &id, Some(&name))
}
