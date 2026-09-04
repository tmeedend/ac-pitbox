//! Turning a parked car's wheels — the road wheels and the steering wheel
//! (`docs/SPEC-preview-3d-kn5.md` §15).
//!
//! **Nothing in a car model says how far they turn.** The car's own
//! `steer.ksanim` poses the driver's limbs and *only* those — measured on the
//! reference library, not one of its animated nodes is outside the driver's
//! rig. AC turns the road wheels from physics and the steering wheel from the
//! player's input, so a still preview has to do both itself.
//!
//! Two rotations, and they do not share an axis:
//!
//! - the **road wheels** turn about the vertical, through their own centre.
//!   The real kingpin leans (caster, inclination), but by a few degrees on a
//!   road car — invisible next to the twenty the wheel itself turns by, and
//!   the axis it leans about is nowhere in the model;
//! - the **steering wheel** turns about its column, which every car orients
//!   differently. The axis is therefore *measured on the wheel itself*: it is
//!   a disc, so the local axis it is flattest along is the one it spins about
//!   (see [`disc_axis`]). Measured over the library: Z on 95 cars, Y on 4 —
//!   no convention to hard-code, but the direction those axes point in world
//!   space is longitudinal every time, which is what says the measurement is
//!   sound (see [`column_axis`]).
//!
//! **What turning actually moves**, measured over the library
//! (`what_turning_the_wheels_actually_moves`): 2 to 27 % of a car's vertices,
//! travelling 0.07 m at the least and 0.44 m at the ninetieth percentile —
//! twice the radius of a steering wheel doing half a turn, which is the figure
//! to expect. One car reads 4.3 m, `ms_citroen_berlingo_2003_hdi`, and it is
//! not the steering: that model's own geometry spans **616 m**, a stray rim
//! vertex nineteen metres from its wheel. Broken before anything turned.

use kn5::{Kn5Mesh, Kn5Node};

/// How far to turn what, in degrees. Zero on both counts leaves the model
/// exactly as it was — the ordinary case, and it costs nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SteerPose {
    /// Front road wheels, about the vertical. The application works this out
    /// from the car's own `STEER_RATIO` (`steering::Steering`).
    pub road_wheel_degrees: f32,
    /// The steering wheel the driver holds, about its column.
    pub steering_wheel_degrees: f32,
}

impl SteerPose {
    pub fn is_straight(&self) -> bool {
        self.road_wheel_degrees == 0.0 && self.steering_wheel_degrees == 0.0
    }
}

/// The two nodes AC steers, by the names every car uses.
///
/// Measured on the reference library: `WHEEL_LF` and `WHEEL_RF` on 144 models
/// of 134, `STEER_HR` on 108 and `STEER_LR` on 69 — the two cockpits, high and
/// low resolution, the same way `COCKPIT_HR` and `COCKPIT_LR` come in pairs.
/// A car naming them otherwise simply keeps its wheels straight; there is
/// nothing to break.
const FRONT_WHEELS: [&str; 2] = ["WHEEL_LF", "WHEEL_RF"];
const STEERING_WHEELS: [&str; 2] = ["STEER_HR", "STEER_LR"];

/// What this node is to the steering, if anything.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Steered {
    /// A front road wheel: turns about the vertical, through its own centre.
    RoadWheel,
    /// The wheel in the driver's hands: turns about its own column.
    SteeringWheel,
}

pub(crate) fn steered(name: &str) -> Option<Steered> {
    if FRONT_WHEELS.iter().any(|n| name.eq_ignore_ascii_case(n)) {
        return Some(Steered::RoadWheel);
    }
    if STEERING_WHEELS.iter().any(|n| name.eq_ignore_ascii_case(n)) {
        return Some(Steered::SteeringWheel);
    }
    None
}

