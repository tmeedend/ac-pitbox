//! Grafting the driver into a car model (spec §4.6).
//!
//! AC keeps a driver in two unrelated places, and neither of them is the car:
//!
//! - the **mannequin**, a KN5 of its own under `<AC>/content/driver/`
//!   (`driver.kn5` for the modern one, `driver_80.kn5` for the seventies…),
//!   named by the car's `driver3d.ini`;
//! - the **wardrobe**, folders of loose `.dds` under `<AC>/content/texture/`,
//!   picked by the *skin*'s `skin.ini` — a suit, a pair of gloves, a helmet.
//!
//! Which is why a driver cannot be listed as one thing: what the user sees is
//! a mannequin plus an outfit, and the two are chosen by two different files.
//! This module only performs the graft; deciding *what* a car wears is AC
//! domain knowledge and lives in the application's own `driver.rs`.
//!
//! The wardrobe overrides work **by file name**: a folder holding
//! `2016_Suit_DIFF.dds` replaces the mannequin's embedded texture of that
//! name, exactly as a car skin overrides a car's. A folder whose files match
//! nothing — a modern helmet handed to a seventies mannequin, which asks for
//! `HELMET_1985.dds` and not `HELMET_2012.dds` — therefore changes nothing at
//! all. Suits and gloves, on the other hand, carry the same file names on
//! every Kunos mannequin and are interchangeable; only the helmet is tied to
//! one.

use std::path::PathBuf;

use kn5::{Kn5Model, Kn5Node, Kn5NodeKind};

/// Name of the dummy wrapping the grafted mannequin, and of the fresh root
/// built above the car (see [`graft`]).
///
/// Visible in `kn5-tool inspect --tree`, which is the point: a node that
/// appeared out of nowhere should say where it came from.
const DRIVER_MARKER: &str = "PITBOX_DRIVER";

/// Préfixe posé sur le nom de **chaque maillage** du mannequin avant la greffe.
///
/// Sans lui, la vue n'a aucun moyen de retrouver le pilote dans le `.glb` : la
/// conversion aplatit l'arbre, donc le dummy [`DRIVER_MARKER`] ne survit pas,
/// et le regroupement par matériau ne garde que le nom du premier maillage de
/// chaque lot. Le préfixe, lui, traverse — un maillage de mannequin ne partage
/// jamais son matériau avec la voiture (les deux listes sont concaténées à la
/// greffe), donc tout lot qui en contient un est du pilote et rien d'autre.
///
/// C'est ce qui permet de le **montrer ou de le retirer sans reconvertir** :
/// au contact d'une clé de mod son, l'aperçu le fait apparaître en fondu au
/// lieu d'attendre une conversion de quatorze mégaoctets (`CarPreview3D`).
///
/// Posé après la pose et l'habillage : les deux se repèrent par nom de nœud
/// (`pose::apply_locals`, `dress`), et renommer avant les aveuglerait. Les
/// *dummies* du rig gardent leur nom pour la même raison — seuls les maillages
/// sont préfixés, et un maillage n'est jamais un os.
pub const DRIVER_MESH_PREFIX: &str = "PITBOX_DRIVER:";

/// A driver, resolved down to files on disk.
///
/// Deliberately says nothing about *how* it was chosen — read from the car's
/// own `driver3d.ini`, or picked by the user in a future driver selector. Both
/// produce this same handful of paths.
#[derive(Debug, Clone)]
pub struct DriverGraft {
    /// The mannequin's KN5, under `<AC>/content/driver/`.
    pub model: PathBuf,
    /// Where the mannequin's **eyes** have to land, in the car's own space —
    /// `[GRAPHICS] DRIVEREYES` of `car.ini`. See [`HEAD_BONE`] for why that one
    /// line places a whole body, and what happens without it.
    pub anchor: Option<[f32; 3]>,
    /// Wardrobe folders, searched in order for a file named after one of the
    /// mannequin's own textures. First hit wins.
    pub texture_dirs: Vec<PathBuf>,
    /// The car's own `driver_base_pos.knh` — the rig laid out **in the car**,
    /// which is what actually seats the mannequin (see [`base_pose`]). Every
    /// one of the 312 cars of the reference install ships one.
    pub base_pose: Option<PathBuf>,
    /// The car's `animations/steer.ksanim`, which poses the arms (see
    /// [`pose`]). It can carry a placement of its own — 59 of the 271 that
    /// name the driver's root node give it a non-identity transform — but the
    /// hierarchy is applied first and the animation simply wins on the nodes
    /// it names, which is what AC's own layering amounts to.
    pub animation: Option<PathBuf>,
    /// `[STEER_ANIMATION] LOCK` of `driver3d.ini`: the total travel the
    /// animation spans, in degrees. 360 on 271 cars of the reference install,
    /// 180 on fourteen, six other values besides — which is why an angle is
    /// only meaningful against it.
    pub lock_degrees: f32,
    /// How far the wheel is turned, in degrees, 0 being centred. A user
    /// setting, clamped to the car's own lock.
    pub steer_degrees: f32,
}

