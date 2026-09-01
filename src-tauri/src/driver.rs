//! Which driver a car seats, and what it wears (SPEC §4.6).
//!
//! Assetto Corsa splits a driver in two, and the split is the whole difficulty
//! of ever offering a list of them:
//!
//! | | Where it lives | Who chooses it |
//! | --- | --- | --- |
//! | **mannequin** (3D) | `<AC>/content/driver/<name>.kn5` | the car, in `driver3d.ini` |
//! | **wardrobe** (textures) | `<AC>/content/texture/driver_{suit,gloves,helmet}/…` | the skin, in `skin.ini` |
//!
//! A `skin.ini` names its wardrobe **under the mannequin's own name**:
//!
//! ```ini
//! [driver_80]                  ; only read when driver3d.ini asked for driver_80
//! SUIT=\plain\red              ; → content/texture/driver_suit/plain/red/
//! GLOVES=\classicpastel\blue_lite
//! HELMET=\helmet_1985\blue
//! ```
//!
//! …and the folder it points at holds `.dds` files named exactly as the
//! mannequin's materials ask for them. **Only the helmet is tied to the
//! mannequin**, and it took a corpus scan to see it: the five Kunos mannequins
//! all ask for `2016_Suit_DIFF.dds` and `2016_Gloves_DIFF.dds`, so every suit
//! folder (53 of them) and every glove folder (69) works on any of them. The
//! helmet does not — `driver`/`driver_no_HANS` ask for `HELMET_2012`,
//! `driver_80` for `HELMET_1985`, `driver_70` for `HELMET_1975`, `driver_60`
//! for `HELMET_1969` — and a folder of the wrong era simply changes nothing.
//!
//! A driver picker therefore offers **three independent lists**, of which only
//! the helmet is filtered by the car's mannequin. Compatibility is decided by
//! file name, not inferred from what other cars happen to declare.
//!
//! **Where the driver sits** is a fourth file: `<car>/driver_base_pos.knh`,
//! the whole rig laid out in the car's own space. `car.ini`'s `[GRAPHICS]
//! DRIVEREYES` — a pair of eyes in the same space — only stands in for it on
//! a car that ships none.
//!
//! `driver3d.ini`'s own `[MODEL] POSITION` **looks** like the offset that
//! completes them and is not: applying it misplaces thirteen cars of the
//! reference install by up to five metres. It is read here and used nowhere;
//! `kn5_gltf`'s `seating_offset` carries the measurement.
//!
//! Everything here is best-effort: a car whose driver cannot be resolved is
//! previewed without one, which is exactly what the preview did before this
//! module existed.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::acd;

/// A resolved driver, in AC's own vocabulary — the shape a future picker will
/// produce, and the reason resolution is split in two halves: reading what the
/// car and the skin declare ([`outfit_of`]) is separate from turning that into
/// files ([`graft_for`]).
#[derive(Debug, Clone, PartialEq)]
pub struct DriverOutfit {
    /// Mannequin name, without extension: `driver`, `driver_80`, `gt-m_pro`…
    pub model: String,
    /// Where the car puts a pair of eyes, `[GRAPHICS] DRIVEREYES` of
    /// `car.ini` — the one line that says where the driver sits (see
    /// `kn5_gltf::DriverGraft::anchor`).
    pub eyes: Option<[f32; 3]>,
    /// `[MODEL] POSITION` of `driver3d.ini`, **read and not used**. Kept
    /// because a future driver picker will want to show what a car declares,
    /// and because leaving it out would invite the next reader to add it back:
    /// see `kn5_gltf`'s `seating_offset` for the thirteen cars it misplaces.
    pub position: [f32; 3],
    /// Total steering travel the car's animation spans, `[STEER_ANIMATION]
    /// LOCK` in degrees. 360 unless the car says otherwise.
    pub lock: f32,
    /// File name of the steering animation, `[STEER_ANIMATION] NAME`, sought
    /// under the car's own `animations/` folder.
    pub animation: String,
    /// Wardrobe paths as `skin.ini` writes them, relative to their kind's
    /// folder: `plain/red`, `helmet_1985/blue`. `None` when the skin says
    /// nothing, in which case the mannequin keeps its own textures.
    pub suit: Option<String>,
    pub gloves: Option<String>,
    pub helmet: Option<String>,
}

/// Folder under `content/texture/` each wardrobe key points into.
const SUIT_DIR: &str = "driver_suit";
const GLOVES_DIR: &str = "driver_gloves";
const HELMET_DIR: &str = "driver_helmet";

/// Section every `driver3d.ini` carries — the known plaintext that says a
/// `data.acd` key is the right one (see [`acd::read_text`]).
const MODEL_SECTION: &str = "[MODEL]";
/// Section naming the steering animation and the travel it spans.
const STEER_SECTION: &str = "[STEER_ANIMATION]";
/// The driver's rig, laid out in the car's own space, at the root of the car
/// folder. Not named by any ini — AC looks for it under this name, and all 312
/// cars of the reference install ship one.
const BASE_POSE: &str = "driver_base_pos.knh";
/// What `[STEER_ANIMATION] NAME` reads on all 298 cars that ship one — used
/// only when the car does not name it.
const DEFAULT_STEER_ANIMATION: &str = "steer.ksanim";
/// Travel assumed when the car does not say — what 271 of the 312 cars of the
/// reference install declare anyway.
const DEFAULT_LOCK: f32 = 360.0;
/// Same role for `car.ini`, whose `[GRAPHICS]` section carries `DRIVEREYES`.
const GRAPHICS_SECTION: &str = "[GRAPHICS]";

/// The driver AC would seat in this car, ready to graft.
///
/// `None` — never an error — when the car names no driver, when the mannequin
/// is not installed, or when Assetto Corsa itself is not configured: a preview
/// without a driver is the normal outcome in all three cases.
pub fn resolve(
    ac_root: &Path,
    car_dir: &Path,
    car_id: &str,
    skin_dir: Option<&Path>,
    steer_degrees: f32,
    chosen: &OutfitOverride,
) -> Option<kn5_gltf::DriverGraft> {
    let mut outfit = outfit_of(car_dir, car_id, skin_dir)?;
    chosen.apply(&mut outfit);
    graft_for(ac_root, car_dir, &outfit, steer_degrees)
}