/// The extra world-space transform that turns this node's whole subtree.
///
/// Row-vector convention throughout (`world = local × parent`), so a transform
/// applied **after** the accumulated one is appended on the right: the caller
/// does `world × turn(...)`.
pub(crate) fn turn(node: &Kn5Node, what: Steered, world: &[f32; 16], pose: &SteerPose) -> Option<[f32; 16]> {
    let (degrees, axis) = match what {
        Steered::RoadWheel => (pose.road_wheel_degrees, [0.0, 1.0, 0.0]),
        // The column axis is a direction of the node's *local* frame, and the
        // rotation happens in world space: it has to be carried over.
        Steered::SteeringWheel => (pose.steering_wheel_degrees, column_axis(node, world)?),
    };
    if degrees == 0.0 {
        return None;
    }
    // **The pivot is the middle of the geometry, not the node's origin.** A
    // rotation about an axis does not care which point of that axis it is
    // written about, so the two agree whenever the origin sits on the axis —
    // and they part company when it does not. Measured over the library with
    // the origin as pivot: a vertex travelled up to **4,3 m** on the worst car,
    // where a steering wheel turned half a turn should move by twice its own
    // radius. Some models hang the wheel off a node parked at the car's origin,
    // and half a turn about a point two metres away throws it across the
    // cockpit. The middle of the wheel is on its axis by construction.
    let centre = local_bounds(node).map(|(min, max)| {
        [
            (min[0] + max[0]) / 2.0,
            (min[1] + max[1]) / 2.0,
            (min[2] + max[2]) / 2.0,
        ]
    })?;
    Some(about(axis, degrees.to_radians(), transform_point(world, centre)))
}

/// The measured column axis, in world space, or `None` when it does not look
/// like a steering column.
///
/// **The veto is a safety net, not a filter.** Measured on the library, the
/// axis found this way is longitudinal on every car that has one: its sideways
/// component reads 0.000 at the median and 0.070 at the worst, over 99 cars —
/// and that includes the four whose *local* flat axis is Y where the other 95
/// use Z. The convention differs, the direction does not. A wheel that came out
/// pointing across the car would therefore be a measurement gone wrong, and the
/// honest answer is to leave it alone.
fn column_axis(node: &Kn5Node, world: &[f32; 16]) -> Option<[f32; 3]> {
    let axis = transform_direction(world, disc_axis(node)?);
    let length = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if length <= f32::EPSILON || (axis[0] / length).abs() > MAX_LATERAL_AXIS {
        return None;
    }
    Some(axis)
}

/// How far across the car a steering column may point before the measurement
/// is disbelieved. Nothing in the library comes close (worst 0.070).
const MAX_LATERAL_AXIS: f32 = 0.5;

/// Direction the geometry under this node is **flattest** along, in the node's
/// own local frame — a steering wheel is a disc, so that is its column.
///
/// Read off the model rather than assumed, because no convention holds: on the
/// reference library the flattest local axis is X on some cars, Y or Z on
/// others, and the ratio to the next axis says how sure one can be (measured
/// in `flattest_axis_of_every_steering_wheel`).
///
/// `None` when the subtree carries no vertices, or when it is not flat enough
/// to call — a shape that is as thick as it is wide has no obvious axis, and
/// guessing one would send the wheel spinning about something arbitrary.
pub(crate) fn disc_axis(node: &Kn5Node) -> Option<[f32; 3]> {
    let (min, max) = local_bounds(node)?;
    let extents = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
    let flattest = (0..3).min_by(|a, b| extents[*a].total_cmp(&extents[*b]))?;
    let others: Vec<f32> = (0..3).filter(|i| *i != flattest).map(|i| extents[i]).collect();
    let widest = others.iter().copied().fold(0.0f32, f32::max);
    if widest <= 0.0 || extents[flattest] > widest * MAX_DISC_THICKNESS {
        return None;
    }
    let mut axis = [0.0f32; 3];
    axis[flattest] = 1.0;
    Some(axis)
}