#[derive(Debug, Default)]
pub struct DriverStats {
    pub triangles: usize,
    /// Mannequin textures actually replaced by a wardrobe file.
    pub dressed: usize,
    /// Rig nodes placed by the car's base hierarchy, `None` when it had none —
    /// which is what sends the seating back to [`EYES_ABOVE_HEAD_BONE`].
    pub seated: Option<usize>,
    /// Rig nodes moved by the steering animation, `None` when there was none
    /// to apply — the mannequin then keeps the arms-forward rest pose it was
    /// modelled in.
    pub posed: Option<usize>,
    /// Non-fatal problems. A driver that fails to graft is never a reason to
    /// fail a preview: the car alone is what the user came for.
    pub failures: Vec<String>,
}

/// Bone every mannequin hangs its head from, and what a driver is seated by
/// (see [`head_of`] and [`EYES_ABOVE_HEAD_BONE`]).
///
/// **Measured, and the reason the placement works at all.** A mannequin's own
/// coordinates seat nobody: its origin is between its feet, at the centre of a
/// body facing forward, identical from one car to the next. What varies is the
/// car — the seat of a formula car is 30 cm lower than a saloon's, and the
/// wheel is on the right in a Japanese import. The car says where the driver
/// goes in one line, `[GRAPHICS] DRIVEREYES` of its `car.ini`, and the
/// mannequin answers it with a bone that sits at **exactly (0, 1.1994,
/// 0.0305)** in nine of the ten mannequins of the reference install — Kunos'
/// five, and third-party ones (`rss_driver_80`, `gt-m24`, `woman_driver`)
/// which build on the same rig. Aligning one to the other is what seats a
/// driver, rather than standing one through the roof.
///
/// Read from the file rather than hard-coded, because the tenth
/// (`new_driver.kn5`, at 1.1663 / 0.1134) proves the convention is a habit and
/// not a rule.
const HEAD_BONE: &str = "RIG_Head";

/// Where the eyes sit relative to [`HEAD_BONE`] — the head bone is at the base
/// of the skull, `DRIVEREYES` is a pair of eyes, and seating one on the other
/// buries the driver's head in the roof.
///
/// **The last resort**: used only when neither the car's hierarchy nor its
/// animation placed the mannequin. No car of the reference install needs it —
/// the three that ship an empty hierarchy are placed by their animation — but
/// a mod may leave both out, and it is also what proved the hierarchy right:
/// the two agree to within the figures below.
///
/// **Calibrated against the install, not estimated.** On 69 cars drawn at
/// random, the clearance between the top of the helmet and the highest point
/// of the car reads:
///
/// | eye offset | cars whose helmet pierces the roof | median clearance |
/// | --- | --- | --- |
/// | none | **15 of 69**, down to −7 cm | +5 cm |
/// | 10 cm | none, worst +3 cm | +15 cm |
///
/// Two independent measurements agree with that figure. The mannequin's face
/// mesh sits 6.5 cm above the bone and its visor 10.7 cm, so the eyes are
/// between the two. And AC's own `driver3d.ini` hides the helmet, the visor
/// and the face in cockpit view — which it would have no reason to do unless
/// the camera, i.e. `DRIVEREYES`, were *inside* the helmet. 10 cm puts it
/// mid-helmet; anything under 7 cm puts a helmet through a roof.
///
/// Forward as well as up (8 cm): eyes are in the front of a head, not at the
/// top of the neck. That one is small enough to be a matter of taste, and it
/// is the height that decides whether a driver fits.
const EYES_ABOVE_HEAD_BONE: [f32; 3] = [0.0, 0.10, 0.08];

