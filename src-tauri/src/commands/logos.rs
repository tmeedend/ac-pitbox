//! Commands of the brand logos (TAXO§4, §5, §9): the elected logos and the
//! user's choices, which live in `brand_logos.json` and `logos/`.

use super::prelude::*;
use crate::logos::{BrandPref, LogosView};

#[tauri::command]
pub fn get_brand_logos(app: AppHandle, db: State<Db>) -> Result<LogosView, String> {
    let dir = crate::rules::config_dir(&app)?;
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let cars = crate::library::car_badges(&conn, &cfg).map_err(|e| e.to_string())?;
    Ok(crate::logos::view(&cars, &dir))
}

/// His choice for a brand; an empty one is "automatic" again.
#[tauri::command]
pub fn save_brand_logo(app: AppHandle, brand: String, pref: BrandPref) -> Result<(), String> {
    crate::logos::set_pref(&crate::rules::config_dir(&app)?, &brand, pref)
}

/// Copies a file of his into Pit Box's folder and makes it the brand's logo
/// (TAXO§9), keeping his plate setting.
#[tauri::command]
pub fn import_brand_logo(app: AppHandle, brand: String, path: String) -> Result<(), String> {
    let dir = crate::rules::config_dir(&app)?;
    let name = crate::logos::import_file(&dir, &brand, std::path::Path::new(&path))?;
    let mut pref = crate::logos::load_prefs(&dir).remove(&brand).unwrap_or_default();
    pref.custom = Some(name);
    crate::logos::set_pref(&dir, &brand, pref)
}
