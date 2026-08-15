//! Commandes de la bibliothèque (§6) : cartes, fiche détail, champs
//! overlay-éditables, ressources et skins.

use super::prelude::*;

#[tauri::command]
pub fn list_library(app: AppHandle, db: State<Db>) -> Result<Vec<ModCard>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::library::list_cards(&conn, &cfg).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_mod_detail(app: AppHandle, db: State<Db>, id: String) -> Result<Option<ModDetail>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::library::detail(&conn, &cfg, &id).map_err(|e| e.to_string())
}

/// Ouvre le dossier réel d'un mod (voiture/circuit) dans l'explorateur.
/// Appelle directement `Opener::open_path` côté Rust (contourne le scope ACL
/// du plugin, qui refuse par défaut tout chemin non pré-autorisé) : le chemin
/// vient de notre propre résolution `entity_dir`, pas d'une entrée libre côté
/// front, donc pas besoin d'élargir la permission `opener:allow-open-path`
/// avec un scope large.
#[tauri::command]
pub fn open_mod_folder(app: AppHandle, db: State<Db>, id: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let path = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        crate::library::folder_path(&conn, &cfg, &id)?
    };
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Dossier ressources d'un mod (§4.5.2). Le verrou SQLite est relâché en
/// sortant : toutes les opérations sur les ressources sont ensuite du pur
/// système de fichiers, parfois longues (lecture d'un PDF de plusieurs Mo), et
/// n'ont aucune raison de bloquer le reste de l'app.
fn resources_dir_of(app: &AppHandle, db: &State<Db>, id: &str) -> Result<std::path::PathBuf, String> {
    let cfg = crate::config::load(app);
    let library = cfg.library_path.clone().ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let m = crate::overlay::get_mod(&conn, id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::MOD_NOT_FOUND)?;
    Ok(crate::resources::resources_dir(&library, mod_kind(&m.kind), id))
}

/// Liste les fichiers annexes du mod (§4.5.2, « Bloc Ressources ») — lue en
/// direct sur disque à chaque appel, jamais mémorisée en base : un fichier
/// déposé manuellement apparaît sans réimport.
#[tauri::command]
pub fn list_mod_resources(
    app: AppHandle,
    db: State<Db>,
    id: String,
) -> Result<Vec<crate::resources::ResourceFile>, String> {
    Ok(crate::resources::list_resources(&resources_dir_of(&app, &db, &id)?))
}

/// Chemin absolu d'une ressource, pour l'afficher via le protocole `asset://`
/// (§4.5.2, prévisualisation des images). Le front ne construit jamais ce
/// chemin lui-même : il passe par ici pour bénéficier du garde-fou
/// anti-traversée, comme pour l'ouverture.
#[tauri::command]
pub fn get_mod_resource_path(app: AppHandle, db: State<Db>, id: String, rel_path: String) -> Result<String, String> {
    let dir = resources_dir_of(&app, &db, &id)?;
    let path = crate::resources::resolve_resource_path(&dir, &rel_path)?;
    Ok(path.display().to_string())
}

/// Contenu brut d'une ressource, pour la prévisualisation dans la fiche
/// (§4.5.2) : texte, markdown et PDF passent par ici plutôt que par
/// `asset://`, ce qui évite au front de dépendre du CORS du protocole
/// personnalisé. Les images, elles, restent servies en `asset://`
/// (`get_mod_resource_path`) — un `<img>` n'a pas besoin de l'octet en
/// mémoire.
#[tauri::command]
pub fn read_mod_resource(
    app: AppHandle,
    db: State<Db>,
    id: String,
    rel_path: String,
) -> Result<tauri::ipc::Response, String> {
    let dir = resources_dir_of(&app, &db, &id)?;
    Ok(tauri::ipc::Response::new(crate::resources::read_resource(
        &dir, &rel_path,
    )?))
}

/// Liste ce qu'un mod installe hors de `content/<type>/<id>` (§4.5.3) —
/// onglet « Ajouts au jeu » de la fiche.
#[tauri::command]
pub fn list_mod_extras(app: AppHandle, db: State<Db>, id: String) -> Result<Vec<crate::extras::ExtraFile>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let m = crate::overlay::get_mod(&conn, &id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::MOD_NOT_FOUND)?;
    Ok(crate::extras::list(&conn, &cfg, mod_kind(&m.kind), &id))
}

/// Ouvre un fichier du dossier ressources avec l'application par défaut de
/// l'OS (§4.5.2). `rel_path` est résolu et validé côté serveur (garde-fou
/// anti-traversée) plutôt que de faire confiance à un chemin absolu envoyé
/// par le front — même rationale que `open_mod_folder`.
#[tauri::command]
pub fn open_mod_resource(app: AppHandle, db: State<Db>, id: String, rel_path: String) -> Result<(), String> {
    let dir = resources_dir_of(&app, &db, &id)?;
    let path = crate::resources::resolve_resource_path(&dir, &rel_path)?;
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Fonctionnalités CSP effectivement détectées pour un mod (§6.4bis) : sert à
/// griser les réglages météo/saison non supportés sur l'écran de session.
#[tauri::command]
pub fn get_mod_csp_features(app: AppHandle, db: State<Db>, id: String) -> Result<Vec<String>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::library::mod_csp_features(&conn, &cfg, &id)
}

#[tauri::command]
pub fn set_favorite(db: State<Db>, id: String, favorite: bool) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::overlay::set_favorite(&conn, &id, favorite).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_manual_tags(db: State<Db>, id: String, tags: Vec<String>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::overlay::set_manual_tags(&conn, &id, &tags).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_mod_field(db: State<Db>, id: String, field: String, value: Option<String>) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::overlay::set_mod_field(&conn, &id, &field, value.as_deref()).map_err(|e| e.to_string())
}

/// Skins d'une voiture pour la fiche détail (mod ou voiture de base, §6.3/§12bis).
#[tauri::command]
pub fn list_mod_skins(app: AppHandle, db: State<Db>, id: String) -> Result<Vec<crate::library::SkinItem>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(crate::library::list_mod_skins(&conn, &cfg, &id))
}