/// Reads the mannequin, dresses it, and seats it in the car's space.
///
/// **The car gets a fresh root**, rather than the driver being pushed among
/// the existing root's children. The mannequin's coordinates are expressed in
/// car's *object* space — the space its hierarchy places it in — which is the
/// space the car's own root node transforms *into*, not out of. Grafting under
/// that root would apply the car's root transform to the driver a second time.
/// It is the identity on every model met so far, so the difference is
/// invisible today; it costs one dummy node not to depend on that.
pub fn graft(host: &mut Kn5Model, wanted: &DriverGraft) -> DriverStats {
    let mut stats = DriverStats::default();

    let bytes = match std::fs::read(&wanted.model) {
        Ok(bytes) => bytes,
        Err(e) => {
            stats.failures.push(format!("{} : {e}", wanted.model.display()));
            return stats;
        }
    };
    let mut driver = match kn5::parse(&bytes) {
        Ok(driver) => driver,
        Err(e) => {
            stats.failures.push(format!("{} : {e}", wanted.model.display()));
            return stats;
        }
    };

    stats.dressed = dress(&mut driver, &wanted.texture_dirs, &mut stats.failures);
    stats.triangles = driver.triangle_count();
    // Le socle avant la pose : la hiérarchie place le corps, l'animation
    // reprend par-dessus les membres qu'elle nomme.
    stats.seated = base_pose(&mut driver, wanted, &mut stats.failures);
    stats.posed = pose(&mut driver, wanted, &mut stats.failures);

    mark_driver_meshes(&mut driver.root);

    let offset = seating_offset(&driver, wanted, &mut stats);
    let placed = crate::extconfig::merge_assets(
        host,
        driver,
        DRIVER_MARKER,
        crate::extconfig::compose([1.0, 1.0, 1.0], [0.0, 0.0, 0.0], offset),
    );

    let car = std::mem::replace(
        &mut host.root,
        Kn5Node {
            name: DRIVER_MARKER.to_string(),
            active: true,
            kind: Kn5NodeKind::Dummy { transform: IDENTITY },
            children: Vec::new(),
        },
    );
    host.root.children.push(car);
    host.root.children.push(placed);
    stats
}

/// Préfixe le nom de chaque maillage du mannequin (voir [`DRIVER_MESH_PREFIX`]).
fn mark_driver_meshes(node: &mut Kn5Node) {
    if !matches!(node.kind, Kn5NodeKind::Dummy { .. }) {
        node.name = format!("{DRIVER_MESH_PREFIX}{}", node.name);
    }
    for child in &mut node.children {
        mark_driver_meshes(child);
    }
}

/// Lays the rig out where the car puts it, from its own `driver_base_pos.knh`.
///
/// **This is what seats the driver**, and it took a while to find. The
/// steering animation looks as though it seats him — on two thirds of the
/// cars its rig lands within 6 cm of where the car's `DRIVEREYES` says the
/// head should be — but it does not: its root node is the identity on every
/// file, and what looked like a placement was the rig's own shape happening to
/// start near the seat. The remaining third gave it away, a right-hand-drive
/// Miata among them whose driver sat on the left (reported by the user).
///
/// With the hierarchy read, **all 269 measurable cars of the reference install
/// land within 6 cm sideways**, none apart — no second population at all,
/// where the animation alone left 38 cars 35 cm out or more. The vertical
/// residual against `DRIVEREYES` settles at a median of +6.7 cm, which is the
/// eye-above-bone offset of [`EYES_ABOVE_HEAD_BONE`] measured a third way.
///
/// Returns the number of nodes placed, `None` when the car ships no hierarchy —
/// then, and only then, the driver is seated by his eyes instead.
fn base_pose(driver: &mut Kn5Model, wanted: &DriverGraft, failures: &mut Vec<String>) -> Option<usize> {
    let path = wanted.base_pose.as_ref()?;
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) => {
            failures.push(format!("{} : {e}", path.display()));
            return None;
        }
    };
    let hierarchy = match kn5::parse_hierarchy(&bytes) {
        Ok(hierarchy) => hierarchy,
        Err(e) => {
            failures.push(format!("{} : {e}", path.display()));
            return None;
        }
    };
    // Une hiérarchie **vide** — le seul `SCENE_ROOT`, sans rig dessous — n'est
    // pas un défaut : trois voitures de l'install en livrent une, et c'est une
    // façon de ne rien dire. Le repli sur les yeux s'en charge, en silence.
    if hierarchy.nodes.len() <= 1 {
        log::debug!("driver: {} carries no rig, falling back on DRIVEREYES", path.display());
        return None;
    }
    let seated = crate::pose::apply_hierarchy(driver, &hierarchy);
    if seated == 0 {
        // Là, en revanche, il y a bien un rig et le mannequin n'en reconnaît
        // aucun nœud : les deux ne parlent pas de la même chose.
        failures.push(format!(
            "{} : no node of the mannequin answers to this hierarchy",
            path.display()
        ));
        return None;
    }
    Some(seated)
}

