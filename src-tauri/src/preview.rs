//! Aperçu 3D des voitures : cache disque et orchestration de la conversion
//! (`docs/SPEC-preview-3d-kn5.md` §5.3 et §7).
//!
//! Le `.glb` produit ne transite **jamais** par l'IPC (§7.2) : il est écrit
//! dans le cache, et l'UI ne reçoit qu'une URL servie par le protocole
//! `carpreview` (voir `lib.rs`). Un modèle de 30 Mo sérialisé en base64
//! deviendrait ~40 Mo de chaîne à parser côté JS — blocage de l'UI et pic
//! mémoire garantis.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::SystemTime;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::config::AppConfig;

/// Version du convertisseur, incluse dans la clé de cache.
///
/// **À incrémenter dès que le rendu produit change** — mapping matériaux,
/// filtrage de nœuds, taille des textures. Sans ça, un utilisateur qui met
/// l'app à jour continue de voir les anciens `.glb` : le §10 liste
/// « cache non versionné » parmi les pièges connus, et c'est celui qui se
/// remarque le plus tard.
const CONVERTER_VERSION: u32 = 1;

/// Plafond du cache (§5.3). Au-delà, éviction du plus ancien utilisé.
const CACHE_MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Ce que l'UI reçoit d'une conversion réussie (§7.1).
///
/// Nommé `CarPreview` et non `PreviewHandle` comme dans la spec : ce dernier
/// nom est déjà pris dans le crate par `music::PreviewHandle`, qui est un état
/// partagé de lecture audio — deux types homonymes aux rôles sans rapport
/// coûteraient plus cher que l'écart de nommage.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CarPreview {
    /// URL à donner à `GLTFLoader`, servie par le protocole `carpreview`.
    pub url: String,
    pub triangle_count: u32,
    pub material_count: u32,
    pub texture_count: u32,
    pub from_cache: bool,
}

/// État partagé : sérialise les conversions et permet d'abandonner celles
/// devenues obsolètes (§7.3).
#[derive(Default)]
pub struct PreviewState {
    /// Incrémenté à chaque demande. Une conversion dont le jeton n'est plus
    /// le dernier a été remplacée par une sélection plus récente.
    generation: AtomicU64,
    /// Une conversion à la fois : elles saturent déjà tous les cœurs par le
    /// transcodage parallèle des textures, en lancer deux ne ferait que les
    /// ralentir mutuellement.
    slot: Mutex<()>,
}

impl PreviewState {
    /// Prend un jeton pour la demande qui commence.
    pub fn next_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn is_current(&self, token: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == token
    }
}

/// Dossier du cache, créé au besoin.
pub fn cache_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("dossier de cache indisponible : {e}"))?
        .join("previews");
    std::fs::create_dir_all(&dir).map_err(|e| format!("création du cache : {e}"))?;
    Ok(dir)
}

/// Clé de cache d'un couple (modèle, skin).
///
/// Inclut la date et la taille du `.kn5` : réimporter une version modifiée
/// d'un mod invalide l'entrée sans qu'on ait à s'en occuper. Inclut le skin,
/// puisqu'il surcharge les textures (§4.3). Inclut la version du
/// convertisseur, qui invalide tout le cache d'un coup.
fn cache_key(model: &Path, skin: Option<&str>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(model.to_string_lossy().to_lowercase().as_bytes());
    if let Ok(meta) = std::fs::metadata(model) {
        hasher.update(meta.len().to_le_bytes());
        if let Ok(modified) = meta.modified() {
            if let Ok(since) = modified.duration_since(SystemTime::UNIX_EPOCH) {
                hasher.update(since.as_nanos().to_le_bytes());
            }
        }
    }
    hasher.update(skin.unwrap_or("").as_bytes());
    hasher.update(CONVERTER_VERSION.to_le_bytes());
    // 32 caractères hexadécimaux suffisent largement à éviter toute collision
    // sur quelques milliers d'entrées, et gardent des noms de fichier courts.
    format!("{:x}", hasher.finalize())[..32].to_string()
}

