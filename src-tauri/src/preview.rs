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

/// Version du convertisseur, **préfixe du nom de chaque entrée du cache**.
///
/// **À incrémenter dès que le rendu produit change** — mapping matériaux,
/// filtrage de nœuds, taille des textures. Sans ça, un utilisateur qui met
/// l'app à jour continue de voir les anciens `.glb` : le §10 liste
/// « cache non versionné » parmi les pièges connus, et c'est celui qui se
/// remarque le plus tard.
///
/// Dans le **nom** et non dans le hachage, alors que l'un ou l'autre suffirait
/// à ne plus servir une entrée périmée : seul le nom permet aussi de la
/// *reconnaître* pour libérer sa place. Trois incréments en une session de
/// travail avaient laissé plusieurs centaines de Mo d'entrées mortes, que rien
/// n'aurait effacées avant que le plafond de 2 Gio ne finisse par les évincer.
const CONVERTER_VERSION: u32 = 15;

/// Default cache ceiling (§5.3). Beyond it, the least recently used entries
/// are evicted. Only a default: the real ceiling is a setting, carried by
/// [`PreviewState`] — the frontend pushes it in at startup and on every
/// change (`set_preview_cache_cap`).
const DEFAULT_CACHE_MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Bounds accepted for that setting. The floor is not cosmetic: a ceiling
/// under one entry would evict a model the moment it is written, so every
/// preview would reconvert on every visit — a setting that turns the cache
/// off without saying so.
const CACHE_CAP_MIN_BYTES: u64 = 512 * 1024 * 1024;
const CACHE_CAP_MAX_BYTES: u64 = 20 * 1024 * 1024 * 1024;

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
pub struct PreviewState {
    /// Incrémenté à chaque demande. Une conversion dont le jeton n'est plus
    /// le dernier a été remplacée par une sélection plus récente.
    generation: AtomicU64,
    /// Une conversion à la fois : elles saturent déjà tous les cœurs par le
    /// transcodage parallèle des textures, en lancer deux ne ferait que les
    /// ralentir mutuellement.
    slot: Mutex<()>,
    /// Le ménage des entrées d'une version antérieure a-t-il déjà eu lieu ?
    /// Une fois par exécution suffit — le dossier ne se périme pas tout seul.
    swept: std::sync::atomic::AtomicBool,
    /// Cache ceiling in bytes, as the user set it. Held here rather than read
    /// from `ui_prefs.json`: that file's schema belongs to the frontend (see
    /// `ui_prefs.rs`), so the frontend pushes the value in rather than the
    /// backend reaching into it. Until it does, the default applies — which
    /// only matters for a conversion asked before the UI has booted, and there
    /// is none.
    cap: AtomicU64,
}

impl Default for PreviewState {
    fn default() -> Self {
        Self {
            generation: AtomicU64::new(0),
            slot: Mutex::new(()),
            swept: std::sync::atomic::AtomicBool::new(false),
            cap: AtomicU64::new(DEFAULT_CACHE_MAX_BYTES),
        }
    }
}

impl PreviewState {
    /// Prend un jeton pour la demande qui commence.
    pub fn next_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn is_current(&self, token: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == token
    }

    fn cache_cap(&self) -> u64 {
        self.cap.load(Ordering::Relaxed)
    }
}

/// Brings `bytes` into the accepted range. An out-of-bounds value is clamped
/// rather than refused: it reaches us from a slider, and a settings screen has
/// no useful way to report "this number is impossible".
fn clamp_cap(bytes: u64) -> u64 {
    bytes.clamp(CACHE_CAP_MIN_BYTES, CACHE_CAP_MAX_BYTES)
}

/// Applies the cache ceiling and enforces it **right away** (§5.3).
///
/// Evicting on the spot rather than at the next conversion is what makes the
/// setting legible: someone who lowers the ceiling to free disk space expects
/// the space to be free when the figure next to the slider updates, not after
/// they next open a car.
pub fn set_cache_cap(app: &tauri::AppHandle, state: &PreviewState, bytes: u64) -> Result<(), String> {
    let cap = clamp_cap(bytes);
    state.cap.store(cap, Ordering::Relaxed);
    evict_to(&cache_dir(app)?, cap);
    Ok(())
}

