//! Commands of the brand logos (TAXO§4, §5, §9): the elected logos and the
//! user's choices, which live in `brand_logos.json` and `logos/`.

use super::prelude::*;
use crate::logos::{BrandPref, LogosView};
use tauri::Manager;

/// Off the window's thread (`off_window`), and the base lock released before
/// the files are read: the view decodes every car's badge (256 ms for 356
/// badges on the first call of a session, measured in release), and it is
/// asked for at startup, alongside the library listing. It held the lock and
/// the window's thread for all of it.
#[tauri::command]
pub async fn get_brand_logos(app: AppHandle) -> Result<LogosView, String> {
    off_window(app, |app| {
        let dir = crate::rules::config_dir(app)?;
        let cfg = crate::config::load(app);
        crate::timing::step("cmd.get_brand_logos", || {
            let cars = crate::library::car_badges_shared(&app.state::<Db>(), &cfg)?;
            Ok(crate::logos::view(&cars, &dir))
        })
    })
    .await
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
