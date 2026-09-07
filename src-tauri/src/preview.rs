//! Aperçu 3D des voitures : cache disque et orchestration de la conversion
//! (`docs/SPEC-preview-3d-kn5.md` §5.3 et §7).
//!
//! Le `.glb` produit ne transite **jamais** par l'IPC (§7.2) : il est écrit
//! dans le cache, et l'UI ne reçoit qu'une URL servie par le protocole
//! `carpreview` (voir `lib.rs`). Un modèle de 30 Mo sérialisé en base64
//! deviendrait ~40 Mo de chaîne à parser côté JS — blocage de l'UI et pic
//! mémoire garantis.

use std::collections::BTreeSet;
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
const CONVERTER_VERSION: u32 = 47;

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
    /// Le ménage des **vignettes** d'une version antérieure a-t-il déjà eu
    /// lieu ? Une fois par exécution suffit : rien ne les écrit hors de l'app,
    /// et leur dossier n'a pas de passe d'éviction où s'accrocher.
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

/// Bytes currently held by the cache, entries, blobs and counter files alike.
pub fn cache_usage(app: &tauri::AppHandle) -> Result<u64, String> {
    Ok(dir_size(&cache_dir(app)?))
}

/// Octets occupés par un dossier et son sous-dossier de blobs.
///
/// Un seul niveau de récursion, et c'est voulu : le cache n'a qu'un
/// sous-dossier, et une descente générale irait un jour compter autre chose.
fn dir_size(dir: &Path) -> u64 {
    let mut total = 0u64;
    let Ok(read) = std::fs::read_dir(dir) else { return 0 };
    for entry in read.flatten() {
        match entry.metadata() {
            Ok(meta) if meta.is_file() => total += meta.len(),
            Ok(meta) if meta.is_dir() && entry.file_name() == BLOBS => {
                if let Ok(blobs) = std::fs::read_dir(entry.path()) {
                    total += blobs
                        .flatten()
                        .filter_map(|b| b.metadata().ok())
                        .filter(|m| m.is_file())
                        .map(|m| m.len())
                        .sum::<u64>();
                }
            }
            _ => {}
        }
    }
    total
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
/// repassera à la conversion suivante.
///
/// **Appelé à chaque passe d'éviction, et non une fois par exécution.** Un
/// balayage unique au premier aperçu laisse échapper ce qui est écrit
/// *ensuite* par une autre version : deux instances ouvertes en même temps —
/// l'app installée et la version en développement — et celle de l'ancienne
/// version repose ses entrées juste après le ménage de l'autre. Constaté :
/// quatre entrées `v25` survivantes, écrites dans la même minute, encore là
/// après 312 conversions en `v32` et sept incréments de version. Rattachée à
/// [`evict_to`], qui liste déjà le dossier à chaque conversion, la reprise ne
/// coûte rien de plus et ne dépend plus de l'ordre de démarrage.
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
        // Un dossier n'est pas une entrée périmée, et `remove_file` y échouera
        // à chaque conversion : le journal se remplirait pour rien.
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
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
        sweep_unreferenced_blobs(dir);
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

/// Clé de cache d'un couple (modèle, skin).
///
/// Inclut la date et la taille du `.kn5` : réimporter une version modifiée
/// d'un mod invalide l'entrée sans qu'on ait à s'en occuper. Inclut le skin,
/// puisqu'il surcharge les textures (§4.3). La version du convertisseur, elle,
/// est portée par le nom du fichier (voir [`CONVERTER_VERSION`]).
fn cache_key(
    model: &Path,
    skin: Option<&str>,
    configs: &[PathBuf],
    limits: &kn5_gltf::SteerLimits,
    driver: Option<&kn5_gltf::DriverGraft>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(model.to_string_lossy().to_lowercase().as_bytes());
    stamp(&mut hasher, model);
    hasher.update(skin.unwrap_or("").as_bytes());
    // **L'angle de braquage n'est pas ici, et c'est le but.** Il l'a été, et
    // chaque valeur essayée laissait alors une entrée complète — onze entrées
    // de 42 Mo pour une seule voiture, mesuré sur le poste, parce qu'un curseur
    // se balaye. Seules les deux valeurs que la voiture déclare de sa direction
    // entrent dans la clé : elles sont écrites dans le `.glb`, donc corriger un
    // `car.ini` doit bien invalider l'entrée.
    hasher.update(limits.lock.to_le_bytes());
    hasher.update(limits.ratio.to_le_bytes());
    // Le pilote entre dans la clé, et **rien du tout quand il n'y en a pas** :
    // une voiture montrée sans lui garde la clé qu'elle avait avant que le
    // pilote n'existe, donc son entrée de cache. Deux entrées coexistent pour
    // une voiture qu'on regarde des deux façons — c'est le prix de ne pas
    // convertir un mannequin de quatorze mégaoctets pour qui ne l'affiche
    // jamais.
    if let Some(driver) = driver {
        hasher.update(driver.model.to_string_lossy().to_lowercase().as_bytes());
        stamp(&mut hasher, &driver.model);
        // L'ancrage (`DRIVEREYES`) autant que l'offset : une voiture dont on
        // corrige la position d'assise doit reconvertir, pas resservir un
        // pilote assis à l'ancienne place.
        hasher.update([u8::from(driver.anchor.is_some())]);
        for component in driver.anchor.unwrap_or_default() {
            hasher.update(component.to_le_bytes());
        }
        // La pose fait partie du modèle produit, donc de son identité :
        // l'animation elle-même — corriger un `steer.ksanim` doit invalider
        // l'entrée — et l'angle auquel on l'a échantillonnée.
        for source in [&driver.base_pose, &driver.animation].into_iter().flatten() {
            hasher.update(source.to_string_lossy().to_lowercase().as_bytes());
            stamp(&mut hasher, source);
        }
        // La course de l'animation, oui — elle est écrite dans le `.glb` et sert
        // à y choisir une image. L'**angle**, non : il ne décide plus de rien
        // dans le fichier.
        hasher.update(driver.lock_degrees.to_le_bytes());
        for dir in &driver.texture_dirs {
            hasher.update(dir.to_string_lossy().to_lowercase().as_bytes());
        }
    }
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

/// Ce que l'appelant veut voir, en plus de la voiture elle-même.
///
/// Groupés parce qu'ils vont ensemble et qu'ils partent ensemble dans la clé
/// de cache : chacun d'eux décide du `.glb` produit, aucun ne s'applique après
/// coup.
///
/// **L'angle de braquage n'en fait pas partie**, et c'est le résultat d'un
/// correctif : roues, volant et bras du pilote tournent tous à l'affichage, si
/// bien que l'angle ne décide plus de rien dans le fichier produit.
#[derive(Debug, Clone, Copy, Default)]
pub struct PreviewRequest<'a> {
    /// Livrée dont les textures surchargent celles du modèle (§4.3).
    pub skin_id: Option<&'a str>,
    /// Le pilote à greffer, et la tenue qu'on lui impose. `None` = personne au
    /// volant, et la conversion ne lit alors aucun mannequin.
    pub driver: Option<&'a crate::driver::DriverView>,
}

/// Tout ce qu'une voiture apporte à sa propre conversion, résolu **sans rien
/// lire de lourd** : quelques `stat`, un `ext_config.ini`, un `car.ini`.
///
/// Extrait de [`prepare`] parce que la voie des **vignettes de grille**
/// (`gridthumbs.rs`) en a besoin deux fois sans convertir : pour connaître le
/// nom d'entrée d'une voiture — donc savoir si sa vignette est déjà là — et
/// pour la convertir ailleurs que dans le cache (§5.3 de `SPEC-grille.md`).
struct CarSources {
    resolved: kn5_gltf::ResolvedModel,
    /// Livrée résolue : c'est elle qui désigne le dossier où vivent le
    /// `ext_config.ini` et les KN5 de jante (§4.3).
    skin_dir: Option<PathBuf>,
    csp: kn5_gltf::CspConfig,
    /// Ce que la voiture déclare de sa direction. **Pas l'angle** : il n'est
    /// plus cuit dans le modèle, seulement décrit, et c'est la vue qui le
    /// tourne. Ces deux nombres-là, en revanche, sont écrits dans le `.glb` et
    /// font donc partie de son identité.
    limits: kn5_gltf::SteerLimits,
    ac_install: Option<PathBuf>,
}

fn resolve_car(
    app: &tauri::AppHandle,
    car_dir: &Path,
    car_id: &str,
    skin_id: Option<&str>,
) -> Result<CarSources, String> {
    let resolved = kn5_gltf::resolve_model(car_dir).ok_or(crate::errors::PREVIEW_MODEL_NOT_FOUND)?;
    let skin_dir = kn5_gltf::resolve_skin(car_dir, skin_id);
    let ac_install = crate::config::load(app).ac_install_path;
    let csp = kn5_gltf::CspConfig::locate(car_dir, skin_dir.as_deref(), ac_install.as_deref(), car_id);
    // L'angle demandé est celui des **roues** ; l'animation de braquage, elle,
    // est indexée sur celui du volant. La démultiplication de la voiture fait
    // le pont, et la butée borne les deux.
    let steering = crate::steering::read(car_dir, car_id);
    Ok(CarSources {
        resolved,
        skin_dir,
        csp,
        limits: kn5_gltf::SteerLimits {
            lock: steering.lock,
            ratio: steering.ratio,
        },
        ac_install,
    })
}

/// Le ménage des dossiers **hors éviction**, une seule fois par exécution.
///
/// Vignettes de corps et vignettes de grille portent le même préfixe de
/// version que les entrées d'aperçu : elles se périment donc avec le
/// convertisseur, et rien d'autre ne les ramasserait — leurs dossiers n'ont pas
/// de passe d'éviction où s'accrocher. Le dossier des aperçus, lui, est repris
/// par [`evict_to`] à chaque conversion.
///
/// Au premier aperçu demandé, pas au démarrage : qui n'en ouvre jamais ne paie
/// rien.
fn sweep_side_stores(app: &tauri::AppHandle, state: &PreviewState) {
    if state.swept.swap(true, Ordering::Relaxed) {
        return;
    }
    if let Ok(thumbs) = thumb_dir(app) {
        sweep_foreign_versions(&thumbs);
    }
    if let Ok(grid) = crate::gridthumbs::dir(app) {
        sweep_foreign_versions(&grid);
    }
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
    car_id: &str,
    what: &PreviewRequest<'_>,
    token: u64,
) -> Result<CarPreview, String> {
    let PreviewRequest { skin_id, driver } = *what;
    let sources = resolve_car(app, car_dir, car_id, skin_id)?;
    let CarSources {
        ref resolved,
        ref skin_dir,
        ref csp,
        limits,
        ref ac_install,
    } = sources;
    let dir = cache_dir(app)?;
    sweep_side_stores(app, state);
    // Résolu avant la clé, comme le skin et pour la même raison : c'est lui qui
    // en fait partie, pas la case à cocher. Deux voitures qui portent le même
    // mannequin dans la même tenue n'en partagent pas l'entrée pour autant —
    // le modèle de la voiture est dans la clé aussi.
    //
    // **Le mannequin est greffé volant droit, et son angle n'entre plus nulle
    // part.** Ses bras sont désormais posés à l'affichage, par le squelette et
    // l'animation écrits dans le `.glb` (`kn5_gltf::rig`) : l'angle ne décide
    // plus du fichier produit, donc plus de la clé de cache. C'était le dernier
    // endroit où bouger le curseur coûtait une conversion.
    let driver = match (driver, ac_install.as_deref()) {
        (Some(view), Some(ac)) => crate::driver::resolve(ac, car_dir, car_id, skin_dir.as_deref(), 0.0, &view.outfit),
        _ => None,
    };
    let stem = entry_stem(&cache_key(
        &resolved.path,
        skin_id,
        csp.sources(),
        &limits,
        driver.as_ref(),
    ));
    let file = dir.join(format!("{stem}.gltf"));

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

    let conversion = convert_car(app, &sources, driver.as_ref(), true)?;

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

/// Parse le KN5, y greffe ce qu'il faut et le convertit.
///
/// Le cœur commun de [`prepare`] et de [`prepare_scratch`] : les deux
/// produisent exactement le même modèle, seul l'endroit où il atterrit change.
///
/// `progress` dit s'il faut émettre `preview://progress`. La voie des vignettes
/// de grille ne le fait **pas** : cet événement pilote le squelette de
/// chargement de la fiche détail, et une file de trois cents conversions le
/// ferait clignoter derrière un aperçu qui, lui, ne charge rien.
fn convert_car(
    app: &tauri::AppHandle,
    sources: &CarSources,
    driver: Option<&kn5_gltf::DriverGraft>,
    progress: bool,
) -> Result<kn5_gltf::Conversion, String> {
    let CarSources {
        resolved,
        skin_dir,
        csp,
        limits,
        ..
    } = sources;
    let bytes = std::fs::read(&resolved.path).map_err(|e| format!("{} : {e}", resolved.path.display()))?;
    let mut model = kn5::parse(&bytes).map_err(|e| match e {
        // Un KN5 chiffré (CSP) n'a pas la bonne magie : c'est la seule
        // détection dont on dispose, et on ne tente rien de plus (§4.5).
        kn5::Kn5Error::NotAKn5File => crate::errors::PREVIEW_PROTECTED.to_string(),
        other => format!("{} : {other}", resolved.path.display()),
    })?;

    // Deuxième détection, complémentaire à la magie ci-dessus (§4.5bis) : un
    // modèle peut avoir un en-tête KN5 parfaitement valide et pourtant ne rien
    // avoir d'affichable. **Mesuré depuis** : leurs sommets sont intacts —
    // normales et tangentes unitaires à 100 %, dimensions d'une voiture,
    // identifiants de matériaux valides — et seuls leurs *triangles* relient
    // n'importe quoi. C'est la signature d'un tampon d'index brouillé, donc
    // d'une protection : le fichier reste valide en apparence et n'est
    // exploitable qu'avec la clé. Repli silencieux sur la photo, jamais de
    // rendu sur une géométrie qu'on ne saurait pas reconstituer.
    let (agreeing, total) = kn5_gltf::winding_consistency(&model);
    if !kn5_gltf::is_geometry_sane(agreeing, total) {
        return Err(crate::errors::PREVIEW_PROTECTED.to_string());
    }

    // Beaucoup de mods de préparation livrent un KN5 volontairement incomplet
    // et laissent CSP y greffer, skin par skin, les pièces qui changent :
    // jantes, boucliers, optiques. Sans cette passe, l'aperçu montre une
    // voiture trouée alors que le jeu l'affiche entière. Après le contrôle
    // d'enroulement ci-dessus, qui doit juger le modèle d'origine et lui seul.
    let ext = kn5_gltf::apply_ext_config(&mut model, &resolved.path, skin_dir.as_deref(), csp);
    for failure in &ext.failures {
        log::warn!("preview: remplacement CSP ignoré — {failure}");
    }

    // Le pilote **après** les greffes CSP : celles-ci visent des nœuds de la
    // voiture par motif de nom, et un mannequin déjà en place pourrait s'y
    // faire prendre. Après, il n'est visible que de la conversion.
    if let Some(driver) = driver {
        let stats = kn5_gltf::graft_driver(&mut model, driver);
        for failure in &stats.failures {
            log::warn!("preview: pilote ignoré — {failure}");
        }
        log::debug!(
            "preview: pilote {} greffé — {} triangles, {} texture(s) habillée(s), {}{}",
            driver.model.display(),
            stats.triangles,
            stats.dressed,
            match stats.seated {
                Some(nodes) => format!("{nodes} nœud(s) assis"),
                None => "assis par DRIVEREYES".to_string(),
            },
            match stats.posed {
                Some(nodes) => format!(", {nodes} nœud(s) posé(s)"),
                None => ", pose de repos".to_string(),
            }
        );
    }

    // Le mod déclare lui-même ce que sont ses surfaces — verre, chrome, cuir,
    // carbone. C'est la seule façon de le savoir : le KN5 seul ne le dit pas
    // (SPEC §4.5ter).
    let options = kn5_gltf::ConvertOptions {
        // Rangement éclaté : c'est le cache, et deux skins d'une même voiture y
        // partagent tout sauf leur livrée (voir `write_entry`).
        layout: kn5_gltf::glb::Layout::Split,
        geometry: kn5_gltf::GeometryOptions {
            steering: *limits,
            ..Default::default()
        },
        // L'animation de braquage voyage jusqu'au convertisseur : c'est elle
        // qui devient l'animation glTF du mannequin. Relue plutôt que reprise
        // de la greffe — quelques millisecondes contre un aller-retour de
        // structure à travers trois modules.
        driver_rig: driver.and_then(|graft| {
            let path = graft.animation.as_ref()?;
            let bytes = std::fs::read(path).ok()?;
            let animation = kn5::parse_animation(&bytes).ok()?;
            Some(kn5_gltf::DriverRigSource {
                animation,
                lock_degrees: graft.lock_degrees,
            })
        }),
        surfaces: kn5_gltf::material_overrides(
            csp,
            skin_dir
                .as_deref()
                .and_then(|d| d.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
                .as_str(),
        ),
        ..Default::default()
    };
    let conversion = kn5_gltf::convert(&model, skin_dir.as_deref(), &options, &|stage| {
        if progress {
            use tauri::Emitter;
            let _ = app.emit("preview://progress", stage.as_str());
        }
    })?;

    for warning in &conversion.texture_warnings {
        log::warn!("preview: texture `{}` ignorée — {}", warning.name, warning.reason);
    }
    Ok(conversion)
}

// --- Vignettes de la grille : la voie parallèle (`SPEC-grille.md` §5.3) -----

/// Sous-dossier du brouillon, à côté des entrées de cache.
///
/// **Il n'est pas une entrée du cache et n'en suit pas les règles** : ni
/// [`evict_to`] ni [`dir_size`] ne le regardent, tous deux ne parcourant qu'un
/// niveau et ne connaissant que [`BLOBS`]. C'est exactement ce qu'on veut —
/// convertir trois cents voitures pour leurs vignettes ne doit rien peser dans
/// un plafond qui protège les voitures que l'utilisateur consulte vraiment.
const SCRATCH: &str = "scratch";

/// Le dossier de brouillon, **vidé d'abord**.
///
/// Un modèle à la fois sur le disque : la conversion précédente est effacée
/// avant que la suivante n'écrive. Le pic disque d'une génération complète est
/// donc celui d'une seule voiture, pas de trois cents.
fn reset_scratch(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = cache_dir(app)?.join(SCRATCH);
    // Best-effort : un fichier encore ouvert par la webview qui vient de lire
    // le modèle précédent ne doit pas empêcher la conversion suivante.
    if let Err(e) = std::fs::remove_dir_all(&dir) {
        if e.kind() != std::io::ErrorKind::NotFound {
            log::warn!("preview: brouillon de vignette non vidé — {e}");
        }
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("création du brouillon : {e}"))?;
    Ok(dir)
}

/// Efface le brouillon. Appelé quand le frontend a fini d'en tirer son image,
/// et au démarrage : une fermeture brutale en laisserait un derrière elle.
pub fn release_scratch(app: &tauri::AppHandle) {
    let Ok(dir) = cache_dir(app) else { return };
    if let Err(e) = std::fs::remove_dir_all(dir.join(SCRATCH)) {
        if e.kind() != std::io::ErrorKind::NotFound {
            log::warn!("preview: brouillon de vignette non effacé — {e}");
        }
    }
}

/// Le nom d'entrée d'une voiture **sans pilote**, sans rien convertir.
///
/// C'est la moitié « voiture » de l'identité d'une vignette de grille : elle
/// porte déjà le `.kn5` et sa date, la livrée, les `ext_config.ini` et la
/// version du convertisseur, donc un mod mis à jour se régénère tout seul.
/// Quelques `stat`, aucun octet de géométrie lu.
pub fn car_entry_stem(
    app: &tauri::AppHandle,
    car_dir: &Path,
    car_id: &str,
    skin_id: Option<&str>,
) -> Result<String, String> {
    let sources = resolve_car(app, car_dir, car_id, skin_id)?;
    Ok(entry_stem(&cache_key(
        &sources.resolved.path,
        skin_id,
        sources.csp.sources(),
        &sources.limits,
        None,
    )))
}

/// Convertit une voiture **hors du cache**, pour en tirer une vignette.
///
/// Renvoie l'URL du modèle dans le brouillon. L'appelant rend l'image puis
/// appelle [`release_scratch`] : convertir → rendre → écrire le PNG → jeter, la
/// séquence du §5.3. Rien n'entre dans le cache LRU, donc rien n'en sort.
///
/// **Sans jeton de génération**, comme les vignettes de corps : une vignette ne
/// périme pas l'aperçu de la fiche et ne se périme pas elle-même. Le verrou de
/// conversion, lui, s'applique — une conversion à la fois, sinon les deux se
/// disputent tous les cœurs du transcodage de textures.
pub fn prepare_scratch(
    app: &tauri::AppHandle,
    state: &PreviewState,
    car_dir: &Path,
    car_id: &str,
    skin_id: Option<&str>,
) -> Result<String, String> {
    let sources = resolve_car(app, car_dir, car_id, skin_id)?;
    sweep_side_stores(app, state);
    let stem = entry_stem(&cache_key(
        &sources.resolved.path,
        skin_id,
        sources.csp.sources(),
        &sources.limits,
        None,
    ));

    let _slot = state
        .slot
        .lock()
        .map_err(|_| "verrou d'aperçu empoisonné".to_string())?;
    let dir = reset_scratch(app)?;
    // **Sur un pool restreint** : trois cents vignettes qui prennent la machine
    // en entier rendent l'app inutilisable pendant qu'elles se produisent, et
    // la génération est explicitement un travail de fond (§5.4). L'aperçu de la
    // fiche, lui, garde tous les cœurs — quelqu'un l'attend.
    let conversion = kn5_gltf::with_background_pool(|| convert_car(app, &sources, None, false))?;
    write_entry(&dir, &stem, &conversion)?;
    Ok(format!("http://carpreview.localhost/{SCRATCH}/{stem}.gltf"))
}

/// Ce que le plateau d'essayage de l'écran Pilote reçoit
/// (`docs/SPEC-ecran-pilote.md` §5.1).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriverPreview {
    /// URL à donner à `GLTFLoader`, servie par le même protocole que les
    /// voitures.
    pub url: String,
    pub triangle_count: u32,
    pub from_cache: bool,
    pub rig: DriverRig,
}

/// Les repères du rig, en mètres, dans l'espace du `.glb`.
///
/// **Renvoyés au frontend plutôt que cuits dans le modèle** : le volant est un
/// objet de présentation que l'application dessine (§D5), pas une pièce du
/// mannequin. Trois lignes de `TorusGeometry` côté three.js valent mieux qu'un
/// maillage généré en Rust et transporté dans chaque entrée de cache.
// `default` sur la désérialisation : le fichier `.rig` posé à côté du `.glb`
// est un cache, et un champ ajouté ici ne doit pas rendre illisibles ceux
// écrits par la version précédente — c'est une reconversion évitable.
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DriverRig {
    /// Poignet gauche puis droit. `None` quand le mannequin n'a pas d'os de
    /// main sous un nom connu — le plateau se passe alors de volant plutôt que
    /// d'en poser un au hasard.
    pub hands: Option<[[f32; 3]; 2]>,
    /// Là où les doigts se referment, 13 cm devant les poignets : c'est par là
    /// que passe le volant, pas par les poignets.
    pub grip: Option<[[f32; 3]; 2]>,
    pub head: Option<[f32; 3]>,
    pub hips: Option<[f32; 3]>,
}

impl From<kn5_gltf::DriverRig> for DriverRig {
    fn from(rig: kn5_gltf::DriverRig) -> Self {
        Self {
            hands: rig.hands,
            grip: rig.grip,
            head: rig.head,
            hips: rig.hips,
        }
    }
}

/// Prépare le mannequin seul, habillé, pour le plateau d'essayage.
///
/// Même cache et même éviction que les voitures — c'est le même protocole qui
/// sert les deux, et un pilote pèse moins qu'une voiture. La clé n'a en
/// revanche rien à voir : elle ne tient qu'au mannequin, à sa pose et à sa
/// garde-robe, donc **deux voitures qui habillent et assoient le même corps
/// pareil partagent l'entrée**, ce qui est exactement ce qu'on veut d'un choix
/// global.
///
/// `token` à `None` = « ne me périme pas, et ne périme personne ». C'est le
/// mode des **vignettes de corps** (§9.1), qui en demandent quarante-cinq à la
/// file : avec un jeton, chacune rendrait obsolète la conversion du plateau
/// lancée juste avant, et le plateau ne se chargerait jamais. Le verrou de
/// conversion, lui, s'applique quand même — une conversion à la fois.
pub fn prepare_driver(
    app: &tauri::AppHandle,
    state: &PreviewState,
    graft: &kn5_gltf::DriverGraft,
    token: Option<u64>,
) -> Result<DriverPreview, String> {
    let dir = cache_dir(app)?;
    sweep_side_stores(app, state);
    let stem = driver_entry_stem(graft);
    let file = dir.join(format!("{stem}.gltf"));

    if let Ok(meta) = std::fs::metadata(&file) {
        // Le rig est relu avec les compteurs : sans lui on saurait afficher le
        // pilote mais plus où poser son volant, et reparser quinze mégaoctets
        // de mannequin pour trois vecteurs à chaque changement de casque
        // annulerait tout l'intérêt du cache.
        if let (true, Some(rig)) = (meta.len() > 0, read_rig(&dir, &stem)) {
            touch(&file);
            return Ok(DriverPreview {
                url: url_for(&stem),
                triangle_count: read_counts(&dir, &stem).unwrap_or_default().0,
                from_cache: true,
                rig,
            });
        }
    }

    let _slot = state
        .slot
        .lock()
        .map_err(|_| "verrou d'aperçu empoisonné".to_string())?;
    if token.is_some_and(|token| !state.is_current(token)) {
        return Err(crate::errors::PREVIEW_SUPERSEDED.to_string());
    }

    let (model, stats, rig) = kn5_gltf::standalone_driver(graft)?;
    for failure in &stats.failures {
        log::warn!("preview: pilote — {failure}");
    }
    log::debug!(
        "preview: mannequin {} seul — {} triangles, {} texture(s) habillée(s)",
        graft.model.display(),
        stats.triangles,
        stats.dressed
    );

    // `mannequin` : ce qu'on convertit ici est une personne, seule, et rien sur
    // une personne ne renvoie d'image nette. C'est le seul endroit qui le
    // sache — la famille de shader ne suffit pas (`woman_driver` habille son
    // visage d'un shader de carrosserie).
    let options = kn5_gltf::ConvertOptions {
        mannequin: true,
        layout: kn5_gltf::glb::Layout::Split,
        ..Default::default()
    };
    let conversion = kn5_gltf::convert(&model, None, &options, &|stage| {
        use tauri::Emitter;
        let _ = app.emit("preview://progress", stage.as_str());
    })?;
    for warning in &conversion.texture_warnings {
        log::warn!("preview: texture `{}` ignorée — {}", warning.name, warning.reason);
    }

    let rig = DriverRig::from(rig);
    write_entry(&dir, &stem, &conversion)?;
    write_rig(&dir, &stem, &rig);
    evict_to(&dir, state.cache_cap());

    Ok(DriverPreview {
        url: url_for(&stem),
        triangle_count: conversion.triangle_count,
        from_cache: false,
        rig,
    })
}

/// Le nom d'entrée d'un mannequin habillé et posé, **sans rien convertir**.
///
/// Séparé de [`prepare_driver`] parce que les vignettes de corps s'en servent
/// comme identité : savoir si la vignette d'un corps est déjà rendue ne doit
/// pas coûter une conversion, et elle doit se périmer exactement quand
/// l'entrée de cache correspondante se périmerait.
pub fn driver_entry_stem(graft: &kn5_gltf::DriverGraft) -> String {
    format!("{}d{}", version_prefix(), driver_cache_key(graft))
}

/// Clé de cache d'un mannequin habillé et posé.
///
/// Le mannequin, sa garde-robe, **et la pose** : celle-ci vient de la voiture
/// (`driver_base_pos.knh` et `steer.ksanim`), donc deux voitures qui habillent
/// le même corps pareil ne partagent l'entrée que si elles l'assoient pareil —
/// ce qui est bien ce qu'on veut, puisque c'est la pose qui décide de l'écart
/// des mains, donc du volant qu'on leur dessine.
fn driver_cache_key(graft: &kn5_gltf::DriverGraft) -> String {
    let mut hasher = Sha256::new();
    hasher.update(graft.model.to_string_lossy().to_lowercase().as_bytes());
    stamp(&mut hasher, &graft.model);
    for dir in &graft.texture_dirs {
        hasher.update(dir.to_string_lossy().to_lowercase().as_bytes());
    }
    for source in [&graft.base_pose, &graft.animation].into_iter().flatten() {
        hasher.update(source.to_string_lossy().to_lowercase().as_bytes());
        stamp(&mut hasher, source);
    }
    hasher.update(graft.lock_degrees.to_le_bytes());
    hasher.update(graft.steer_degrees.to_le_bytes());
    format!("{:x}", hasher.finalize())[..32].to_string()
}

// --- Vignettes de corps (§9.1) ----------------------------------------------

/// Où vivent les vignettes de corps : à côté du cache d'aperçus, **et hors de
/// son plafond**.
///
/// Le pourquoi est mesuré. Rendre les 45 vignettes de l'installation de
/// référence convertit 45 mannequins, soit ~180 Mo de `.glb` dans un pool qui
/// est déjà à son plafond de 2 Gio : chacune pousse dehors une entrée plus
/// ancienne, y compris les mannequins des vignettes précédentes, qu'il faut
/// alors reconvertir à la visite suivante. Une vignette pèse quelques dizaines
/// de kilo-octets et n'a aucune raison d'entrer dans cette compétition — donc
/// elle n'y entre pas, et le `.glb` qui l'a produite peut être évincé sans
/// qu'on ait à le reconvertir jamais.
fn thumb_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("dossier de cache indisponible : {e}"))?
        .join("bodies");
    std::fs::create_dir_all(&dir).map_err(|e| format!("création du cache de vignettes : {e}"))?;
    Ok(dir)
}