/// Ce que le frontend demande pour le pilote de l'aperçu : l'angle du volant,
/// et la tenue qu'il impose éventuellement.
///
/// Un seul objet plutôt que trois paramètres : la commande d'aperçu en portait
/// déjà quatre, et « pas de pilote » se dit alors par l'absence de l'objet
/// entier plutôt que par une combinaison de `None`.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriverView {
    /// Degrés, 0 = volant droit.
    #[serde(default)]
    pub steer: f32,
    #[serde(default, flatten)]
    pub outfit: OutfitOverride,
}

/// Ce que l'utilisateur impose par-dessus ce que la voiture et son skin
/// déclarent.
///
/// Une pièce à `None` laisse celle du skin. Le **mannequin, lui, n'a pas le
/// même statut que les trois autres** et c'est l'asymétrie qui structure tout
/// l'écran Pilote (`docs/SPEC-ecran-pilote.md` §1.3) : la tenue ne tient qu'au
/// `skin.ini`, un fichier de skin, alors que le mannequin est nommé par
/// `driver3d.ini`, donc par le `data.acd` que le serveur de course vérifie.
/// Le substituer ne vaut **que dans l'aperçu** — d'où le bandeau permanent que
/// l'écran affiche tant que dure ce mode (§10.1).
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutfitOverride {
    /// Mannequin substitué à celui de la voiture, sans extension. `None` =
    /// celui que `driver3d.ini` nomme.
    pub model: Option<String>,
    pub suit: Option<String>,
    pub gloves: Option<String>,
    pub helmet: Option<String>,
}

impl OutfitOverride {
    fn apply(&self, outfit: &mut DriverOutfit) {
        // Corps substitué : la garde-robe du skin tombe avec lui (§10.1).
        // Elle est lue sous le nom de l'ancien mannequin — la section
        // `[driver_80]` d'un `skin.ini` ne dit rien du `driver_60` qu'on vient
        // de mettre à sa place, et la lui appliquer quand même reviendrait à
        // croire que deux mannequins nomment leurs textures pareil, ce qui est
        // justement faux pour les casques.
        if let Some(model) = self.model.as_deref().filter(|m| !m.is_empty() && *m != outfit.model) {
            outfit.model = model.to_string();
            outfit.suit = None;
            outfit.gloves = None;
            outfit.helmet = None;
        }
        // Puis ce qui est demandé seulement : une pièce non choisie garde
        // celle du skin, elle ne devient pas nue.
        for (chosen, target) in [
            (&self.suit, &mut outfit.suit),
            (&self.gloves, &mut outfit.gloves),
            (&self.helmet, &mut outfit.helmet),
        ] {
            if let Some(value) = chosen.as_ref().filter(|v| !v.is_empty()) {
                *target = Some(value.clone());
            }
        }
    }
}

/// Reads what the car and its skin declare, without touching the AC install.
pub fn outfit_of(car_dir: &Path, car_id: &str, skin_dir: Option<&Path>) -> Option<DriverOutfit> {
    let ini = driver3d_ini(car_dir, car_id)?;
    let model = ini_value(&ini, MODEL_SECTION, "NAME")?.to_string();
    if model.is_empty() {
        return None;
    }
    let position = ini_value(&ini, MODEL_SECTION, "POSITION")
        .and_then(parse_position)
        .unwrap_or([0.0; 3]);
    let lock = ini_value(&ini, STEER_SECTION, "LOCK")
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|v| *v > 0.0)
        .unwrap_or(DEFAULT_LOCK);
    // A bare file name and nothing else: the value comes out of a mod's own
    // file, and `animations/..\..\something` has no business being opened.
    let animation = ini_value(&ini, STEER_SECTION, "NAME")
        .map(str::trim)
        .filter(|name| !name.is_empty() && !name.contains(['\\', '/', ':']) && *name != "..")
        .unwrap_or(DEFAULT_STEER_ANIMATION)
        .to_string();

    // The skin's wardrobe is read under the mannequin's name: a `skin.ini`
    // written for `driver_80` says nothing about the `driver` a CSP config may
    // have substituted, and applying it anyway would dress the wrong body.
    let wardrobe = skin_dir
        .map(|dir| dir.join("skin.ini"))
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default();
    let section = format!("[{model}]");

    Some(DriverOutfit {
        suit: wardrobe_path(&wardrobe, &section, "SUIT"),
        gloves: wardrobe_path(&wardrobe, &section, "GLOVES"),
        helmet: wardrobe_path(&wardrobe, &section, "HELMET"),
        eyes: car_ini(car_dir, car_id)
            .as_deref()
            .and_then(|text| ini_value(text, GRAPHICS_SECTION, "DRIVEREYES"))
            .and_then(parse_position),
        model,
        position,
        lock,
        animation,
    })
}

/// Turns a resolved outfit into the files the converter needs.
///
/// The wardrobe folders come **before** nothing else: they are the only
/// sources of override the graft knows about. The car skin's own loose `.dds`
/// — some mods drop `2016_Helmet_Base_D.dds` straight into the skin folder —
/// are handled a layer further down, by the texture loader, which already
/// prefers a skin file over an embedded blob for every texture in the model.
///
/// The steering animation and the rig layout come from the **car**, not the AC
/// root: they are the two pieces of a driver a car keeps to itself, because
/// both were authored for that car's own cockpit — where the seat is, and how
/// the arms reach its steering wheel.
pub fn graft_for(
    ac_root: &Path,
    car_dir: &Path,
    outfit: &DriverOutfit,
    steer_degrees: f32,
) -> Option<kn5_gltf::DriverGraft> {
    let model = body_file(ac_root, &outfit.model);
    if !model.is_file() {
        // Common enough to not deserve a warning at every preview: a mod car
        // may ask for a mannequin its author shipped separately, or not at all.
        log::debug!("driver: mannequin {} not installed", model.display());
        return None;
    }

    let textures = ac_root.join("content").join("texture");
    let dirs = [
        (HELMET_DIR, outfit.helmet.as_deref()),
        (GLOVES_DIR, outfit.gloves.as_deref()),
        (SUIT_DIR, outfit.suit.as_deref()),
    ];
    let texture_dirs = dirs
        .iter()
        .filter_map(|(kind, wanted)| wardrobe_dir(&textures.join(kind), (*wanted)?))
        .collect();

    let animation = car_dir.join("animations").join(&outfit.animation);
    let base_pose = car_dir.join(BASE_POSE);

    Some(kn5_gltf::DriverGraft {
        model,
        anchor: outfit.eyes,
        texture_dirs,
        base_pose: base_pose.is_file().then_some(base_pose),
        animation: animation.is_file().then_some(animation),
        lock_degrees: outfit.lock,
        steer_degrees,
    })
}

