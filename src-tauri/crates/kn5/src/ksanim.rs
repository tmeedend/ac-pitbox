//! `.ksanim` — the animation files Assetto Corsa ships beside a car
//! (`<car>/animations/steer.ksanim`, `car_door_L.ksanim`, `shift.ksanim`…).
//!
//! Same charter as the rest of this crate: pure parsing, no I/O. The layout
//! below was read off real files and is documented in
//! `docs/kn5-format.md`; nothing here is transcribed from third-party code.
//!
//! # Layout
//!
//! | | |
//! | --- | --- |
//! | `u32` | version, **1 or 2**, and the two do not share a frame layout |
//! | `u32` | number of animated nodes |
//! | *per node* | `u32` name length, the name, `u32` frame count, then the frames |
//!
//! A frame is a node's **local** transform, the same slot as a dummy node's
//! matrix in the model — so applying a frame means replacing that matrix, and
//! the hierarchy does the rest.
//!
//! - **version 2** (271 of the 298 cars that ship one): a quaternion `xyzw`,
//!   a translation `xyz` and a scale `xyz` — ten floats, 40 bytes.
//! - **version 1** (the other 27): a plain 4×4 matrix, 64 bytes.
//!
//! Both are returned as a matrix here, in the model's own row-vector
//! convention (translation in `[12..15]`, `world = local × parent`): the
//! caller has one representation to deal with, and it is the one the node tree
//! already speaks.
//!
//! # What the frames mean
//!
//! **It says what the limbs do, not where the body is.** A car places its
//! driver in `driver_base_pos.knh` (see `crate::knh`): 212 of the 271
//! animations that name the driver's root node leave it at the identity, and
//! trusting the other 59 to place anyone misseats about one car in six.
//!
//! A `steer.ksanim` holds the driver's whole rig — 60 nodes on the Kunos one —
//! over 100 frames spanning the car's full steering lock, `[STEER_ANIMATION]
//! LOCK` of `driver3d.ini` (360° on 271 cars, 180° on 14, and six other values
//! besides). **The middle frame is the wheel centred**, measured on
//! `ks_abarth500_assetto_corse`: its two hands land at x = 0.511 and 0.141,
//! centred on 0.326 at height 0.96, against a steering wheel hub at
//! (0.331, 0.964) — a nine-and-three grip on the rim. A quarter of the way in,
//! the same hands sit one high one low, which is that grip rotated by a
//! quarter of the lock.

use crate::error::Result;
use crate::limits::Limits;
use crate::reader::Reader;

/// One animated node: a name to match against the model's tree, and its local
/// transform at each frame.
#[derive(Debug, Clone)]
pub struct Kn5AnimatedNode {
    pub name: String,
    /// Local transforms, one per frame, row-vector convention.
    pub frames: Vec<[f32; 16]>,
}

#[derive(Debug, Clone)]
pub struct Kn5Animation {
    pub version: u32,
    pub nodes: Vec<Kn5AnimatedNode>,
}

impl Kn5Animation {
    /// Frames the animation holds, taken from its first node.
    ///
    /// Every node of every file met so far carries the same count, but nothing
    /// in the format says it must, so the per-node vectors stay independent
    /// and this is only the headline figure.
    pub fn frame_count(&self) -> usize {
        self.nodes.first().map_or(0, |node| node.frames.len())
    }

    /// The node of that name, matched **on the end of the name**.
    ///
    /// AC prefixes a mannequin's nodes with `DRIVER:`, and a mod that dropped
    /// the prefix still animates the same rig.
    pub fn node(&self, name: &str) -> Option<&Kn5AnimatedNode> {
        self.node_at(name).map(|(_, node)| node)
    }

    /// Same lookup, **with the index of the node that answered** — voir
    /// `Kn5Hierarchy::local_at` pour ce que l'index sert à distinguer.
    pub fn node_at(&self, name: &str) -> Option<(usize, &Kn5AnimatedNode)> {
        self.nodes
            .iter()
            .enumerate()
            .find(|(_, node)| ends_with_ignore_case(&node.name, name))
    }
}

