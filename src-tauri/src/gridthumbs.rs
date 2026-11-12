//! Regenerated thumbnails for the library grid (GRILLE§5).
//!
//! The grid shows whatever the mod author shipped as `preview.png`: renders on
//! black, renders on white, in-game captures, photographs. The eye re-adapts to
//! a new background on every card, so it never gets to compare the *shapes* —
//! which is what identifying a car is. Rendering all of them through the same
//! rig, with the same framing and the same light, is what turns that grid into
//! a catalogue.
//!
//! Three properties hold this module together, and each of them is a decision
//! taken against an obvious alternative:
//!
//! **The store sits outside the preview cache ceiling.** A thumbnail is a PNG
//! of a couple hundred kilobytes that must survive; the `.glb` that produced it
//! is twenty megabytes that must not. Same reasoning — and same measurement —
//! as the driver body thumbnails: 312 conversions poured into a pool that is
//! already at its 2 GiB cap evict the entries the user actually consults, and
//! the cache starts working against them (GRILLE§5.3).
//!
//! **A thumbnail identity is the car cache entry name plus the template.** The
//! first half already tracks the `.kn5`, its date, the skin, the CSP configs and
//! the converter version, so a mod updated on disk regenerates on its own; the
//! second is what makes "Appliquer" in the settings screen mean something.
//! Nothing here needs a migration or an invalidation pass.
//!
//! **A failure is remembered, next to the image it could not produce.** Some
//! cars are encrypted and will never be renderable (GRILLE§7): retrying fourteen
//! protected models on every launch would cost fourteen KN5 parses for an
//! answer that cannot change until the mod itself does — and the fingerprint in
//! the name is exactly what notices that it did.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Rendering template, as the settings screen holds it (GRILLE§5.6).
///
/// Every field is an integer in user-facing units — degrees for angles,
/// percentages for the rest — because that is what the sliders produce and what
/// `ui_prefs.json` stores. The frontend owns the values and the defaults; the
/// backend only needs them to be *stable*, since they are half of a thumbnail
/// identity.
///
/// What is **not** here is as deliberate as what is: format, transparency,
/// fixed exposure and the absence of post-processing are the properties that
/// guarantee 312 cars are comparable, so they are not settings at all.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridTemplate {
    /// Camera rotation around the vertical axis, in degrees.
    pub azimuth: i32,
    /// Camera pitch above the horizon, in degrees.
    pub elevation: i32,
    /// Vertical field of view, in degrees.
    pub fov: i32,
    /// Framing margin around the bounding box, in percent.
    pub margin: i32,
    /// Contact shadow opacity, in percent.
    pub shadow: i32,
    /// Where the camera looks, as a percentage of the model radius above its
    /// centre. Negative lifts the car in the frame — which is what a preset
    /// with a reflection needs, the reflection taking the space below.
    pub height: i32,
    /// Pool of light painted on the ground, in percent. Zero means no floor at
    /// all, and the render then carries nothing but the car.
    pub floor: i32,
    /// Mirror reflection on that floor, in percent. It needs the pool: a
    /// reflection is a modulation of a floor's brightness, and there is nothing
    /// to modulate on a transparent background.
    pub reflection: i32,
    /// Blur of that reflection, in tenths. It is what separates a floor read as
    /// lacquered from one read as wet — the single most visible knob of a
    /// showcase preset once the reflection is there at all.
    pub reflection_blur: i32,
    /// Backdrop baked into the image, in percent. Zero leaves the frame
    /// transparent — the card owns the background, which is the rule the whole
    /// design rests on. Above zero the render carries its own, which is the
    /// only way a regenerated thumbnail can be **indistinguishable** from an
    /// untouched `preview.png` sitting next to it in the same grid.
    pub background: i32,
    /// Same camera distance for every car, as the game shoots its previews,
    /// instead of fitting each car to the frame — 0 or 100.
    ///
    /// `serde(default)` because templates saved before it existed must still
    /// read: they mean "fitted", which is what 0 says.
    #[serde(default)]
    pub true_scale: i32,
    /// Light from the showroom environment, in percent of the renderer's own
    /// level. Defaults to 100 when absent: that is the level every template
    /// rendered at before it was a setting.
    #[serde(default = "neutral")]
    pub ambient: i32,
    /// Each image exposed to its own brightness, as the game's previews are
    /// — 0 or 100. Absent means off: fixed exposure, as before it existed.
    #[serde(default)]
    pub auto_exposure: i32,
    /// The game's own preview camera, the same for every car — 0 or 100.
    /// Absent means off: the orbit, as before it existed.
    #[serde(default)]
    pub game_framing: i32,
    /// Colour saturation, in percent. Absent means 100: the colours as
    /// rendered, as before it existed.
    #[serde(default = "neutral")]
    pub saturation: i32,
    /// Cool tint, in percent of the game's own cast. Absent means 0.
    #[serde(default)]
    pub cool_tint: i32,
    /// Window reflections, in percent of the paint's. Absent means 100: the
    /// glass as rendered, as before it existed.
    #[serde(default = "neutral")]
    pub glass: i32,
    /// Chrome reflections, in percent of the paint's. Absent means 100: the
    /// chrome as rendered, as before it existed.
    #[serde(default = "neutral")]
    pub chrome: i32,
    /// Size of the disc of light on the floor, in percent of a radius. Absent
    /// means 0: the wide glow, as before it existed.
    #[serde(default)]
    pub pool_size: i32,
    /// Softness of that disc's edge, in percent of its radius. Only means
    /// something with a disc, and only counts then.
    #[serde(default = "soft_edge")]
    pub pool_edge: i32,
    /// Version of the **frontend renderer**, incremented whenever the drawing
    /// code changes the pixels it produces.
    ///
    /// The exact analogue of `preview::CONVERTER_VERSION`, and it exists for
    /// the same reason, learnt the same way: a rendering fixed in the app is a
    /// rendering nothing on disk knows about. The thumbnails were carrying the
    /// `.kn5`, the skin, the CSP configs, the converter and the template —
    /// everything except the code that draws. So the first rendering bug (a
    /// backdrop that covered the car, every Officiel thumbnail solid black)
    /// left its images served for ever, correct by every measure the name
    /// could check.
    pub renderer: i32,
    /// The two ends of the card's radial gradient, as CSS colours. They belong
    /// to the card, not to the render — except when the backdrop bakes them in,
    /// which is exactly when they enter the fingerprint below.
    pub mat_hi: String,
    pub mat_lo: String,
}

