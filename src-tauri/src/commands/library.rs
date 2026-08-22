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

/// Les deux racines d'où peut venir une annexe (§4.5.2) : le dossier
/// ressources de la bibliothèque, et le dossier du mod lui-même pour les
/// documents que la règle d'or (§4.5.1) interdit d'en sortir.
///
/// Le verrou SQLite est relâché en sortant : tout ce qui suit est du pur
/// système de fichiers, parfois long (lecture d'un PDF de plusieurs Mo), et
/// n'a aucune raison de bloquer le reste de l'app.
struct ResourceRoots {
    resources: std::path::PathBuf,
    mod_dir: Option<std::path::PathBuf>,
}

fn resource_roots(app: &AppHandle, db: &State<Db>, id: &str) -> Result<ResourceRoots, String> {
    let cfg = crate::config::load(app);
    let library = cfg.library_path.clone().ok_or(crate::errors::LIBRARY_NOT_CONFIGURED)?;
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let m = crate::overlay::get_mod(&conn, id)
        .map_err(|e| e.to_string())?
        .ok_or(crate::errors::MOD_NOT_FOUND)?;
    Ok(ResourceRoots {
        resources: crate::resources::resources_dir(&library, mod_kind(&m.kind), id),
        // Absent pour un mod dont le dossier n'est pas résolvable (version
        // manquante, AC non configuré) : la liste se réduit alors au dossier
        // ressources au lieu d'échouer entièrement.
        mod_dir: crate::library::folder_path(&conn, &cfg, id).ok(),
    })
}

/// Racine contre laquelle résoudre un `rel_path`, selon d'où le front dit que
/// l'entrée vient. Le choix de la racine n'ouvre aucune brèche : le garde-fou
/// anti-traversée s'applique ensuite à celle qui a été retenue, donc au pire
/// le front lit dans l'autre dossier du même mod.
fn resource_root(app: &AppHandle, db: &State<Db>, id: &str, in_mod: bool) -> Result<std::path::PathBuf, String> {
    let roots = resource_roots(app, db, id)?;
    if in_mod {
        roots
            .mod_dir
            .ok_or_else(|| format!("dossier introuvable pour « {id} »"))
    } else {
        Ok(roots.resources)
    }
}

/// Liste les fichiers annexes du mod (§4.5.2, « Bloc Ressources ») — lue en
/// direct sur disque à chaque appel, jamais mémorisée en base : un fichier
/// déposé manuellement apparaît sans réimport.
///
/// Deux provenances réunies dans une seule liste : ce qui a été rangé à part à
/// l'import, et les documents restés à la racine du dossier du mod parce que
/// la règle d'or interdit de les en sortir (§4.5.1). Un `readme.txt` posé par
/// l'auteur au milieu du circuit se lit donc comme les autres.
#[tauri::command]
pub fn list_mod_resources(
    app: AppHandle,
    db: State<Db>,
    id: String,
) -> Result<Vec<crate::resources::ResourceFile>, String> {
    let roots = resource_roots(&app, &db, &id)?;
    let mut out = crate::resources::list_resources(&roots.resources);
    if let Some(dir) = &roots.mod_dir {
        out.extend(crate::resources::list_in_mod_documents(dir));
    }
    Ok(out)
}

/// Chemin absolu d'une ressource, pour l'afficher via le protocole `asset://`
/// (§4.5.2, prévisualisation des images). Le front ne construit jamais ce
/// chemin lui-même : il passe par ici pour bénéficier du garde-fou
/// anti-traversée, comme pour l'ouverture.
#[tauri::command]
pub fn get_mod_resource_path(
    app: AppHandle,
    db: State<Db>,
    id: String,
    rel_path: String,
    in_mod: bool,
) -> Result<String, String> {
    let dir = resource_root(&app, &db, &id, in_mod)?;
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
    in_mod: bool,
) -> Result<tauri::ipc::Response, String> {
    let dir = resource_root(&app, &db, &id, in_mod)?;
    Ok(tauri::ipc::Response::new(crate::resources::read_resource(
        &dir, &rel_path,
    )?))
}

/// Décisions que l'app a prises seule au dernier import de ce mod (§4.6) —
/// bloc « Décisions d'import » de la fiche.
#[tauri::command]
pub fn list_import_decisions(db: State<Db>, id: String) -> Result<Vec<crate::overlay::ImportJournalEntry>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::overlay::decisions_for_mod(&conn, &id).map_err(|e| e.to_string())
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
    Ok(crate::extras::list(&conn, &cfg, mod_kind(&m.kind).into(), &id))
}

/// Ouvre un fichier du dossier ressources avec l'application par défaut de
/// l'OS (§4.5.2). `rel_path` est résolu et validé côté serveur (garde-fou
/// anti-traversée) plutôt que de faire confiance à un chemin absolu envoyé
/// par le front — même rationale que `open_mod_folder`.
#[tauri::command]
pub fn open_mod_resource(
    app: AppHandle,
    db: State<Db>,
    id: String,
    rel_path: String,
    in_mod: bool,
) -> Result<(), String> {
    let dir = resource_root(&app, &db, &id, in_mod)?;
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
