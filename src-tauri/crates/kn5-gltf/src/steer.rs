//! Les roues braquées d'une voiture à l'arrêt — les roues avant et le volant
//! (`docs/SPEC-preview-3d-kn5.md` §15).
//!
//! **Rien dans un modèle de voiture ne dit de combien elles tournent.** Le
//! `steer.ksanim` d'une voiture pose les membres du pilote et **eux seuls** —
//! mesuré sur la bibliothèque, pas un de ses nœuds animés n'est en dehors du
//! rig. AC tourne les roues depuis la physique et le volant depuis la manette.
//!
//! **Et la rotation n'est pas cuite dans le modèle converti.** Elle l'a été, et
//! c'était une erreur mesurée à l'usage : l'angle entrait alors dans la clé de
//! cache, si bien que chaque valeur essayée laissait une entrée complète —
//! onze entrées de 42 Mo pour une seule voiture, sur un plafond de 2 Gio, parce
//! qu'un curseur se balaye. Ce module ne fait donc tourner personne : il
//! **décrit** ce qui tourne, et la vue applique l'angle à l'affichage.
//!
//! Deux rotations, et elles ne partagent pas leur axe :
//!
//! - les **roues avant** tournent autour de la verticale, par leur centre. Le
//!   vrai pivot de fusée est incliné (chasse, inclinaison), mais de quelques
//!   degrés sur une voiture de route — invisible à côté de la vingtaine dont la
//!   roue tourne, et l'axe de cette inclinaison n'est nulle part dans le
//!   modèle ;
//! - le **volant** tourne autour de sa colonne, que chaque voiture oriente à sa
//!   façon. L'axe est donc *mesuré sur le volant lui-même* : c'est un disque,
//!   donc l'axe local selon lequel il est plat est celui autour duquel il
//!   tourne (voir [`disc_axis`]). Mesuré sur la bibliothèque : Z sur 95
//!   voitures, Y sur 4 — aucune convention à coder en dur, mais la direction
//!   que ces axes désignent dans l'espace de la voiture est longitudinale à
//!   chaque fois, et c'est ce qui dit que la mesure tient (voir
//!   [`column_axis`]).

use kn5::{Kn5Mesh, Kn5Node};

/// Ce que la voiture déclare de sa direction, et qui décide de combien ses
/// roues tournent pour un angle de volant donné.
///
/// Traverse jusqu'ici plutôt que de rester dans l'application parce que c'est
/// le convertisseur qui l'écrit dans le modèle : la vue tourne les roues sans
/// rien savoir du `car.ini`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SteerLimits {
    /// Course du volant, du centre à la butée, en degrés (`STEER_LOCK`).
    pub lock: f32,
    /// Degrés de volant par degré de roue (`STEER_RATIO`).
    pub ratio: f32,
}

/// Ce que la bibliothèque déclare le plus souvent — voir
/// `steering::how_steering_is_written_across_a_corpus`.
impl Default for SteerLimits {
    fn default() -> Self {
        Self {
            lock: 360.0,
            ratio: 14.0,
        }
    }
}

/// Un nœud qui tourne avec le braquage, décrit pour la vue.
///
/// Écrit dans le `.glb` en `extras` du nœud glTF, avec les sommets exprimés
/// **relativement au pivot** et le pivot posé en translation : la vue n'a plus
/// qu'à écrire une rotation sur le nœud, et tout ce qu'il porte suit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SteerNode {
    /// Regroupe les maillages d'un même nœud braqué. Deux roues tournent du
    /// même angle autour du même axe mais **pas autour du même pivot**, donc
    /// elles ne peuvent pas fusionner, même à matériau égal.
    pub group: u32,
    /// Centre de rotation, en espace monde.
    pub pivot: [f32; 3],
    /// Axe de rotation, en espace monde, normalisé.
    pub axis: [f32; 3],
    /// Facteur appliqué à l'angle demandé, qui est celui des **roues** : 1
    /// pour une roue, la démultiplication de la voiture pour le volant, qui
    /// tourne d'autant plus.
    pub gain: f32,
    /// Au-delà de cet angle **aux roues**, ce nœud-ci ne tourne plus. `None`
    /// quand rien ne l'arrête.
    ///
    /// **Seul le volant en a un.** Les roues, non — délibérément. La butée
    /// déclarée (`STEER_LOCK / STEER_RATIO`) est juste : mesurée sur les
    /// voitures de route de l'installation, elle donne 33,6° à la MX-5, 32,1°
    /// à l'AE86, 29,6° à l'Abarth 500, ce qui est la réalité. Mais AC met dans
    /// `STEER_LOCK` le débattement **utile** d'un volant de simulation, pas la
    /// butée mécanique, et les voitures de course y déclarent très peu :
    /// 20,2° à la 488 GT3, **12,0° à la Huracán GT3**. Une voiture de course
    /// posée pour la photo se retrouvait roues quasi droites (signalé à
    /// l'écran). L'aperçu n'est pas une simulation : le réglage donne l'angle
    /// des roues, point. Le volant, lui, garde sa butée — c'est la seule chose
    /// qu'`AC` dise sans ambiguïté de la course du volant.
    pub limit: Option<f32>,
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