/// Poses the rig from the car's own steering animation, when it has one.
///
/// **This is what puts the hands on the wheel**, and nothing computes it: the
/// modder posed the arms for their own steering wheel and shipped the result,
/// so there is no reach to solve and no wheel geometry to measure. 298 of the
/// 312 cars of the reference install ship one.
///
/// Returns the number of rig nodes actually moved, `None` when there was no
/// animation to apply — which is what tells [`seating_offset`] whether the
/// mannequin has placed itself.
fn pose(driver: &mut Kn5Model, wanted: &DriverGraft, failures: &mut Vec<String>) -> Option<usize> {
    let path = wanted.animation.as_ref()?;
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) => {
            failures.push(format!("{} : {e}", path.display()));
            return None;
        }
    };
    let animation = match kn5::parse_animation(&bytes) {
        Ok(animation) => animation,
        Err(e) => {
            failures.push(format!("{} : {e}", path.display()));
            return None;
        }
    };
    let frame = crate::pose::frame_for(&animation, wanted.lock_degrees, wanted.steer_degrees);
    let posed = crate::pose::apply(driver, &animation, frame);
    // Zéro nœud posé sur une animation lisible : ses noms ne désignent rien
    // dans ce mannequin. Le dire, parce que le pilote gardera alors les bras
    // tendus sans qu'aucune erreur ne soit remontée par ailleurs.
    if posed == 0 {
        failures.push(format!(
            "{} : no node of the mannequin answers to this animation",
            path.display()
        ));
        return None;
    }
    Some(posed)
}

/// Translation that seats this mannequin in this car.
///
/// **Nothing at all, normally.** The car's own `driver_base_pos.knh` has
/// already laid the rig out where the driver goes ([`base_pose`]), in the
/// car's own space, and it is the modder's own placement — there is nothing
/// left to add. A steering animation counts too, when there is no usable
/// hierarchy: on the three cars of the install that ship an empty one, the
/// animation alone lands the head within the eye offset below, so it is
/// placing the body just as absolutely.
///
/// One rule, and no threshold: **whatever file placed the driver is trusted**.
/// An earlier version second-guessed the animation against `DRIVEREYES` past
/// 15 cm, which was a workaround for not having read the hierarchy — with it
/// read, there is nothing left for a threshold to catch.
///
/// **`[MODEL] POSITION` of `driver3d.ini` is deliberately not added**, and
/// that cost a round of wrong previews. It reads as an offset and is not one:
/// 288 of the 301 cars that declare it write `0,0,0`, and the thirteen others
/// write values that visibly break the placement when applied — 50 cm forward
/// on `j8_eunos_roadster_tuned` (body through the steering wheel), 25 cm
/// sideways on the Porsche 919, five metres down on `ks_mercedes_c9`, and
/// `1,1,1` on four `ddm_*` cars, which is not a position at all. All of them
/// are seated correctly by the game, so the game does not add it either.
///
/// The fallback below is for a car that ships no hierarchy: the mannequin then
/// sits at the car's origin, standing on the road through the floor, and its
/// eyes are brought onto `DRIVEREYES` instead — see [`EYES_ABOVE_HEAD_BONE`],
/// which is what that costs in accuracy. With neither hierarchy nor eyes there
/// is nothing left to go on, and it is reported rather than silently drawn.
fn seating_offset(driver: &Kn5Model, wanted: &DriverGraft, stats: &mut DriverStats) -> [f32; 3] {
    if stats.seated.is_some() || stats.posed.is_some() {
        return [0.0; 3];
    }
    let Some(anchor) = wanted.anchor else {
        stats.failures.push(format!(
            "{} : the car has neither a driver hierarchy nor DRIVEREYES to seat it by",
            wanted.model.display()
        ));
        return [0.0; 3];
    };
    let Some(head) = head_of(driver) else {
        stats.failures.push(format!(
            "{} : no `{HEAD_BONE}` node to seat it by",
            wanted.model.display()
        ));
        return [0.0; 3];
    };
    [
        anchor[0] - head[0] - EYES_ABOVE_HEAD_BONE[0],
        anchor[1] - head[1] - EYES_ABOVE_HEAD_BONE[1],
        anchor[2] - head[2] - EYES_ABOVE_HEAD_BONE[2],
    ]
}