impl GridTemplate {
    /// Eight hex characters of the template, to append to a car entry name.
    ///
    /// Short on purpose: it is a suffix on a name that already carries a
    /// 32-character hash, and it only has to separate the handful of templates
    /// one user will ever try.
    pub(crate) fn fingerprint(&self) -> String {
        let mut hasher = Sha256::new();
        for value in [
            self.azimuth,
            self.elevation,
            self.fov,
            self.margin,
            self.shadow,
            self.height,
            self.floor,
            self.reflection,
            self.reflection_blur,
            self.background,
            self.renderer,
        ] {
            hasher.update(value.to_le_bytes());
        }
        // **The mat enters the fingerprint only when it is baked.** It is a
        // card colour, so changing it normally costs nothing and must not
        // reprint 312 images — that is what keeps a theme change free. But a
        // preset with a backdrop paints those very colours *into* the PNG, and
        // an image that no longer matches its card is the one defect this whole
        // arrangement exists to avoid. Conditional, therefore: not a special
        // case, the honest reading of "the fingerprint covers what decides the
        // image".
        if self.background > 0 {
            hasher.update(self.mat_hi.as_bytes());
            hasher.update(self.mat_lo.as_bytes());
        }
        // **Hashed only when on**, for the same reason as the mat above: a field
        // added to the template must not rename every thumbnail already on
        // disk. Off, it changes no pixel, so it has no business in the name.
        if self.true_scale > 0 {
            hasher.update(b"true-scale");
            hasher.update(self.true_scale.to_le_bytes());
        }
        // Same rule for the ambient light, around its own default: at 100 it
        // renders as before it existed, so it must not rename those images.
        if self.ambient != NEUTRAL {
            hasher.update(b"ambient");
            hasher.update(self.ambient.to_le_bytes());
        }
        if self.auto_exposure > 0 {
            hasher.update(b"auto-exposure");
            hasher.update(self.auto_exposure.to_le_bytes());
        }
        if self.game_framing > 0 {
            hasher.update(b"game-framing");
            hasher.update(self.game_framing.to_le_bytes());
        }
        if self.saturation != NEUTRAL {
            hasher.update(b"saturation");
            hasher.update(self.saturation.to_le_bytes());
        }
        if self.cool_tint > 0 {
            hasher.update(b"cool-tint");
            hasher.update(self.cool_tint.to_le_bytes());
        }
        if self.glass != NEUTRAL {
            hasher.update(b"glass");
            hasher.update(self.glass.to_le_bytes());
        }
        if self.chrome != NEUTRAL {
            hasher.update(b"chrome");
            hasher.update(self.chrome.to_le_bytes());
        }
        // The edge belongs to the disc: without one it changes no pixel, so it
        // stays out of the name, like the mat without a baked backdrop.
        if self.pool_size > 0 {
            hasher.update(b"pool");
            hasher.update(self.pool_size.to_le_bytes());
            hasher.update(self.pool_edge.to_le_bytes());
        }
        format!("{:x}", hasher.finalize())[..8].to_string()
    }
}

/// The value of a percentage field that changes nothing — ambient light,
/// saturation, window reflections — and that templates saved before the field
/// existed therefore read as.
const NEUTRAL: i32 = 100;

fn neutral() -> i32 {
    NEUTRAL
}

/// The disc edge a template saved before the setting reads as.
fn soft_edge() -> i32 {
    30
}

/// What the grid knows about one car thumbnail, without converting anything.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GridThumb {
    /// Entry name, to hand back when saving or when recording a failure. The
    /// frontend never builds it: identity belongs here, with the fingerprints
    /// it is made of.
    pub stem: String,
    /// Path of the PNG, when it is already rendered.
    pub path: Option<String>,
    /// Why this car will not render, when a previous attempt said so. An i18n
    /// key (`errors.preview*`), never a sentence.
    pub failed: Option<String>,
    /// The image was rendered by the game: the car is protected
    /// ([`write_from_game`]).
    pub from_game: bool,
}

/// Where thumbnails live: next to the preview cache, and **outside its
/// ceiling** — see the module header.
pub fn dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("dossier de cache indisponible : {e}"))?
        .join("gridthumbs");
    std::fs::create_dir_all(&dir).map_err(|e| format!("création du cache de vignettes : {e}"))?;
    Ok(dir)
}

/// Name of the thumbnail produced for `car_stem` under `template`.
///
/// `car_stem` is the car preview cache entry name, version prefix included
/// (`crate::preview::car_entry_stem`), so incrementing the converter retires
/// every thumbnail with it — and the sweep that already clears foreign versions
/// picks them up.
pub fn entry_stem(car_stem: &str, template: &GridTemplate) -> String {
    format!("{car_stem}-t{}", template.fingerprint())
}

