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
//! a pair (mannequin, outfit), and the two are chosen by two different files.
//! This module only performs the graft; deciding *which* pair belongs to a car
//! is AC domain knowledge and lives in the application's own `driver.rs`.
//!
//! The wardrobe overrides work **by file name**: a folder holding
//! `2016_Suit_DIFF.dds` replaces the mannequin's embedded texture of that
//! name, exactly as a car skin overrides a car's. A folder whose files match
//! nothing — a modern helmet handed to a seventies mannequin, whose material
//! asks for `HELMET_1985.dds` — therefore changes nothing at all, which is the
//! honest outcome: we do not know how to translate one wardrobe into another.

use std::path::PathBuf;

use kn5::{Kn5Model, Kn5Node, Kn5NodeKind};

/// Name of the dummy wrapping the grafted mannequin, and of the fresh root
/// built above the car (see [`graft`]).
///
/// Visible in `kn5-tool inspect --tree`, which is the point: a node that
/// appeared out of nowhere should say where it came from.
const DRIVER_MARKER: &str = "PITBOX_DRIVER";

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
    /// `[MODEL] POSITION` of the car's `driver3d.ini`, added on top of the
    /// anchor and applied as-is: the conversion changes no frame of reference
    /// (see `geometry.rs`), so AC's three numbers are already glTF's.
    pub position: [f32; 3],
    /// Wardrobe folders, searched in order for a file named after one of the
    /// mannequin's own textures. First hit wins.
    pub texture_dirs: Vec<PathBuf>,
}

#[derive(Debug, Default)]
pub struct DriverStats {
    pub triangles: usize,
    /// Mannequin textures actually replaced by a wardrobe file.
    pub dressed: usize,
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
/// the car's *object* space — the space `POSITION` offsets from — which is the
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

    let offset = seating_offset(&driver, wanted, &mut stats.failures);
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

/// Translation that seats this mannequin in this car: whatever brings its eyes
/// onto the car's anchor, plus the car's own fine-tuning offset.
///
/// Without an anchor — a car with no `car.ini` to read, a `DRIVEREYES` that is
/// not there — the offset is the fine-tuning alone, which places the mannequin
/// at the car's origin: standing on the road, through the floor. Reported as a
/// failure rather than silently drawn, because it is not a driver seated
/// badly, it is a driver not seated at all.
fn seating_offset(driver: &Kn5Model, wanted: &DriverGraft, failures: &mut Vec<String>) -> [f32; 3] {
    let Some(anchor) = wanted.anchor else {
        failures.push(format!(
            "{} : the car declares no DRIVEREYES to seat it by",
            wanted.model.display()
        ));
        return wanted.position;
    };
    let Some(head) = head_of(driver) else {
        failures.push(format!(
            "{} : no `{HEAD_BONE}` node to seat it by",
            wanted.model.display()
        ));
        return wanted.position;
    };
    [
        anchor[0] - head[0] - EYES_ABOVE_HEAD_BONE[0] + wanted.position[0],
        anchor[1] - head[1] - EYES_ABOVE_HEAD_BONE[1] + wanted.position[1],
        anchor[2] - head[2] - EYES_ABOVE_HEAD_BONE[2] + wanted.position[2],
    ]
}

/// Where the mannequin's head bone sits in its own space.
///
/// Matched on the **end** of the node name, not the whole of it: AC prefixes
/// every node of a mannequin with `DRIVER:`, but a mod that dropped the prefix
/// still has a rig, and the bone is unambiguous either way.
fn head_of(driver: &Kn5Model) -> Option<[f32; 3]> {
    crate::geometry::node_world_centers(driver)
        .into_iter()
        .find(|(name, _)| {
            name.len() >= HEAD_BONE.len() && name[name.len() - HEAD_BONE.len()..].eq_ignore_ascii_case(HEAD_BONE)
        })
        .map(|(_, center)| center)
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

    // Rule: the mannequin's **eyes** land on the car's anchor, not its head
    // bone — the offset between the two is what keeps a helmet under a roof
    // (§4.6, and [`EYES_ABOVE_HEAD_BONE`]).
    #[test]
    fn the_mannequins_eyes_land_on_the_cars_anchor() {
        let driver = mannequin_with_head_at([0.0, 1.1994, 0.0305]);
        let anchor = [0.330737, 1.19075, -0.490002];
        let mut failures = Vec::new();

        let offset = seating_offset(
            &driver,
            &DriverGraft {
                model: PathBuf::new(),
                anchor: Some(anchor),
                position: [0.0; 3],
                texture_dirs: Vec::new(),
            },
            &mut failures,
        );

        assert!(
            failures.is_empty(),
            "a mannequin with a head bone seats without complaint"
        );
        for axis in 0..3 {
            let head_in_car = [0.0, 1.1994, 0.0305][axis] + offset[axis];
            assert!(
                (head_in_car + EYES_ABOVE_HEAD_BONE[axis] - anchor[axis]).abs() < 1e-5,
                "axis {axis}: the eyes, not the bone, sit on DRIVEREYES"
            );
        }
    }

    // Rule: the car's own `POSITION` is added on top of the seating, not
    // instead of it.
    #[test]
    fn the_cars_position_offsets_the_seating() {
        let driver = mannequin_with_head_at([0.0, 1.1994, 0.0305]);
        let seat = |position| {
            seating_offset(
                &driver,
                &DriverGraft {
                    model: PathBuf::new(),
                    anchor: Some([0.0, 1.0, 0.0]),
                    position,
                    texture_dirs: Vec::new(),
                },
                &mut Vec::new(),
            )
        };
        let plain = seat([0.0; 3]);
        let nudged = seat([0.01, 0.02, 0.03]);
        for axis in 0..3 {
            assert!(
                (nudged[axis] - plain[axis] - [0.01, 0.02, 0.03][axis]).abs() < 1e-6,
                "axis {axis}: POSITION moves the mannequin from where it was seated"
            );
        }
    }

    // Rule: a car that declares no eyes leaves the mannequin unseated, and
    // says so — a driver at the car's origin stands through the floor.
    #[test]
    fn a_car_without_driver_eyes_is_reported() {
        let driver = mannequin_with_head_at([0.0, 1.1994, 0.0305]);
        let mut failures = Vec::new();
        let offset = seating_offset(
            &driver,
            &DriverGraft {
                model: PathBuf::new(),
                anchor: None,
                position: [0.0, 0.5, 0.0],
                texture_dirs: Vec::new(),
            },
            &mut failures,
        );
        assert_eq!(offset, [0.0, 0.5, 0.0], "nothing but the car's own offset is applied");
        assert_eq!(failures.len(), 1, "and it is reported rather than silently drawn");
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
                anchor: Some([0.0; 3]),
                position: [0.0; 3],
                texture_dirs: Vec::new(),
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
}
