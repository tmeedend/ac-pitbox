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

use crate::acd;

mod bodies;
mod choices;
#[cfg(test)]
mod measurements;

pub use bodies::{bodies, is_driver_model, BodyList};
pub use choices::{choices, DriverChoices};

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

/// Ce que le frontend demande pour le pilote de l'aperçu : la tenue qu'il
/// impose éventuellement.
///
/// Un objet plutôt que des paramètres épars : « pas de pilote » se dit alors
/// par l'absence de l'objet entier plutôt que par une combinaison de `None`.
///
/// **L'angle de braquage n'est plus ici** : il tourne aussi les roues avant et
/// le volant de la voiture (`steering`), donc il vaut avec ou sans pilote et
/// voyage à part.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriverView {
    #[serde(default, flatten)]
    pub outfit: OutfitOverride,
}

/// Ce que l'utilisateur impose par-dessus ce que la voiture et son skin
/// déclarent.
///
/// Une pièce à `None` laisse celle du skin. Le **mannequin, lui, n'a pas le
/// même statut que les trois autres** et c'est l'asymétrie qui structure tout
/// l'écran Pilote (`docs/SPEC-ecran-pilote.md` PILOTE§1.3) : la tenue ne tient qu'au
/// `skin.ini`, un fichier de skin, alors que le mannequin est nommé par
/// `driver3d.ini`, donc par le `data.acd` que le serveur de course vérifie.
/// Le substituer ne vaut **que dans l'aperçu** — d'où le bandeau permanent que
/// l'écran affiche tant que dure ce mode (PILOTE§10.1).
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
        // Corps substitué : la garde-robe du skin tombe avec lui (PILOTE§10.1).
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

/// Le mannequin seul, habillé comme l'écran Pilote le demande — sans habitacle
/// autour (`docs/SPEC-ecran-pilote.md` PILOTE§5.1).
///
/// Même résolution que [`resolve`], à une chose près : **l'ancrage tombe**. Il
/// pose le corps sur le `DRIVEREYES` de la voiture, c'est-à-dire à sa place
/// dans un habitacle qui n'est pas là.
///
/// L'assise et l'animation de braquage, elles, **restent**, et c'est la
/// correction du premier essai : sans elles le mannequin garde sa pose de
/// modélisation, bras écartés de 55 cm, et le volant générique qu'on lui pose
/// entre les mains a le diamètre d'un volant de car. Avec elles, l'écart
/// tombe à 35–43 cm selon la voiture — la taille de son vrai volant.
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
    Some(graft)
}

/// The car that seats every body in the gallery thumbnails: a base-game car,
/// so present on every install that has not deleted it by hand.
const THUMBNAIL_CAR: &str = "abarth500";

/// A body alone, as its gallery thumbnail shows it (SESSION§5): in its own
/// textures, and seated by one reference car rather than by the session's.
///
/// **Nothing here depends on the session**, and that is the point. The
/// thumbnail's identity is the key of this graft, so a thumbnail posed by the
/// session car was a new thumbnail for every car picked: the whole gallery,
/// one conversion per body, again at each car change — for a pose that differs
/// by a few centimetres of hand spacing at 104 px. The livery's wardrobe went
/// the same way; the fitting stage shows it, the thumbnail only has to tell
/// one body's geometry from another's.
///
/// Without the reference car the body keeps its modelling pose, arms wide:
/// still a thumbnail, and the one place where an unusual install shows.
pub fn thumbnail_body(ac_root: &Path, body: &str) -> Option<kn5_gltf::DriverGraft> {
    let car_dir = ac_root.join("content").join("cars").join(THUMBNAIL_CAR);
    let seat = outfit_of(&car_dir, THUMBNAIL_CAR, None).unwrap_or_else(|| {
        log::debug!("driver: {THUMBNAIL_CAR} missing, thumbnails keep the modelling pose");
        DriverOutfit {
            model: String::new(),
            eyes: None,
            position: [0.0; 3],
            lock: DEFAULT_LOCK,
            animation: DEFAULT_STEER_ANIMATION.to_string(),
            suit: None,
            gloves: None,
            helmet: None,
        }
    });
    let outfit = DriverOutfit {
        model: body.to_string(),
        ..seat
    };
    let mut graft = graft_for(ac_root, &car_dir, &outfit, 0.0)?;
    graft.anchor = None;
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

    /// Règle PILOTE§10.1 : substituer le corps supprime la référence « livrée ».
    ///
    /// La garde-robe du `skin.ini` est écrite sous le nom de l'ancien
    /// mannequin ; la garder reviendrait à habiller le nouveau avec des
    /// fichiers qui ne le concernent pas — ce que l'écran annonce d'ailleurs
    /// en toutes lettres avant de le faire (bannière d'invalidation, PILOTE§10.2).
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

    /// An AC root with one installed body, and the reference car when asked.
    fn fake_thumbnail_root(base: &Path, with_reference_car: bool) -> PathBuf {
        let root = base.join("ac");
        let drivers = root.join("content").join("driver");
        std::fs::create_dir_all(&drivers).expect("content/driver");
        std::fs::write(drivers.join("gt.kn5"), b"kn5").expect("body file");
        if with_reference_car {
            let car = root.join("content").join("cars").join(THUMBNAIL_CAR);
            std::fs::create_dir_all(car.join("data")).expect("reference car data");
            std::fs::create_dir_all(car.join("animations")).expect("reference car animations");
            std::fs::write(car.join("data").join("driver3d.ini"), DRIVER3D).expect("driver3d.ini");
            std::fs::write(car.join(BASE_POSE), b"knh").expect("base pose");
            std::fs::write(car.join("animations").join("steer.ksanim"), b"ksanim").expect("steer animation");
        }
        root
    }

    // Rule (SESSION§5): a body thumbnail depends on the body alone — seated by
    // the reference car, in its own textures — so picking another session car
    // never re-renders the gallery.
    #[test]
    fn a_body_thumbnail_is_seated_by_the_reference_car_in_its_own_textures() {
        let base = crate::testutil::temp_dir("driver-thumb-body");
        let root = fake_thumbnail_root(&base, true);
        let car = root.join("content").join("cars").join(THUMBNAIL_CAR);

        let graft = thumbnail_body(&root, "gt").expect("a graft for an installed body");

        assert_eq!(
            graft.model,
            root.join("content").join("driver").join("gt.kn5"),
            "the body asked for"
        );
        assert_eq!(
            graft.base_pose,
            Some(car.join(BASE_POSE)),
            "seated by the reference car"
        );
        assert_eq!(
            graft.animation,
            Some(car.join("animations").join("steer.ksanim")),
            "hands on the reference car's wheel"
        );
        assert!(
            graft.texture_dirs.is_empty(),
            "no livery wardrobe: the body's own textures"
        );
        assert_eq!(graft.anchor, None, "no cockpit around a thumbnail");
    }

    // Rule: a missing reference car costs the pose, never the thumbnail.
    #[test]
    fn a_body_thumbnail_survives_a_missing_reference_car() {
        let base = crate::testutil::temp_dir("driver-thumb-nocar");
        let root = fake_thumbnail_root(&base, false);

        let graft = thumbnail_body(&root, "gt").expect("still a graft without the reference car");

        assert_eq!(graft.base_pose, None, "modelling pose");
        assert_eq!(graft.animation, None, "no steering animation either");
        assert_eq!(graft.lock_degrees, DEFAULT_LOCK, "default lock");
        assert!(
            thumbnail_body(&root, "not_installed").is_none(),
            "an absent body has no thumbnail"
        );
    }
}