/// Le nom d'entrée est-il bien un nom d'entrée ?
///
/// Il fait l'aller-retour par le frontend — rendu par `grid_thumbnail`, rendu
/// tel quel à `save_grid_thumbnail` — donc il revient d'une source qu'on ne
/// contrôle pas entièrement, et il sert à composer un nom de fichier. Forme
/// attendue : `v<chiffres>-<hexadécimal>-t<hexadécimal>`, rien d'autre, et
/// surtout aucun séparateur.
pub fn is_entry_stem(stem: &str) -> bool {
    let Some((car, template)) = stem.rsplit_once("-t") else {
        return false;
    };
    let Some((version, key)) = car.strip_prefix('v').and_then(|rest| rest.split_once('-')) else {
        return false;
    };
    !version.is_empty()
        && version.chars().all(|c| c.is_ascii_digit())
        && !key.is_empty()
        && key.chars().all(|c| c.is_ascii_hexdigit())
        && !template.is_empty()
        && template.chars().all(|c| c.is_ascii_hexdigit())
}

/// What exists on disk for this entry: the image, or the reason there is none.
pub fn look_up(app: &tauri::AppHandle, stem: &str) -> GridThumb {
    let dir = dir(app).ok();
    let png = dir.as_ref().map(|d| d.join(format!("{stem}.png")));
    let failed = dir
        .as_ref()
        .and_then(|d| std::fs::read_to_string(d.join(format!("{stem}.fail"))).ok())
        .map(|reason| reason.trim().to_string())
        .filter(|reason| !reason.is_empty());
    let from_game = dir.as_ref().is_some_and(|d| d.join(format!("{stem}.game")).is_file());
    GridThumb {
        stem: stem.to_string(),
        path: png.filter(|p| p.is_file()).map(|p| p.to_string_lossy().into_owned()),
        failed,
        from_game,
    }
}

/// Stores the PNG the frontend just rendered, and returns its path.
///
/// Written under a temporary name then renamed: a thumbnail truncated by a
/// brutal shutdown would be served forever afterwards, since its name is what
/// says it exists.
pub fn write(app: &tauri::AppHandle, stem: &str, png: &[u8]) -> Result<PathBuf, String> {
    write_in(&dir(app)?, stem, png)
}

/// [`write`] into a given store folder: the in-game renderer (`gameshot`)
/// stores its images through here, without an app handle.
pub fn write_in(dir: &std::path::Path, stem: &str, png: &[u8]) -> Result<PathBuf, String> {
    if !is_entry_stem(stem) {
        return Err(format!("nom de vignette refusé : {stem}"));
    }
    let file = dir.join(format!("{stem}.png"));
    let tmp = dir.join(format!("{stem}.{}.tmp", std::process::id()));
    std::fs::write(&tmp, png).map_err(|e| format!("{} : {e}", tmp.display()))?;
    std::fs::rename(&tmp, &file).map_err(|e| format!("{} : {e}", file.display()))?;
    // A car that used to fail and now renders must stop being skipped.
    let _ = std::fs::remove_file(dir.join(format!("{stem}.fail")));
    Ok(file)
}

/// Stores an image the game rendered (GRILLE§7.1), and says so next to it.
///
/// The `.game` mark is what keeps a protected car known as one once its image
/// exists: the failure mark goes with the render, and without this one nothing
/// would tell it from any other thumbnail — the settings screen could not say
/// how many protected cars are done. Best-effort: a missing mark costs a count,
/// not an image.
pub fn write_from_game(dir: &std::path::Path, stem: &str, png: &[u8]) -> Result<PathBuf, String> {
    let file = write_in(dir, stem, png)?;
    if let Err(e) = std::fs::write(dir.join(format!("{stem}.game")), b"") {
        log::warn!("gridthumbs: {stem} not marked as rendered by the game — {e}");
    }
    Ok(file)
}

/// Remembers that this entry cannot be rendered, and why (GRILLE§7).
///
/// `reason` is an i18n key. Best-effort: failing to write it costs one retry at
/// the next launch, not a bug — but it is logged, because a store that silently
/// forgets its failures looks exactly like a generation pass that never ends.
pub fn mark_failed(app: &tauri::AppHandle, stem: &str, reason: &str) {
    if !is_entry_stem(stem) {
        log::warn!("gridthumbs: nom de vignette refusé — {stem}");
        return;
    }
    let Ok(dir) = dir(app) else { return };
    // Une clé i18n, pas un roman : ce que le frontend passe finit relu et
    // affiché, et un message technique de plusieurs kilo-octets n'a rien à
    // faire dans un marqueur qu'on garde pour toujours.
    let reason: String = reason.trim().chars().filter(|c| !c.is_control()).take(120).collect();
    if let Err(e) = std::fs::write(dir.join(format!("{stem}.fail")), reason.as_bytes()) {
        log::warn!("gridthumbs: échec de {stem} non mémorisé — {e}");
    }
}

/// Oublie ce qu'on sait de cette entrée : l'image et le marqueur d'échec.
///
/// C'est le « refaire celle-là » d'une seule voiture. Il n'y a rien à
/// invalider pour ça — le nom d'entrée ne change pas — donc il suffit de
/// retirer le fichier pour que la prochaine demande reparte de la conversion.
///
/// Rendu au bouton qui l'appelle : `true` s'il y avait effectivement quelque
/// chose à oublier. Utile pour ne pas annoncer un travail qui n'a pas eu lieu.
pub fn forget(app: &tauri::AppHandle, stem: &str) -> Result<bool, String> {
    if !is_entry_stem(stem) {
        return Err(format!("nom de vignette refusé : {stem}"));
    }
    let dir = dir(app)?;
    let png = std::fs::remove_file(dir.join(format!("{stem}.png"))).is_ok();
    let failed = std::fs::remove_file(dir.join(format!("{stem}.fail"))).is_ok();
    // The game mark goes with its image: it says how that image was made.
    let _ = std::fs::remove_file(dir.join(format!("{stem}.game")));
    Ok(png || failed)
}