/// Case-insensitive suffix match on a node name.
pub(crate) fn ends_with_ignore_case(name: &str, suffix: &str) -> bool {
    name.len() >= suffix.len() && name[name.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
}

/// Smallest a frame can be, whichever version — used to tie the frame count to
/// the bytes that actually remain (see [`Reader::count`]).
const MIN_FRAME_BYTES: usize = 40;
/// Smallest a node entry can be: a length, one character of name, a count.
const MIN_NODE_BYTES: usize = 9;

/// Parses a `.ksanim` with the default [`Limits`].
pub fn parse_animation(bytes: &[u8]) -> Result<Kn5Animation> {
    parse_animation_with_limits(bytes, &Limits::default())
}

/// Same, with explicit caps. Like the model parser, every allocation is tied
/// to a validated count — these files come from mods too.
pub fn parse_animation_with_limits(bytes: &[u8], limits: &Limits) -> Result<Kn5Animation> {
    let mut r = Reader::new(bytes);
    let version = r.u32()?;
    let count = r.count("animated_node_count", limits.max_animated_nodes, MIN_NODE_BYTES)?;

    let mut nodes = Vec::with_capacity(count);
    for _ in 0..count {
        let name = r.string("animated_node_name", limits.max_string_bytes)?;
        let frames = r.count("frame_count", limits.max_frames, MIN_FRAME_BYTES)?;
        let mut transforms = Vec::with_capacity(frames);
        for _ in 0..frames {
            transforms.push(match version {
                1 => r.f32s::<16>()?,
                // Anything that is not version 1 is read as version 2 rather
                // than refused: the two layouts are the only ones met, and a
                // file whose version byte we do not know still has a good
                // chance of being the recent one. A wrong guess costs a
                // mannequin in a strange pose, never a crash — every read
                // below is bounds-checked.
                _ => compose(r.f32s::<4>()?, r.f32s::<3>()?, r.f32s::<3>()?),
            });
        }
        nodes.push(Kn5AnimatedNode {
            name,
            frames: transforms,
        });
    }
    Ok(Kn5Animation { version, nodes })
}

/// Builds the row-vector matrix of a quaternion, a translation and a scale.
///
/// Row-vector means the rotation rows are the transformed basis vectors and
/// the translation is the last **row**, not the last column — the convention
/// the KN5 node tree uses (`Kn5NodeKind::Dummy`). Getting it transposed leaves
/// the hierarchy standing but scatters every limb, which is exactly how it
/// announces itself.
fn compose(rotation: [f32; 4], translation: [f32; 3], scale: [f32; 3]) -> [f32; 16] {
    let [x, y, z, w] = rotation;
    let (x2, y2, z2) = (x + x, y + y, z + z);
    let (xx, xy, xz) = (x * x2, x * y2, x * z2);
    let (yy, yz, zz) = (y * y2, y * z2, z * z2);
    let (wx, wy, wz) = (w * x2, w * y2, w * z2);

    let rows = [
        [1.0 - (yy + zz), xy + wz, xz - wy],
        [xy - wz, 1.0 - (xx + zz), yz + wx],
        [xz + wy, yz - wx, 1.0 - (xx + yy)],
    ];

    let mut m = [0.0f32; 16];
    for (row, basis) in rows.iter().enumerate() {
        for (col, value) in basis.iter().enumerate() {
            m[row * 4 + col] = scale[row] * value;
        }
    }
    m[12] = translation[0];
    m[13] = translation[1];
    m[14] = translation[2];
    m[15] = 1.0;
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One version 2 frame as the file stores it: rotation, translation, scale.
    type Frame = ([f32; 4], [f32; 3], [f32; 3]);

    fn v2_file(nodes: &[(&str, &[Frame])]) -> Vec<u8> {
        let mut out = 2u32.to_le_bytes().to_vec();
        out.extend_from_slice(&(nodes.len() as u32).to_le_bytes());
        for (name, frames) in nodes {
            out.extend_from_slice(&(name.len() as u32).to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(&(frames.len() as u32).to_le_bytes());
            for (rotation, translation, scale) in *frames {
                for value in rotation.iter().chain(translation).chain(scale) {
                    out.extend_from_slice(&value.to_le_bytes());
                }
            }
        }
        out
    }

    const NEUTRAL: Frame = ([0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);

    // Rule: a version 2 frame is ten floats, and an identity one composes into
    // the identity matrix — the check that catches a transposed convention.
    #[test]
    fn version_2_frames_are_read_as_local_matrices() {
        let translated = ([0.0, 0.0, 0.0, 1.0], [0.3, 0.45, -0.5], [1.0, 1.0, 1.0]);
        let bytes = v2_file(&[("DRIVER:RIG_Center", &[NEUTRAL, translated])]);

        let anim = parse_animation(&bytes).expect("a well-formed file parses");

        assert_eq!(anim.version, 2);
        assert_eq!(anim.nodes.len(), 1, "one animated node");
        assert_eq!(anim.frame_count(), 2, "two frames");
        assert_eq!(
            anim.nodes[0].frames[0],
            [
                1.0, 0.0, 0.0, 0.0, //
                0.0, 1.0, 0.0, 0.0, //
                0.0, 0.0, 1.0, 0.0, //
                0.0, 0.0, 0.0, 1.0,
            ],
            "a neutral frame is the identity"
        );
        let moved = anim.nodes[0].frames[1];
        assert_eq!(
            [moved[12], moved[13], moved[14]],
            [0.3, 0.45, -0.5],
            "the translation sits in the last row, not the last column"
        );
    }

    // Rule: a quarter turn about Y sends +X to −Z under this convention. A
    // matrix written transposed passes every "is it the identity" check and
    // fails this one.
    #[test]
    fn a_rotation_turns_the_axes_the_expected_way() {
        let half = std::f32::consts::FRAC_1_SQRT_2;
        let quarter = ([0.0, half, 0.0, half], [0.0; 3], [1.0; 3]);
        let bytes = v2_file(&[("RIG_Head", &[quarter])]);

        let anim = parse_animation(&bytes).expect("parses");
        let m = anim.nodes[0].frames[0];
        // Row 0 is the image of +X: row-vector convention, `v × M`.
        let x_axis = [m[0], m[1], m[2]];
        assert!(
            x_axis[0].abs() < 1e-6 && x_axis[1].abs() < 1e-6 && (x_axis[2] + 1.0).abs() < 1e-6,
            "+X must land on −Z, got {x_axis:?}"
        );
    }

    // Rule: version 1 stores the matrix outright — 64 bytes, not 40.
    #[test]
    fn version_1_frames_are_read_as_raw_matrices() {
        let mut bytes = 1u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&(4u32).to_le_bytes());
        bytes.extend_from_slice(b"HEAD");
        bytes.extend_from_slice(&1u32.to_le_bytes());
        let matrix: [f32; 16] = [
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.1, 0.2, 0.3, 1.0,
        ];
        for value in matrix {
            bytes.extend_from_slice(&value.to_le_bytes());
        }

        let anim = parse_animation(&bytes).expect("parses");

        assert_eq!(anim.version, 1);
        assert_eq!(anim.nodes[0].frames[0], matrix, "the matrix is taken as written");
    }

    // Rule: nodes are matched on the end of their name, so a mod that dropped
    // AC's `DRIVER:` prefix still animates the same rig.
    #[test]
    fn a_node_is_found_with_or_without_the_driver_prefix() {
        let bytes = v2_file(&[("DRIVER:RIG_Head", &[NEUTRAL])]);
        let anim = parse_animation(&bytes).expect("parses");

        assert!(anim.node("RIG_Head").is_some(), "matched on the suffix");
        assert!(anim.node("rig_head").is_some(), "and case-insensitively");
        assert!(anim.node("DRIVER:RIG_Head").is_some(), "the whole name works too");
        assert!(anim.node("RIG_Hips").is_none(), "another bone is not a match");
    }

    // Rule: the file comes from a mod, so a count it declares can never drive
    // an allocation the bytes could not hold.
    #[test]
    fn an_absurd_count_is_refused_rather_than_reserved() {
        let mut bytes = 2u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&i32::MAX.to_le_bytes());
        assert!(parse_animation(&bytes).is_err(), "a huge node count is refused");

        let mut bytes = 2u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&(-1i32).to_le_bytes());
        assert!(parse_animation(&bytes).is_err(), "a negative one too");
    }

    // Rule: truncation at any offset is an error, never a panic — same
    // guarantee the model parser gives.
    #[test]
    fn truncation_at_any_offset_errors_without_panic() {
        let whole = v2_file(&[("DRIVER:RIG_Center", &[NEUTRAL, NEUTRAL])]);
        for cut in 0..whole.len() {
            let _ = parse_animation(&whole[..cut]);
        }
    }
}