/// Prépare l'aperçu d'une voiture : renvoie l'entrée de cache si elle existe,
/// convertit sinon.
///
/// `car_dir` est résolu par l'appelant (bibliothèque d'abord, `content/`
/// ensuite) : le module ne connaît ni l'overlay ni la bibliothèque.
pub fn prepare(
    app: &tauri::AppHandle,
    state: &PreviewState,
    car_dir: &Path,
    skin_id: Option<&str>,
    token: u64,
) -> Result<CarPreview, String> {
    let resolved = kn5_gltf::resolve_model(car_dir).ok_or(crate::errors::PREVIEW_MODEL_NOT_FOUND)?;
    let dir = cache_dir(app)?;
    let key = cache_key(&resolved.path, skin_id);
    let file = dir.join(format!("{key}.glb"));

    if let Ok(meta) = std::fs::metadata(&file) {
        if meta.len() > 0 {
            // Touche la date pour que l'éviction LRU (§5.3) voie bien cette
            // entrée comme récemment utilisée : sans ça, une voiture consultée
            // tous les jours finirait évincée avant une convertie une fois.
            touch(&file);
            let counts = read_counts(&dir, &key).unwrap_or_default();
            return Ok(CarPreview {
                url: url_for(&key),
                triangle_count: counts.0,
                material_count: counts.1,
                texture_count: counts.2,
                from_cache: true,
            });
        }
    }

    // Une seule conversion à la fois, et on abandonne celles qu'une sélection
    // plus récente a rendues inutiles — l'utilisateur qui parcourt la liste
    // vite ne doit pas laisser une file de conversions orphelines (§7.3).
    let _slot = state
        .slot
        .lock()
        .map_err(|_| "verrou d'aperçu empoisonné".to_string())?;
    if !state.is_current(token) {
        return Err(crate::errors::PREVIEW_SUPERSEDED.to_string());
    }

    let bytes = std::fs::read(&resolved.path).map_err(|e| format!("{} : {e}", resolved.path.display()))?;
    let model = kn5::parse(&bytes).map_err(|e| match e {
        // Un KN5 chiffré (CSP) n'a pas la bonne magie : c'est la seule
        // détection dont on dispose, et on ne tente rien de plus (§4.5).
        kn5::Kn5Error::NotAKn5File => crate::errors::PREVIEW_PROTECTED.to_string(),
        other => format!("{} : {other}", resolved.path.display()),
    })?;

    let skin_dir = kn5_gltf::resolve_skin(car_dir, skin_id);
    let conversion = kn5_gltf::convert(
        &model,
        skin_dir.as_deref(),
        &kn5_gltf::ConvertOptions::default(),
        &|stage| {
            use tauri::Emitter;
            let _ = app.emit("preview://progress", stage.as_str());
        },
    )?;

    for warning in &conversion.texture_warnings {
        log::warn!("preview: texture `{}` ignorée — {}", warning.name, warning.reason);
    }

    write_entry(&dir, &key, &conversion)?;
    evict_over_cap(&dir);

    Ok(CarPreview {
        url: url_for(&key),
        triangle_count: conversion.triangle_count,
        material_count: conversion.material_count,
        texture_count: conversion.texture_count,
        from_cache: false,
    })
}

/// URL servie par le protocole custom (§7.2).
///
/// Forme Windows d'un scheme custom sous Tauri v2 — l'app ne cible que
/// Windows (§Stack).
fn url_for(key: &str) -> String {
    format!("http://carpreview.localhost/{key}.glb")
}

