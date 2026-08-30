//! Posing a rig from a `.ksanim` frame (spec §4.6bis).
//!
//! An animation holds one **local** transform per node per frame — the same
//! slot a dummy node carries in the model. Posing is therefore nothing but
//! swapping those matrices in; the hierarchy walk in `geometry` does the rest,
//! and every rigid mesh hanging off a bone (a helmet under `RIG_Head`) follows
//! for free.
//!
//! What does **not** follow for free is the skinned geometry — the suit and the
//! gloves are bound to the rig by weights, not by parentage. See
//! `geometry::skin`: posing without skinning would leave the body in its bind
//! pose while the head moved away from it, which is the one failure mode worth
//! naming out loud.
//!
//! **Frames are snapped, not interpolated.** A steering animation is 100
//! frames over the car's full lock, so one frame is 3.6° of wheel on the usual
//! 360° car — finer than anyone can see on a still preview, and interpolating
//! would mean blending quaternions we deliberately turned into matrices at
//! parse time.

use kn5::{Kn5Animation, Kn5Model, Kn5Node, Kn5NodeKind};

/// Applies one frame of `animation` to `model`, in place. Returns how many
/// nodes were actually posed.
///
/// A node the animation does not name keeps the transform it was shipped
/// with — which is how the twenty-odd animations that carry no head chain
/// still pose a body correctly.
pub(crate) fn apply(model: &mut Kn5Model, animation: &Kn5Animation, frame: usize) -> usize {
    let mut posed = 0;
    pose_node(&mut model.root, animation, frame, &mut posed);
    posed
}

fn pose_node(node: &mut Kn5Node, animation: &Kn5Animation, frame: usize, posed: &mut usize) {
    if let Kn5NodeKind::Dummy { transform } = &mut node.kind {
        if let Some(animated) = animation.node(&node.name) {
            // `min` rather than a bounds check that gives up: a node with
            // fewer frames than the rest still has a last pose, and holding it
            // beats leaving that limb in its bind pose while the others move.
            if let Some(frame) = animated.frames.get(frame.min(animated.frames.len().saturating_sub(1))) {
                *transform = *frame;
                *posed += 1;
            }
        }
    }
    for child in &mut node.children {
        pose_node(child, animation, frame, posed);
    }
}

/// Frame that shows the wheel turned by `steer_degrees`.
///
/// The animation spans the car's whole steering lock — `[STEER_ANIMATION]
/// LOCK` of `driver3d.ini` — from full left at the first frame to full right
/// at the last, with the **middle frame centred** (measured, see
/// `kn5::ksanim`). An angle beyond the car's own lock is clamped rather than
/// wrapped: a wheel does not keep turning past its stop.
pub(crate) fn frame_for(animation: &Kn5Animation, lock_degrees: f32, steer_degrees: f32) -> usize {
    let frames = animation.frame_count();
    if frames <= 1 {
        return 0;
    }
    // A lock of zero would divide by nothing; centre is the honest answer.
    let ratio = if lock_degrees.abs() < f32::EPSILON {
        0.0
    } else {
        (steer_degrees / lock_degrees).clamp(-0.5, 0.5)
    };
    let position = (ratio + 0.5) * (frames - 1) as f32;
    (position.round().max(0.0) as usize).min(frames - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn animation(frames: usize) -> Kn5Animation {
        Kn5Animation {
            version: 2,
            nodes: vec![kn5::Kn5AnimatedNode {
                name: "DRIVER:RIG_Center".to_string(),
                frames: (0..frames)
                    .map(|i| {
                        let mut m = [0.0f32; 16];
                        m[0] = 1.0;
                        m[5] = 1.0;
                        m[10] = 1.0;
                        m[15] = 1.0;
                        // A marker in the translation, so a test can tell one
                        // frame from another.
                        m[12] = i as f32;
                        m
                    })
                    .collect(),
            }],
        }
    }

    // Rule: the middle frame is the wheel centred, the ends are the stops
    // (§4.6bis). Off by one here and every driver preview is slightly turned.
    #[test]
    fn the_middle_frame_is_the_wheel_centred() {
        let anim = animation(101);
        assert_eq!(frame_for(&anim, 360.0, 0.0), 50, "centred is the middle frame");
        assert_eq!(frame_for(&anim, 360.0, -180.0), 0, "full left is the first");
        assert_eq!(frame_for(&anim, 360.0, 180.0), 100, "full right is the last");
        assert_eq!(frame_for(&anim, 360.0, 90.0), 75, "a quarter of the lock, a quarter in");
    }

    // Rule: a car's own lock decides what an angle means — 90° is half of a
    // 180° car's travel and a quarter of a 360° one's.
    #[test]
    fn the_angle_is_read_against_the_cars_own_lock() {
        let anim = animation(101);
        assert_eq!(frame_for(&anim, 180.0, 90.0), 100, "90° is full lock on a 180° car");
        assert_eq!(frame_for(&anim, 360.0, 90.0), 75, "and a quarter turn on a 360° one");
    }

    // Rule: past the stop, the wheel stays at the stop.
    #[test]
    fn an_angle_beyond_the_lock_is_clamped_not_wrapped() {
        let anim = animation(101);
        assert_eq!(frame_for(&anim, 180.0, 900.0), 100, "far past full right");
        assert_eq!(frame_for(&anim, 180.0, -900.0), 0, "and past full left");
        assert_eq!(frame_for(&anim, 0.0, 90.0), 50, "a car with no lock stays centred");
    }

    // Rule: posing replaces a dummy's local transform, and leaves a node the
    // animation does not name exactly as it was.
    #[test]
    fn only_the_named_nodes_move() {
        let identity = {
            let mut m = [0.0f32; 16];
            m[0] = 1.0;
            m[5] = 1.0;
            m[10] = 1.0;
            m[15] = 1.0;
            m
        };
        let dummy = |name: &str| Kn5Node {
            name: name.to_string(),
            active: true,
            kind: Kn5NodeKind::Dummy { transform: identity },
            children: Vec::new(),
        };
        let mut model = Kn5Model {
            version: 6,
            extra: None,
            textures: Vec::new(),
            materials: Vec::new(),
            root: Kn5Node {
                children: vec![dummy("DRIVER:RIG_Center"), dummy("DRIVER:RIG_Foot_L")],
                ..dummy("root")
            },
        };

        let posed = apply(&mut model, &animation(101), 7);

        assert_eq!(posed, 1, "one node of the two is animated");
        let Kn5NodeKind::Dummy { transform } = model.root.children[0].kind else {
            panic!("still a dummy");
        };
        assert_eq!(transform[12], 7.0, "the named node took frame 7");
        let Kn5NodeKind::Dummy { transform } = model.root.children[1].kind else {
            panic!("still a dummy");
        };
        assert_eq!(transform, identity, "the other kept what it was shipped with");
    }

    // Rule: a node with fewer frames than the rest holds its last pose rather
    // than snapping back to the bind one.
    #[test]
    fn a_short_track_holds_its_last_frame() {
        let anim = animation(3);
        let mut model = Kn5Model {
            version: 6,
            extra: None,
            textures: Vec::new(),
            materials: Vec::new(),
            root: Kn5Node {
                name: "DRIVER:RIG_Center".to_string(),
                active: true,
                kind: Kn5NodeKind::Dummy { transform: [0.0; 16] },
                children: Vec::new(),
            },
        };

        assert_eq!(apply(&mut model, &anim, 99), 1, "posed anyway");
        let Kn5NodeKind::Dummy { transform } = model.root.kind else {
            panic!("still a dummy");
        };
        assert_eq!(transform[12], 2.0, "held on the last frame it has");
    }
}