/// Le mannequin seul, habillé comme l'écran Pilote le demande — pas de
/// voiture autour (`docs/SPEC-ecran-pilote.md` §5.1).
///
/// Même résolution que [`resolve`] jusqu'à la garde-robe, puis on retire les
/// trois choses qui parlent de la voiture : l'assise, la pose des bras et
/// l'ancrage. Elles ont un sens dans un habitacle et aucun sur un plateau, où
/// le pilote est seul devant un volant générique — c'est la pose de repos du
/// mannequin qui sert, et elle tient déjà les mains à un volant sur 41 des 44
/// mannequins de l'installation de référence (voir `kn5_gltf::DriverRig`).
///
/// La voiture reste dans la boucle malgré tout : c'est son `driver3d.ini` qui
/// nomme le corps, et le `skin.ini` de sa livrée qui fournit les pièces que
/// l'utilisateur n'a pas choisies.
pub fn standalone(
    ac_root: &Path,
    car_dir: &Path,
    car_id: &str,
    skin_dir: Option<&Path>,
    chosen: &OutfitOverride,
) -> Option<kn5_gltf::DriverGraft> {
    let mut outfit = outfit_of(car_dir, car_id, skin_dir)?;
    chosen.apply(&mut outfit);
    let mut graft = graft_for(ac_root, car_dir, &outfit, 0.0)?;
    graft.anchor = None;
    graft.base_pose = None;
    graft.animation = None;
    Some(graft)
}

/// Joins a `skin.ini` wardrobe path onto its kind's folder, refusing anything
/// that would leave it.
///
/// The value comes out of a mod's own file, so `..` and drive letters are
/// treated as what they would be: an attempt to read outside `content/texture`.
fn wardrobe_dir(kind_dir: &Path, wanted: &str) -> Option<PathBuf> {
    let mut path = kind_dir.to_path_buf();
    for part in wanted.split(['\\', '/']).filter(|p| !p.is_empty()) {
        if part == "." || part == ".." || part.contains(':') {
            log::warn!("driver: wardrobe path `{wanted}` refused");
            return None;
        }
        path.push(part);
    }
    if path == kind_dir {
        return None;
    }
    path.is_dir().then_some(path)
}

fn driver3d_ini(car_dir: &Path, car_id: &str) -> Option<String> {
    data_file(car_dir, car_id, "driver3d.ini", MODEL_SECTION)
}

fn car_ini(car_dir: &Path, car_id: &str) -> Option<String> {
    data_file(car_dir, car_id, "car.ini", GRAPHICS_SECTION)
}

/// One of a car's physics files, from the unpacked `data/` folder or from
/// `data.acd`.
///
/// Unpacked first: a mod that ships both has edited the loose one, and it is
/// what AC itself reads.
fn data_file(car_dir: &Path, car_id: &str, name: &str, marker: &str) -> Option<String> {
    let loose = car_dir.join("data").join(name);
    match std::fs::read_to_string(&loose) {
        Ok(text) => return Some(text),
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            log::warn!("driver: {} unreadable — {e}", loose.display());
        }
        Err(_) => {}
    }
    acd::read_text(car_dir, car_id, name, marker)
}

/// Reads one wardrobe key, `None` when it is absent or empty.
fn wardrobe_path(text: &str, section: &str, key: &str) -> Option<String> {
    ini_value(text, section, key)
        .map(|v| v.trim_matches(['\\', '/']).to_string())
        .filter(|v| !v.is_empty())
}

/// `KEY=value` inside a named section, comments stripped. Section names are
/// compared case-insensitively — `[DRIVER_80]` and `[driver_80]` both occur.
fn ini_value<'a>(text: &'a str, section: &str, key: &str) -> Option<&'a str> {
    let mut inside = false;
    for line in text.lines() {
        let line = line.split(';').next().unwrap_or("").trim();
        if line.starts_with('[') {
            inside = line.eq_ignore_ascii_case(section);
            continue;
        }
        if !inside {
            continue;
        }
        if let Some((name, value)) = line.split_once('=') {
            if name.trim().eq_ignore_ascii_case(key) {
                return Some(value.trim());
            }
        }
    }
    None
}

/// `POSITION=x,y,z`, in metres. A malformed one is dropped rather than
/// half-read: a driver an axis off is worse than a driver at the origin.
fn parse_position(value: &str) -> Option<[f32; 3]> {
    let mut out = [0.0f32; 3];
    let mut parts = value.split(',');
    for slot in &mut out {
        *slot = parts.next()?.trim().parse().ok()?;
    }
    parts.next().is_none().then_some(out)
}

// --- Ce qu'on peut proposer à l'utilisateur (§4.6ter) ------------------------

/// Un dossier de garde-robe, tel qu'il s'offre au choix.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WardrobeOption {
    /// Valeur telle que `skin.ini` l'écrit : `plain/red`.
    pub id: String,
    /// Ce qu'on affiche. Les noms de dossier AC ne se traduisent pas.
    pub label: String,
    /// La vignette qu'AC range à côté des `.dds`, quand il y en a une —
    /// 173 des 176 dossiers de casque en ont, d'où l'intérêt d'un menu
    /// illustré plutôt qu'une liste de noms.
    pub thumbnail: Option<String>,
}

/// Ce qu'une voiture donnée permet de choisir.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriverChoices {
    /// Le mannequin sur lequel ces listes ont été calculées : celui de la
    /// voiture, ou celui que l'utilisateur lui a substitué.
    pub model: String,
    /// `true` quand ce mannequin n'est pas celui que la voiture nomme — le
    /// mode « corps substitué » de l'écran Pilote (§10), qui ne vaut que dans
    /// l'aperçu.
    pub substituted: bool,
    /// Époque de la boîte à casques du mannequin, clé de la table [`ERAS`].
    /// `None` = mannequin qui nomme ses images autrement, donc aucun casque du
    /// jeu ne s'y pose (§11.1).
    pub era: Option<&'static str>,
    pub suits: Vec<WardrobeOption>,
    pub gloves: Vec<WardrobeOption>,
    pub helmets: Vec<WardrobeOption>,
}

