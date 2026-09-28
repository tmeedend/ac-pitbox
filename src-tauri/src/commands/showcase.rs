//! The showcase as the screens see it (ESPACE§5.2, ESPACE§7.1): what a
//! deletion would do, and where the files of a mod in the showcase could come
//! back from. The removal itself is a lot (`bulk_ops::bulk_showcase`).

use super::prelude::*;

/// What the delete confirmation says about each mod, read before anything is
/// done.
#[tauri::command]
pub fn showcase_plan(
    app: AppHandle,
    db: State<Db>,
    ids: Vec<String>,
) -> Result<Vec<crate::showcase::PlanEntry>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::showcase::plan(&conn, &cfg, &ids)
}

/// Where the files of a mod in the showcase could come back from.
#[tauri::command]
pub fn showcase_sources(app: AppHandle, db: State<Db>, id: String) -> Result<crate::showcase::Sources, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::showcase::sources(&conn, &cfg, &id)
}
