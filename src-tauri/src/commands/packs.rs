//! Fiche d'un pack (§4.4) : ce que l'archive a livré ensemble, et ce qu'elle
//! pose dans le jeu sans que ça appartienne à un mod en particulier.
//!
//! Les ressources d'un pack sont les jumelles de celles d'une app
//! (`commands::addons`) : même dossier, mêmes garde-fous, seul le segment de
//! catégorie change. Le `rel_path` est toujours résolu et validé côté serveur
//! plutôt que reçu en absolu du front.

use super::prelude::*;

/// La fiche : membres, ajouts au jeu, tailles, date d'entrée en bibliothèque.
#[tauri::command]
pub fn get_pack_detail(app: AppHandle, db: State<Db>, pack: String) -> Result<crate::packs::PackDetail, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::packs::detail(&conn, &cfg, &pack)
}

/// Fichiers annexes du pack (§4.5.2) — notices et documents livrés à côté des
/// mods. Ils apparaissent aussi sur la fiche de chaque membre, marqués « du
/// pack » ; ici ils sont chez eux.
#[tauri::command]
pub fn list_pack_resources(app: AppHandle, pack: String) -> Result<Vec<crate::resources::ResourceFile>, String> {
    let cfg = crate::config::load(&app);
    Ok(crate::resources::list_resources(&crate::packs::resources_dir(
        &cfg, &pack,
    )?))
}

/// Ouvre une ressource du pack avec l'application par défaut de l'OS.
#[tauri::command]
pub fn open_pack_resource(app: AppHandle, pack: String, rel_path: String) -> Result<(), String> {
    let cfg = crate::config::load(&app);
    let dir = crate::packs::resources_dir(&cfg, &pack)?;
    let path = crate::resources::resolve_resource_path(&dir, &rel_path)?;
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Chemin absolu d'une ressource du pack, pour l'afficher via `asset://`.
#[tauri::command]
pub fn get_pack_resource_path(app: AppHandle, pack: String, rel_path: String) -> Result<String, String> {
    let cfg = crate::config::load(&app);
    let dir = crate::packs::resources_dir(&cfg, &pack)?;
    Ok(crate::resources::resolve_resource_path(&dir, &rel_path)?
        .display()
        .to_string())
}

/// Contenu brut d'une ressource du pack, pour la prévisualisation en fiche.
#[tauri::command]
pub fn read_pack_resource(app: AppHandle, pack: String, rel_path: String) -> Result<tauri::ipc::Response, String> {
    let cfg = crate::config::load(&app);
    let dir = crate::packs::resources_dir(&cfg, &pack)?;
    Ok(tauri::ipc::Response::new(crate::resources::read_resource(
        &dir, &rel_path,
    )?))
}

/// Ce que le pack installe dans le jeu (§4.5.3) — le trou que cette fiche
/// vient combler : `list_mod_extras` ne regarde que `extras/<type>/<id>`, donc
/// les fichiers rattachés au pack n'étaient affichés nulle part.
#[tauri::command]
pub fn list_pack_extras(app: AppHandle, db: State<Db>, pack: String) -> Result<Vec<crate::extras::ExtraFile>, String> {
    let cfg = crate::config::load(&app);
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    Ok(crate::extras::list(&conn, &cfg, crate::extras::OwnerKind::Pack, &pack))
}