/// Les tenues qui marcheront réellement sur le mannequin de cette voiture.
///
/// **Une seule règle de compatibilité, et elle couvre les trois listes** : un
/// dossier est retenu s'il contient un fichier que le mannequin utilise comme
/// `txDiffuse`. Mesuré sur le parc, ça donne exactement ce qu'il faut :
///
/// - les combinaisons et les gants passent partout, parce que les cinq
///   mannequins Kunos réclament tous `2016_Suit_DIFF.dds` et
///   `2016_Gloves_DIFF.dds` — 53 et 67 dossiers, universels ;
/// - les casques se filtrent tout seuls par époque, `HELMET_2012` contre
///   `HELMET_1985`, `HELMET_1975`, `HELMET_1969` — 176 dossiers, dont 100
///   pour les voitures modernes ;
/// - les dossiers `_nm`, qui ne portent que des cartes de normales partagées,
///   tombent d'eux-mêmes : une normale n'est pas une `txDiffuse`, donc ce
///   n'est pas un choix de tenue.
///
/// Aucune liste codée en dur, donc, et un mannequin de mod inconnu est traité
/// comme les autres.
pub fn choices(ac_root: &Path, car_dir: &Path, car_id: &str, body: Option<&str>) -> Option<DriverChoices> {
    let declared = outfit_of(car_dir, car_id, None)?.model;
    // Le corps substitué commande les trois listes (§1.3) : c'est lui qui
    // porte les noms de texture, donc lui qui décide de ce qui s'y pose.
    let model = body
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .unwrap_or(&declared)
        .to_string();
    let diffuse = diffuse_textures(&body_file(ac_root, &model))?;

    let textures = ac_root.join("content").join("texture");
    Some(DriverChoices {
        suits: wardrobe_options(&textures.join(SUIT_DIR), &diffuse),
        gloves: wardrobe_options(&textures.join(GLOVES_DIR), &diffuse),
        helmets: wardrobe_options(&textures.join(HELMET_DIR), &diffuse),
        era: era_of(&diffuse),
        substituted: model != declared,
        model,
    })
}

/// Le `.kn5` d'un mannequin dans l'installation d'AC.
fn body_file(ac_root: &Path, model: &str) -> PathBuf {
    ac_root.join("content").join("driver").join(format!("{model}.kn5"))
}

/// Noms des textures que le mannequin échantillonne comme couleur de base.
///
/// Lit le KN5 pour de bon : le parsing coûte deux millisecondes, c'est le
/// transcodage des textures qui est cher et on ne le fait pas ici.
fn diffuse_textures(mannequin: &Path) -> Option<BTreeSet<String>> {
    let bytes = std::fs::read(mannequin)
        .inspect_err(|e| log::warn!("driver: {} illisible — {e}", mannequin.display()))
        .ok()?;
    let model = kn5::parse(&bytes)
        .inspect_err(|e| log::warn!("driver: {} illisible — {e}", mannequin.display()))
        .ok()?;
    let names: BTreeSet<String> = model
        .materials
        .iter()
        .filter_map(|m| m.texture_for("txDiffuse"))
        .filter(|name| !name.is_empty())
        .map(|name| name.to_lowercase())
        .collect();
    (!names.is_empty()).then_some(names)
}

/// Parcourt un dossier de garde-robe et retient les feuilles utilisables.
fn wardrobe_options(kind_dir: &Path, diffuse: &BTreeSet<String>) -> Vec<WardrobeOption> {
    let mut out = Vec::new();
    collect_wardrobe(kind_dir, kind_dir, diffuse, 0, &mut out);
    out.sort_by_key(|option| option.id.to_lowercase());
    out
}

/// Profondeur maximale explorée sous un dossier de garde-robe. AC range en
/// `<famille>/<couleur>` ; une de plus laisse la place à un mod fantaisiste,
/// sans transformer le scan en parcours de disque.
const WARDROBE_DEPTH: usize = 3;

fn collect_wardrobe(root: &Path, dir: &Path, diffuse: &BTreeSet<String>, depth: usize, out: &mut Vec<WardrobeOption>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<PathBuf> = Vec::new();
    let mut folders: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => folders.push(path),
            Ok(_) => files.push(path),
            Err(_) => {}
        }
    }

    let matches = |path: &Path| {
        path.file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .is_some_and(|n| diffuse.contains(&n))
    };
    if files.iter().any(|f| matches(f)) {
        if let Ok(rel) = dir.strip_prefix(root) {
            let id = rel.to_string_lossy().replace('\\', "/");
            out.push(WardrobeOption {
                label: id.replace('/', " · "),
                thumbnail: thumbnail_of(&files, &matches).map(|p| p.to_string_lossy().into_owned()),
                id,
            });
        }
    }
    if depth < WARDROBE_DEPTH {
        for folder in folders {
            collect_wardrobe(root, &folder, diffuse, depth + 1, out);
        }
    }
}

/// La vignette d'un dossier : l'image qui porte le nom d'une de ses textures
/// utiles, à défaut n'importe laquelle.
fn thumbnail_of(files: &[PathBuf], matches: &dyn Fn(&Path) -> bool) -> Option<PathBuf> {
    let is_image = |p: &PathBuf| {
        p.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("png"))
    };
    let stems: BTreeSet<String> = files
        .iter()
        .filter(|p| matches(p))
        .filter_map(|p| p.file_stem())
        .map(|s| s.to_string_lossy().to_lowercase())
        .collect();
    files
        .iter()
        .filter(|p| is_image(p))
        .find(|p| {
            p.file_stem()
                .map(|s| s.to_string_lossy().to_lowercase())
                .is_some_and(|s| stems.contains(&s))
        })
        .or_else(|| files.iter().find(|p| is_image(p)))
        .cloned()
}

// --- Les corps installés (§9) -----------------------------------------------