/// Counters for the generation report and the settings screen (GRILLE§8.2).
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GridThumbStats {
    pub generated: u32,
    pub failed: u32,
    pub bytes: u64,
}

pub fn stats(app: &tauri::AppHandle) -> Result<GridThumbStats, String> {
    let dir = dir(app)?;
    let mut stats = GridThumbStats::default();
    let Ok(read) = std::fs::read_dir(&dir) else {
        return Ok(stats);
    };
    for entry in read.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        if name.ends_with(".png") {
            stats.generated += 1;
            stats.bytes += meta.len();
        } else if name.ends_with(".fail") {
            stats.failed += 1;
        }
    }
    Ok(stats)
}

/// Empties the store and returns the bytes freed.
///
/// Failures go with the images: someone who asks for a clean slate is usually
/// asking precisely for the protected cars to be tried again.
pub fn clear(app: &tauri::AppHandle) -> Result<u64, String> {
    let dir = dir(app)?;
    let mut freed = 0u64;
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_file() && std::fs::remove_file(entry.path()).is_ok() {
            freed += meta.len();
        }
    }
    Ok(freed)
}

/// Removes the images no live preset claims any more.
///
/// Not a nicety. Changing a framing value rewrites 312 names, and nothing else
/// would ever collect the previous set — the store has no eviction pass to hook
/// onto, by design.
///
/// Takes **every** live template rather than one, and that is the whole point:
/// the grid binds a preset per density (dense and comfortable), so two sets of
/// images are legitimately alive at once. Keeping both is what makes switching
/// density instant once they are produced; keeping only the last one applied
/// would make every switch a five-minute job.
///
/// An empty list keeps nothing — it is the honest reading of "no preset is in
/// use", and it cannot happen from the UI, which always binds two.
pub fn sweep_other_templates(app: &tauri::AppHandle, templates: &[GridTemplate]) -> Result<u32, String> {
    let dir = dir(app)?;
    let keep: Vec<String> = templates.iter().map(|t| format!("-t{}", t.fingerprint())).collect();
    let mut removed = 0u32;
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
        let path = entry.path();
        let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
            continue;
        };
        if keep.iter().any(|suffix| stem.ends_with(suffix)) || !entry.metadata().is_ok_and(|m| m.is_file()) {
            continue;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => removed += 1,
            Err(e) => log::warn!("gridthumbs: vignette d'un preset disparu non supprimée — {e}"),
        }
    }
    if removed > 0 {
        log::info!("gridthumbs: {removed} vignette(s) d'un preset disparu effacée(s)");
    }
    Ok(removed)
}

/// A preset bound to a density of the grid, as the frontend holds it: its id,
/// which a frozen thumbnail is named after (ESPACE§3.3), and the template its
/// images are rendered under.
#[derive(Debug, Clone, Deserialize)]
pub struct LivePreset {
    pub id: String,
    pub template: GridTemplate,
}

/// The thumbnails already rendered for this car under each of `presets`, in
/// the livery `skin_id` — what a skeleton freezes (ESPACE§3.3). Nothing is
/// converted: a car with no model to name its entry after has none.
pub fn rendered(
    app: &tauri::AppHandle,
    car_dir: &std::path::Path,
    car_id: &str,
    skin_id: Option<&str>,
    presets: &[LivePreset],
) -> Vec<crate::skeleton::ThumbFile> {
    if presets.is_empty() {
        return Vec::new();
    }
    let (Ok(store), Ok(car_stem)) = (dir(app), crate::preview::car_entry_stem(app, car_dir, car_id, skin_id)) else {
        return Vec::new();
    };
    presets
        .iter()
        .filter_map(|p| {
            let path = store.join(format!("{}.png", entry_stem(&car_stem, &p.template)));
            path.is_file().then(|| crate::skeleton::ThumbFile {
                preset: p.id.clone(),
                fingerprint: p.template.fingerprint(),
                path,
            })
        })
        .collect()
}

/// The grid thumbnails each of these cars shows right now, to be frozen with
/// its skeleton (ESPACE§3.3): cars that still have their files, in the livery
/// of their thumbnails (`preferred.ts`). Empty when `presets` is — the thumbnails
/// are off, or the user unchecked them.
///
/// The base first, its lock given back, then the disk: a few `stat` per car.
pub fn card_thumbs(
    app: &tauri::AppHandle,
    ids: &[String],
    presets: &[LivePreset],
) -> std::collections::HashMap<String, Vec<crate::skeleton::ThumbFile>> {
    if presets.is_empty() {
        return Default::default();
    }
    let cars = cars_with_files(app, ids);
    let prefs = crate::ui_prefs::load(app);
    cars.into_iter()
        .filter_map(|(id, dir)| {
            let skin = thumbnail_skin(&prefs, &id);
            let thumbs = rendered(app, &dir, &id, skin.as_deref(), presets);
            (!thumbs.is_empty()).then_some((id, thumbs))
        })
        .collect()
}

/// The cars among `ids` that still have their files — not in the showcase,
/// not untouched base content — with their model folder. The base is read and
/// its lock given back before any file is touched by the caller.
fn cars_with_files(app: &tauri::AppHandle, ids: &[String]) -> Vec<(String, PathBuf)> {
    library_cars(app, ids)
        .into_iter()
        .filter(|car| !car.is_stock || car.unmanaged)
        .map(|car| (car.id, car.dir))
        .collect()
}

/// A car of the library that is not in the showcase, with its model folder.
struct LibraryCar {
    id: String,
    dir: PathBuf,
    is_stock: bool,
    /// Base content touched by the user: its files are its own.
    unmanaged: bool,
}