/// A disc is at most this fraction as thick as it is wide. A steering wheel
/// with its column stub is thicker than a coin, hence a generous figure — but
/// not so generous that a lump gets called a disc.
const MAX_DISC_THICKNESS: f32 = 0.6;

/// Bounding box of every vertex under this node, in the node's own frame —
/// i.e. its own local transform is **not** applied, its children's are.
fn local_bounds(node: &Kn5Node) -> Option<([f32; 3], [f32; 3])> {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    let mut any = false;
    for child in &node.children {
        walk_bounds(child, &IDENTITY, &mut min, &mut max, &mut any);
    }
    // A node can carry geometry itself as well as children.
    if let Some(mesh) = node.mesh() {
        add_vertices(mesh, &IDENTITY, &mut min, &mut max, &mut any);
    }
    any.then_some((min, max))
}

fn walk_bounds(node: &Kn5Node, parent: &[f32; 16], min: &mut [f32; 3], max: &mut [f32; 3], any: &mut bool) {
    let world = match node.transform() {
        Some(local) => multiply(local, parent),
        None => *parent,
    };
    if let Some(mesh) = node.mesh() {
        add_vertices(mesh, &world, min, max, any);
    }
    for child in &node.children {
        walk_bounds(child, &world, min, max, any);
    }
}

fn add_vertices(mesh: &Kn5Mesh, world: &[f32; 16], min: &mut [f32; 3], max: &mut [f32; 3], any: &mut bool) {
    for vertex in &mesh.vertices {
        let p = transform_point(world, vertex.position);
        for axis in 0..3 {
            min[axis] = min[axis].min(p[axis]);
            max[axis] = max[axis].max(p[axis]);
        }
        *any = true;
    }
}

/// Rotation of `angle` about `axis`, through `pivot`, row-vector convention.
fn about(axis: [f32; 3], angle: f32, pivot: [f32; 3]) -> [f32; 16] {
    let length = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if length <= f32::EPSILON {
        return IDENTITY;
    }
    let (x, y, z) = (axis[0] / length, axis[1] / length, axis[2] / length);
    let (s, c) = angle.sin_cos();
    let t = 1.0 - c;
    // Rodrigues, transposed for row vectors: `v' = v × R`.
    let rotation = [
        t * x * x + c,
        t * x * y + s * z,
        t * x * z - s * y,
        0.0,
        t * x * y - s * z,
        t * y * y + c,
        t * y * z + s * x,
        0.0,
        t * x * z + s * y,
        t * y * z - s * x,
        t * z * z + c,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
    ];
    let to_origin = translation([-pivot[0], -pivot[1], -pivot[2]]);
    let back = translation(pivot);
    multiply(&multiply(&to_origin, &rotation), &back)
}

fn translation(t: [f32; 3]) -> [f32; 16] {
    let mut m = IDENTITY;
    m[12] = t[0];
    m[13] = t[1];
    m[14] = t[2];
    m
}

const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, //
    0.0, 0.0, 0.0, 1.0,
];

/// `a × b`, row-vector convention — the same product `geometry` composes the
/// hierarchy with, kept here so this module stands on its own.
fn multiply(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut out = [0.0f32; 16];
    for row in 0..4 {
        for column in 0..4 {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += a[row * 4 + k] * b[k * 4 + column];
            }
            out[row * 4 + column] = sum;
        }
    }
    out
}

fn transform_point(m: &[f32; 16], p: [f32; 3]) -> [f32; 3] {
    [
        p[0] * m[0] + p[1] * m[4] + p[2] * m[8] + m[12],
        p[0] * m[1] + p[1] * m[5] + p[2] * m[9] + m[13],
        p[0] * m[2] + p[1] * m[6] + p[2] * m[10] + m[14],
    ]
}