/// Where the mannequin's head bone sits in its own space.
fn head_of(driver: &Kn5Model) -> Option<[f32; 3]> {
    bone_of(&crate::geometry::node_world_centers(driver), HEAD_BONE)
}

/// One bone of a rig, by name, among centres already computed.
///
/// Matched on the **end** of the node name, not the whole of it: AC prefixes
/// every node of a mannequin with `DRIVER:`, but a mod that dropped the prefix
/// still has a rig, and the bones are unambiguous either way.
fn bone_of(centers: &[(String, [f32; 3])], bone: &str) -> Option<[f32; 3]> {
    centers
        .iter()
        .find(|(name, _)| name.len() >= bone.len() && name[name.len() - bone.len()..].eq_ignore_ascii_case(bone))
        .map(|(_, center)| *center)
}

const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, //
    0.0, 0.0, 0.0, 1.0,
];

/// Swaps in the wardrobe files, in place, **before** the mannequin is merged
/// into the car.
///
/// Order matters: merging renames a texture whose name already exists in the
/// car with different bytes, and comparing the *dressed* bytes is what makes
/// that comparison mean anything. A generic name like `MAT_white.dds`, which
/// both a car and a pair of gloves may carry, is exactly the case.
fn dress(driver: &mut Kn5Model, dirs: &[PathBuf], failures: &mut Vec<String>) -> usize {
    if dirs.is_empty() {
        return 0;
    }
    let mut dressed = 0;
    for texture in &mut driver.textures {
        let Some((bytes, path)) = read_first(dirs, &texture.name, failures) else {
            continue;
        };
        log::debug!("driver: {} dressed from {}", texture.name, path.display());
        texture.data = bytes;
        dressed += 1;
    }
    dressed
}

/// The first wardrobe folder holding a file of that name.
fn read_first(dirs: &[PathBuf], name: &str, failures: &mut Vec<String>) -> Option<(Vec<u8>, PathBuf)> {
    for dir in dirs {
        let path = dir.join(name);
        match std::fs::read(&path) {
            Ok(bytes) => return Some((bytes, path)),
            // Absent is the ordinary case: a wardrobe folder holds the two or
            // three files it changes, never the mannequin's whole set.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => failures.push(format!("{} : {e}", path.display())),
        }
    }
    None
}

// --- Le mannequin seul, pour le plateau d'essayage --------------------------

/// The bones the fitting stage needs, on top of the head it already knew.
///
/// **The measurement that decided how the stage is built.** In its *rest*
/// pose a mannequin holds its hands 55 cm apart — 41 of the 44 offerable ones
/// to the millimetre, at (±0.277, 1.027, 0.530). That looks like a grip and is
/// not one: no steering wheel is 55 cm across, and a ring drawn through those
/// hands reads as a bus wheel the fingers do not touch. Apply the car's own
/// hierarchy and steering animation and the same hands close to **35–43 cm**,
/// car by car — a real wheel, of that car's real size. So the stage poses the
/// driver like the car does, and the ring is read off the result.
const HAND_BONES: [&str; 2] = ["RIG_HAND_L", "RIG_HAND_R"];
/// Where the fingers actually close, left then right — the second phalanx of
/// each middle finger.
///
/// **A wrist is not a grip**, and the difference is not subtle: measured on
/// twelve posed cars, the midpoint of the two middle fingers sits a steady
/// **13 cm in front** of the midpoint of the two wrists (+0.127 to +0.138 m,
/// twelve out of twelve). A ring drawn on the wrists is therefore a ring the
/// hands do not touch — reported from the screen before it was measured here.
///
/// The numbering is AC's, and it is asymmetric: the left hand carries
/// `HAND_Index1..3` / `HAND_Middle1..3`, the right one `HAND_Index4..6` /
/// `HAND_Middle4..6`. Hence `Middle2` and `Middle5` rather than a shared name
/// with a side suffix.
const GRIP_BONES: [&str; 2] = ["HAND_Middle2", "HAND_Middle5"];
/// The other end of the torso, which frames the bust.
const HIPS_BONE: &str = "RIG_Hips";

/// Where a mannequin's rig sits in the space of the model it is converted
/// into — metres, and the same axes the `.glb` uses.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DriverRig {
    /// Wrists, left then right. `None` when the mannequin has no hand bone
    /// under a name we know.
    pub hands: Option<[[f32; 3]; 2]>,
    /// Where the fingers close, left then right — see [`GRIP_BONES`]. This is
    /// what a steering wheel has to pass through; [`DriverRig::hands`] is 13 cm
    /// behind it.
    pub grip: Option<[[f32; 3]; 2]>,
    pub head: Option<[f32; 3]>,
    pub hips: Option<[f32; 3]>,
}