/// La vignette déjà rendue pour cette entrée, s'il y en a une.
pub fn body_thumb(app: &tauri::AppHandle, stem: &str) -> Option<PathBuf> {
    let file = thumb_dir(app).ok()?.join(format!("{stem}.png"));
    file.is_file().then_some(file)
}

/// Range le PNG rendu par le frontend et renvoie son chemin.
pub fn write_body_thumb(app: &tauri::AppHandle, stem: &str, png: &[u8]) -> Result<PathBuf, String> {
    let file = thumb_dir(app)?.join(format!("{stem}.png"));
    std::fs::write(&file, png).map_err(|e| format!("{} : {e}", file.display()))?;
    Ok(file)
}

/// Le rig, à côté du `.glb`. Best-effort : une écriture manquée coûte une
/// reconversion, pas un bug.
fn write_rig(dir: &Path, key: &str, rig: &DriverRig) {
    match serde_json::to_vec(rig) {
        Ok(bytes) => {
            if let Err(e) = std::fs::write(dir.join(format!("{key}.rig")), bytes) {
                log::warn!("preview: rig de pilote non écrit — {e}");
            }
        }
        Err(e) => log::warn!("preview: rig de pilote non sérialisé — {e}"),
    }
}

fn read_rig(dir: &Path, key: &str) -> Option<DriverRig> {
    let bytes = std::fs::read(dir.join(format!("{key}.rig"))).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// URL servie par le protocole custom (§7.2).
///
/// Forme Windows d'un scheme custom sous Tauri v2 — l'app ne cible que
/// Windows (§Stack).
fn url_for(key: &str) -> String {
    format!("http://carpreview.localhost/{key}.gltf")
}

/// Sous-dossier des blobs adressés par leur contenu, et le préfixe que les URI
/// du document portent — les deux doivent rester identiques, puisque
/// `GLTFLoader` résout une URI relative depuis l'adresse du `.gltf`.
const BLOBS: &str = "blobs";

/// Range un blob sous le nom de son empreinte, et rend ce nom.
///
/// **Adressé par son contenu**, donc écrit une seule fois quel que soit le
/// nombre d'entrées qui le réclament : c'est tout le mécanisme de la
/// déduplication entre skins. Deux skins d'une même voiture partagent leur
/// géométrie au bit près (mesuré : une seule variante pour trois skins) et les
/// deux tiers de leurs textures — seule la livrée diffère.
///
/// Écrit sous un nom temporaire puis renommé. Ce n'est pas une précaution de
/// principe : un blob tronqué par une fermeture brutale serait ensuite
/// **réputé bon** par toutes les entrées qui le citent, puisqu'on ne vérifie
/// que son existence. Le renommage est atomique, donc un blob présent est un
/// blob complet.
fn write_blob(dir: &Path, bytes: &[u8], extension: &str) -> Result<String, String> {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let name = format!("{:x}.{extension}", hasher.finalize());
    let blobs = dir.join(BLOBS);
    let path = blobs.join(&name);
    if path.is_file() {
        return Ok(name);
    }
    std::fs::create_dir_all(&blobs).map_err(|e| format!("{} : {e}", blobs.display()))?;
    let staging = blobs.join(format!("{name}.part"));
    std::fs::write(&staging, bytes).map_err(|e| format!("{} : {e}", staging.display()))?;
    // Une course entre deux conversions ne peut pas produire un contenu
    // différent — le nom EST le contenu — donc un renommage qui écrase est
    // sans danger, et un échec veut dire que l'autre a gagné.
    if let Err(e) = std::fs::rename(&staging, &path) {
        let _ = std::fs::remove_file(&staging);
        if !path.is_file() {
            return Err(format!("{} : {e}", path.display()));
        }
    }
    Ok(name)
}

/// Écrit le document et ses blobs, plus les compteurs à renvoyer sur un futur
/// succès de cache. Fichier séparé pour les compteurs plutôt que relecture du
/// document : reparser des mégaoctets de glTF pour trois entiers serait
/// absurde.
///
/// **Un `.gltf` et des blobs, pas un `.glb`.** Le fichier autonome reste ce
/// que produit `kn5-tool`, parce qu'il doit s'ouvrir tel quel ailleurs ; dans
/// le cache, il faisait écrire à chaque skin d'une même voiture une copie
/// complète de la géométrie et des textures. Mesuré sur trois skins de deux
/// voitures : **−53 % et −61 %** (§15.0quater).
fn write_entry(dir: &Path, key: &str, conversion: &kn5_gltf::Conversion) -> Result<(), String> {
    let mut json = conversion.document.json.clone();

    let geometry = write_blob(dir, &conversion.document.buffer, "bin")?;
    json["buffers"][0]["uri"] = serde_json::json!(format!("{BLOBS}/{geometry}"));
    for (index, bytes) in conversion.document.images.iter().enumerate() {
        // L'extension suit le type MIME que la passe de textures a décidé :
        // elle ne sert qu'à ce que le protocole rende le bon `Content-Type`,
        // mais sans lui la webview refuse l'image sans un mot.
        let extension = match json["images"][index]["mimeType"].as_str() {
            Some("image/jpeg") => "jpg",
            _ => "png",
        };
        let name = write_blob(dir, bytes, extension)?;
        json["images"][index]["uri"] = serde_json::json!(format!("{BLOBS}/{name}"));
    }

    let file = dir.join(format!("{key}.gltf"));
    let text = serde_json::to_vec(&json).map_err(|e| format!("document illisible : {e}"))?;
    // Le document APRÈS ses blobs, jamais l'inverse : une entrée qui cite un
    // blob absent est un modèle cassé, alors qu'un blob que rien ne cite
    // encore sera repris par le balayage.
    std::fs::write(&file, &text).map_err(|e| format!("{} : {e}", file.display()))?;
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
    // Avant de mesurer : ce qui appartient à une autre version ne se compte
    // pas, il se reprend. Sinon une entrée morte pousse une entrée vivante
    // dehors, et le plafond se remplit de fichiers que rien ne sert jamais.
    sweep_foreign_versions(dir);

    // Les entrées, de la plus récemment utilisée à la plus ancienne. C'est
    // dans cet ordre qu'on décide **qui reste**, et non qui part : un blob
    // n'appartient à personne, sa place ne se libère qu'une fois la dernière
    // entrée qui le cite disparue. Compter en gardant, c'est compter juste du
    // premier coup ; compter en supprimant demanderait de refaire la somme
    // après chaque suppression.
    let mut entries: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
    let Ok(read) = std::fs::read_dir(dir) else { return };
    for entry in read.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "gltf") {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let used = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        entries.push((used, meta.len(), path));
    }
    entries.sort_by_key(|(used, _, _)| std::cmp::Reverse(*used));

    let sizes = blob_sizes(dir);
    let mut kept: BTreeSet<String> = BTreeSet::new();
    let mut total = 0u64;
    let mut doomed: Vec<PathBuf> = Vec::new();

    for (_, size, path) in entries {
        // Dès qu'une entrée ne rentre plus, tout ce qui est plus ancien part
        // aussi — sans quoi une petite entrée oubliée depuis un mois survivrait
        // à une grosse consultée hier, et le « moins récemment utilisé » ne
        // voudrait plus rien dire.
        if !doomed.is_empty() {
            doomed.push(path);
            continue;
        }
        let referenced = blob_references(&path);
        let added: u64 = referenced
            .iter()
            .filter(|name| !kept.contains(*name))
            .filter_map(|name| sizes.get(name))
            .sum();
        // La plus récente est gardée quoi qu'il arrive : un plafond plus petit
        // qu'une seule entrée l'évincerait à l'instant où elle est écrite, donc
        // la ferait reconvertir à chaque visite — un cache désactivé sans le
        // dire. `CACHE_CAP_MIN_BYTES` rend le cas improbable, il ne le rend pas
        // impossible (une voiture de 500 Mo).
        if total > 0 && total + size + added > cap {
            doomed.push(path);
            continue;
        }
        total += size + added;
        kept.extend(referenced);
    }

    if doomed.is_empty() {
        return;
    }
    for path in doomed {
        if std::fs::remove_file(&path).is_ok() {
            let _ = std::fs::remove_file(path.with_extension("txt"));
            let _ = std::fs::remove_file(path.with_extension("rig"));
            log::warn!("preview: entrée de cache évincée ({})", path.display());
        }
    }

    // Puis les blobs que plus personne ne cite. Relire les entrées survivantes
    // plutôt que de faire confiance à `kept` : une suppression a pu échouer, et
    // effacer le blob d'une entrée toujours là donnerait un modèle sans
    // texture — un dégât silencieux, bien pire qu'un octet de trop.
    sweep_unreferenced_blobs(dir);
}

