//! Commandes de session (§8) : météo, lancement via Content Manager et
//! aperçu 3D natif.

use super::prelude::*;

/// Météos installées (pour le sélecteur de lancement).
#[tauri::command]
pub fn list_weather(app: AppHandle) -> Vec<String> {
    crate::library::list_weather(&crate::config::load(&app))
}

/// Stack météo détectée (CSP/SOL/vanilla) — §8.5.
#[tauri::command]
pub fn weather_stack(app: AppHandle) -> crate::weather::WeatherStack {
    crate::weather::detect_stack(&crate::config::load(&app))
}

/// Intentions météo résolues selon la stack (dégradé gracieux).
#[tauri::command]
pub fn weather_options(app: AppHandle) -> Vec<crate::weather::WeatherOption> {
    crate::weather::options(&crate::config::load(&app))
}

/// Température + vent **recommandés** (air/piste/vent) pour une intention +
/// heure + saison optionnelle (§8.5/§8.6/§8.6bis). L'écran de session propose
/// ces valeurs par défaut ; l'air et la piste restent ensuite modifiables à la
/// main tant que la météo/saison ne change pas.
#[tauri::command]
pub fn weather_conditions(intent: String, hour: f32, season: Option<String>) -> crate::weather::ImplicitConditions {
    crate::weather::implicit_conditions(&intent, hour, season.as_deref())
}

/// Construit le preset Quick Drive et lance la session via Content Manager (§8.3).
#[tauri::command]
pub fn launch_session(app: AppHandle, db: State<Db>, setup: crate::launch::RaceSetup) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::launch::launch(&conn, &cfg, &setup)
}

/// Ouvre Content Manager sans argument (§12bis.5).
#[tauri::command]
pub fn open_content_manager(app: AppHandle) -> Result<(), String> {
    crate::launch::open_content_manager(&crate::config::load(&app))
}

/// Lance un replay dans Content Manager (§6.1, onglet Médias).
#[tauri::command]
pub fn launch_replay(app: AppHandle, replay_path: std::path::PathBuf) -> Result<(), String> {
    crate::launch::launch_replay(&crate::config::load(&app), &replay_path)
}

/// Lance l'aperçu 3D natif (`acShowroom.exe`, distinct de Content Manager)
/// ciblé sur une voiture (+ skin optionnel). Process indépendant, affiché
/// par-dessus l'app avec les réglages vidéo du jeu : l'utilisateur le ferme
/// lui-même pour revenir à Pit Box.
#[tauri::command]
pub fn open_native_showroom(app: AppHandle, car_id: String, skin_id: Option<String>) -> Result<(), String> {
    crate::showroom::open_native_showroom(&crate::config::load(&app), &car_id, skin_id.as_deref())
}

/// Showrooms installés dans AC, pour le choix de scène des réglages.
#[tauri::command]
pub fn list_showrooms(app: AppHandle) -> Vec<crate::showroom::ShowroomOption> {
    crate::showroom::list_showrooms(&crate::config::load(&app))
}