/// The mannequin alone, dressed and posed — what the fitting stage of the
/// driver screen shows (`docs/SPEC-ecran-pilote.md` §5.1).
///
/// Deliberately not [`graft`] with an empty host, but for **one** of that
/// function's four jobs rather than three: the body is not offset onto the
/// car's `DRIVEREYES` (`anchor` is ignored, and the caller need not fill it),
/// because a stage has no cockpit to fit into. The other two — laying the rig
/// out from the car's `driver_base_pos.knh`, and posing the arms with its
/// steering animation — are kept, and they are what makes the hands close on
/// a wheel-sized ring instead of hanging 55 cm apart (see [`HAND_BONES`]).
///
/// The rig comes back **after** posing, for the same reason.
pub fn standalone(wanted: &DriverGraft) -> Result<(Kn5Model, DriverStats, DriverRig), String> {
    let mut stats = DriverStats::default();
    let bytes = std::fs::read(&wanted.model).map_err(|e| format!("{} : {e}", wanted.model.display()))?;
    let mut driver = kn5::parse(&bytes).map_err(|e| format!("{} : {e}", wanted.model.display()))?;

    stats.dressed = dress(&mut driver, &wanted.texture_dirs, &mut stats.failures);
    stats.triangles = driver.triangle_count();
    // Même ordre que dans `graft` : la hiérarchie place le corps, l'animation
    // reprend par-dessus les membres qu'elle nomme.
    stats.seated = base_pose(&mut driver, wanted, &mut stats.failures);
    stats.posed = pose(&mut driver, wanted, &mut stats.failures);

    let centers = crate::geometry::node_world_centers(&driver);
    let pair = |bones: [&str; 2]| {
        bone_of(&centers, bones[0])
            .zip(bone_of(&centers, bones[1]))
            .map(|(left, right)| [left, right])
    };
    let rig = DriverRig {
        hands: pair(HAND_BONES),
        grip: pair(GRIP_BONES),
        head: bone_of(&centers, HEAD_BONE),
        hips: bone_of(&centers, HIPS_BONE),
    };
    Ok((driver, stats, rig))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A model with nothing in it, to stand in for a car.
    fn bare_model() -> Kn5Model {
        Kn5Model {
            version: 6,
            extra: None,
            textures: Vec::new(),
            materials: Vec::new(),
            root: Kn5Node {
                name: "root".to_string(),
                active: true,
                kind: Kn5NodeKind::Dummy { transform: IDENTITY },
                children: Vec::new(),
            },
        }
    }

    /// A mannequin reduced to what seats it: a head bone at the place the
    /// Kunos rig puts one.
    fn mannequin_with_head_at(position: [f32; 3]) -> Kn5Model {
        let mut model = bare_model();
        model.root.children.push(Kn5Node {
            name: "DRIVER:RIG_Head".to_string(),
            active: true,
            kind: Kn5NodeKind::Dummy {
                transform: crate::extconfig::compose([1.0; 3], [0.0; 3], position),
            },
            children: Vec::new(),
        });
        model
    }

    /// A graft with nothing but what a test names.
    fn wanted(anchor: Option<[f32; 3]>) -> DriverGraft {
        DriverGraft {
            model: PathBuf::new(),
            anchor,
            texture_dirs: Vec::new(),
            base_pose: None,
            animation: None,
            lock_degrees: 360.0,
            steer_degrees: 0.0,
        }
    }

    /// Head bone of the Kunos rig, as measured on nine of the ten mannequins.
    const KUNOS_HEAD: [f32; 3] = [0.0, 1.1994, 0.0305];

    // Rule: the mannequin's **eyes** land on the car's anchor, not its head
    // bone — the offset between the two is what keeps a helmet under a roof
    // (§4.6, and [`EYES_ABOVE_HEAD_BONE`]).
    #[test]
    fn the_mannequins_eyes_land_on_the_cars_anchor() {
        let driver = mannequin_with_head_at(KUNOS_HEAD);
        let anchor = [0.330737, 1.19075, -0.490002];
        let mut stats = DriverStats::default();

        let offset = seating_offset(&driver, &wanted(Some(anchor)), &mut stats);

        assert!(
            stats.failures.is_empty(),
            "a mannequin with a head bone seats without complaint"
        );
        for axis in 0..3 {
            let head_in_car = KUNOS_HEAD[axis] + offset[axis];
            assert!(
                (head_in_car + EYES_ABOVE_HEAD_BONE[axis] - anchor[axis]).abs() < 1e-5,
                "axis {axis}: the eyes, not the bone, sit on DRIVEREYES"
            );
        }
    }

    // Rule: a car that placed nobody and declares no eyes leaves the mannequin
    // at the origin, and says so — that driver stands through the floor.
    #[test]
    fn a_car_that_places_nobody_is_reported() {
        let driver = mannequin_with_head_at(KUNOS_HEAD);
        let mut stats = DriverStats::default();

        let offset = seating_offset(&driver, &wanted(None), &mut stats);

        assert_eq!(offset, [0.0; 3], "nothing to move it by");
        assert_eq!(stats.failures.len(), 1, "and it is reported rather than silently drawn");
    }

    // Rule: a car that laid the rig out itself is not second-guessed — its
    // hierarchy is the modder's own placement, `DRIVEREYES` or not (§4.6bis).
    #[test]
    fn a_seated_mannequin_keeps_the_placement_its_car_gave_it() {
        let driver = mannequin_with_head_at([0.0, 0.93, -0.19]);
        // An anchor that disagrees by 36 cm: an earlier version dragged the
        // mannequin across the car for exactly this.
        let mut stats = DriverStats {
            seated: Some(60),
            ..DriverStats::default()
        };

        let offset = seating_offset(&driver, &wanted(Some([-0.36, 0.98, -0.32])), &mut stats);

        assert_eq!(offset, [0.0; 3], "the hierarchy had already placed it");
        assert!(stats.failures.is_empty(), "and there is nothing to complain about");
    }

    // Rule: an animation placed the driver too, when no hierarchy did — the
    // three cars shipping an empty one rely on it (§4.6bis).
    #[test]
    fn a_posed_mannequin_is_left_where_its_animation_put_it() {
        let driver = mannequin_with_head_at([0.386, 1.107, -0.205]);
        let mut stats = DriverStats {
            posed: Some(56),
            ..DriverStats::default()
        };

        let offset = seating_offset(&driver, &wanted(Some([0.375, 1.152, -0.084])), &mut stats);

        assert_eq!(offset, [0.0; 3], "the animation placed it, to the millimetre");
        assert!(stats.failures.is_empty());
    }

    // Rule: a driver whose KN5 cannot be read is reported, never fatal — the
    // car alone still makes a preview (§4.6).
    #[test]
    fn a_missing_mannequin_is_reported_and_nothing_else() {
        let mut host = bare_model();
        let before = format!("{:?}", host.root);
        let stats = graft(
            &mut host,
            &DriverGraft {
                model: PathBuf::from("nowhere-at-all/driver.kn5"),
                ..wanted(Some([0.0; 3]))
            },
        );
        assert_eq!(stats.failures.len(), 1, "the unreadable file is reported");
        assert_eq!(stats.triangles, 0, "nothing was grafted");
        assert_eq!(format!("{:?}", host.root), before, "the car is left untouched");
    }

    // Rule: a wardrobe folder replaces a mannequin texture by file name, and
    // the first folder listed wins.
    #[test]
    fn the_first_wardrobe_folder_holding_the_file_wins() {
        let base = crate::testutil::temp_dir("driver-dress");
        let first = base.join("helmet");
        let second = base.join("suit");
        std::fs::create_dir_all(&first).expect("wardrobe folder");
        std::fs::create_dir_all(&second).expect("wardrobe folder");
        std::fs::write(first.join("skin.dds"), b"helmet").expect("wardrobe file");
        std::fs::write(second.join("skin.dds"), b"suit").expect("wardrobe file");

        let mut driver = bare_model();
        driver.textures.push(kn5::Kn5Texture {
            kind: 1,
            name: "skin.dds".to_string(),
            data: b"embedded".to_vec(),
        });
        driver.textures.push(kn5::Kn5Texture {
            kind: 1,
            name: "untouched.dds".to_string(),
            data: b"embedded".to_vec(),
        });

        let dirs = vec![first, second];
        let mut failures = Vec::new();
        let dressed = dress(&mut driver, &dirs, &mut failures);

        assert_eq!(dressed, 1, "one texture had a wardrobe file");
        assert_eq!(driver.textures[0].data, b"helmet", "the first folder listed wins");
        assert_eq!(
            driver.textures[1].data, b"embedded",
            "a texture no folder names keeps the mannequin's own"
        );
        assert!(failures.is_empty(), "an absent file is not a failure");
    }

    /// Grafts a real driver into every car of a real install and reports what
    /// happened — the measurement to re-run whenever the seating or the
    /// skinning changes.
    ///
    /// **No anchor is resolved here**: `DRIVEREYES` lives in the encrypted
    /// `data.acd`, which this crate deliberately does not read (the
    /// application does, and hands the result in). Nothing is lost — the
    /// hierarchy seats the driver on every car of the install, and the anchor
    /// is only the fallback for a mod that ships none.
    ///
    /// ```text
    /// PITBOX_AC_ROOT="D:\...\assettocorsa" cargo test -p kn5-gltf -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "needs a real Assetto Corsa install; measurement, not a check"]
    fn every_installed_car_seats_its_driver() {
        let Ok(ac_root) = std::env::var("PITBOX_AC_ROOT") else {
            eprintln!("PITBOX_AC_ROOT unset, skipping");
            return;
        };
        let root = std::path::PathBuf::from(ac_root);
        let started = std::time::Instant::now();
        let (mut total, mut animated, mut posed_ok, mut skinned) = (0, 0, 0, 0);
        let (mut hierarchies, mut seated_ok) = (0, 0);
        let mut complaints: Vec<String> = Vec::new();

        for entry in std::fs::read_dir(root.join("content").join("cars"))
            .expect("read content/cars")
            .flatten()
        {
            let car_dir = entry.path();
            let Some(model) = crate::resolve_model(&car_dir) else {
                continue;
            };
            let Ok(bytes) = std::fs::read(&model.path) else {
                continue;
            };
            let Ok(mut host) = kn5::parse(&bytes) else { continue };
            total += 1;

            // The mannequin is not resolved here — that is the application's
            // job — so the reference one stands in for every car. What is under
            // test is the seating and the skinning, not the wardrobe.
            let mannequin = root.join("content").join("driver").join("driver.kn5");
            let animation = car_dir.join("animations").join("steer.ksanim");
            let has_animation = animation.is_file();
            animated += usize::from(has_animation);
            let base_pose = car_dir.join("driver_base_pos.knh");
            let has_base_pose = base_pose.is_file();
            hierarchies += usize::from(has_base_pose);

            let stats = graft(
                &mut host,
                &DriverGraft {
                    model: mannequin,
                    anchor: None,
                    texture_dirs: Vec::new(),
                    base_pose: has_base_pose.then_some(base_pose),
                    animation: has_animation.then_some(animation),
                    lock_degrees: 360.0,
                    steer_degrees: 0.0,
                },
            );
            posed_ok += usize::from(stats.posed.is_some());
            seated_ok += usize::from(stats.seated.is_some());
            for failure in &stats.failures {
                complaints.push(format!("{}: {failure}", car_dir.display()));
            }

            // The point of the whole exercise: once posed, the skinned meshes
            // must have followed the rig. A body left in its bind pose while
            // the head moved shows up as a mesh whose centre is metres from
            // the head's.
            let (meshes, _) = crate::geometry::flatten(&host, &crate::GeometryOptions::default());
            let suit = meshes.iter().find(|m| m.name.to_lowercase().ends_with("suit"));
            let head = crate::geometry::node_world_centers(&host)
                .into_iter()
                .find(|(name, _)| name.to_lowercase().ends_with("rig_head"))
                .map(|(_, c)| c);
            if let (Some(suit), Some(head)) = (suit, head) {
                let centre = suit.positions.iter().fold([0.0f32; 3], |mut acc, p| {
                    for axis in 0..3 {
                        acc[axis] += p[axis] / suit.positions.len() as f32;
                    }
                    acc
                });
                let apart = ((centre[0] - head[0]).powi(2) + (centre[1] - head[1]).powi(2)).sqrt();
                if apart < 1.0 {
                    skinned += 1;
                } else {
                    complaints.push(format!("{}: suit {apart:.2} m from the head", car_dir.display()));
                }
            }
        }

        eprintln!("\n=== drivers: {total} cars in {:.2?} ===", started.elapsed());
        eprintln!("  ship a driver_base_pos.knh  {hierarchies}");
        eprintln!("  seated by it                {seated_ok}");
        eprintln!("  ship a steer.ksanim         {animated}");
        eprintln!("  posed by it                 {posed_ok}");
        eprintln!("  suit followed the rig       {skinned}");
        for complaint in complaints.iter().take(20) {
            eprintln!("  ! {complaint}");
        }
        if complaints.len() > 20 {
            eprintln!("  … and {} more", complaints.len() - 20);
        }
    }
}