/// Décrit le nœud braqué qu'on vient de rencontrer, ou `None` quand il n'y a
/// rien de fiable à faire tourner ici.
pub(crate) fn describe(
    node: &Kn5Node,
    what: Steered,
    world: &[f32; 16],
    limits: &SteerLimits,
    group: u32,
) -> Option<SteerNode> {
    // **L'angle demandé est celui des roues**, pas celui du volant. C'est ce
    // qu'on veut régler en regardant une voiture à l'arrêt, et c'est ce qui se
    // voit : un volant tourné d'un demi-tour ne braque les roues que de treize
    // degrés sur une démultiplication de quatorze, et le curseur, gradué au
    // volant, s'arrêtait alors bien avant la butée (signalé à l'écran).
    if limits.ratio.abs() < f32::EPSILON {
        return None;
    }
    let (gain, limit, axis) = match what {
        Steered::RoadWheel => (1.0, None, [0.0, 1.0, 0.0]),
        // Le volant tourne, lui, autant de fois plus que la démultiplication,
        // et s'arrête à la course que la voiture déclare.
        Steered::SteeringWheel => (
            limits.ratio,
            Some(limits.lock / limits.ratio),
            column_axis(node, world)?,
        ),
    };
    // **Le pivot est le milieu de la géométrie, pas l'origine du nœud.** Une
    // rotation ne dépend pas du point de son axe qu'on choisit, donc les deux
    // s'accordent tant que l'origine est sur l'axe — et divergent sinon.
    // Mesuré avec l'origine pour pivot : un sommet parcourait jusqu'à **4,3 m**
    // sur la pire voiture, là où un volant qui fait un demi-tour se déplace de
    // deux fois son rayon. Certains mods accrochent le volant à un nœud posé à
    // l'origine de la voiture, et un demi-tour autour d'un point à deux mètres
    // l'envoie à travers l'habitacle. Le milieu du volant est sur son axe par
    // construction.
    let centre = local_bounds(node).map(|(min, max)| {
        [
            (min[0] + max[0]) / 2.0,
            (min[1] + max[1]) / 2.0,
            (min[2] + max[2]) / 2.0,
        ]
    })?;
    let length = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if length <= f32::EPSILON {
        return None;
    }
    Some(SteerNode {
        group,
        pivot: transform_point(world, centre),
        axis: [axis[0] / length, axis[1] / length, axis[2] / length],
        gain,
        limit,
    })
}