/// Époque d'un mannequin, lue sur le nom de la texture de casque qu'il
/// échantillonne, et clé i18n du libellé que l'écran en affiche.
///
/// **Table maintenue en code, indexée sur le préfixe** (§6.3) : c'est une
/// convention de nommage Kunos, pas une donnée du format, et un mannequin de
/// mod qui nomme ses images autrement tombe simplement en `None` — sans
/// erreur, et l'écran le dit en toutes lettres plutôt que de proposer un choix
/// sans effet (§11.1). Mesuré sur les 52 mannequins de l'installation de
/// référence : les quatre préfixes ci-dessous couvrent tous ceux dont un
/// casque du jeu peut changer l'apparence, les autres (`RSS_Helmet`,
/// `HELMET_HR2`, `helmet_2019`, `2016_Suit_DIFFc` de `yk2_kana`) portent leur
/// casque avec eux.
const ERAS: [(&str, &str); 4] = [
    ("helmet_2012", "modern"),
    ("helmet_1985", "1980s"),
    ("helmet_1975", "1970s"),
    ("helmet_1969", "1960s"),
];

fn era_of(diffuse: &BTreeSet<String>) -> Option<&'static str> {
    ERAS.iter()
        .find(|(prefix, _)| diffuse.iter().any(|name| name.starts_with(prefix)))
        .map(|(_, era)| *era)
}

/// Un mannequin installé, tel qu'il s'offre au choix (§9.1).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyOption {
    /// Nom de fichier sans extension, tel que `driver3d.ini` l'écrirait :
    /// `driver_60`, `gt-m_pro`. Ne se traduit pas.
    pub id: String,
    /// Clé de la table [`ERAS`], ou `None` — un mannequin sur lequel aucun
    /// casque du jeu ne se pose.
    pub era: Option<&'static str>,
}

/// Les mannequins qu'on peut proposer, triés par nom.
///
/// **Un corps qu'on ne peut pas prendre n'a pas à être montré** (§9.3) : les
/// illisibles et ceux sans squelette sont écartés en silence. Le critère est
/// mesuré, pas supposé — un mannequin sans *skinned mesh* n'a pas de rig, donc
/// ni le `driver_base_pos.knh` de la voiture ni son `steer.ksanim` n'ont prise
/// sur lui, et il s'afficherait dans sa pose de repos au travers de
/// l'habitacle. Sur l'installation de référence ça écarte exactement sept
/// fichiers sur 52 : les six variantes de LOD B, qui sont des copies rigides
/// des mannequins qu'elles doublent, et une blague (`cheems.kn5`, un chien).
///
/// Le coût est celui d'un parcours complet du dossier — **0,3 s pour les 52
/// mannequins de l'installation de référence**, soit 800 Mo lus et parsés, ce
/// qui surprend jusqu'à ce qu'on se rappelle que le gros d'un KN5 est en
/// textures et qu'on ne les décode pas ici. Pas de cache disque, donc : ce
/// serait un fichier de plus à invalider pour économiser un tiers de seconde
/// sur un écran qu'on ouvre rarement.
pub fn bodies(ac_root: &Path) -> Vec<BodyOption> {
    let dir = ac_root.join("content").join("driver");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        log::warn!("driver: {} illisible", dir.display());
        return Vec::new();
    };
    let mut out: Vec<BodyOption> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e.eq_ignore_ascii_case("kn5")))
        .filter_map(|path| {
            let id = path.file_stem()?.to_string_lossy().into_owned();
            let bytes = std::fs::read(&path)
                .inspect_err(|e| log::warn!("driver: {} illisible — {e}", path.display()))
                .ok()?;
            let model = kn5::parse(&bytes)
                .inspect_err(|e| log::debug!("driver: {id} écarté, illisible — {e}"))
                .ok()?;
            if !has_skeleton(&model) {
                log::debug!("driver: {id} écarté, aucun squelette");
                return None;
            }
            let diffuse: BTreeSet<String> = model
                .materials
                .iter()
                .filter_map(|m| m.texture_for("txDiffuse"))
                .map(str::to_lowercase)
                .collect();
            Some(BodyOption {
                era: era_of(&diffuse),
                id,
            })
        })
        .collect();
    out.sort_by_key(|body| body.id.to_lowercase());
    out
}