/// Écrit le `.glb` et, à côté, les compteurs à renvoyer sur un futur succès
/// de cache. Fichier séparé plutôt que relecture du `.glb` : reparser 40 Mo
/// de glTF pour trois entiers serait absurde.
fn write_entry(dir: &Path, key: &str, conversion: &kn5_gltf::Conversion) -> Result<(), String> {
    let file = dir.join(format!("{key}.glb"));
    std::fs::write(&file, &conversion.glb).map_err(|e| format!("{} : {e}", file.display()))?;
    let counts = format!(
        "{} {} {}",
        conversion.triangle_count, conversion.material_count, conversion.texture_count
    );
    let _ = std::fs::write(dir.join(format!("{key}.txt")), counts);
    Ok(())
}

fn read_counts(dir: &Path, key: &str) -> Option<(u32, u32, u32)> {
    let text = std::fs::read_to_string(dir.join(format!("{key}.txt"))).ok()?;
    let mut parts = text.split_whitespace();
    Some((
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ))
}

fn touch(file: &Path) {
    if let Ok(handle) = std::fs::File::options().write(true).open(file) {
        let _ = handle.set_modified(SystemTime::now());
    }
}

fn evict_over_cap(dir: &Path) {
    evict_to(dir, CACHE_MAX_BYTES);
}

/// Ramène le cache sous `cap` en supprimant les entrées les plus anciennement
/// utilisées (§5.3). Le plafond est un paramètre pour que le test puisse
/// prouver l'éviction sans écrire deux gigaoctets.
fn evict_to(dir: &Path, cap: u64) {
    let mut entries: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
    let Ok(read) = std::fs::read_dir(dir) else { return };
    for entry in read.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "glb") {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let used = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        entries.push((used, meta.len(), path));
    }

    let mut total: u64 = entries.iter().map(|(_, size, _)| size).sum();
    if total <= cap {
        return;
    }
    entries.sort_by_key(|(used, _, _)| *used);
    for (_, size, path) in entries {
        if total <= cap {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            let _ = std::fs::remove_file(path.with_extension("txt"));
            total = total.saturating_sub(size);
            log::warn!("preview: entrée de cache évincée ({})", path.display());
        }
    }
}

/// Vide le cache et renvoie le nombre d'octets libérés (§7.1).
pub fn clear_cache(app: &tauri::AppHandle) -> Result<u64, String> {
    let dir = cache_dir(app)?;
    let mut freed = 0u64;
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_file() && std::fs::remove_file(entry.path()).is_ok() {
            freed += meta.len();
        }
    }
    Ok(freed)
}

/// Sert une entrée du cache au protocole custom. Renvoie `None` pour toute
/// clé qui n'est pas un nom d'entrée : le chemin vient de la webview, donc
/// d'une source qu'on ne contrôle pas entièrement.
pub fn cached_file(dir: &Path, requested: &str) -> Option<PathBuf> {
    let name = requested.trim_start_matches('/');
    let stem = name.strip_suffix(".glb")?;
    // Une clé est un hachage hexadécimal : tout le reste (séparateurs, `..`)
    // est refusé avant de toucher au disque.
    if stem.is_empty() || !stem.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let file = dir.join(name);
    file.is_file().then_some(file)
}

/// Répond à une requête du protocole `carpreview` (§7.2).
///
/// Sert le fichier en octets bruts, avec le type MIME du glTF binaire et un
/// `Cache-Control: immutable` — la clé de cache **est** la version, donc le
/// contenu d'une URL donnée ne changera jamais. Gère les requêtes `Range`,
/// pour que la webview puisse streamer un gros modèle au lieu de tout attendre.
pub fn serve(app: &tauri::AppHandle, request: &tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
    use tauri::http::{header, Response, StatusCode};

    let not_found = || {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Vec::new())
            .unwrap_or_default()
    };

    let Ok(dir) = cache_dir(app) else { return not_found() };
    let Some(file) = cached_file(&dir, request.uri().path()) else {
        return not_found();
    };
    let Ok(bytes) = std::fs::read(&file) else {
        return not_found();
    };

    let total = bytes.len() as u64;
    let range = request
        .headers()
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| parse_range(v, total));

    let builder = Response::builder()
        .header(header::CONTENT_TYPE, "model/gltf-binary")
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
        .header(header::ACCEPT_RANGES, "bytes");

    match range {
        Some((start, end)) => builder
            .status(StatusCode::PARTIAL_CONTENT)
            .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{total}"))
            .body(bytes[start as usize..=end as usize].to_vec())
            .unwrap_or_else(|_| not_found()),
        None => builder.body(bytes).unwrap_or_else(|_| not_found()),
    }
}

