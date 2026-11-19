//! Where the ear sits for an audition, and how that becomes the vectors FMOD
//! wants. Pure geometry: no DLL, no thread — the engine thread only stores the
//! latest one and hands its attributes to the system.

use super::sys::Attributes3d;

/// Where the ear sits relative to the car, in the terms the interface has:
/// an orbit angle, a height angle and a distance. Turning that into vectors is
/// this module's job, not the caller's.
#[derive(Debug, Clone, Copy)]
pub struct Listener {
    /// Degrees around the car. 0 faces the nose, 180 the tail.
    pub azimuth: f32,
    /// Degrees above the horizon, clamped: at the poles "up" stops being
    /// definable and the orientation degenerates.
    pub elevation: f32,
    pub distance: f32,
}

impl Default for Listener {
    fn default() -> Self {
        // Three-quarter front, at the distance someone would stand to listen —
        // the same idea as the 3D preview's default framing.
        Listener {
            azimuth: 35.0,
            elevation: 8.0,
            distance: 4.0,
        }
    }
}

impl Listener {
    /// The listener always **faces the car**, which is what a camera orbiting a
    /// model does. That has a consequence worth stating: the source stays dead
    /// ahead, so stereo panning barely moves. What changes is the *timbre*,
    /// through `Event Cone Angle` — measured, and exactly the difference
    /// between standing at the bonnet and standing at the exhaust.
    pub(super) fn attributes(&self) -> Attributes3d {
        let az = self.azimuth.to_radians();
        let el = self.elevation.clamp(-80.0, 80.0).to_radians();
        let d = self.distance.max(0.5);
        let position = [az.sin() * el.cos() * d, el.sin() * d, az.cos() * el.cos() * d];
        let forward = normalize([-position[0], -position[1], -position[2]]);
        let world_up = [0.0, 1.0, 0.0];
        let right = normalize(cross(world_up, forward));
        let up = normalize(cross(forward, right));
        Attributes3d {
            position,
            velocity: [0.0; 3],
            forward,
            up,
        }
    }
}

/// The ear in the driver's seat, for the interior view: a point in the car's
/// space and the direction the head is turned.
///
/// Same frame as [`Listener`] — the car's centre at the origin, +Z the nose,
/// +Y up — so that the two can follow each other without the sound jumping.
/// Unlike the orbit, the ear does **not** face the car: it faces wherever the
/// driver looks, which is what turns the engine from one ear to the other as
/// the head turns.
#[derive(Debug, Clone, Copy)]
pub struct HeadListener {
    pub position: [f32; 3],
    /// Degrees from the nose (+Z) towards +X.
    pub yaw: f32,
    /// Degrees above the horizon, clamped like [`Listener::elevation`].
    pub pitch: f32,
}

impl HeadListener {
    pub(super) fn attributes(&self) -> Attributes3d {
        let yaw = self.yaw.to_radians();
        let pitch = self.pitch.clamp(-80.0, 80.0).to_radians();
        let forward = normalize([yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos()]);
        let right = normalize(cross([0.0, 1.0, 0.0], forward));
        let up = normalize(cross(forward, right));
        Attributes3d {
            position: self.position,
            velocity: [0.0; 3],
            forward,
            up,
        }
    }
}

/// Where the ear is, whichever way the interface put it there.
#[derive(Debug, Clone, Copy)]
pub enum Ear {
    /// Orbiting the car, facing it: the exterior view.
    Orbit(Listener),
    /// In the driver's seat: the interior view.
    Head(HeadListener),
}

impl Default for Ear {
    fn default() -> Self {
        Ear::Orbit(Listener::default())
    }
}

impl From<Listener> for Ear {
    fn from(listener: Listener) -> Self {
        Ear::Orbit(listener)
    }
}

impl Ear {
    pub(super) fn attributes(&self) -> Attributes3d {
        match self {
            Ear::Orbit(listener) => listener.attributes(),
            Ear::Head(head) => head.attributes(),
        }
    }
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len <= f32::EPSILON {
        [0.0, 0.0, 1.0]
    } else {
        [v[0] / len, v[1] / len, v[2] / len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The geometry, without any DLL: the ear must end up where the angles say,
    /// and its basis must be orthonormal or FMOD is entitled to refuse it.
    #[test]
    fn listener_geometry_is_orthonormal_and_faces_the_car() {
        for azimuth in [0.0, 35.0, 90.0, 180.0, 275.0] {
            for elevation in [-45.0, 0.0, 8.0, 60.0] {
                let listener = Listener {
                    azimuth,
                    elevation,
                    distance: 4.0,
                };
                let a = listener.attributes();

                let len = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                let dot = |u: [f32; 3], v: [f32; 3]| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];

                assert!(
                    (len(a.position) - 4.0).abs() < 1e-3,
                    "the ear sits at the requested distance, got {}",
                    len(a.position)
                );
                assert!((len(a.forward) - 1.0).abs() < 1e-4, "forward is normalised");
                assert!((len(a.up) - 1.0).abs() < 1e-4, "up is normalised");
                assert!(
                    dot(a.forward, a.up).abs() < 1e-4,
                    "forward and up must be perpendicular, dot = {}",
                    dot(a.forward, a.up)
                );
                // Facing the car means forward points from the ear back to the
                // origin — the whole reason the source stays centred.
                let to_car = normalize([-a.position[0], -a.position[1], -a.position[2]]);
                assert!(
                    dot(a.forward, to_car) > 0.999,
                    "the listener must look at the car, dot = {}",
                    dot(a.forward, to_car)
                );
            }
        }
    }

    /// In the seat the ear sits where it is put and faces where the head
    /// looks — never back at the car's centre, or turning the head would change
    /// nothing (FMOD§6octies).
    #[test]
    fn head_listener_sits_where_asked_and_faces_where_the_head_looks() {
        let dot = |u: [f32; 3], v: [f32; 3]| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
        let ahead = HeadListener {
            position: [0.33, 0.4, -0.2],
            yaw: 0.0,
            pitch: 0.0,
        }
        .attributes();
        assert_eq!(ahead.position, [0.33, 0.4, -0.2], "the ear is at the eyes");
        assert!(dot(ahead.forward, [0.0, 0.0, 1.0]) > 0.999, "yaw 0 looks at the nose");
        assert!(dot(ahead.up, [0.0, 1.0, 0.0]) > 0.999, "a level head keeps up up");

        let left = HeadListener {
            position: [0.0; 3],
            yaw: 90.0,
            pitch: 20.0,
        }
        .attributes();
        assert!(left.forward[0] > 0.9, "yaw 90 turns towards +X, got {:?}", left.forward);
        assert!(left.forward[1] > 0.3, "a raised pitch looks up, got {:?}", left.forward);
        assert!(dot(left.forward, left.up).abs() < 1e-4, "the basis stays orthonormal");
    }
}