/// Nom et taille de chaque blob présent.
fn blob_sizes(dir: &Path) -> std::collections::BTreeMap<String, u64> {
    let mut sizes = std::collections::BTreeMap::new();
    let Ok(read) = std::fs::read_dir(dir.join(BLOBS)) else {
        return sizes;
    };
    for entry in read.flatten() {
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                sizes.insert(entry.file_name().to_string_lossy().to_string(), meta.len());
            }
        }
    }
    sizes
}

/// Les blobs qu'une entrée cite, lus dans son document.
///
/// Par balayage d'octets et non par analyse JSON : on cherche des noms d'une
/// forme fixe (`blobs/<hexadécimal>.<ext>`) dans un fichier qu'on a écrit
/// soi-même, et parser plusieurs centaines de kilo-octets de glTF par entrée à
/// chaque éviction coûterait mille fois plus cher pour la même réponse.
fn blob_references(entry: &Path) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let Ok(text) = std::fs::read_to_string(entry) else {
        return found;
    };
    let needle = format!("{BLOBS}/");
    for (index, _) in text.match_indices(&needle) {
        let rest = &text[index + needle.len()..];
        // L'empreinte d'abord, jusqu'au point. Pas « tout ce qui est
        // hexadécimal » : le `b` de `.bin` en est un, et le nom se serait
        // arrêté à `.b`.
        let Some(dot) = rest.find('.') else { continue };
        let (stem, tail) = rest.split_at(dot);
        if stem.is_empty() || !stem.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let tail = &tail[1..];
        let end = tail.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(tail.len());
        if matches!(&tail[..end], "bin" | "png" | "jpg") {
            found.insert(format!("{stem}.{}", &tail[..end]));
        }
    }
    found
}