/// Bytes currently held by the cache, entries and counter files alike.
pub fn cache_usage(app: &tauri::AppHandle) -> Result<u64, String> {
    let dir = cache_dir(app)?;
    let mut total = 0u64;
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                total += meta.len();
            }
        }
    }
    Ok(total)
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

/// Préfixe des entrées écrites par la version courante du convertisseur.
fn version_prefix() -> String {
    format!("v{CONVERTER_VERSION}-")
}

/// Nom de fichier (sans extension) d'une entrée du cache.
fn entry_stem(key: &str) -> String {
    format!("{}{key}", version_prefix())
}

/// Efface les entrées écrites par une **autre** version du convertisseur.
///
/// Elles ne seront plus jamais servies — leur nom ne peut plus être demandé —
/// mais elles occupent le disque et poussent les entrées vivantes vers
/// l'éviction. Best-effort : un fichier verrouillé n'est pas un problème, on
/// repassera au prochain démarrage.
fn sweep_foreign_versions(dir: &Path) {
    let prefix = version_prefix();
    let Ok(read) = std::fs::read_dir(dir) else { return };
    let mut removed = 0u32;
    let mut freed = 0u64;
    for entry in read.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(&prefix) {
            continue;
        }
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        match std::fs::remove_file(entry.path()) {
            Ok(()) => {
                removed += 1;
                freed += size;
            }
            Err(e) => log::warn!("preview: entrée de cache périmée {name} non supprimée — {e}"),
        }
    }
    if removed > 0 {
        log::info!(
            "preview: {removed} entrée(s) de cache d'une version antérieure effacée(s), {} Mio libérés",
            freed / (1024 * 1024)
        );
    }
}

/// Mêle la taille et la date d'un fichier au hachage, ou rien du tout s'il
/// n'existe pas — un `ext_config.ini` absent est le cas courant, pas une
/// erreur.
fn stamp(hasher: &mut Sha256, path: &Path) {
    if let Ok(meta) = std::fs::metadata(path) {
        hasher.update(meta.len().to_le_bytes());
        if let Ok(modified) = meta.modified() {
            if let Ok(since) = modified.duration_since(SystemTime::UNIX_EPOCH) {
                hasher.update(since.as_nanos().to_le_bytes());
            }
        }
    }
}

/// Les configs CSP qui peuvent modifier le modèle d'une voiture, dans l'ordre
/// où [`kn5_gltf::apply_ext_config`] les lit.
fn ext_config_paths(car_dir: &Path, skin_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut paths = vec![car_dir.join("extension").join("ext_config.ini")];
    if let Some(skin) = skin_dir {
        paths.push(skin.join("ext_config.ini"));
    }
    paths
}