/// `bytes=start-end`, bornes incluses, `end` optionnel. Renvoie `None` sur
/// tout ce qui n'est pas une plage simple : une plage multiple ou illisible
/// se sert entière, ce qui reste correct.
fn parse_range(value: &str, total: u64) -> Option<(u64, u64)> {
    let spec = value.strip_prefix("bytes=")?;
    if spec.contains(',') || total == 0 {
        return None;
    }
    let (start, end) = spec.split_once('-')?;
    let start: u64 = start.trim().parse().ok()?;
    let end: u64 = match end.trim() {
        "" => total - 1,
        text => text.parse().ok()?,
    };
    let end = end.min(total - 1);
    (start <= end).then_some((start, end))
}

/// Dossier de la voiture, bibliothèque d'abord et `content/` ensuite.
///
/// L'ordre compte : un mod **non déployé** doit avoir un aperçu, sinon la
/// fonctionnalité manque précisément au moment où on choisit quoi installer.
pub fn car_dir(conn: &rusqlite::Connection, cfg: &AppConfig, car_id: &str) -> Option<PathBuf> {
    let managed = crate::overlay::get_mod(conn, car_id)
        .ok()
        .flatten()
        .filter(|m| !m.is_stock)
        .and_then(|m| m.active_version_id)
        .and_then(|vid| crate::overlay::get_version_path(conn, &vid).ok().flatten())
        .and_then(|stored| crate::libpath::resolve(cfg.library_path.as_deref(), &stored))
        .filter(|dir| dir.is_dir());
    if managed.is_some() {
        return managed;
    }
    cfg.ac_install_path
        .as_ref()
        .map(|ac| ac.join("content").join("cars").join(car_id))
        .filter(|dir| dir.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_model(dir: &Path, name: &str, contents: &[u8]) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, contents).unwrap();
        path
    }

    // Règle : la clé de cache change dès que le fichier change, sinon un mod
    // réimporté garderait l'aperçu de son ancienne version (§5.3).
    #[test]
    fn cache_key_follows_the_file_and_the_skin() {
        let base = crate::testutil::temp_dir("preview-key");
        let model = write_model(&base, "car.kn5", b"first");

        let a = cache_key(&model, None);
        assert_eq!(a, cache_key(&model, None), "clé stable à contenu identique");
        assert_ne!(a, cache_key(&model, Some("red")), "le skin fait partie de la clé");

        // Réécriture avec une taille différente : la clé doit bouger même si
        // l'horloge du système de fichiers a une granularité grossière.
        std::fs::write(&model, b"second and longer").unwrap();
        assert_ne!(a, cache_key(&model, None), "un modèle modifié invalide son entrée");
    }

    // Règle : le nom demandé par la webview ne sert jamais à construire un
    // chemin sans être validé — un `..` doit sortir du protocole, pas du
    // dossier de cache.
    #[test]
    fn cached_file_refuses_anything_that_is_not_a_key() {
        let base = crate::testutil::temp_dir("preview-serve");
        let dir = base.join("previews");
        std::fs::create_dir_all(&dir).unwrap();
        let key = "abcdef0123456789abcdef0123456789";
        std::fs::write(dir.join(format!("{key}.glb")), b"glb").unwrap();
        std::fs::write(base.join("secret.txt"), b"nope").unwrap();

        assert!(cached_file(&dir, &format!("/{key}.glb")).is_some(), "clé valide servie");
        assert!(
            cached_file(&dir, "/../secret.txt").is_none(),
            "remontée de dossier refusée"
        );
        assert!(cached_file(&dir, "/nothex.glb").is_none(), "nom non hexadécimal refusé");
        assert!(cached_file(&dir, "/.glb").is_none(), "clé vide refusée");
        assert!(cached_file(&dir, &format!("/{key}")).is_none(), "extension obligatoire");
    }

    // Règle : au-delà du plafond, on évince les entrées les plus anciennement
    // utilisées — et le fichier de compteurs part avec.
    #[test]
    fn eviction_removes_the_least_recently_used_first() {
        let base = crate::testutil::temp_dir("preview-evict");
        let dir = base.join("previews");
        std::fs::create_dir_all(&dir).unwrap();

        let old = dir.join("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.glb");
        let fresh = dir.join("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb.glb");
        std::fs::write(&old, vec![0u8; 16]).unwrap();
        std::fs::write(old.with_extension("txt"), "1 2 3").unwrap();
        std::fs::write(&fresh, vec![0u8; 16]).unwrap();

        // Date d'usage ancienne sur la première entrée.
        let handle = std::fs::File::options().write(true).open(&old).unwrap();
        handle
            .set_modified(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1))
            .unwrap();
        drop(handle);

        // 32 octets en cache pour un plafond de 16 : une seule entrée doit
        // partir, la plus anciennement utilisée.
        evict_to(&dir, 16);
        assert!(!old.exists(), "l'entrée la plus ancienne est évincée");
        assert!(
            !old.with_extension("txt").exists(),
            "son fichier de compteurs part avec elle"
        );
        assert!(fresh.exists(), "l'entrée récente est conservée");

        // Une fois sous le plafond, plus rien ne bouge.
        evict_to(&dir, 16);
        assert!(fresh.exists(), "sous le plafond, rien n'est évincé");
    }

    // Règle : une plage `Range` correcte est honorée, et tout ce qui sort du
    // fichier ou du format simple retombe sur la réponse entière — jamais sur
    // une tranche fausse, qui donnerait un glTF tronqué illisible.
    #[test]
    fn range_header_is_parsed_or_ignored() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some((0, 99)), "plage explicite");
        assert_eq!(parse_range("bytes=500-", 1000), Some((500, 999)), "fin implicite");
        assert_eq!(
            parse_range("bytes=900-5000", 1000),
            Some((900, 999)),
            "fin au-delà du fichier, ramenée à la dernière position"
        );
        assert_eq!(parse_range("bytes=0-0", 1), Some((0, 0)), "fichier d'un seul octet");

        assert_eq!(parse_range("bytes=100-50", 1000), None, "plage inversée");
        assert_eq!(parse_range("bytes=0-10,20-30", 1000), None, "plages multiples");
        assert_eq!(parse_range("octets=0-10", 1000), None, "unité inconnue");
        assert_eq!(parse_range("bytes=0-10", 0), None, "fichier vide");
        assert_eq!(parse_range("bytes=abc-10", 1000), None, "borne illisible");
    }

    // Règle : une entrée servie depuis le cache voit sa date d'usage
    // rafraîchie, sinon une voiture consultée tous les jours finirait évincée
    // avant une convertie une seule fois.
    #[test]
    fn a_cache_hit_refreshes_the_usage_date() {
        let base = crate::testutil::temp_dir("preview-touch");
        let file = base.join("entry.glb");
        std::fs::write(&file, b"glb").unwrap();
        let handle = std::fs::File::options().write(true).open(&file).unwrap();
        handle
            .set_modified(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1))
            .unwrap();
        drop(handle);

        let before = std::fs::metadata(&file).unwrap().modified().unwrap();
        touch(&file);
        let after = std::fs::metadata(&file).unwrap().modified().unwrap();
        assert!(after > before, "la date d'usage avance");
    }
}