/// The cars among `ids` that are not in the showcase, with their folder —
/// base content included. The base is read and its lock given back before any
/// file is touched by the caller.
fn library_cars(app: &tauri::AppHandle, ids: &[String]) -> Vec<LibraryCar> {
    use tauri::Manager;
    let cfg = crate::config::load(app);
    let db = app.state::<crate::overlay::Db>();
    let Ok(conn) = db.0.lock() else {
        log::warn!("gridthumbs: base lock poisoned, no car listed");
        return Vec::new();
    };
    ids.iter()
        .filter_map(|id| {
            let m = crate::overlay::get_mod(&conn, id).ok().flatten()?;
            let car = crate::modscan::ModKind::from_column(&m.kind) == crate::modscan::ModKind::Car;
            (car && !m.showcase).then_some(())?;
            Some(LibraryCar {
                id: id.clone(),
                dir: crate::preview::car_dir(&conn, &cfg, id)?,
                is_stock: m.is_stock,
                unmanaged: m.is_unmanaged,
            })
        })
        .collect()
}

/// [`card_thumbs`] of every car of the library, for an export (EXPORT§4.3).
pub fn library_card_thumbs(
    app: &tauri::AppHandle,
    presets: &[LivePreset],
) -> std::collections::HashMap<String, Vec<crate::skeleton::ThumbFile>> {
    use tauri::Manager;
    if presets.is_empty() {
        return Default::default();
    }
    let ids: Vec<String> = {
        let db = app.state::<crate::overlay::Db>();
        let Ok(conn) = db.0.lock() else {
            log::warn!("gridthumbs: base lock poisoned, no thumbnail exported");
            return Default::default();
        };
        match crate::overlay::list_mods(&conn) {
            Ok(mods) => mods.into_iter().map(|m| m.id_interne).collect(),
            Err(e) => {
                log::warn!("gridthumbs: mods not listed, no thumbnail exported: {e}");
                return Default::default();
            }
        }
    };
    card_thumbs(app, &ids, presets)
}

/// A car whose grid thumbnail only the game can render (GRILLE§7.1), in the
/// livery of its thumbnails.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectedCar {
    pub car_id: String,
    pub skin: Option<String>,
}

/// The protected cars the in-game render still has work for (GRILLE§7.1): a
/// car known as protected under **any** of `templates` — recorded so by the
/// Pit Box renderer, or already rendered by the game — that lacks its image
/// under any of them. A preset added after a run in the game is covered too:
/// its images are missing for the very cars the game rendered before it.
pub fn protected_cars(app: &tauri::AppHandle, templates: &[GridTemplate]) -> Vec<ProtectedCar> {
    let Ok(store) = dir(app) else { return Vec::new() };
    car_entries(app)
        .into_iter()
        .filter(|car| {
            let entries = car.entries(templates);
            known_protected(&store, &entries) && !entries.iter().all(|e| has_image(&store, e))
        })
        .map(|car| ProtectedCar {
            car_id: car.car_id,
            skin: car.skin,
        })
        .collect()
}

/// Where the protected cars stand under the presets in use (GRILLE§7.1),
/// **counted by car**: a car missing from three presets is one car to do, not
/// three.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectedSummary {
    /// Still without an image under at least one of the presets.
    pub pending: u32,
    /// With an image under every preset.
    pub rendered: u32,
}

pub fn protected_summary(app: &tauri::AppHandle, templates: &[GridTemplate]) -> ProtectedSummary {
    let Ok(store) = dir(app) else {
        return ProtectedSummary::default();
    };
    let entries: Vec<Vec<String>> = car_entries(app).iter().map(|car| car.entries(templates)).collect();
    summarize(&store, &entries)
}

/// [`protected_summary`] over the entry names of each car, one per template.
fn summarize(store: &std::path::Path, cars: &[Vec<String>]) -> ProtectedSummary {
    let mut summary = ProtectedSummary::default();
    for entries in cars.iter().filter(|entries| known_protected(store, entries)) {
        if entries.iter().all(|e| has_image(store, e)) {
            summary.rendered += 1;
        } else {
            summary.pending += 1;
        }
    }
    summary
}

/// A view's preset, as the coverage asks about it: its template, and whether
/// it leaves base content as it is (`skipStock`), whose cars then need no
/// image of it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageQuery {
    pub template: GridTemplate,
    pub skip_stock: bool,
}

/// What a view still lacks (GRILLE§5.4): thumbnails Pit Box can render, and
/// protected cars only the game can.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Coverage {
    pub missing: u32,
    pub protected_missing: u32,
}

/// [`Coverage`] of each of `views`, in order. A car is protected for all of
/// them as soon as one says so. A car that failed for another reason is
/// counted nowhere: neither renderer would do better.
pub fn coverage(app: &tauri::AppHandle, views: &[CoverageQuery]) -> Vec<Coverage> {
    let Ok(store) = dir(app) else {
        return vec![Coverage::default(); views.len()];
    };
    let templates: Vec<GridTemplate> = views.iter().map(|v| v.template.clone()).collect();
    let cars: Vec<(bool, Vec<String>)> = car_entries(app)
        .iter()
        .map(|car| (car.is_stock, car.entries(&templates)))
        .collect();
    let skip: Vec<bool> = views.iter().map(|v| v.skip_stock).collect();
    count_coverage(&store, &cars, &skip)
}

/// [`coverage`] over each car's base-content flag and entry names.
fn count_coverage(store: &std::path::Path, cars: &[(bool, Vec<String>)], skip_stock: &[bool]) -> Vec<Coverage> {
    let mut out = vec![Coverage::default(); skip_stock.len()];
    for (is_stock, entries) in cars {
        let protected = known_protected(store, entries);
        for (i, entry) in entries.iter().enumerate() {
            if (*is_stock && skip_stock[i]) || has_image(store, entry) {
                continue;
            }
            if protected {
                out[i].protected_missing += 1;
            } else if !store.join(format!("{entry}.fail")).is_file() {
                out[i].missing += 1;
            }
        }
    }
    out
}

