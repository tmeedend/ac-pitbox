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
}