/// Clé de cache d'un couple (modèle, skin).
///
/// Inclut la date et la taille du `.kn5` : réimporter une version modifiée
/// d'un mod invalide l'entrée sans qu'on ait à s'en occuper. Inclut le skin,
/// puisqu'il surcharge les textures (§4.3). La version du convertisseur, elle,
/// est portée par le nom du fichier (voir [`CONVERTER_VERSION`]).
fn cache_key(model: &Path, skin: Option<&str>, configs: &[PathBuf]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(model.to_string_lossy().to_lowercase().as_bytes());
    stamp(&mut hasher, model);
    hasher.update(skin.unwrap_or("").as_bytes());
    // Les `ext_config.ini` décident des morceaux greffés sur le modèle
    // (`kn5_gltf::apply_ext_config`) : ils font donc partie de l'identité de
    // l'entrée au même titre que le `.kn5`. Sans eux, corriger une ligne de
    // config laisserait l'ancien aperçu servi indéfiniment.
    for config in configs {
        hasher.update(config.to_string_lossy().to_lowercase().as_bytes());
        stamp(&mut hasher, config);
    }
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
    // Une seule fois par exécution : au premier aperçu demandé, pas au
    // démarrage, pour ne rien coûter à qui n'en ouvre jamais.
    if !state.swept.swap(true, Ordering::Relaxed) {
        sweep_foreign_versions(&dir);
    }
    // Le skin est résolu avant la clé : c'est lui qui désigne le dossier où
    // vivent le `ext_config.ini` et les KN5 de jante (§4.3).
    let skin_dir = kn5_gltf::resolve_skin(car_dir, skin_id);
    let configs = ext_config_paths(car_dir, skin_dir.as_deref());
    let stem = entry_stem(&cache_key(&resolved.path, skin_id, &configs));
    let file = dir.join(format!("{stem}.glb"));

    if let Ok(meta) = std::fs::metadata(&file) {
        if meta.len() > 0 {
            // Touche la date pour que l'éviction LRU (§5.3) voie bien cette
            // entrée comme récemment utilisée : sans ça, une voiture consultée
            // tous les jours finirait évincée avant une convertie une fois.
            touch(&file);
            let counts = read_counts(&dir, &stem).unwrap_or_default();
            return Ok(CarPreview {
                url: url_for(&stem),
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
    let mut model = kn5::parse(&bytes).map_err(|e| match e {
        // Un KN5 chiffré (CSP) n'a pas la bonne magie : c'est la seule
        // détection dont on dispose, et on ne tente rien de plus (§4.5).
        kn5::Kn5Error::NotAKn5File => crate::errors::PREVIEW_PROTECTED.to_string(),
        other => format!("{} : {other}", resolved.path.display()),
    })?;

    // Deuxième détection, complémentaire à la magie ci-dessus (§4.5bis) : un
    // modèle peut avoir un en-tête KN5 parfaitement valide et pourtant ne
    // rien avoir d'affichable — deux mods signalés cassés (un carré bleu qui
    // clignote, une pluie de petits polygones bleus) parsent sans la moindre
    // erreur mais n'orientent leurs triangles correctement qu'une fois sur
    // deux, littéralement un résultat de pile ou face, là où les mods sains
    // de la bibliothèque de référence sont tous au-dessus de 99,5 % (voir
    // `kn5_gltf::WINDING_SANITY_THRESHOLD`, mesuré, pas deviné). On ne sait
    // pas si c'est une protection CSP ou une autre corruption qui laisse le
    // magic intact — l'un comme l'autre produisent la même chose côté
    // utilisateur, donc le même traitement : repli silencieux sur la photo,
    // jamais de tentative de rendu sur des triangles qui ne veulent rien dire.
    let (agreeing, total) = kn5_gltf::winding_consistency(&model);
    if !kn5_gltf::is_geometry_sane(agreeing, total) {
        return Err(crate::errors::PREVIEW_PROTECTED.to_string());
    }

    // Beaucoup de mods de préparation livrent un KN5 volontairement incomplet
    // et laissent CSP y greffer, skin par skin, les pièces qui changent :
    // jantes, boucliers, optiques. Sans cette passe, l'aperçu montre une
    // voiture trouée alors que le jeu l'affiche entière. Après le contrôle
    // d'enroulement ci-dessus, qui doit juger le modèle d'origine et lui seul.
    let ext = kn5_gltf::apply_ext_config(&mut model, car_dir, &resolved.path, skin_dir.as_deref());
    for failure in &ext.failures {
        log::warn!("preview: remplacement CSP ignoré — {failure}");
    }

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

    write_entry(&dir, &stem, &conversion)?;
    evict_to(&dir, state.cache_cap());

    Ok(CarPreview {
        url: url_for(&stem),
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
    // Un nom d'entrée est `v<version>-<hachage hexadécimal>` : tout le reste
    // (séparateurs, `..`) est refusé avant de toucher au disque.
    let key = stem
        .strip_prefix('v')
        .and_then(|rest| rest.split_once('-'))
        .filter(|(version, _)| !version.is_empty() && version.chars().all(|c| c.is_ascii_digit()))
        .map(|(_, key)| key)?;
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_hexdigit()) {
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
///
/// **Les en-têtes CORS ne sont pas optionnels.** Un protocole custom vit sur sa
/// propre origine (`http://carpreview.localhost` sous Windows), distincte de
/// celle de la page — `http://localhost:1420` en développement,
/// `http://tauri.localhost` une fois packagé. Toute lecture depuis la page est
/// donc une requête d'origine croisée, refusée par le navigateur avant même
/// que le fichier ne soit lu si `Access-Control-Allow-Origin` manque. Bug réel
/// au premier essai utilisateur : les étapes de conversion s'affichaient
/// normalement (elles passent par l'IPC, pas par ce protocole) puis « aperçu 3D
/// indisponible », alors que le `.glb` était bien écrit dans le cache.
pub fn serve_request(
    app: &tauri::AppHandle,
    request: &tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    match cache_dir(app) {
        Ok(dir) => serve(&dir, request),
        Err(e) => {
            log::warn!("preview: cache indisponible ({e})");
            serve(Path::new(""), request)
        }
    }
}

/// Corps de [`serve_request`], séparé du `AppHandle` pour être testable sans
/// lancer Tauri — c'est ce qui permet de verrouiller les en-têtes CORS par un
/// test plutôt que par un souvenir.
pub fn serve(dir: &Path, request: &tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
    use tauri::http::{header, Method, Response, StatusCode};

    // Appliqués à **toutes** les réponses, y compris les erreurs : une 404 sans
    // en-tête CORS remonte côté page comme une erreur réseau opaque, ce qui
    // masque la vraie cause.
    let with_cors = |builder: tauri::http::response::Builder| {
        builder
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET, HEAD, OPTIONS")
            .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "Range")
            .header(
                header::ACCESS_CONTROL_EXPOSE_HEADERS,
                "Content-Length, Content-Range, Accept-Ranges",
            )
    };

    let not_found = || {
        with_cors(Response::builder().status(StatusCode::NOT_FOUND))
            .body(Vec::new())
            .unwrap_or_default()
    };

    // `Range` n'est pas un en-tête sûr au sens CORS : une lecture partielle
    // déclenche un préchargement `OPTIONS` qu'il faut accepter.
    if request.method() == Method::OPTIONS {
        return with_cors(Response::builder().status(StatusCode::NO_CONTENT))
            .body(Vec::new())
            .unwrap_or_default();
    }

    let Some(file) = cached_file(dir, request.uri().path()) else {
        log::warn!("preview: entrée de cache absente ({})", request.uri().path());
        return not_found();
    };
    let Ok(bytes) = std::fs::read(&file) else {
        log::warn!("preview: entrée de cache illisible ({})", file.display());
        return not_found();
    };

    let total = bytes.len() as u64;
    let range = request
        .headers()
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| parse_range(v, total));

    let builder = with_cors(
        Response::builder()
            .header(header::CONTENT_TYPE, "model/gltf-binary")
            .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
            .header(header::ACCEPT_RANGES, "bytes"),
    );

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

        let a = cache_key(&model, None, &[]);
        assert_eq!(a, cache_key(&model, None, &[]), "clé stable à contenu identique");
        assert_ne!(a, cache_key(&model, Some("red"), &[]), "le skin fait partie de la clé");

        // Réécriture avec une taille différente : la clé doit bouger même si
        // l'horloge du système de fichiers a une granularité grossière.
        std::fs::write(&model, b"second and longer").unwrap();
        assert_ne!(a, cache_key(&model, None, &[]), "un modèle modifié invalide son entrée");
    }

    // Règle : un `ext_config.ini` fait partie de la clé, parce qu'il décide
    // des pièces greffées sur le modèle. Sans ça, corriger une ligne de config
    // laisse l'ancien aperçu troué servi depuis le cache — exactement le piège
    // « cache non versionné » du §10, sous une autre forme.
    #[test]
    fn cache_key_follows_the_ext_config() {
        let base = crate::testutil::temp_dir("preview-key-ext");
        let model = write_model(&base, "car.kn5", b"first");
        let config = base.join("extension").join("ext_config.ini");

        // Absent, le fichier ne doit pas empêcher de calculer une clé : c'est
        // le cas de l'immense majorité des voitures.
        let without = cache_key(&model, None, std::slice::from_ref(&config));
        assert_eq!(
            without,
            cache_key(&model, None, std::slice::from_ref(&config)),
            "clé stable quand la config n'existe pas"
        );

        write_model(config.parent().unwrap(), "ext_config.ini", b"[MODEL_REPLACEMENT_...]");
        let with = cache_key(&model, None, std::slice::from_ref(&config));
        assert_ne!(without, with, "l'apparition d'une config invalide l'entrée");

        std::fs::write(
            &config,
            b"[MODEL_REPLACEMENT_...]
INSERT = part.kn5",
        )
        .unwrap();
        assert_ne!(with, cache_key(&model, None, &[config]), "une config modifiée aussi");
    }

    // Règle : les configs consultées sont celles que `apply_ext_config` lit,
    // dans le même ordre — la voiture puis le skin, du général au particulier.
    #[test]
    fn ext_config_paths_follow_the_car_then_the_skin() {
        let car = Path::new("D:/lib/ks_toyota_ae86_tuned");
        let skin = car.join("skins").join("00_panda");

        assert_eq!(
            ext_config_paths(car, None),
            vec![car.join("extension").join("ext_config.ini")],
            "sans skin, seule la config de la voiture"
        );
        assert_eq!(
            ext_config_paths(car, Some(&skin)),
            vec![
                car.join("extension").join("ext_config.ini"),
                skin.join("ext_config.ini"),
            ],
            "la config du skin vient après celle de la voiture"
        );
    }

    // Règle : le nom demandé par la webview ne sert jamais à construire un
    // chemin sans être validé — un `..` doit sortir du protocole, pas du
    // dossier de cache.
    #[test]
    fn cached_file_refuses_anything_that_is_not_a_key() {
        let base = crate::testutil::temp_dir("preview-serve");
        let dir = base.join("previews");
        std::fs::create_dir_all(&dir).unwrap();
        let stem = entry_stem("abcdef0123456789abcdef0123456789");
        std::fs::write(dir.join(format!("{stem}.glb")), b"glb").unwrap();
        std::fs::write(base.join("secret.txt"), b"nope").unwrap();

        assert!(cached_file(&dir, &format!("/{stem}.glb")).is_some(), "nom valide servi");
        assert!(
            cached_file(&dir, "/../secret.txt").is_none(),
            "remontée de dossier refusée"
        );
        assert!(
            cached_file(&dir, "/v7-nothex.glb").is_none(),
            "clé non hexadécimale refusée"
        );
        assert!(
            cached_file(&dir, "/abcdef0123456789abcdef0123456789.glb").is_none(),
            "préfixe de version obligatoire"
        );
        assert!(cached_file(&dir, "/v7-.glb").is_none(), "clé vide refusée");
        assert!(
            cached_file(&dir, &format!("/{stem}")).is_none(),
            "extension obligatoire"
        );
    }

    // Règle : les entrées d'une version antérieure du convertisseur sont
    // effacées, pas seulement ignorées — sinon elles occupent le disque jusqu'à
    // ce que le plafond finisse par les évincer. Trois incréments de version en
    // une session avaient laissé plusieurs centaines de Mo derrière eux.
    #[test]
    fn entries_from_an_older_converter_are_reclaimed() {
        let base = crate::testutil::temp_dir("preview-sweep");
        let dir = base.join("previews");
        std::fs::create_dir_all(&dir).unwrap();
        let mine = entry_stem("abcdef0123456789abcdef0123456789");
        std::fs::write(dir.join(format!("{mine}.glb")), b"glb").unwrap();
        std::fs::write(dir.join(format!("{mine}.txt")), b"1 2 3").unwrap();
        std::fs::write(dir.join("v1-abcdef0123456789abcdef0123456789.glb"), b"vieux").unwrap();
        std::fs::write(dir.join("v1-abcdef0123456789abcdef0123456789.txt"), b"1 2 3").unwrap();
        // Nom de l'époque où la version vivait dans le hachage, sans préfixe.
        std::fs::write(dir.join("0123456789abcdef0123456789abcdef.glb"), b"ancien").unwrap();

        sweep_foreign_versions(&dir);

        assert!(
            dir.join(format!("{mine}.glb")).is_file() && dir.join(format!("{mine}.txt")).is_file(),
            "l'entrée de la version courante survit, compteurs compris"
        );
        assert!(
            !dir.join("v1-abcdef0123456789abcdef0123456789.glb").exists(),
            "l'entrée d'une version antérieure est effacée"
        );
        assert!(
            !dir.join("v1-abcdef0123456789abcdef0123456789.txt").exists(),
            "son fichier de compteurs part avec"
        );
        assert!(
            !dir.join("0123456789abcdef0123456789abcdef.glb").exists(),
            "et les noms sans préfixe, d'avant ce nommage, aussi"
        );
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

    // Règle : le plafond de cache réglable reste dans ses bornes, et un
    // plafond trop bas ne peut pas transformer le cache en trou noir
    // (§5.3 — bornes de `set_preview_cache_cap`).
    #[test]
    fn cache_cap_is_clamped_into_its_range() {
        assert_eq!(clamp_cap(0), CACHE_CAP_MIN_BYTES, "zero is raised to the floor");
        assert_eq!(clamp_cap(u64::MAX), CACHE_CAP_MAX_BYTES, "the ceiling is capped");
        let inside = 4 * 1024 * 1024 * 1024;
        assert_eq!(clamp_cap(inside), inside, "a value inside the range is left alone");
        assert_eq!(
            clamp_cap(DEFAULT_CACHE_MAX_BYTES),
            DEFAULT_CACHE_MAX_BYTES,
            "the default is itself a legal value"
        );
    }

    /// Requête minimale vers le protocole, pour les tests.
    fn request(method: &str, path: &str, range: Option<&str>) -> tauri::http::Request<Vec<u8>> {
        let mut builder = tauri::http::Request::builder()
            .method(method)
            .uri(format!("http://carpreview.localhost{path}"));
        if let Some(range) = range {
            builder = builder.header(tauri::http::header::RANGE, range);
        }
        builder.body(Vec::new()).unwrap()
    }

    // Règle : **toute** réponse du protocole porte les en-têtes CORS, réussite
    // comme échec.
    //
    // Bug réel, remonté par l'utilisateur au premier essai : les étapes de
    // conversion s'affichaient (elles passent par l'IPC) puis « aperçu 3D
    // indisponible », alors que le `.glb` était bien écrit dans le cache. Un
    // protocole custom vit sur sa propre origine, distincte de celle de la
    // page : sans `Access-Control-Allow-Origin`, le navigateur refuse la
    // lecture avant même d'ouvrir le fichier.
    #[test]
    fn every_response_carries_cors_headers() {
        use tauri::http::header::ACCESS_CONTROL_ALLOW_ORIGIN;

        let base = crate::testutil::temp_dir("preview-cors");
        let dir = base.join("previews");
        std::fs::create_dir_all(&dir).unwrap();
        let key = entry_stem("abcdef0123456789abcdef0123456789");
        std::fs::write(dir.join(format!("{key}.glb")), b"glTFbody").unwrap();

        let ok = serve(&dir, &request("GET", &format!("/{key}.glb"), None));
        assert_eq!(ok.status(), 200, "entrée servie");
        assert_eq!(
            ok.headers()
                .get(ACCESS_CONTROL_ALLOW_ORIGIN)
                .map(|v| v.to_str().unwrap()),
            Some("*"),
            "sans cet en-tête la page ne peut pas lire le modèle"
        );
        assert_eq!(ok.body(), b"glTFbody", "corps complet");

        let missing = serve(&dir, &request("GET", "/00000000000000000000000000000000.glb", None));
        assert_eq!(missing.status(), 404, "entrée absente");
        assert!(
            missing.headers().contains_key(ACCESS_CONTROL_ALLOW_ORIGIN),
            "une 404 sans CORS remonte en erreur réseau opaque, ce qui masque la cause"
        );

        // `Range` n'étant pas un en-tête sûr au sens CORS, une lecture
        // partielle commence par un préchargement `OPTIONS`.
        let preflight = serve(&dir, &request("OPTIONS", &format!("/{key}.glb"), None));
        assert_eq!(preflight.status(), 204, "préchargement accepté");
        assert!(
            preflight.headers().contains_key(ACCESS_CONTROL_ALLOW_ORIGIN),
            "préchargement porte les en-têtes CORS"
        );

        let partial = serve(&dir, &request("GET", &format!("/{key}.glb"), Some("bytes=4-7")));
        assert_eq!(partial.status(), 206, "contenu partiel");
        assert_eq!(partial.body(), b"body", "tranche demandée");
        assert!(
            partial.headers().contains_key(ACCESS_CONTROL_ALLOW_ORIGIN),
            "réponse partielle porte les en-têtes CORS"
        );
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
