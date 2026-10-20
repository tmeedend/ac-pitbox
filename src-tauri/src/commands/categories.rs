//! Forcing the category of cars and tracks (REGLES§3 level 4), one mod or a
//! selection. Façades only: the decision's meaning is `categories.rs`'s.

use super::prelude::*;
use crate::categories::CategoryOp;
use crate::modscan::ModKind;

/// Lays one gesture on the category of every mod of `ids`, all or nothing:
/// force, add, remove or go back to what the rules compute. A few SQLite
/// writes, so it stays on the window's thread and two gestures in a row run in
/// the order they were made.
#[tauri::command]
pub fn apply_category_op(app: AppHandle, db: State<Db>, ids: Vec<String>, op: CategoryOp) -> Result<(), String> {
    let rules = crate::rules::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::categories::apply(&conn, &rules, &ids, &op)
}

/// The categories a picker offers for a kind (`"Car"` or `"Track"`): what the
/// rules can produce, then what the user decided on other mods.
#[tauri::command]
pub fn list_category_choices(app: AppHandle, db: State<Db>, kind: String) -> Result<Vec<String>, String> {
    let rules = crate::rules::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::categories::vocabulary(&conn, &rules, ModKind::from_column(&kind)).map_err(|e| e.to_string())
}