/// A direction, so the translation row is left out.
fn transform_direction(m: &[f32; 16], d: [f32; 3]) -> [f32; 3] {
    [
        d[0] * m[0] + d[1] * m[4] + d[2] * m[8],
        d[0] * m[1] + d[1] * m[5] + d[2] * m[9],
        d[0] * m[2] + d[1] * m[6] + d[2] * m[10],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use kn5::Kn5NodeKind;

    fn disc(thickness: f32) -> Kn5Node {
        // Un disque dans le plan XY : plat le long de Z.
        let mut vertices = Vec::new();
        for step in 0..16 {
            let angle = step as f32 / 16.0 * std::f32::consts::TAU;
            for z in [-thickness / 2.0, thickness / 2.0] {
                vertices.push(kn5::Kn5Vertex {
                    position: [angle.cos() * 0.2, angle.sin() * 0.2, z],
                    normal: [0.0, 0.0, 1.0],
                    uv: [0.0, 0.0],
                    tangent: [1.0, 0.0, 0.0],
                });
            }
        }
        Kn5Node {
            name: "STEER_HR".to_string(),
            active: true,
            kind: Kn5NodeKind::Mesh(Kn5Mesh {
                cast_shadows: true,
                is_visible: true,
                is_transparent: false,
                indices: vec![0, 1, 2],
                vertices,
                material_id: 0,
                layer: 0,
                lod_in: 0.0,
                lod_out: 0.0,
                bounding_sphere_center: [0.0; 3],
                bounding_sphere_radius: 1.0,
                is_renderable: true,
            }),
            children: Vec::new(),
        }
    }

    // Règle : l'axe du volant se **mesure** sur sa géométrie — c'est la
    // direction selon laquelle il est plat. Aucune convention de nom ou d'axe
    // ne tient sur le corpus.
    #[test]
    fn a_flat_disc_names_the_axis_it_is_flat_along() {
        assert_eq!(disc_axis(&disc(0.05)), Some([0.0, 0.0, 1.0]), "plat en Z, donc axe Z");
    }

    // Règle : et un objet qui n'est pas plat n'a pas d'axe évident. Le faire
    // tourner autour d'une direction arbitraire serait pire que ne rien faire.
    #[test]
    fn a_lump_has_no_axis() {
        assert_eq!(disc_axis(&disc(0.4)), None, "aussi épais que large : rien à conclure");
    }

    // Règle : la rotation se fait autour du **centre du nœud**, pas de
    // l'origine du modèle — sinon une roue braquée part à l'autre bout de la
    // voiture.
    #[test]
    fn the_turn_pivots_on_the_node_not_the_origin() {
        let mut world = IDENTITY;
        world[12] = 0.8; // la roue avant gauche, à 80 cm de l'axe
        world[14] = 1.4;
        let pose = SteerPose {
            road_wheel_degrees: 90.0,
            steering_wheel_degrees: 0.0,
        };
        let node = disc(0.05);
        let turn = turn(&node, Steered::RoadWheel, &world, &pose).expect("un angle non nul tourne");
        let moved = multiply(&world, &turn);
        assert!(
            (moved[12] - 0.8).abs() < 1e-4 && (moved[14] - 1.4).abs() < 1e-4,
            "le centre de la roue ne bouge pas : {:?}",
            [moved[12], moved[13], moved[14]]
        );
        // Un point 30 cm devant la roue se retrouve 30 cm sur son côté.
        let ahead = transform_point(&moved, [0.0, 0.0, 0.3]);
        assert!(
            (ahead[2] - 1.4).abs() < 1e-3 && ahead[0].abs() > 1.0,
            "braquée d'un quart de tour : {ahead:?}"
        );
    }

    // Règle : un angle nul ne produit aucune transformation — la conversion
    // d'une voiture roues droites doit rester octet pour octet celle d'avant.
    #[test]
    fn a_straight_wheel_is_left_alone() {
        let node = disc(0.05);
        assert_eq!(turn(&node, Steered::RoadWheel, &IDENTITY, &SteerPose::default()), None);
        assert!(SteerPose::default().is_straight());
    }

    /// Ce que le braquage déplace **réellement**, voiture par voiture : la
    /// mesure qui dit que la rotation ne fuit pas ailleurs que sur les roues.
    ///
    /// ```text
    /// PITBOX_CARS_ROOT="D:\AC-Library\cars" cargo test -p kn5-gltf -- --ignored --nocapture what_turning
    /// ```
    #[test]
    #[ignore = "needs a real corpus of cars; measurement, not a check"]
    fn what_turning_the_wheels_actually_moves() {
        let Ok(root) = std::env::var("PITBOX_CARS_ROOT") else {
            eprintln!("PITBOX_CARS_ROOT unset, skipping");
            return;
        };
        let pose = SteerPose {
            road_wheel_degrees: 13.0,
            steering_wheel_degrees: 180.0,
        };
        let (mut cars, mut wheels_moved, mut nothing_moved) = (0, 0, 0);
        let mut moved_share: Vec<f32> = Vec::new();
        let mut travel: Vec<f32> = Vec::new();
        for entry in std::fs::read_dir(&root).expect("read the corpus root").flatten() {
            let car = entry.path();
            if !car.is_dir() {
                continue;
            }
            let Some(version) = std::fs::read_dir(&car)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .find(|p| p.is_dir())
            else {
                continue;
            };
            let Some(model) = crate::resolve_model(&version) else {
                continue;
            };
            let Ok(bytes) = std::fs::read(&model.path) else {
                continue;
            };
            let Ok(parsed) = kn5::parse(&bytes) else { continue };
            cars += 1;

            let straight = crate::GeometryOptions::default();
            let turned = crate::GeometryOptions {
                steer: pose,
                ..Default::default()
            };
            let (before, _) = crate::geometry::flatten(&parsed, &straight);
            let (after, _) = crate::geometry::flatten(&parsed, &turned);
            if before.len() != after.len() {
                eprintln!("  {} : le braquage a changé le découpage !", car.display());
                continue;
            }
            let (mut moved_vertices, mut total_vertices, mut worst) = (0usize, 0usize, 0.0f32);
            let mut culprit = String::new();
            for (a, b) in before.iter().zip(after.iter()) {
                for (p, q) in a.positions.iter().zip(b.positions.iter()) {
                    total_vertices += 1;
                    let d = ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt();
                    if d > 1e-5 {
                        moved_vertices += 1;
                        if d > worst {
                            worst = d;
                            culprit = a.name.clone();
                        }
                    }
                }
            }
            // Un demi-mètre est déjà large : un volant d'un rayon de 18 cm qui
            // fait un demi-tour déplace ses rayons de 36 cm, et une roue avant
            // braquée de 13° de moins de 10. Au-delà, c'est la voiture qu'il
            // faut regarder, pas le braquage.
            if worst > 0.8 {
                eprintln!(
                    "  {} : {worst:.2} m sur `{culprit}`",
                    car.file_name().unwrap().to_string_lossy()
                );
            }
            if moved_vertices == 0 {
                nothing_moved += 1;
                continue;
            }
            wheels_moved += 1;
            moved_share.push(moved_vertices as f32 / total_vertices.max(1) as f32);
            travel.push(worst);
        }
        moved_share.sort_by(|a, b| a.partial_cmp(b).unwrap());
        travel.sort_by(|a, b| a.partial_cmp(b).unwrap());
        eprintln!("{cars} voitures, {wheels_moved} braquées, {nothing_moved} inchangées");
        if !moved_share.is_empty() {
            eprintln!(
                "part des sommets déplacés : min {:.2} % · médiane {:.2} % · max {:.2} %",
                moved_share[0] * 100.0,
                moved_share[moved_share.len() / 2] * 100.0,
                moved_share[moved_share.len() - 1] * 100.0
            );
            eprintln!(
                "déplacement maximal : min {:.3} m · médiane {:.3} m · 90e {:.3} m · max {:.3} m",
                travel[0],
                travel[travel.len() / 2],
                travel[travel.len() * 9 / 10],
                travel[travel.len() - 1]
            );
        }
    }

    /// Selon quel axe local chaque volant de la bibliothèque est plat, et à
    /// quel point le verdict est net.
    ///
    /// ```text
    /// PITBOX_CARS_ROOT="D:\AC-Library\cars" cargo test -p kn5-gltf -- --ignored --nocapture flattest
    /// ```
    #[test]
    #[ignore = "needs a real corpus of cars; measurement, not a check"]
    fn flattest_axis_of_every_steering_wheel() {
        let Ok(root) = std::env::var("PITBOX_CARS_ROOT") else {
            eprintln!("PITBOX_CARS_ROOT unset, skipping");
            return;
        };
        let mut axes = [0usize; 3];
        let (mut undecided, mut missing, mut total) = (0, 0, 0);
        let mut ratios: Vec<f32> = Vec::new();
        let mut lateral: Vec<f32> = Vec::new();
        for entry in std::fs::read_dir(&root).expect("read the corpus root").flatten() {
            let car = entry.path();
            if !car.is_dir() {
                continue;
            }
            let Some(version) = std::fs::read_dir(&car)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .find(|p| p.is_dir())
            else {
                continue;
            };
            let Some(model) = crate::resolve_model(&version) else {
                continue;
            };
            let Ok(bytes) = std::fs::read(&model.path) else {
                continue;
            };
            let Ok(parsed) = kn5::parse(&bytes) else { continue };
            total += 1;
            let mut found = None;
            parsed.visit_nodes(&mut |node| {
                if found.is_none() && steered(&node.name) == Some(Steered::SteeringWheel) {
                    found = Some(node.clone());
                }
            });
            let Some(node) = found else {
                missing += 1;
                continue;
            };
            let Some(([lo, hi], _)) = local_bounds(&node).map(|(a, b)| ([a, b], ())) else {
                missing += 1;
                continue;
            };
            let extents = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
            let flattest = (0..3).min_by(|a, b| extents[*a].total_cmp(&extents[*b])).unwrap();
            let widest = (0..3)
                .filter(|i| *i != flattest)
                .map(|i| extents[i])
                .fold(0.0f32, f32::max);
            if widest > 0.0 {
                ratios.push(extents[flattest] / widest);
            }
            match disc_axis(&node) {
                Some(_) => {
                    axes[flattest] += 1;
                    // L'axe ramené dans l'espace de la voiture : une colonne de
                    // direction est longitudinale et penchée, jamais latérale.
                    let world = crate::geometry::node_world_matrices(&parsed);
                    if let Some(m) = world.get(&node.name) {
                        let mut a = [0.0f32; 3];
                        a[flattest] = 1.0;
                        let d = transform_direction(m, a);
                        let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-6);
                        lateral.push((d[0] / n).abs());
                    }
                }
                None => undecided += 1,
            }
        }
        ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
        eprintln!("{total} voitures, {missing} sans volant nommé, {undecided} trop épais pour conclure");
        eprintln!("axe plat : X {} · Y {} · Z {}", axes[0], axes[1], axes[2]);
        if !ratios.is_empty() {
            eprintln!(
                "épaisseur / largeur : min {:.3} médiane {:.3} max {:.3}",
                ratios[0],
                ratios[ratios.len() / 2],
                ratios[ratios.len() - 1]
            );
        }
        lateral.sort_by(|a, b| a.partial_cmp(b).unwrap());
        if !lateral.is_empty() {
            eprintln!(
                "part latérale de l'axe (0 = longitudinal) : médiane {:.3} · 90e {:.3} · max {:.3} · au-dessus de 0,5 : {}",
                lateral[lateral.len() / 2],
                lateral[lateral.len() * 9 / 10],
                lateral[lateral.len() - 1],
                lateral.iter().filter(|v| **v > 0.5).count()
            );
        }
    }
}