/// L'axe de la colonne de direction, en espace monde, ou `None` quand rien ici
/// n'y ressemble.
///
/// **Deux critères, tous deux géométriques.** Le plan d'un volant contient
/// forcément la direction 9 h – 3 h, c'est-à-dire la largeur de la voiture :
/// l'axe local le plus **latéral** est donc dans le plan de la roue, jamais sa
/// colonne, et on l'écarte d'abord. Les deux qui restent sont perpendiculaires
/// entre eux et tous deux perpendiculaires à cette largeur : ils vivent donc
/// dans le plan vertical longitudinal, à 90° l'un de l'autre. Des deux, la
/// colonne est la **moins verticale** — un volant fait face au pilote, donc sa
/// colonne pointe vers l'arrière en montant un peu, quand la hauteur du volant,
/// elle, est presque debout.
///
/// **Deux critères de forme ont été essayés avant, et tous deux échouent.**
/// « Le plus plat des trois » : un `STEER_HR` ne contient pas qu'une couronne,
/// il porte la colonne, les palettes, parfois un écran, et ces excroissances
/// s'étendent le long de l'axe qu'on cherche à trouver court — neuf voitures y
/// perdaient leur volant. « Le plus court des deux restants » : un volant de
/// formule est large et bas, donc sa hauteur est plus courte que sa
/// profondeur, et sept voitures ressortaient avec une colonne inclinée à 75° ou
/// plus, c'est-à-dire debout. Prendre les déciles plutôt que la boîte entière
/// était pire encore : les sommets d'un volant sont massés au centre (boutons,
/// écran, moyeu), donc le décile mesure ce paquet-là et plus du tout la
/// couronne.
///
/// **Ce qui dit que le critère tient.** Sur la bibliothèque, 103 volants sur
/// les 108 nommés se décrivent, et l'axe trouvé est longitudinal à chaque fois
/// — composante latérale 0,000 à la médiane, 0,070 au pire. Son inclinaison
/// médiane est de **20°**, ce qui est la nappe d'une vraie colonne de
/// direction ; l'inclinaison ne peut pas dépasser 45° par construction, donc
/// c'est la médiane qui informe, pas la borne. Les cinq indécis sont des
/// monoplaces dont le nœud est plus profond que large.
///
/// Le veto latéral reste en garde-fou pour les voitures qu'on n'a pas vues :
/// un axe qui ressortirait en travers de la voiture serait une mesure ratée, et
/// la réponse honnête est alors de ne rien tourner.
fn column_axis(node: &Kn5Node, world: &[f32; 16]) -> Option<[f32; 3]> {
    let (min, max) = local_bounds(node)?;
    let extents = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
    // La direction que chaque axe local désigne dans l'espace de la voiture.
    let directions: Vec<[f32; 3]> = (0..3)
        .map(|axis| {
            let mut local = [0.0f32; 3];
            local[axis] = 1.0;
            let d = transform_direction(world, local);
            let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            if length <= f32::EPSILON {
                [0.0; 3]
            } else {
                [d[0] / length, d[1] / length, d[2] / length]
            }
        })
        .collect();
    let across = (0..3).max_by(|a, b| directions[*a][0].abs().total_cmp(&directions[*b][0].abs()))?;
    let column = (0..3)
        .filter(|axis| *axis != across)
        .min_by(|a, b| directions[*a][1].abs().total_cmp(&directions[*b][1].abs()))?;
    // Garde-fou de forme, et le seul : un volant est **plus large que profond**.
    // Un nœud qui porterait ce nom sans être une roue — un cube, un moyeu —
    // s'arrête ici plutôt que de tourner autour d'une direction arbitraire.
    if extents[across] <= 0.0 || extents[column] >= extents[across] {
        return None;
    }
    let axis = directions[column];
    if axis[0].abs() > MAX_LATERAL_AXIS {
        return None;
    }
    Some(axis)
}

