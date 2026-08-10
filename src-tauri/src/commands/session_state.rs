//! Commandes du duo de session persisté (§8.6) — voir `session_state.rs`.

use super::prelude::*;
use crate::session_state::{LaunchState, SessionPicks};

#[tauri::command]
pub fn get_session_picks(app: AppHandle) -> SessionPicks {
    crate::session_state::load(&app)
}

#[tauri::command]
pub fn save_session_picks(app: AppHandle, picks: SessionPicks) -> Result<(), String> {
    crate::session_state::save(&app, &picks)
}

#[tauri::command]
pub fn get_launch_state(app: AppHandle) -> LaunchState {
    crate::session_state::load_launch_state(&app)
}

#[tauri::command]
pub fn save_launch_state(app: AppHandle, state: LaunchState) -> Result<(), String> {
    crate::session_state::save_launch_state(&app, &state)
}
