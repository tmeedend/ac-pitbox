//! Commandes du duo de session persisté (§8.6) — voir `session_state.rs`.

use super::prelude::*;
use crate::session_state::SessionPicks;

#[tauri::command]
pub fn get_session_picks(app: AppHandle) -> SessionPicks {
    crate::session_state::load(&app)
}

#[tauri::command]
pub fn save_session_picks(app: AppHandle, picks: SessionPicks) -> Result<(), String> {
    crate::session_state::save(&app, &picks)
}
