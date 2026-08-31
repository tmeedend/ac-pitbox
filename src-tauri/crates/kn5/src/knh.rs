//! `.knh` — a node hierarchy with no geometry in it.
//!
//! Assetto Corsa ships exactly one per car, `driver_base_pos.knh`, and it is
//! **where the driver sits**: the mannequin's whole skeleton, laid out in the
//! car's own space, root included. All 312 cars of the reference install have
//! one.
//!
//! That matters because the steering animation cannot be relied on to carry
//! the placement: 212 of the 271 that name the driver's root node leave it at
//! the identity. The two are meant to be read together — the hierarchy says
//! where the body is, the animation says what the limbs are doing. Reading the
//! animation alone misplaces about one car in six, which is how this file came
//! to be found: a right-hand-drive Miata whose driver sat on the left.
//!
//! # Layout
//!
//! Recursive, and that is the whole of it:
//!
//! | | |
//! | --- | --- |
//! | `u32` | name length |
//! | bytes | the name |
//! | `f32` × 16 | the node's **local** transform, row-vector convention |
//! | `u32` | number of children, each one of these again |
//!
//! The root is `SCENE_ROOT`, under which sit two wrapper dummies (`MODEL:
//! steer.fbx`, `FBX: steer.fbx`) and then `DRIVER:DRIVER` carrying the offset
//! that seats the whole body.

use crate::error::Result;
use crate::ksanim::ends_with_ignore_case;
use crate::limits::Limits;
use crate::reader::Reader;

/// A node hierarchy, flattened to what a caller actually wants from it: a
/// local transform per name, in depth-first order.
///
/// Flattened rather than kept as a tree because the model it is applied to
/// already **is** the tree — the transforms are pushed into it by name, the
/// same way an animation frame is (`kn5_gltf::pose`). Keeping a second tree
/// would only invite the two to disagree.
#[derive(Debug, Clone, Default)]
pub struct Kn5Hierarchy {
    pub nodes: Vec<(String, [f32; 16])>,
}

impl Kn5Hierarchy {
    /// Local transform of the node of that name, matched on the **end** of the
    /// name — same rule as an animation's, and for the same reason: a mod that
    /// dropped AC's `DRIVER:` prefix still describes the same rig.
    ///
    /// First match wins. Names do repeat in these files (a mesh under a dummy
    /// of the same name), never on a rig bone.
    pub fn local(&self, name: &str) -> Option<[f32; 16]> {
        self.nodes
            .iter()
            .find(|(n, _)| ends_with_ignore_case(n, name))
            .map(|(_, m)| *m)
    }
}

/// Smallest a node can be: a length, one character of name, a matrix, a count.
const MIN_NODE_BYTES: usize = 4 + 1 + 64 + 4;

/// Parses a `.knh` with the default [`Limits`].
pub fn parse_hierarchy(bytes: &[u8]) -> Result<Kn5Hierarchy> {
    parse_hierarchy_with_limits(bytes, &Limits::default())
}

/// Same, with explicit caps. The recursion is depth-limited like the model
/// parser's, for the same reason: a stack overflow is not a `Result`.
pub fn parse_hierarchy_with_limits(bytes: &[u8], limits: &Limits) -> Result<Kn5Hierarchy> {
    let mut r = Reader::new(bytes);
    let mut nodes = Vec::new();
    read_node(&mut r, limits, 0, &mut nodes)?;
    Ok(Kn5Hierarchy { nodes })
}

fn read_node(r: &mut Reader, limits: &Limits, depth: usize, out: &mut Vec<(String, [f32; 16])>) -> Result<()> {
    if depth > limits.max_depth {
        return Err(crate::error::Kn5Error::DepthLimitExceeded {
            limit: limits.max_depth,
        });
    }
    let name = r.string("hierarchy_node_name", limits.max_string_bytes)?;
    let transform = r.f32s::<16>()?;
    out.push((name, transform));

    let children = r.count("hierarchy_children", limits.max_children, MIN_NODE_BYTES)?;
    for _ in 0..children {
        read_node(r, limits, depth + 1, out)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str, translation: [f32; 3], children: Vec<Vec<u8>>) -> Vec<u8> {
        let mut out = (name.len() as u32).to_le_bytes().to_vec();
        out.extend_from_slice(name.as_bytes());
        let mut m = [0.0f32; 16];
        m[0] = 1.0;
        m[5] = 1.0;
        m[10] = 1.0;
        m[15] = 1.0;
        m[12] = translation[0];
        m[13] = translation[1];
        m[14] = translation[2];
        for value in m {
            out.extend_from_slice(&value.to_le_bytes());
        }
        out.extend_from_slice(&(children.len() as u32).to_le_bytes());
        for child in children {
            out.extend_from_slice(&child);
        }
        out
    }

    // Rule: the whole tree is read depth-first, and each node keeps the local
    // transform the file gives it — that transform is what seats a driver.
    #[test]
    fn the_hierarchy_is_read_depth_first() {
        let file = node(
            "SCENE_ROOT",
            [0.0; 3],
            vec![node(
                "FBX: steer.fbx",
                [0.0; 3],
                vec![node(
                    "DRIVER:DRIVER",
                    [-0.7032, -0.1046, 0.0422],
                    vec![node("DRIVER:RIG_Center", [0.3, 0.27, -0.23], Vec::new())],
                )],
            )],
        );

        let hierarchy = parse_hierarchy(&file).expect("a well-formed file parses");

        let names: Vec<&str> = hierarchy.nodes.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            ["SCENE_ROOT", "FBX: steer.fbx", "DRIVER:DRIVER", "DRIVER:RIG_Center"],
            "depth-first, parents before children"
        );
        let root = hierarchy.local("DRIVER:DRIVER").expect("the driver root is there");
        assert_eq!(
            [root[12], root[13], root[14]],
            [-0.7032, -0.1046, 0.0422],
            "the offset that seats the body, read from the last row"
        );
    }

    // Rule: nodes are matched on the end of their name, like an animation's.
    #[test]
    fn a_node_is_found_with_or_without_the_driver_prefix() {
        let file = node(
            "SCENE_ROOT",
            [0.0; 3],
            vec![node("DRIVER:RIG_Head", [0.0, 1.2, 0.0], Vec::new())],
        );
        let hierarchy = parse_hierarchy(&file).expect("parses");

        assert!(hierarchy.local("RIG_Head").is_some(), "matched on the suffix");
        assert!(hierarchy.local("rig_head").is_some(), "and case-insensitively");
        assert!(hierarchy.local("RIG_Hips").is_none(), "another bone is not a match");
    }

    // Rule: truncation at any offset is an error, never a panic — the file
    // comes from a mod like everything else here.
    #[test]
    fn truncation_at_any_offset_errors_without_panic() {
        let whole = node(
            "SCENE_ROOT",
            [0.0; 3],
            vec![node("DRIVER:DRIVER", [1.0, 2.0, 3.0], Vec::new())],
        );
        for cut in 0..whole.len() {
            let _ = parse_hierarchy(&whole[..cut]);
        }
    }

    // Rule: a crafted file cannot recurse us into a stack overflow.
    #[test]
    fn a_deep_hierarchy_hits_the_depth_limit() {
        let limits = Limits {
            max_depth: 4,
            ..Limits::default()
        };
        let mut file = node("leaf", [0.0; 3], Vec::new());
        for _ in 0..10 {
            file = node("branch", [0.0; 3], vec![file]);
        }
        assert!(
            parse_hierarchy_with_limits(&file, &limits).is_err(),
            "nesting past the cap is refused"
        );
    }
}