/// Whether the Pit Box renderer recorded this entry as a protected model.
fn is_protected(store: &std::path::Path, entry: &str) -> bool {
    std::fs::read_to_string(store.join(format!("{entry}.fail")))
        .is_ok_and(|reason| reason.trim() == crate::errors::PREVIEW_PROTECTED)
}

/// A car is protected once any of its entries says so: recorded by the Pit
/// Box renderer, or rendered by the game ([`write_from_game`]).
fn known_protected(store: &std::path::Path, entries: &[String]) -> bool {
    entries
        .iter()
        .any(|e| is_protected(store, e) || store.join(format!("{e}.game")).is_file())
}

fn has_image(store: &std::path::Path, entry: &str) -> bool {
    store.join(format!("{entry}.png")).is_file()
}

/// A car of the library as its thumbnails are named: its id, the livery of
/// its thumbnails, its entry name, which each template suffixes, and whether
/// it is base content.
struct CarEntry {
    car_id: String,
    skin: Option<String>,
    stem: String,
    is_stock: bool,
}

impl CarEntry {
    fn entries(&self, templates: &[GridTemplate]) -> Vec<String> {
        templates.iter().map(|t| entry_stem(&self.stem, t)).collect()
    }
}

/// Every car of the library with a model to render, base content included.
fn car_entries(app: &tauri::AppHandle) -> Vec<CarEntry> {
    use tauri::Manager;
    let ids: Vec<String> = {
        let db = app.state::<crate::overlay::Db>();
        let Ok(conn) = db.0.lock() else {
            log::warn!("gridthumbs: base lock poisoned, no car listed");
            return Vec::new();
        };
        match crate::overlay::list_mods(&conn) {
            Ok(mods) => mods.into_iter().map(|m| m.id_interne).collect(),
            Err(e) => {
                log::warn!("gridthumbs: mods not listed, no car listed: {e}");
                return Vec::new();
            }
        }
    };
    let prefs = crate::ui_prefs::load(app);
    library_cars(app, &ids)
        .into_iter()
        .filter_map(|car| {
            let skin = thumbnail_skin(&prefs, &car.id);
            let stem = crate::preview::car_entry_stem(app, &car.dir, &car.id, skin.as_deref()).ok()?;
            Some(CarEntry {
                car_id: car.id,
                skin,
                stem,
                is_stock: car.is_stock,
            })
        })
        .collect()
}