/// Un rig que la pose de la voiture et son animation de volant peuvent bouger.
fn has_skeleton(model: &kn5::Kn5Model) -> bool {
    let mut found = false;
    model.visit_nodes(&mut |node| {
        if matches!(node.kind, kn5::Kn5NodeKind::SkinnedMesh(_)) {
            found = true;
        }
    });
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAR_INI: &str = "\
[BASIC]
TOTALMASS=1050

[GRAPHICS]
DRIVEREYES=0.330737,1.19075,-0.490002
ONBOARD_EXPOSURE=20
";

    const DRIVER3D: &str = "\
[MODEL]
NAME=driver_80
POSITION=-0.01, 0.02, 0.03

[STEER_ANIMATION]
NAME=steer.ksanim

[HIDE_OBJECT_0]
NAME=DRIVER:HELMET1985
";

    const SKIN_INI: &str = "\
[driver_80]
SUIT=\\plain\\red
GLOVES=\\classicpastel\\blue_lite
HELMET=\\helmet_1985\\blue

[CREW]
SUIT=\\type1\\black_black
";

    /// A car folder with a loose `data/driver3d.ini` and one skin.
    fn fake_car(base: &Path, skin_ini: Option<&str>) -> (PathBuf, Option<PathBuf>) {
        let car = base.join("ks_fake");
        std::fs::create_dir_all(car.join("data")).expect("car data folder");
        std::fs::write(car.join("data").join("driver3d.ini"), DRIVER3D).expect("driver3d.ini");
        std::fs::write(car.join("data").join("car.ini"), CAR_INI).expect("car.ini");
        let skin = skin_ini.map(|text| {
            let dir = car.join("skins").join("red");
            std::fs::create_dir_all(&dir).expect("skin folder");
            std::fs::write(dir.join("skin.ini"), text).expect("skin.ini");
            dir
        });
        (car, skin)
    }

    // Rule: the mannequin comes from the car's driver3d.ini, the wardrobe from
    // the skin's skin.ini — and the wardrobe is read under the mannequin's own
    // name (§4.6).
    #[test]
    fn the_car_names_the_mannequin_and_the_skin_dresses_it() {
        let base = crate::testutil::temp_dir("driver-outfit");
        let (car, skin) = fake_car(&base, Some(SKIN_INI));

        let outfit = outfit_of(&car, "ks_fake", skin.as_deref()).expect("an outfit");

        assert_eq!(outfit.model, "driver_80", "mannequin read from [MODEL] NAME");
        assert_eq!(outfit.position, [-0.01, 0.02, 0.03], "POSITION read as three metres");
        assert_eq!(
            outfit.eyes,
            Some([0.330737, 1.19075, -0.490002]),
            "DRIVEREYES read from car.ini — the one line that seats the mannequin"
        );
        assert_eq!(outfit.suit.as_deref(), Some("plain\\red"), "leading separator stripped");
        assert_eq!(outfit.gloves.as_deref(), Some("classicpastel\\blue_lite"));
        assert_eq!(outfit.helmet.as_deref(), Some("helmet_1985\\blue"));
    }

    // Rule: a `skin.ini` written for another mannequin dresses nobody — its
    // file names would not match the materials of the one actually loaded.
    #[test]
    fn a_wardrobe_written_for_another_mannequin_is_ignored() {
        let base = crate::testutil::temp_dir("driver-other");
        let (car, skin) = fake_car(&base, Some("[driver]\nSUIT=\\sparco\\red\n"));

        let outfit = outfit_of(&car, "ks_fake", skin.as_deref()).expect("an outfit");

        assert_eq!(outfit.model, "driver_80", "the car still names its mannequin");
        assert_eq!(outfit.suit, None, "the [driver] section is not ours");
    }

    // Rule: no skin, or a skin without `skin.ini`, still yields a driver — he
    // simply wears what the mannequin was shipped with.
    #[test]
    fn a_skin_without_a_wardrobe_still_yields_a_driver() {
        let base = crate::testutil::temp_dir("driver-bare");
        let (car, _) = fake_car(&base, None);

        let outfit = outfit_of(&car, "ks_fake", None).expect("an outfit");

        assert_eq!(outfit.model, "driver_80");
        assert!(
            outfit.suit.is_none() && outfit.gloves.is_none() && outfit.helmet.is_none(),
            "nothing to dress it with"
        );
    }

    // Rule: a wardrobe path never leaves `content/texture/<kind>` — the value
    // comes out of a mod's own file.
    #[test]
    fn a_wardrobe_path_cannot_escape_its_folder() {
        let base = crate::testutil::temp_dir("driver-escape");
        let kind = base.join("driver_suit");
        let inside = kind.join("plain").join("red");
        std::fs::create_dir_all(&inside).expect("wardrobe folder");

        assert_eq!(wardrobe_dir(&kind, "plain\\red"), Some(inside), "an ordinary path");
        assert_eq!(wardrobe_dir(&kind, "..\\..\\windows"), None, "climbing out is refused");
        assert_eq!(wardrobe_dir(&kind, "C:\\windows"), None, "an absolute path is refused");
        assert_eq!(wardrobe_dir(&kind, "plain\\green"), None, "a folder that is not there");
    }

    // Rule: a malformed POSITION is dropped whole, never half-read.
    #[test]
    fn a_malformed_position_falls_back_to_the_origin() {
        assert_eq!(parse_position("0, 0, 0"), Some([0.0; 3]));
        assert_eq!(parse_position("-0.0,0.1,0.2"), Some([-0.0, 0.1, 0.2]));
        assert_eq!(parse_position("0, 0"), None, "two numbers are not a position");
        assert_eq!(parse_position("0, 0, 0, 0"), None, "nor are four");
        assert_eq!(parse_position("0, x, 0"), None, "nor is a word");
    }

    /// Règle §10.1 : substituer le corps supprime la référence « livrée ».
    ///
    /// La garde-robe du `skin.ini` est écrite sous le nom de l'ancien
    /// mannequin ; la garder reviendrait à habiller le nouveau avec des
    /// fichiers qui ne le concernent pas — ce que l'écran annonce d'ailleurs
    /// en toutes lettres avant de le faire (bannière d'invalidation, §10.2).
    #[test]
    fn a_substituted_body_drops_the_wardrobe_of_the_livery() {
        let tmp = crate::testutil::temp_dir("driver_substitute");
        let (car, skin) = fake_car(&tmp, Some(SKIN_INI));
        let mut outfit = outfit_of(&car, "ks_fake", skin.as_deref()).expect("un pilote");
        assert!(outfit.helmet.is_some(), "la livrée habille bien le mannequin déclaré");

        OutfitOverride {
            model: Some("driver_60".into()),
            helmet: Some("helmet_1969/clark".into()),
            ..Default::default()
        }
        .apply(&mut outfit);

        assert_eq!(
            outfit.model, "driver_60",
            "le corps demandé remplace celui de la voiture"
        );
        assert_eq!(
            outfit.helmet.as_deref(),
            Some("helmet_1969/clark"),
            "la pièce choisie, elle, s'applique"
        );
        assert_eq!(
            outfit.suit, None,
            "la combinaison de la livrée n'a plus de destinataire"
        );
        assert_eq!(outfit.gloves, None, "les gants non plus");
    }

    /// Le même corps que celui de la voiture n'est pas une substitution : la
    /// livrée reste la référence, et ses pièces avec elle.
    #[test]
    fn asking_for_the_car_own_body_changes_nothing() {
        let tmp = crate::testutil::temp_dir("driver_same_body");
        let (car, skin) = fake_car(&tmp, Some(SKIN_INI));
        let mut outfit = outfit_of(&car, "ks_fake", skin.as_deref()).expect("un pilote");
        let before = outfit.clone();

        OutfitOverride {
            model: Some("driver_80".into()),
            ..Default::default()
        }
        .apply(&mut outfit);

        assert_eq!(outfit, before, "rien ne bouge quand on redemande le corps déclaré");
    }

    /// Règle §6.2 : l'époque se lit sur la texture de casque que le mannequin
    /// échantillonne, et rien d'autre — un mannequin de mod qui nomme ses
    /// images à lui n'a pas d'époque, il n'a pas non plus de casque à proposer.
    #[test]
    fn an_era_is_read_on_the_helmet_texture_the_mannequin_asks_for() {
        let era = |names: &[&str]| era_of(&names.iter().map(|n| n.to_string()).collect());
        assert_eq!(era(&["2016_suit_diff.dds", "helmet_2012.dds"]), Some("modern"));
        assert_eq!(era(&["helmet_1985.dds"]), Some("1980s"));
        assert_eq!(era(&["helmet_1975.dds"]), Some("1970s"));
        assert_eq!(era(&["helmet_1969.dds"]), Some("1960s"));
        assert_eq!(era(&["rss_helmet.dds", "2016_suit_diff.dds"]), None, "un casque de mod");
        assert_eq!(era(&[]), None, "et un mannequin sans texture du tout");
    }

    /// Ce que le `.glb` d'un mannequin garde comme noms : c'est ce qui décide
    /// si le frontend peut retrouver la texture d'une pièce pour l'échanger
    /// lui-même, au lieu de redemander une conversion à chaque survol.
    ///
    /// ```text
    /// PITBOX_AC_ROOT="D:\...\assettocorsa" cargo test --lib driver -- --ignored --nocapture what_the_glb
    /// ```
    #[test]
    #[ignore = "needs a real Assetto Corsa install; measurement, not a check"]
    fn what_the_glb_keeps_of_the_names() {
        let Ok(ac_root) = std::env::var("PITBOX_AC_ROOT") else {
            eprintln!("PITBOX_AC_ROOT unset, skipping");
            return;
        };
        let root = PathBuf::from(ac_root);
        let graft = kn5_gltf::DriverGraft {
            model: body_file(&root, "driver"),
            anchor: None,
            texture_dirs: Vec::new(),
            base_pose: None,
            animation: None,
            lock_degrees: 360.0,
            steer_degrees: 0.0,
        };
        let (model, stats, rig) = kn5_gltf::standalone_driver(&graft).expect("mannequin converti");
        eprintln!("rig: {rig:?}");
        eprintln!("stats: {} triangles, {} habillées", stats.triangles, stats.dressed);

        let conversion =
            kn5_gltf::convert(&model, None, &kn5_gltf::ConvertOptions::default(), &|_| {}).expect("conversion");
        // Chunk JSON d'un GLB : en-tête de 12 octets, puis longueur + type.
        let len = u32::from_le_bytes(conversion.glb[12..16].try_into().unwrap()) as usize;
        let json: serde_json::Value = serde_json::from_slice(&conversion.glb[20..20 + len]).expect("json");
        for key in ["images", "textures", "materials"] {
            let names: Vec<String> = json[key]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|v| v["name"].as_str().unwrap_or("<sans nom>").to_string())
                        .collect()
                })
                .unwrap_or_default();
            eprintln!("{key} ({}) : {}", names.len(), names.join(", "));
        }
    }

    /// Où un mannequin tient ses mains, sa tête et ses pieds dans sa **pose de
    /// repos** — celle qu'il a sans voiture autour de lui, donc celle du
    /// plateau d'essayage (SPEC-ecran-pilote §5.1).
    ///
    /// La question à laquelle ce test répond : peut-on poser un volant
    /// générique à un endroit fixe, ou faut-il le calculer par mannequin ?
    ///
    /// ```text
    /// PITBOX_AC_ROOT="D:\...\assettocorsa" cargo test --lib driver -- --ignored --nocapture where_the_hands
    /// ```
    #[test]
    #[ignore = "needs a real Assetto Corsa install; measurement, not a check"]
    fn where_the_hands_rest() {
        let Ok(ac_root) = std::env::var("PITBOX_AC_ROOT") else {
            eprintln!("PITBOX_AC_ROOT unset, skipping");
            return;
        };
        let root = PathBuf::from(ac_root);
        let wanted = ["RIG_HAND_L", "RIG_HAND_R", "RIG_Head", "RIG_Hips"];
        for body in bodies(&root) {
            let Ok(bytes) = std::fs::read(body_file(&root, &body.id)) else {
                continue;
            };
            let Ok(model) = kn5::parse(&bytes) else { continue };
            let centers = kn5_gltf::node_world_centers(&model);
            let mut line = format!("{:32}", body.id);
            for name in wanted {
                let found = centers
                    .iter()
                    .find(|(n, _)| n.len() >= name.len() && n[n.len() - name.len()..].eq_ignore_ascii_case(name));
                match found {
                    Some((_, c)) => line.push_str(&format!(" {name}[{:+.3} {:+.3} {:+.3}]", c[0], c[1], c[2])),
                    None => line.push_str(&format!(" {name}[—]")),
                }
            }
            eprintln!("{line}");
        }
    }

    /// Les corps que l'écran proposerait sur l'installation de référence, et
    /// ceux qu'il écarte — la mesure qui a fixé le critère de [`bodies`].
    ///
    /// ```text
    /// PITBOX_AC_ROOT="D:\...\assettocorsa" cargo test --lib driver -- --ignored --nocapture which_bodies
    /// ```
    #[test]
    #[ignore = "needs a real Assetto Corsa install; measurement, not a check"]
    fn which_bodies_can_be_offered() {
        let Ok(ac_root) = std::env::var("PITBOX_AC_ROOT") else {
            eprintln!("PITBOX_AC_ROOT unset, skipping");
            return;
        };
        let root = PathBuf::from(ac_root);
        let installed = std::fs::read_dir(root.join("content").join("driver"))
            .expect("read content/driver")
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("kn5")))
            .count();
        let offered = bodies(&root);
        let mut by_era: std::collections::BTreeMap<&str, Vec<&str>> = Default::default();
        for body in &offered {
            by_era.entry(body.era.unwrap_or("—")).or_default().push(&body.id);
        }
        for (era, ids) in &by_era {
            eprintln!("{era:>8} : {:3} — {}", ids.len(), ids.join(", "));
        }
        eprintln!(
            "
=== {} corps proposés sur {installed} installés ===",
            offered.len()
        );
    }

    /// Ce que chaque mannequin de l'install peut porter, par la règle de
    /// [`choices`] — la mesure qui a servi à la fixer.
    ///
    /// Attendu, mesuré indépendamment avant d'écrire le code : 53 combinaisons
    /// et 69 paires de gants pour **tous** les mannequins Kunos, et des
    /// casques filtrés par époque — 100 en 2012, 44 en 1975, 21 en 1969, 11 en
    /// 1985. Un écart ici veut dire que la règle a changé de sens.
    ///
    /// ```text
    /// PITBOX_AC_ROOT="D:\...\assettocorsa" cargo test --lib driver -- --ignored --nocapture what_each
    /// ```
    #[test]
    #[ignore = "needs a real Assetto Corsa install; measurement, not a check"]
    fn what_each_mannequin_can_wear() {
        let Ok(ac_root) = std::env::var("PITBOX_AC_ROOT") else {
            eprintln!("PITBOX_AC_ROOT unset, skipping");
            return;
        };
        let root = PathBuf::from(ac_root);
        let textures = root.join("content").join("texture");
        let mut mannequins: Vec<PathBuf> = std::fs::read_dir(root.join("content").join("driver"))
            .expect("read content/driver")
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("kn5")))
            .collect();
        mannequins.sort();

        eprintln!(
            "
{:28} {:>6} {:>7} {:>8}  vignettes",
            "mannequin", "suits", "gloves", "helmets"
        );
        for mannequin in &mannequins {
            let Some(diffuse) = diffuse_textures(mannequin) else {
                eprintln!(
                    "  {:26} illisible",
                    mannequin.file_stem().unwrap_or_default().to_string_lossy()
                );
                continue;
            };
            let suits = wardrobe_options(&textures.join(SUIT_DIR), &diffuse);
            let gloves = wardrobe_options(&textures.join(GLOVES_DIR), &diffuse);
            let helmets = wardrobe_options(&textures.join(HELMET_DIR), &diffuse);
            let with_thumb = suits
                .iter()
                .chain(&gloves)
                .chain(&helmets)
                .filter(|o| o.thumbnail.is_some())
                .count();
            let total = suits.len() + gloves.len() + helmets.len();
            eprintln!(
                "  {:26} {:>6} {:>7} {:>8}  {with_thumb}/{total}",
                mannequin.file_stem().unwrap_or_default().to_string_lossy(),
                suits.len(),
                gloves.len(),
                helmets.len()
            );
        }
    }

    /// Where the whole pipeline actually puts each driver, against what the car
    /// says with `DRIVEREYES`.
    ///
    /// The instrument that matters, and the one the crate-level test cannot
    /// be: only the application resolves the real mannequin (`driver3d.ini`
    /// lives in the encrypted container) and the real anchor. A driver placed
    /// by a metre reads here as a metre, before anyone has to notice it on
    /// screen.
    ///
    /// The residual is expected to be the eye-above-bone offset — a few
    /// centimetres up and forward, near zero sideways. Anything past 15 cm on
    /// any axis is a car to go and look at.
    ///
    /// ```text
    /// PITBOX_AC_ROOT="D:\...\assettocorsa" cargo test --lib driver -- --ignored --nocapture where_every
    /// ```
    #[test]
    #[ignore = "needs a real Assetto Corsa install; measurement, not a check"]
    fn where_every_installed_driver_lands() {
        let Ok(ac_root) = std::env::var("PITBOX_AC_ROOT") else {
            eprintln!("PITBOX_AC_ROOT unset, skipping");
            return;
        };
        let root = PathBuf::from(ac_root);
        let mut checked = 0usize;
        let mut off: Vec<(f32, String, [f32; 3], [f32; 3])> = Vec::new();
        let mut residuals: Vec<[f32; 3]> = Vec::new();

        for entry in std::fs::read_dir(root.join("content").join("cars"))
            .expect("read content/cars")
            .flatten()
        {
            let car_dir = entry.path();
            let car_id = car_dir.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let Some(outfit) = outfit_of(&car_dir, &car_id, None) else {
                continue;
            };
            let Some(eyes) = outfit.eyes else { continue };
            let Some(wanted) = graft_for(&root, &car_dir, &outfit, 0.0) else {
                continue;
            };

            // Grafted into an empty car, so what comes out is the driver alone,
            // already in the car's own space — offset, hierarchy and all.
            let mut host = kn5::Kn5Model {
                version: 6,
                extra: None,
                textures: Vec::new(),
                materials: Vec::new(),
                root: kn5::Kn5Node {
                    name: "root".to_string(),
                    active: true,
                    kind: kn5::Kn5NodeKind::Dummy {
                        transform: [
                            1.0, 0.0, 0.0, 0.0, //
                            0.0, 1.0, 0.0, 0.0, //
                            0.0, 0.0, 1.0, 0.0, //
                            0.0, 0.0, 0.0, 1.0,
                        ],
                    },
                    children: Vec::new(),
                },
            };
            let stats = kn5_gltf::graft_driver(&mut host, &wanted);
            let Some(head) = kn5_gltf::node_world_centers(&host)
                .into_iter()
                .find(|(name, _)| name.to_lowercase().ends_with("rig_head"))
                .map(|(_, c)| c)
            else {
                continue;
            };

            checked += 1;
            let residual = [eyes[0] - head[0], eyes[1] - head[1], eyes[2] - head[2]];
            residuals.push(residual);
            let worst = residual.iter().fold(0.0f32, |a, v| a.max(v.abs()));
            if worst > 0.15 {
                off.push((
                    worst,
                    format!(
                        "{car_id} [{}{}]",
                        if stats.seated.is_some() { "knh" } else { "eyes" },
                        if stats.posed.is_some() { "+anim" } else { "" }
                    ),
                    head,
                    eyes,
                ));
            }
        }

        for (axis, label) in [(0, "x"), (1, "y"), (2, "z")] {
            let mut values: Vec<f32> = residuals.iter().map(|r| r[axis]).collect();
            values.sort_by(|a, b| a.total_cmp(b));
            let n = values.len();
            eprintln!(
                "  residual {label}: min {:+.3}  p10 {:+.3}  median {:+.3}  p90 {:+.3}  max {:+.3}",
                values[0],
                values[n / 10],
                values[n / 2],
                values[9 * n / 10],
                values[n - 1]
            );
        }
        off.sort_by(|a, b| b.0.total_cmp(&a.0));
        eprintln!("\n=== {checked} drivers placed, {} past 15 cm ===", off.len());
        for (worst, who, head, eyes) in off.iter().take(30) {
            eprintln!(
                "  {worst:.2} m  {who:52} head {:?} eyes {:?}",
                head.map(|v| (v * 1000.0).round() / 1000.0),
                eyes.map(|v| (v * 1000.0).round() / 1000.0)
            );
        }
    }
}