/// How far across the car a steering column may point before the measurement
/// is disbelieved. Nothing in the library comes close (worst 0.070).
const MAX_LATERAL_AXIS: f32 = 0.5;

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

    // Règle : l'axe du volant se **mesure**. L'axe latéral est dans le plan de
    // la roue par construction, donc écarté ; des deux qui restent, la colonne
    // est le plus court.
    #[test]
    fn the_column_is_the_short_axis_that_is_not_the_lateral_one() {
        assert_eq!(
            column_axis(&disc(0.05), &IDENTITY),
            Some([0.0, 0.0, 1.0]),
            "plat en Z, X étant latéral : la colonne est Z"
        );
    }

    // Règle : et deux directions qui se valent ne désignent rien. Faire tourner
    // le volant autour d'un axe tiré au hasard serait pire que ne rien faire.
    #[test]
    fn a_lump_has_no_column() {
        assert_eq!(
            column_axis(&disc(0.4), &IDENTITY),
            None,
            "aussi épais que large : rien à conclure"
        );
    }

    // Règle : un axe qui ressortirait en travers de la voiture est une mesure
    // ratée. Aucune voiture de la bibliothèque n'en produit ; le veto est là
    // pour celles qu'on n'a pas vues.
    #[test]
    fn an_axis_pointing_across_the_car_is_disbelieved() {
        // Un quart de tour autour de Y envoie l'axe Z du disque sur X.
        let sideways = [
            0.0, 0.0, -1.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            1.0, 0.0, 0.0, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ];
        assert_eq!(column_axis(&disc(0.05), &sideways), None);
    }

    // Règle : le pivot décrit est le **centre de la géométrie**, pas l'origine
    // du nœud — sinon un volant accroché à un nœud posé à l'origine de la
    // voiture traverse l'habitacle au premier demi-tour.
    #[test]
    fn the_pivot_is_the_middle_of_the_geometry() {
        let mut node = disc(0.05);
        for vertex in match &mut node.kind {
            Kn5NodeKind::Mesh(mesh) => &mut mesh.vertices,
            _ => unreachable!(),
        } {
            vertex.position[1] += 0.8;
        }
        let described = describe(&node, Steered::SteeringWheel, &IDENTITY, &SteerLimits::default(), 3)
            .expect("un volant se décrit");
        assert!(
            (described.pivot[1] - 0.8).abs() < 1e-4,
            "le pivot suit la géométrie : {:?}",
            described.pivot
        );
        assert_eq!(described.group, 3, "le groupe est celui qu'on lui donne");
    }

    // Règle : l'angle demandé est celui des **roues**. Une roue le prend tel
    // quel, le volant le multiplie par la démultiplication, et les deux
    // s'arrêtent ensemble à la butée de la voiture ramenée aux roues.
    #[test]
    fn the_angle_asked_for_is_the_one_the_road_wheels_take() {
        let limits = SteerLimits {
            lock: 480.0,
            ratio: 12.0,
        };
        let wheel =
            describe(&disc(0.05), Steered::RoadWheel, &IDENTITY, &limits, 0).expect("une roue se décrit toujours");
        assert_eq!(wheel.axis, [0.0, 1.0, 0.0], "autour de la verticale");
        assert_eq!(wheel.gain, 1.0, "la roue tourne de l'angle demandé");
        assert_eq!(
            wheel.limit, None,
            "et rien ne l'arrête : l'aperçu n'est pas une simulation"
        );

        let rim = describe(&disc(0.05), Steered::SteeringWheel, &IDENTITY, &limits, 1).expect("un volant se décrit");
        assert_eq!(rim.gain, 12.0, "le volant tourne douze fois plus que la roue");
        assert_eq!(rim.limit, Some(40.0), "et bute à 480° de volant, soit 40° de roue");
    }

    // Règle : une démultiplication nulle ne décrit rien plutôt que de diviser
    // par zéro. Elle ne devrait pas exister, mais elle vient d'un fichier de
    // mod.
    #[test]
    fn a_zero_ratio_describes_nothing() {
        let broken = SteerLimits {
            lock: 400.0,
            ratio: 0.0,
        };
        assert_eq!(describe(&disc(0.05), Steered::RoadWheel, &IDENTITY, &broken, 0), None);
    }

    /// Combien de volants de la bibliothèque se décrivent, et vers où pointe
    /// l'axe trouvé.
    ///
    /// ```text
    /// PITBOX_CARS_ROOT=... cargo test -p kn5-gltf -- --ignored --nocapture columns
    /// ```
    #[test]
    #[ignore = "needs a real corpus of cars; measurement, not a check"]
    fn columns_of_every_steering_wheel() {
        let Ok(root) = std::env::var("PITBOX_CARS_ROOT") else {
            eprintln!("PITBOX_CARS_ROOT unset, skipping");
            return;
        };
        let (mut total, mut missing, mut undecided, mut decided) = (0, 0, 0, 0);
        let mut lateral: Vec<f32> = Vec::new();
        let mut tilt: Vec<f32> = Vec::new();
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
            let worlds = crate::geometry::node_world_matrices(&parsed);
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
            let world = worlds.get(&node.name).copied().unwrap_or(IDENTITY);
            match column_axis(&node, &world) {
                Some(axis) => {
                    decided += 1;
                    lateral.push(axis[0].abs());
                    // Inclinaison de la colonne sur l'horizontale.
                    tilt.push(axis[1].abs().asin().to_degrees());
                }
                None => {
                    undecided += 1;
                    eprintln!(
                        "  {} : volant non décrit",
                        car.file_name().unwrap_or_default().to_string_lossy()
                    );
                }
            }
        }
        lateral.sort_by(f32::total_cmp);
        tilt.sort_by(f32::total_cmp);
        eprintln!("{total} voitures, {missing} sans volant nommé, {decided} décrits, {undecided} indécis");
        if !lateral.is_empty() {
            eprintln!(
                "part latérale de l'axe (0 = longitudinal) : médiane {:.3} · 90e {:.3} · max {:.3}",
                lateral[lateral.len() / 2],
                lateral[lateral.len() * 9 / 10],
                lateral[lateral.len() - 1]
            );
            eprintln!(
                "inclinaison de la colonne : min {:.0}° · médiane {:.0}° · max {:.0}°",
                tilt[0],
                tilt[tilt.len() / 2],
                tilt[tilt.len() - 1]
            );
        }
    }
}