/// The livery a car's thumbnails are rendered in, as `preferred.ts` keeps it
/// in `ui_prefs.json`: a JSON object serialized into the value of
/// `pitbox.thumbSkin.<id>`. `None` is the car's default livery — not the
/// session's, which used to drive the thumbnails (GRILLE§5.4).
fn thumbnail_skin(prefs: &serde_json::Value, car_id: &str) -> Option<String> {
    let raw = prefs.get(format!("pitbox.thumbSkin.{car_id}"))?.as_str()?;
    let skin: serde_json::Value = serde_json::from_str(raw).ok()?;
    skin.get("id")?.as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rule (ESPACE§3.3, GRILLE§5.4): the livery of the thumbnails is the one
    /// chosen for them, read as `preferred.ts` writes it — a JSON object
    /// inside a string — and never the session's.
    #[test]
    fn the_thumbnail_skin_is_read_as_the_screen_writes_it() {
        let prefs = serde_json::json!({
            "pitbox.thumbSkin.lanzo": "{\"id\":\"red\",\"name\":\"Red\",\"preview\":null}",
            "pitbox.thumbSkin.broken": "not json",
            "pitbox.skin.session": "{\"id\":\"blue\",\"name\":\"Blue\",\"preview\":null}",
        });
        assert_eq!(thumbnail_skin(&prefs, "lanzo").as_deref(), Some("red"));
        assert_eq!(thumbnail_skin(&prefs, "broken"), None, "a damaged value is no livery");
        assert_eq!(thumbnail_skin(&prefs, "other"), None, "nor a missing one");
        assert_eq!(
            thumbnail_skin(&prefs, "session"),
            None,
            "the session's livery does not count"
        );
    }

    fn template() -> GridTemplate {
        GridTemplate {
            azimuth: 318,
            elevation: 8,
            fov: 22,
            margin: 6,
            shadow: 35,
            height: 0,
            floor: 0,
            reflection: 0,
            reflection_blur: 5,
            background: 0,
            true_scale: 0,
            ambient: 100,
            auto_exposure: 0,
            game_framing: 0,
            saturation: 100,
            cool_tint: 0,
            glass: 100,
            chrome: 100,
            pool_size: 0,
            pool_edge: 30,
            renderer: 1,
            mat_hi: "#2b2d33".to_string(),
            mat_lo: "#17181c".to_string(),
        }
    }

    /// GRILLE§5.6 — every template value is part of a thumbnail identity: two
    /// templates that differ anywhere must not share an image.
    #[test]
    fn every_template_field_changes_the_entry_name() {
        let base = template();
        let name = entry_stem("v46-abc", &base);
        let mut variants = vec![base.clone(); 23];
        variants[0].azimuth += 1;
        variants[1].elevation += 1;
        variants[2].fov += 1;
        variants[3].margin += 1;
        variants[4].ambient += 10;
        variants[5].saturation += 5;
        variants[6].cool_tint = 50;
        variants[7].shadow += 1;
        variants[8].height += 1;
        variants[9].floor += 1;
        variants[10].reflection += 1;
        variants[11].background += 1;
        variants[12].renderer += 1;
        variants[13].reflection_blur += 1;
        variants[14].true_scale = 100;
        variants[15].ambient += 5;
        variants[16].auto_exposure = 100;
        variants[17].game_framing = 100;
        variants[18].saturation -= 5;
        variants[19].cool_tint = 100;
        variants[20].glass += 10;
        variants[21].pool_size = 100;
        variants[22].chrome += 10;
        for variant in variants {
            assert_ne!(
                name,
                entry_stem("v46-abc", &variant),
                "un gabarit modifié doit donner une autre vignette : {variant:?}"
            );
        }
    }

    /// GRILLE§5.6 — the disc edge only shapes a disc: without a disc it must
    /// not rename a single image, with one it must.
    #[test]
    fn the_circle_edge_counts_only_with_a_circle() {
        let mut glow = template();
        let mut softer = template();
        softer.pool_edge += 20;
        assert_eq!(
            entry_stem("v46-aaa", &glow),
            entry_stem("v46-aaa", &softer),
            "sans cercle, le bord ne change aucune image"
        );
        glow.pool_size = 100;
        softer.pool_size = 100;
        assert_ne!(
            entry_stem("v46-aaa", &glow),
            entry_stem("v46-aaa", &softer),
            "avec un cercle, un autre bord est une autre image"
        );
    }

    /// The mat is a card colour, not a render value — until a preset bakes it
    /// into the image, and then it is both. Getting this backwards costs either
    /// 312 needless regenerations (always hashing it) or a baked backdrop that
    /// no longer matches its card (never hashing it).
    #[test]
    fn the_mat_counts_only_when_it_is_baked() {
        let mut plain = template();
        let mut repainted = template();
        repainted.mat_hi = "#000000".to_string();
        assert_eq!(
            entry_stem("v46-aaa", &plain),
            entry_stem("v46-aaa", &repainted),
            "sans fond cuit, changer la couleur de carte ne périme aucune image"
        );

        plain.background = 100;
        repainted.background = 100;
        assert_ne!(
            entry_stem("v46-aaa", &plain),
            entry_stem("v46-aaa", &repainted),
            "avec fond cuit, la couleur est dans l'image : elle doit périmer"
        );
    }

    /// Two presets are legitimately alive at once — one per grid density — so
    /// the sweep keeps both sets. Keeping only the last applied would turn
    /// every density switch into a five-minute job.
    #[test]
    fn the_sweep_keeps_every_live_preset() {
        let dir = crate::testutil::temp_dir("gridthumbs-sweep");
        let store = dir.join("gridthumbs");
        std::fs::create_dir_all(&store).unwrap();

        let mut vitrine = template();
        vitrine.floor = 85;
        let mut gone = template();
        gone.floor = 140;

        let names = [
            entry_stem("v46-aaa", &template()),
            entry_stem("v46-aaa", &vitrine),
            entry_stem("v46-aaa", &gone),
        ];
        for name in &names {
            std::fs::write(store.join(format!("{name}.png")), b"png").unwrap();
        }

        // Ce que ferait `sweep_other_templates` avec les deux presets vivants :
        // le corps du balayage, sans `AppHandle` — le module n'a pas de quoi en
        // fabriquer un, et c'est la règle qu'on veut prouver, pas le chemin du
        // dossier.
        let keep: Vec<String> = [template(), vitrine]
            .iter()
            .map(|t| format!("-t{}", t.fingerprint()))
            .collect();
        for entry in std::fs::read_dir(&store).unwrap().flatten() {
            let path = entry.path();
            let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
            if !keep.iter().any(|suffix| stem.ends_with(suffix)) {
                std::fs::remove_file(&path).unwrap();
            }
        }

        assert!(
            store.join(format!("{}.png", names[0])).is_file(),
            "le preset de la grille dense survit"
        );
        assert!(
            store.join(format!("{}.png", names[1])).is_file(),
            "celui de la grille confortable aussi"
        );
        assert!(
            !store.join(format!("{}.png", names[2])).is_file(),
            "celui d'un preset supprimé part : rien d'autre ne le ramasserait"
        );
    }

    /// The entry name makes a round trip through the frontend before coming
    /// back as half a file name: anything that is not one is refused.
    #[test]
    fn only_an_entry_name_is_accepted_as_one() {
        assert!(is_entry_stem(&entry_stem("v46-abc123", &template())), "un vrai nom");
        for refused in [
            "",
            "v46-abc123",
            "..-t0011aabb",
            "v46-abc123-tzz",
            "v46-../x-t0011aabb",
            "46-abc123-t0011aabb",
            "v46-abc/123-t0011aabb",
            "v46-abc123-t0011aabb/../evil",
        ] {
            assert!(!is_entry_stem(refused), "doit être refusé : {refused}");
        }
    }

    /// GRILLE§5.6 — true scale, the ambient light, auto exposure, game
    /// framing, colour grade, window and chrome reflections arrived after thumbnails
    /// were already on disk.
    /// A template saved before it must still deserialize, mean "fitted", and
    /// keep the very name its images were stored under: otherwise adding a
    /// field would silently regenerate every thumbnail of every preset.
    #[test]
    fn a_template_saved_before_the_newer_fields_keeps_its_thumbnails() {
        let old: GridTemplate = serde_json::from_value(serde_json::json!({
            "azimuth": 318, "elevation": 8, "fov": 22, "margin": 6, "key": 100, "fill": 25,
            "rim": 60, "shadow": 35, "height": 0, "floor": 0, "reflection": 0,
            "reflectionBlur": 5, "background": 0, "renderer": 1,
            "matHi": "#2b2d33", "matLo": "#17181c"
        }))
        .expect("un gabarit sans trueScale se relit");
        assert_eq!(old.true_scale, 0, "absent, il vaut « ajusté »");
        assert_eq!(old.auto_exposure, 0, "absent, l'exposition reste fixe");
        assert_eq!(old.game_framing, 0, "absent, la caméra reste en orbite");
        assert_eq!(old.saturation, 100, "absent, les couleurs restent celles du rendu");
        assert_eq!(old.cool_tint, 0, "absent, aucune teinte");
        assert_eq!(old.glass, 100, "absent, les vitres restent celles du rendu");
        assert_eq!(old.chrome, 100, "absent, les chromes restent ceux du rendu");
        assert_eq!(old.pool_size, 0, "absent, la lueur diffuse d'avant");
        assert_eq!(
            old.ambient, 100,
            "absent, la lumière ambiante est celle d'avant le réglage"
        );
        assert_eq!(old, template(), "relu à l'identique du gabarit de référence");
        // The literal is the name this template's images carry (SHA-256 of
        // its eleven values, recomputed outside the code). Comparing two
        // templates through the same hasher would not see a hash that moved
        // for everyone. It moved once on purpose, when the three lamps were
        // removed (2026-11-12) and every thumbnail had to be rendered again;
        // their values, still in old presets, are read and ignored.
        assert_eq!(
            entry_stem("v46-abc", &old),
            "v46-abc-t685ef6a5",
            "le nom de ses vignettes, calculé hors du code"
        );
    }

    /// GRILLE§7.1 — protected cars are counted **by car**, under the presets
    /// in use: a car still missing from one preset is to do, a car the game
    /// rendered everywhere it was missing is done, and a car the Pit Box
    /// renderer handled is no protected car at all.
    #[test]
    fn protected_cars_are_counted_by_car() {
        let dir = crate::testutil::temp_dir("gridthumbs-protected");
        let protected = crate::errors::PREVIEW_PROTECTED;
        let cars = vec![
            vec!["v1-0a-t01".to_string(), "v1-0a-t02".to_string()],
            vec!["v1-0b-t01".to_string(), "v1-0b-t02".to_string()],
            vec!["v1-0c-t01".to_string(), "v1-0c-t02".to_string()],
            vec!["v1-0d-t01".to_string(), "v1-0d-t02".to_string()],
            vec!["v1-0e-t01".to_string(), "v1-0e-t02".to_string()],
            vec!["v1-0f-t01".to_string(), "v1-0f-t02".to_string()],
        ];
        // a: protected under both presets.
        std::fs::write(dir.join("v1-0a-t01.fail"), protected).unwrap();
        std::fs::write(dir.join("v1-0a-t02.fail"), protected).unwrap();
        // b: rendered by the game under both.
        for e in ["v1-0b-t01", "v1-0b-t02"] {
            write_from_game(&dir, e, b"png").unwrap();
        }
        // c: rendered by the game under one, still protected under the other.
        write_from_game(&dir, "v1-0c-t01", b"png").unwrap();
        std::fs::write(dir.join("v1-0c-t02.fail"), protected).unwrap();
        // d: rendered by Pit Box.
        write_in(&dir, "v1-0d-t01", b"png").unwrap();
        // e: failed for another reason than protection.
        std::fs::write(dir.join("v1-0e-t01.fail"), "decode failed").unwrap();
        // f: rendered by the game under one, never tried under the other — a
        // preset added after the run in the game.
        write_from_game(&dir, "v1-0f-t01", b"png").unwrap();

        assert_eq!(
            summarize(&dir, &cars),
            ProtectedSummary {
                pending: 3,
                rendered: 1
            },
            "a, c et f restent à faire, b est fait, d et e ne sont pas protégées"
        );

        // What each view lacks: the Pit Box renderer's work and the game's
        // apart. d misses t02 (never tried), e failed for good under t01.
        assert_eq!(
            count_coverage(
                &dir,
                &cars.iter().map(|e| (false, e.clone())).collect::<Vec<_>>(),
                &[false, false]
            ),
            vec![
                Coverage {
                    missing: 0,
                    protected_missing: 1
                },
                Coverage {
                    missing: 2,
                    protected_missing: 3
                },
            ],
            "t01 : a ; t02 : a, c, f pour le jeu, d et e pour Pit Box"
        );
        // A view that leaves base content as it is needs nothing of it.
        assert_eq!(
            count_coverage(&dir, &[(true, cars[3].clone())], &[true, true]),
            vec![Coverage::default(); 2],
            "une voiture de base sous un preset qui la laisse telle quelle ne manque à rien"
        );
    }

    /// GRILLE§7.1 — an image from the game replaces the failure mark and leaves
    /// its own: the car stays known as protected once rendered.
    #[test]
    fn a_game_render_replaces_the_failure_and_says_where_it_comes_from() {
        let dir = crate::testutil::temp_dir("gridthumbs-game-mark");
        std::fs::write(dir.join("v1-0a-t01.fail"), crate::errors::PREVIEW_PROTECTED).unwrap();
        write_from_game(&dir, "v1-0a-t01", b"png").unwrap();
        assert!(dir.join("v1-0a-t01.png").is_file(), "l'image est rangée");
        assert!(!dir.join("v1-0a-t01.fail").exists(), "le marqueur d'échec disparaît");
        assert!(dir.join("v1-0a-t01.game").is_file(), "la marque du jeu le remplace");
    }

    /// GRILLE§5.7 — the car own fingerprint stays in the name, so an updated mod
    /// regenerates without any invalidation pass.
    #[test]
    fn entry_name_keeps_the_car_fingerprint() {
        let base = template();
        assert_ne!(
            entry_stem("v46-abc", &base),
            entry_stem("v46-def", &base),
            "deux voitures différentes ne partagent pas leur vignette"
        );
        assert!(
            entry_stem("v46-abc", &base).starts_with("v46-abc"),
            "le nom d'entrée de la voiture reste le préfixe : c'est lui que le balayage de version reconnaît"
        );
    }
}