/// Efface les blobs qu'aucune entrée ne cite plus.
///
/// Appelé après une éviction, et **aussi après un balayage de version** : un
/// incrément du convertisseur change les octets produits, donc les empreintes,
/// donc laisse derrière lui l'intégralité des anciens blobs — de très loin le
/// plus gros tas de fichiers morts que ce cache puisse accumuler.
fn sweep_unreferenced_blobs(dir: &Path) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    let mut referenced: BTreeSet<String> = BTreeSet::new();
    for entry in read.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "gltf") {
            referenced.extend(blob_references(&path));
        }
    }

    let Ok(blobs) = std::fs::read_dir(dir.join(BLOBS)) else {
        return;
    };
    let (mut removed, mut freed) = (0u32, 0u64);
    for blob in blobs.flatten() {
        let name = blob.file_name().to_string_lossy().to_string();
        if referenced.contains(&name) {
            continue;
        }
        let size = blob.metadata().map(|m| m.len()).unwrap_or(0);
        match std::fs::remove_file(blob.path()) {
            Ok(()) => {
                removed += 1;
                freed += size;
            }
            Err(e) => log::warn!("preview: blob {name} non supprimé — {e}"),
        }
    }
    if removed > 0 {
        log::info!(
            "preview: {removed} blob(s) sans référence effacé(s), {} Mio libérés",
            freed / (1024 * 1024)
        );
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
        // Les blobs sont dans un sous-dossier : sans cette branche, « vider le
        // cache » laissait derrière lui la quasi-totalité des octets.
        if meta.is_dir() && entry.file_name() == BLOBS {
            freed += dir_size(&dir.join(BLOBS));
            let _ = std::fs::remove_dir_all(entry.path());
        }
        // Le brouillon d'une vignette en cours : il ne compte dans aucun
        // plafond, mais « vider le cache » doit le vider aussi — sinon vingt
        // mégaoctets survivent à un bouton qui promet le contraire.
        if meta.is_dir() && entry.file_name() == SCRATCH {
            freed += dir_size(&dir.join(SCRATCH));
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
    Ok(freed)
}

/// Sert une entrée du cache au protocole custom. Renvoie `None` pour toute
/// clé qui n'est pas un nom d'entrée : le chemin vient de la webview, donc
/// d'une source qu'on ne contrôle pas entièrement.
pub fn cached_file(dir: &Path, requested: &str) -> Option<PathBuf> {
    let name = requested.trim_start_matches('/');
    // Le brouillon des vignettes de grille (§5.3) : mêmes noms, même
    // validation, un dossier plus bas. Le préfixe est retiré **une seule
    // fois** — la suite passe par le contrôle habituel, qui refuse tout ce qui
    // n'est pas un nom d'entrée. Les URI du document restent relatives
    // (`blobs/…`), donc la webview les résout d'elle-même dans le brouillon.
    match name.strip_prefix(SCRATCH).and_then(|rest| rest.strip_prefix('/')) {
        Some(rest) => cached_entry(&dir.join(SCRATCH), rest),
        None => cached_entry(dir, name),
    }
}

/// Le contrôle de nom lui-même, sans le détour par le brouillon.
fn cached_entry(dir: &Path, name: &str) -> Option<PathBuf> {
    // Un blob : `blobs/<empreinte hexadécimale>.<extension>`. Le nom est un
    // hachage, donc entièrement hexadécimal — rien d'autre n'est accepté, et
    // surtout aucun séparateur de plus.
    if let Some(blob) = name.strip_prefix(&format!("{BLOBS}/")) {
        let (stem, extension) = blob.rsplit_once('.')?;
        if stem.is_empty()
            || !stem.chars().all(|c| c.is_ascii_hexdigit())
            || !matches!(extension, "bin" | "png" | "jpg")
        {
            return None;
        }
        let file = dir.join(BLOBS).join(blob);
        return file.is_file().then_some(file);
    }

    let stem = name.strip_suffix(".gltf")?;
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

    // Le type suit l'extension : la webview refuse une image servie en
    // `model/gltf-binary`, et sans type correct sur le `.bin` certains chemins
    // de `GLTFLoader` refont une requête pour rien.
    let content_type = match file.extension().and_then(|e| e.to_str()) {
        Some("gltf") => "model/gltf+json",
        Some("png") => "image/png",
        Some("jpg") => "image/jpeg",
        _ => "application/octet-stream",
    };
    let builder = with_cors(
        Response::builder()
            .header(header::CONTENT_TYPE, content_type)
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

    /// Ce que la moitié de la bibliothèque déclare de sa direction.
    const STRAIGHT: kn5_gltf::SteerLimits = kn5_gltf::SteerLimits {
        lock: 360.0,
        ratio: 14.0,
    };

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

        let a = cache_key(&model, None, &[], &STRAIGHT, None);
        assert_eq!(
            a,
            cache_key(&model, None, &[], &STRAIGHT, None),
            "clé stable à contenu identique"
        );
        assert_ne!(
            a,
            cache_key(&model, Some("red"), &[], &STRAIGHT, None),
            "le skin fait partie de la clé"
        );

        // Réécriture avec une taille différente : la clé doit bouger même si
        // l'horloge du système de fichiers a une granularité grossière.
        std::fs::write(&model, b"second and longer").unwrap();
        assert_ne!(
            a,
            cache_key(&model, None, &[], &STRAIGHT, None),
            "un modèle modifié invalide son entrée"
        );
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
        let without = cache_key(&model, None, std::slice::from_ref(&config), &STRAIGHT, None);
        assert_eq!(
            without,
            cache_key(&model, None, std::slice::from_ref(&config), &STRAIGHT, None),
            "clé stable quand la config n'existe pas"
        );

        write_model(config.parent().unwrap(), "ext_config.ini", b"[MODEL_REPLACEMENT_...]");
        let with = cache_key(&model, None, std::slice::from_ref(&config), &STRAIGHT, None);
        assert_ne!(without, with, "l'apparition d'une config invalide l'entrée");

        std::fs::write(
            &config,
            b"[MODEL_REPLACEMENT_...]
INSERT = part.kn5",
        )
        .unwrap();
        assert_ne!(
            with,
            cache_key(&model, None, &[config], &STRAIGHT, None),
            "une config modifiée aussi"
        );
    }

    // Règle : le pilote fait partie de la clé — sinon cocher la case
    // laisserait servir l'aperçu sans pilote déjà en cache (§4.6). Et son
    // absence n'y ajoute **rien** : les entrées écrites avant qu'il n'existe
    // restent valides.
    #[test]
    fn cache_key_follows_the_driver() {
        let base = crate::testutil::temp_dir("preview-key-driver");
        let model = write_model(&base, "car.kn5", b"first");
        let mannequin = write_model(&base, "driver_80.kn5", b"mannequin");

        let animation = write_model(&base, "steer.ksanim", b"pose");
        let base_pose = write_model(&base, "driver_base_pos.knh", b"seat");

        let graft = kn5_gltf::DriverGraft {
            model: mannequin.clone(),
            anchor: Some([0.33, 1.19, -0.49]),
            texture_dirs: vec![base.join("suit")],
            base_pose: Some(base_pose.clone()),
            animation: Some(animation.clone()),
            lock_degrees: 360.0,
            steer_degrees: 0.0,
        };

        let without = cache_key(&model, None, &[], &STRAIGHT, None);
        let with = cache_key(&model, None, &[], &STRAIGHT, Some(&graft));
        assert_ne!(without, with, "afficher le pilote change l'entrée");
        assert_eq!(
            with,
            cache_key(&model, None, &[], &STRAIGHT, Some(&graft)),
            "clé stable à pilote identique"
        );

        let dressed = kn5_gltf::DriverGraft {
            texture_dirs: vec![base.join("other-suit")],
            ..graft.clone()
        };
        assert_ne!(
            with,
            cache_key(&model, None, &[], &STRAIGHT, Some(&dressed)),
            "la tenue aussi"
        );

        let seated = kn5_gltf::DriverGraft {
            anchor: Some([0.33, 1.10, -0.49]),
            ..graft.clone()
        };
        assert_ne!(
            with,
            cache_key(&model, None, &[], &STRAIGHT, Some(&seated)),
            "et son assise"
        );

        // **L'angle de braquage ne fait plus partie de la clé du tout**, pilote
        // ou pas : roues, volant et bras tournent à l'affichage. C'est la règle
        // qui a fait tomber les onze entrées de 42 Mo d'une même voiture.
        let turned = kn5_gltf::DriverGraft {
            steer_degrees: 45.0,
            ..graft.clone()
        };
        assert_eq!(
            with,
            cache_key(&model, None, &[], &STRAIGHT, Some(&turned)),
            "l'angle ne décide plus de rien dans le fichier produit"
        );
        // Ce que la voiture déclare de sa direction est écrit dans le `.glb`,
        // donc corriger un `car.ini` doit invalider l'entrée.
        let quicker = kn5_gltf::SteerLimits {
            lock: 400.0,
            ratio: 12.0,
        };
        assert_ne!(
            cache_key(&model, None, &[], &STRAIGHT, None),
            cache_key(&model, None, &[], &quicker, None),
            "la démultiplication déclarée fait partie de l'entrée"
        );

        // Mannequin réécrit : même chemin, contenu différent.
        std::fs::write(&mannequin, b"mannequin, but longer").unwrap();
        assert_ne!(
            with,
            cache_key(&model, None, &[], &STRAIGHT, Some(&graft)),
            "un mannequin modifié invalide son entrée"
        );

        // Animation réécrite : c'est elle qui pose les mains, la corriger doit
        // se voir.
        let with_fresh_mannequin = cache_key(&model, None, &[], &STRAIGHT, Some(&graft));
        std::fs::write(&animation, b"a different pose entirely").unwrap();
        let with_fresh_animation = cache_key(&model, None, &[], &STRAIGHT, Some(&graft));
        assert_ne!(
            with_fresh_mannequin, with_fresh_animation,
            "une animation modifiée aussi"
        );

        // Hiérarchie réécrite : c'est elle qui assoit le pilote.
        std::fs::write(&base_pose, b"a different seat entirely").unwrap();
        assert_ne!(
            with_fresh_animation,
            cache_key(&model, None, &[], &STRAIGHT, Some(&graft)),
            "et la hiérarchie qui l'assoit"
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
        std::fs::write(dir.join(format!("{stem}.gltf")), b"glb").unwrap();
        std::fs::write(base.join("secret.txt"), b"nope").unwrap();

        assert!(
            cached_file(&dir, &format!("/{stem}.gltf")).is_some(),
            "nom valide servi"
        );
        assert!(
            cached_file(&dir, "/../secret.txt").is_none(),
            "remontée de dossier refusée"
        );
        assert!(
            cached_file(&dir, "/v7-nothex.gltf").is_none(),
            "clé non hexadécimale refusée"
        );
        assert!(
            cached_file(&dir, "/abcdef0123456789abcdef0123456789.gltf").is_none(),
            "préfixe de version obligatoire"
        );
        assert!(cached_file(&dir, "/v7-.gltf").is_none(), "clé vide refusée");
        assert!(
            cached_file(&dir, &format!("/{stem}")).is_none(),
            "extension obligatoire"
        );
    }

    /// Le brouillon des vignettes de grille (`SPEC-grille.md` §5.3) est servi
    /// par le même protocole, un dossier plus bas, et **avec la même
    /// validation** : le préfixe n'ouvre pas une porte dérobée.
    #[test]
    fn cached_file_serves_the_scratch_folder_under_the_same_rules() {
        let base = crate::testutil::temp_dir("preview-scratch-serve");
        let dir = base.join("previews");
        let scratch = dir.join(SCRATCH);
        std::fs::create_dir_all(scratch.join(BLOBS)).unwrap();
        let stem = entry_stem("abcdef0123456789abcdef0123456789");
        std::fs::write(scratch.join(format!("{stem}.gltf")), b"glb").unwrap();
        std::fs::write(scratch.join(BLOBS).join("beef.bin"), b"bin").unwrap();
        std::fs::write(dir.join("secret.txt"), b"nope").unwrap();

        assert!(
            cached_file(&dir, &format!("/{SCRATCH}/{stem}.gltf")).is_some(),
            "le modèle du brouillon est servi"
        );
        assert!(
            cached_file(&dir, &format!("/{SCRATCH}/{BLOBS}/beef.bin")).is_some(),
            "ses blobs le sont aussi : les URI du document sont relatives, la webview les résout ici"
        );
        assert!(
            cached_file(&dir, &format!("/{SCRATCH}/../secret.txt")).is_none(),
            "remontée de dossier refusée sous le brouillon aussi"
        );
        assert!(
            cached_file(&dir, &format!("/{stem}.gltf")).is_none(),
            "une entrée du brouillon n'est pas servie comme une entrée du cache"
        );
    }

    /// §5.3 — le brouillon ne pèse dans aucun plafond : ni l'éviction ni la
    /// mesure d'occupation ne le regardent. Sans quoi convertir trois cents
    /// voitures pour leurs vignettes évincerait les aperçus que l'utilisateur
    /// consulte vraiment, et le cache travaillerait contre lui.
    #[test]
    fn the_scratch_folder_stays_out_of_the_cache_ceiling() {
        let base = crate::testutil::temp_dir("preview-scratch-cap");
        let dir = base.join("previews");
        let scratch = dir.join(SCRATCH);
        std::fs::create_dir_all(&scratch).unwrap();
        let kept = entry_stem("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        std::fs::write(dir.join(format!("{kept}.gltf")), vec![0u8; 4096]).unwrap();
        let draft = entry_stem("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
        std::fs::write(scratch.join(format!("{draft}.gltf")), vec![0u8; 1024 * 1024]).unwrap();

        assert_eq!(dir_size(&dir), 4096, "l'occupation du cache ignore le brouillon");
        // Un plafond ridicule : tout ce que l'éviction voit devrait partir,
        // sauf l'entrée la plus récente, qu'elle garde toujours.
        evict_to(&dir, 1);
        assert!(
            scratch.join(format!("{draft}.gltf")).is_file(),
            "l'éviction ne touche pas au brouillon"
        );
    }

    /// Une conversion minimale, montée à la main : un tampon de géométrie et
    /// deux images, ce qu'il faut pour éprouver le rangement et rien de plus.
    fn fake_conversion(geometry: &[u8], images: Vec<Vec<u8>>) -> kn5_gltf::Conversion {
        let json = serde_json::json!({
            "buffers": [ { "byteLength": geometry.len() } ],
            "images": images
                .iter()
                .enumerate()
                .map(|(index, _)| serde_json::json!({
                    "mimeType": if index == 0 { "image/jpeg" } else { "image/png" },
                }))
                .collect::<Vec<_>>(),
        });
        kn5_gltf::Conversion {
            document: kn5_gltf::glb::Document {
                json,
                buffer: geometry.to_vec(),
                images,
            },
            geometry: Default::default(),
            triangle_count: 1,
            material_count: 1,
            texture_count: 2,
            texture_warnings: Vec::new(),
        }
    }

    // Règle : **chaque URI qu'une entrée écrit est servie par le protocole.**
    // C'est le point de rupture de tout ce rangement : le document est lu par
    // `GLTFLoader`, qui résout ses URI relatives depuis l'adresse du `.gltf` —
    // une seule qui ne résout pas donne une voiture sans texture, sans erreur
    // visible et sans rien dans le journal.
    #[test]
    fn every_uri_an_entry_writes_is_served_back() {
        let base = crate::testutil::temp_dir("preview-roundtrip");
        let dir = base.join("previews");
        std::fs::create_dir_all(&dir).unwrap();

        let images = vec![b"a jpeg, pretend".to_vec(), b"a png, pretend".to_vec()];
        let conversion = fake_conversion(b"geometry bytes", images.clone());
        let stem = entry_stem("abcdef0123456789abcdef0123456789");
        write_entry(&dir, &stem, &conversion).expect("écrit");

        let document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join(format!("{stem}.gltf"))).unwrap()).unwrap();

        let mut uris = vec![document["buffers"][0]["uri"].as_str().unwrap().to_string()];
        for index in 0..images.len() {
            uris.push(document["images"][index]["uri"].as_str().unwrap().to_string());
        }
        for uri in &uris {
            let served = cached_file(&dir, &format!("/{uri}"))
                .unwrap_or_else(|| panic!("`{uri}` doit être servie par le protocole"));
            assert!(served.is_file(), "`{uri}` pointe sur un fichier réel");
        }

        // L'extension suit le type MIME, sans quoi la webview refuse l'image.
        assert!(
            uris[1].ends_with(".jpg"),
            "un JPEG s'annonce comme tel, got {}",
            uris[1]
        );
        assert!(uris[2].ends_with(".png"), "et un PNG aussi, got {}", uris[2]);
        assert_eq!(
            std::fs::read(cached_file(&dir, &format!("/{}", uris[1])).unwrap()).unwrap(),
            images[0],
            "le blob rend bien les octets qu'on lui a confiés"
        );
    }

    // Règle : deux entrées qui produisent les mêmes octets partagent leur blob,
    // écrit une seule fois. C'est tout l'intérêt du rangement — mesuré sur
    // trois skins d'une voiture, une seule variante de géométrie pour les trois.
    #[test]
    fn two_entries_with_the_same_geometry_write_it_once() {
        let base = crate::testutil::temp_dir("preview-share");
        let dir = base.join("previews");
        std::fs::create_dir_all(&dir).unwrap();

        let geometry = b"the very same vertices";
        write_entry(
            &dir,
            &entry_stem("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            &fake_conversion(geometry, vec![b"livery one".to_vec()]),
        )
        .expect("écrit");
        write_entry(
            &dir,
            &entry_stem("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            &fake_conversion(geometry, vec![b"livery two".to_vec()]),
        )
        .expect("écrit");

        let blobs: Vec<_> = std::fs::read_dir(dir.join(BLOBS))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(
            blobs.iter().filter(|n| n.ends_with(".bin")).count(),
            1,
            "une seule géométrie pour les deux entrées, got {blobs:?}"
        );
        assert_eq!(
            blobs.iter().filter(|n| n.ends_with(".jpg")).count(),
            2,
            "mais deux livrées, got {blobs:?}"
        );
    }

    // Règle : un blob n'est effacé qu'une fois la DERNIÈRE entrée qui le cite
    // partie. C'est toute la correction de la déduplication : deux skins d'une
    // même voiture partagent leur géométrie au bit près, et évincer l'un ne
    // doit pas vider l'autre de sa substance.
    #[test]
    fn a_shared_blob_survives_until_its_last_reader_is_gone() {
        let base = crate::testutil::temp_dir("preview-blobs");
        let dir = base.join("previews");
        std::fs::create_dir_all(dir.join(BLOBS)).unwrap();

        let shared = "a".repeat(64) + ".bin";
        let alone = "b".repeat(64) + ".png";
        std::fs::write(dir.join(BLOBS).join(&shared), vec![0u8; 400]).unwrap();
        std::fs::write(dir.join(BLOBS).join(&alone), vec![0u8; 400]).unwrap();

        let old = dir.join(format!("{}.gltf", entry_stem("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")));
        let fresh = dir.join(format!("{}.gltf", entry_stem("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")));
        // L'ancienne cite les deux blobs, la récente n'en cite qu'un.
        std::fs::write(&old, format!(r#"{{"a":"{BLOBS}/{shared}","b":"{BLOBS}/{alone}"}}"#)).unwrap();
        std::fs::write(&fresh, format!(r#"{{"a":"{BLOBS}/{shared}"}}"#)).unwrap();
        let now = SystemTime::now();
        std::fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_modified(now - std::time::Duration::from_secs(600))
            .unwrap();

        // Un plafond qui ne laisse de la place que pour la plus récente.
        evict_to(&dir, 500);

        assert!(!old.exists(), "la moins récente est évincée");
        assert!(fresh.is_file(), "la plus récente reste");
        assert!(
            dir.join(BLOBS).join(&shared).is_file(),
            "le blob que la survivante cite encore doit rester"
        );
        assert!(
            !dir.join(BLOBS).join(&alone).exists(),
            "celui que plus personne ne cite est repris"
        );
    }

    // Règle : les noms de blobs se relisent correctement dans un document.
    // Écrit parce que la première version se trompait : elle lisait « tout ce
    // qui est hexadécimal », or le `b` de `.bin` en est un — chaque référence
    // ressortait tronquée en `<empreinte>.b`, donc introuvable, donc **tous les
    // blobs auraient été effacés à la première éviction**.
    #[test]
    fn blob_references_read_the_whole_name_extension_included() {
        let base = crate::testutil::temp_dir("preview-refs");
        std::fs::create_dir_all(&base).unwrap();
        let entry = base.join("doc.gltf");
        let geometry = "0".repeat(64);
        let image = "f".repeat(64);
        std::fs::write(
            &entry,
            format!(r#"{{"buffers":[{{"uri":"{BLOBS}/{geometry}.bin"}}],"images":[{{"uri":"{BLOBS}/{image}.jpg"}}]}}"#),
        )
        .unwrap();

        let found = blob_references(&entry);
        assert!(
            found.contains(&format!("{geometry}.bin")),
            "la géométrie, extension comprise, got {found:?}"
        );
        assert!(found.contains(&format!("{image}.jpg")), "et l'image");
        assert_eq!(found.len(), 2, "rien de plus, got {found:?}");
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
        std::fs::write(dir.join(format!("{mine}.gltf")), b"glb").unwrap();
        std::fs::write(dir.join(format!("{mine}.txt")), b"1 2 3").unwrap();
        std::fs::write(dir.join("v1-abcdef0123456789abcdef0123456789.gltf"), b"vieux").unwrap();
        std::fs::write(dir.join("v1-abcdef0123456789abcdef0123456789.txt"), b"1 2 3").unwrap();
        // Nom de l'époque où la version vivait dans le hachage, sans préfixe.
        std::fs::write(dir.join("0123456789abcdef0123456789abcdef.gltf"), b"ancien").unwrap();

        sweep_foreign_versions(&dir);

        assert!(
            dir.join(format!("{mine}.gltf")).is_file() && dir.join(format!("{mine}.txt")).is_file(),
            "l'entrée de la version courante survit, compteurs compris"
        );
        assert!(
            !dir.join("v1-abcdef0123456789abcdef0123456789.gltf").exists(),
            "l'entrée d'une version antérieure est effacée"
        );
        assert!(
            !dir.join("v1-abcdef0123456789abcdef0123456789.txt").exists(),
            "son fichier de compteurs part avec"
        );
        assert!(
            !dir.join("0123456789abcdef0123456789abcdef.gltf").exists(),
            "et les noms sans préfixe, d'avant ce nommage, aussi"
        );
    }

    // Règle : la reprise a lieu à **chaque** passe d'éviction, y compris très
    // en dessous du plafond. Bug réel : quatre entrées `v25` posées par une
    // seconde instance restée ouverte survivaient à 312 conversions en `v32`,
    // le balayage n'ayant lieu qu'une fois par exécution, avant elles.
    #[test]
    fn a_stale_entry_written_after_the_first_sweep_is_still_reclaimed() {
        let base = crate::testutil::temp_dir("preview-sweep-late");
        let dir = base.join("previews");
        std::fs::create_dir_all(&dir).unwrap();
        let mine = entry_stem("abcdef0123456789abcdef0123456789");
        std::fs::write(dir.join(format!("{mine}.gltf")), b"glb").unwrap();
        std::fs::write(dir.join("v25-d0123456789abcdef0123456789abcdef.gltf"), b"vieux").unwrap();
        std::fs::write(dir.join("v25-d0123456789abcdef0123456789abcdef.rig"), b"rig").unwrap();

        // Un plafond très large : rien à évincer, et la reprise doit avoir
        // lieu quand même.
        evict_to(&dir, u64::MAX);

        assert!(
            dir.join(format!("{mine}.gltf")).is_file(),
            "l'entrée de la version courante survit"
        );
        assert!(
            !dir.join("v25-d0123456789abcdef0123456789abcdef.gltf").exists()
                && !dir.join("v25-d0123456789abcdef0123456789abcdef.rig").exists(),
            "celle d'une autre version part, plafond ou pas"
        );
    }

    // Règle : au-delà du plafond, on évince les entrées les plus anciennement
    // utilisées — et le fichier de compteurs part avec.
    #[test]
    fn eviction_removes_the_least_recently_used_first() {
        let base = crate::testutil::temp_dir("preview-evict");
        let dir = base.join("previews");
        std::fs::create_dir_all(&dir).unwrap();

        // Des noms de la version courante : `evict_to` reprend d'abord les
        // entrées d'une autre version, et sans préfixe elles partiraient là
        // au lieu d'être évincées par ancienneté.
        let old = dir.join(format!("{}.gltf", entry_stem("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")));
        let fresh = dir.join(format!("{}.gltf", entry_stem("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")));
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
        std::fs::write(dir.join(format!("{key}.gltf")), b"glTFbody").unwrap();

        let ok = serve(&dir, &request("GET", &format!("/{key}.gltf"), None));
        assert_eq!(ok.status(), 200, "entrée servie");
        assert_eq!(
            ok.headers()
                .get(ACCESS_CONTROL_ALLOW_ORIGIN)
                .map(|v| v.to_str().unwrap()),
            Some("*"),
            "sans cet en-tête la page ne peut pas lire le modèle"
        );
        assert_eq!(ok.body(), b"glTFbody", "corps complet");

        let missing = serve(&dir, &request("GET", "/00000000000000000000000000000000.gltf", None));
        assert_eq!(missing.status(), 404, "entrée absente");
        assert!(
            missing.headers().contains_key(ACCESS_CONTROL_ALLOW_ORIGIN),
            "une 404 sans CORS remonte en erreur réseau opaque, ce qui masque la cause"
        );

        // `Range` n'étant pas un en-tête sûr au sens CORS, une lecture
        // partielle commence par un préchargement `OPTIONS`.
        let preflight = serve(&dir, &request("OPTIONS", &format!("/{key}.gltf"), None));
        assert_eq!(preflight.status(), 204, "préchargement accepté");
        assert!(
            preflight.headers().contains_key(ACCESS_CONTROL_ALLOW_ORIGIN),
            "préchargement porte les en-têtes CORS"
        );

        let partial = serve(&dir, &request("GET", &format!("/{key}.gltf"), Some("bytes=4-7")));
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
        let file = base.join("entry.gltf");
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
