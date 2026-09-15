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
    /// Key light intensity, in percent.
    pub key: i32,
    /// Fill light, in percent of the key.
    pub fill: i32,
    /// Rim light, in percent of the key.
    pub rim: i32,
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
    fn fingerprint(&self) -> String {
        let mut hasher = Sha256::new();
        for value in [
            self.azimuth,
            self.elevation,
            self.fov,
            self.margin,
            self.key,
            self.fill,
            self.rim,
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
        format!("{:x}", hasher.finalize())[..8].to_string()
    }
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
    GridThumb {
        stem: stem.to_string(),
        path: png.filter(|p| p.is_file()).map(|p| p.to_string_lossy().into_owned()),
        failed,
    }
}

/// Stores the PNG the frontend just rendered, and returns its path.
///
/// Written under a temporary name then renamed: a thumbnail truncated by a
/// brutal shutdown would be served forever afterwards, since its name is what
/// says it exists.
pub fn write(app: &tauri::AppHandle, stem: &str, png: &[u8]) -> Result<PathBuf, String> {
    if !is_entry_stem(stem) {
        return Err(format!("nom de vignette refusé : {stem}"));
    }
    let dir = dir(app)?;
    let file = dir.join(format!("{stem}.png"));
    let tmp = dir.join(format!("{stem}.{}.tmp", std::process::id()));
    std::fs::write(&tmp, png).map_err(|e| format!("{} : {e}", tmp.display()))?;
    std::fs::rename(&tmp, &file).map_err(|e| format!("{} : {e}", file.display()))?;
    // A car that used to fail and now renders must stop being skipped.
    let _ = std::fs::remove_file(dir.join(format!("{stem}.fail")));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn template() -> GridTemplate {
        GridTemplate {
            azimuth: 318,
            elevation: 8,
            fov: 22,
            margin: 6,
            key: 100,
            fill: 25,
            rim: 60,
            shadow: 35,
            height: 0,
            floor: 0,
            reflection: 0,
            reflection_blur: 5,
            background: 0,
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
        let mut variants = vec![base.clone(); 14];
        variants[0].azimuth += 1;
        variants[1].elevation += 1;
        variants[2].fov += 1;
        variants[3].margin += 1;
        variants[4].key += 1;
        variants[5].fill += 1;
        variants[6].rim += 1;
        variants[7].shadow += 1;
        variants[8].height += 1;
        variants[9].floor += 1;
        variants[10].reflection += 1;
        variants[11].background += 1;
        variants[12].renderer += 1;
        variants[13].reflection_blur += 1;
        for variant in variants {
            assert_ne!(
                name,
                entry_stem("v46-abc", &variant),
                "un gabarit modifié doit donner une autre vignette : {variant:?}"
            );
        }
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
        vitrine.rim = 140;
        let mut gone = template();
        gone.rim = 200;

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
