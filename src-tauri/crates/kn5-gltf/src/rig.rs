//! Le mannequin exporté **vivant** : squelette, peau et animation de braquage
//! écrits dans le glTF au lieu d'être cuits dans les sommets
//! (`docs/SPEC-preview-3d-kn5.md` §4.6bis).
//!
//! **Pourquoi.** La pose des bras était figée à la conversion, donc l'angle de
//! braquage entrait dans la clé de cache dès qu'un pilote était greffé : chaque
//! valeur essayée coûtait une conversion complète et laissait une entrée de
//! plusieurs dizaines de mégaoctets. Les roues et le volant s'en sont
//! affranchis en se faisant *décrire* plutôt que tourner (voir
//! [`crate::steer`]) ; les bras, eux, ne peuvent pas : ils sont skinnés, donc
//! leur position dépend d'os. Il faut sortir les os.
//!
//! **Ce que ça demande, et ce qui était déjà là.** Un `Kn5SkinnedMesh` porte
//! exactement ce que glTF réclame — un nom et une `inverse_bind_matrix` par os
//! (`inverseBindMatrices`), quatre poids et quatre indices d'os par sommet
//! (`WEIGHTS_0` / `JOINTS_0`). Le parseur les lit depuis le début ; jusqu'ici
//! `geometry::convert_skinned_mesh` s'en servait pour mélanger les matrices et
//! jeter le résultat dans les sommets. Ce module écrit les mêmes chiffres dans
//! le fichier.
//!
//! **Les matrices ne sont pas transposées, et ce n'est pas un oubli.** Le KN5
//! travaille en ligne-vecteur (`v × M`) rangé par lignes, glTF en
//! colonne-vecteur (`M × v`) rangé par colonnes. Or le rangement par colonnes
//! de `Mᵀ` est **la même suite d'octets** que le rangement par lignes de `M` :
//! les deux conventions se croisent et s'annulent. La translation en indices
//! 12–14 des deux côtés en est le témoin visible.
//!
//! **La pose de repos reste celle du volant droit.** `driver::graft` continue
//! de poser l'assise (`driver_base_pos.knh`) et l'image centrale de
//! l'animation, exactement comme avant — tout ce qui a été mesuré sur l'assise
//! tient donc sans changer d'un iota. L'animation écrite par-dessus ne fait que
//! *rejouer* ce que la conversion figeait, et la vue y choisit une image.

use std::collections::BTreeMap;

use kn5::{Kn5Animation, Kn5Mesh, Kn5Node, Kn5NodeKind};

use crate::geometry::FlatMesh;

/// Nom du dummy qui enveloppe le mannequin greffé (voir `driver::graft`).
pub(crate) const DRIVER_WRAPPER: &str = "PITBOX_DRIVER";

/// Translation, rotation, échelle — ce que glTF anime. Un nœud animé ne peut
/// pas porter de matrice, la spec l'interdit, donc tout est décomposé.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trs {
    pub translation: [f32; 3],
    /// Quaternion `xyzw`, comme glTF l'attend.
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

impl Default for Trs {
    fn default() -> Self {
        Self {
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        }
    }
}

/// Un os : son nom, son parent dans le même tableau, sa transformation locale.
#[derive(Debug, Clone)]
pub struct Joint {
    pub name: String,
    pub parent: Option<usize>,
    pub rest: Trs,
}

/// Un maillage skinné, sommets **en espace de liaison** — non transformés.
#[derive(Debug, Clone)]
pub struct SkinnedMesh {
    pub mesh: FlatMesh,
    /// Quatre indices d'os par sommet, dans [`Rig::joints`].
    pub joints: Vec<[u16; 4]>,
    /// Les quatre poids correspondants.
    pub weights: Vec<[f32; 4]>,
    /// Les os que ce maillage utilise, dans l'ordre où ses indices les
    /// désignent — index dans [`Rig::joints`], résolus après la marche.
    pub bones: Vec<usize>,
    /// Les mêmes, par nom, tels que le maillage les nomme. Sert à les résoudre
    /// une fois la hiérarchie connue, et à rien d'autre.
    bone_names: Vec<String>,
    /// La matrice de liaison inverse de chacun, dans le même ordre.
    pub inverse_binds: Vec<[f32; 16]>,
}

/// Un maillage rigide accroché à un os — un casque sous `RIG_Head`.
///
/// **Il ne suffit pas de l'écrire à plat.** Aujourd'hui il suit par la
/// transformation accumulée, cuite dans ses sommets ; avec un squelette vivant
/// il resterait en arrière pendant que la tête tourne. C'est le piège de ce
/// lot, et `rh_schuberth_helmet_driver_19` en est le cas réel : cinq maillages
/// statiques accrochés à `DRIVER:RIG_Head`.
#[derive(Debug, Clone)]
pub struct AttachedMesh {
    pub joint: usize,
    /// Sommets dans le repère de l'os, pas en espace monde.
    pub mesh: FlatMesh,
}

/// Une piste d'animation : un os, et sa transformation locale image par image.
#[derive(Debug, Clone)]
pub struct Track {
    pub joint: usize,
    pub frames: Vec<Trs>,
}

/// Le mannequin, prêt à être écrit en glTF.
#[derive(Debug, Clone)]
pub struct Rig {
    pub joints: Vec<Joint>,
    pub skinned: Vec<SkinnedMesh>,
    pub attached: Vec<AttachedMesh>,
    pub tracks: Vec<Track>,
    /// Nombre d'images de l'animation, zéro quand il n'y en a pas.
    pub frames: usize,
    /// Les trois images qui comptent : volant à fond d'un côté, droit, à fond
    /// de l'autre — **mesurées sur l'animation**, pas déduites de sa longueur.
    ///
    /// `pose::frame_for` suppose la première image à une butée, la dernière à
    /// l'autre et celle du milieu au volant droit. Mesuré sur 120 animations de
    /// la bibliothèque, c'est faux : l'écart maximal à l'image du milieu tombe à
    /// **20 % du clip sur 39 voitures et 80 % sur 57**, et presque jamais aux
    /// extrémités. Une animation de braquage est une oscillation complète —
    /// droit, à fond, droit, à fond de l'autre côté, droit — et lire ses bouts
    /// comme des butées rend le volant droit précisément là où on le voulait à
    /// fond.
    pub extremes: (usize, usize, usize),
    /// Course que l'animation couvre en entier, en degrés **au volant**
    /// (`[STEER_ANIMATION] LOCK`).
    pub lock_degrees: f32,
    /// Angle de roue au-delà duquel le volant de la voiture est en butée. Les
    /// bras suivent le volant, donc ils s'arrêtent avec lui.
    pub wheel_limit: f32,
}

impl Rig {
    /// Y a-t-il de quoi animer ? Sans peau ni piste, il n'y a rien à gagner à
    /// exporter un squelette, et le chemin cuit reste le bon.
    pub fn is_worth_exporting(&self) -> bool {
        !self.skinned.is_empty() && self.frames > 1
    }
}

/// Extrait le mannequin greffé du modèle, ou `None` s'il n'y en a pas.
///
/// **Ne modifie pas le modèle** : `geometry::flatten` sait de son côté ignorer
/// le sous-arbre, en le reconnaissant au même nom. Deux passes plutôt qu'une
/// parce qu'elles ne produisent pas la même chose — l'une des sommets en espace
/// monde, l'autre une hiérarchie — et les mêler rendrait les deux illisibles.
pub(crate) fn extract(model: &kn5::Kn5Model, animation: Option<&Kn5Animation>) -> Option<Rig> {
    let wrapper = find_wrapper(&model.root)?;
    let mut rig = Rig {
        joints: Vec::new(),
        skinned: Vec::new(),
        attached: Vec::new(),
        tracks: Vec::new(),
        frames: 0,
        extremes: (0, 0, 0),
        lock_degrees: 360.0,
        wheel_limit: 25.0,
    };
    walk(wrapper, None, &mut rig);

    // Les os d'un maillage skinné sont nommés : on les traduit en index de
    // squelette une fois la hiérarchie connue.
    let by_name: BTreeMap<String, usize> = rig
        .joints
        .iter()
        .enumerate()
        .map(|(index, joint)| (joint.name.clone(), index))
        .collect();
    resolve_bones(&mut rig, &by_name);

    if let Some(animation) = animation {
        rig.frames = animation.frame_count();
        // **Une entrée d'animation ne pilote qu'un seul os**, le premier
        // rencontré — exactement la règle de `pose::apply_locals`, et pour la
        // même raison : `node_at` compare la **fin** du nom, pour qu'un mod
        // ayant laissé tomber le préfixe `DRIVER:` décrive quand même le
        // rig, et une entrée peut donc répondre à plusieurs os.
        //
        // Sans ce garde-fou, la même piste part sur plusieurs os et le
        // mannequin s'agite sur place au lieu de tourner un volant : mesuré au
        // banc, les mains décrivaient une oscillation de ±70° pour un balayage
        // cumulé de 7,8° sur toute l'animation, là où on attend une rotation
        // franche.
        let mut used = std::collections::HashSet::new();
        for (index, joint) in rig.joints.iter().enumerate() {
            let Some((entry, animated)) = animation.node_at(&joint.name) else {
                continue;
            };
            if !used.insert(entry) {
                continue;
            }
            let frames: Vec<Trs> = animated.frames.iter().map(decompose).collect();
            if frames.len() > 1 {
                rig.tracks.push(Track { joint: index, frames });
            }
        }
        rig.extremes = extremes(animation);
    }
    Some(rig)
}

/// Les trois images qui comptent : à fond d'un côté, droit, à fond de l'autre.
///
/// Le volant droit est l'image du **milieu** — c'est la seule chose que la
/// mesure confirme sans réserve, et c'est aussi la pose que la greffe cuit. Les
/// deux extrêmes sont cherchés de part et d'autre, comme les images qui
/// s'écartent le plus de celle-là, sur l'os qui bouge le plus.
fn extremes(animation: &Kn5Animation) -> (usize, usize, usize) {
    let frames = animation.frame_count();
    if frames < 3 {
        return (0, 0, frames.saturating_sub(1));
    }
    let middle = frames / 2;
    let spread = |node: &kn5::Kn5AnimatedNode, index: usize| -> f32 {
        match (node.frames.get(index), node.frames.get(middle)) {
            (Some(frame), Some(reference)) => frame.iter().zip(reference).map(|(a, b)| (a - b).abs()).sum(),
            _ => 0.0,
        }
    };
    let mut best = (0.0f32, middle, middle);
    for node in &animation.nodes {
        let low = (0..middle).max_by(|a, b| spread(node, *a).total_cmp(&spread(node, *b)));
        let high = (middle + 1..frames).max_by(|a, b| spread(node, *a).total_cmp(&spread(node, *b)));
        if let (Some(low), Some(high)) = (low, high) {
            let reach = spread(node, low) + spread(node, high);
            if reach > best.0 {
                best = (reach, low, high);
            }
        }
    }
    (best.1, middle, best.2)
}

/// Le dummy qui enveloppe le mannequin, cherché **sous** la racine : celle-ci
/// porte le même nom (voir `driver::graft`), et se prendre pour le mannequin
/// emporterait la voiture avec.
fn find_wrapper(root: &Kn5Node) -> Option<&Kn5Node> {
    for child in &root.children {
        if child.name == DRIVER_WRAPPER {
            return Some(child);
        }
        if let Some(found) = find_wrapper(child) {
            return Some(found);
        }
    }
    None
}

fn walk(node: &Kn5Node, parent: Option<usize>, rig: &mut Rig) {
    match &node.kind {
        Kn5NodeKind::Dummy { transform } => {
            let index = rig.joints.len();
            rig.joints.push(Joint {
                name: node.name.clone(),
                parent,
                rest: decompose(transform),
            });
            for child in &node.children {
                walk(child, Some(index), rig);
            }
        }
        Kn5NodeKind::Mesh(mesh) => {
            if let Some(joint) = parent {
                if let Some(flat) = rigid(&node.name, mesh) {
                    rig.attached.push(AttachedMesh { joint, mesh: flat });
                }
            }
            for child in &node.children {
                walk(child, parent, rig);
            }
        }
        Kn5NodeKind::SkinnedMesh(skinned) => {
            if let Some(built) = skin(&node.name, skinned) {
                rig.skinned.push(built);
            }
            for child in &node.children {
                walk(child, parent, rig);
            }
        }
    }
}

/// Traduit les noms d'os de chaque peau en index de squelette.
///
/// Un os que la hiérarchie ne porte pas fait tomber la peau entière : mieux
/// vaut un mannequin cuit qu'un mannequin dont un membre part à l'origine.
fn resolve_bones(rig: &mut Rig, by_name: &BTreeMap<String, usize>) {
    rig.skinned.retain_mut(|skinned| {
        let resolved: Option<Vec<usize>> = skinned.bone_names.iter().map(|n| by_name.get(n).copied()).collect();
        match resolved {
            Some(indices) => {
                skinned.bones = indices;
                true
            }
            None => false,
        }
    });
}

/// Un maillage rigide, sommets laissés dans le repère de son os.
fn rigid(name: &str, mesh: &Kn5Mesh) -> Option<FlatMesh> {
    if mesh.vertices.is_empty() || mesh.indices.len() < 3 || !mesh.is_renderable || !mesh.is_visible {
        return None;
    }
    Some(crate::geometry::bind_pose_mesh(name, mesh))
}

fn skin(name: &str, skinned: &kn5::Kn5SkinnedMesh) -> Option<SkinnedMesh> {
    let mesh = &skinned.mesh;
    if mesh.vertices.is_empty() || mesh.indices.len() < 3 || !mesh.is_visible {
        return None;
    }
    if skinned.bones.is_empty() || skinned.skin.len() != mesh.vertices.len() {
        return None;
    }
    let mut joints = Vec::with_capacity(mesh.vertices.len());
    let mut weights = Vec::with_capacity(mesh.vertices.len());
    for binding in &skinned.skin {
        // Les indices sont rangés en flottants dans le fichier (§3.4).
        let mut slot = [0u16; 4];
        for (out, raw) in slot.iter_mut().zip(binding.bone_indices) {
            let index = raw.max(0.0).round() as usize;
            if index >= skinned.bones.len() {
                return None;
            }
            *out = index as u16;
        }
        joints.push(slot);
        weights.push(binding.weights);
    }
    Some(SkinnedMesh {
        mesh: crate::geometry::bind_pose_mesh(name, mesh),
        joints,
        weights,
        bones: Vec::new(),
        bone_names: skinned.bones.iter().map(|b| b.name.clone()).collect(),
        inverse_binds: skinned.bones.iter().map(|b| b.inverse_bind_matrix).collect(),
    })
}

/// Décompose une transformation en translation, rotation, échelle.
///
/// Le rangement est celui de glTF (voir l'en-tête du module) : la translation
/// occupe les indices 12–14 et la base tient dans les trois premières colonnes.
/// Une transformation en miroir — déterminant négatif — voit son échelle en X
/// niée, faute de quoi le quaternion extrait serait celui de la rotation
/// *sans* le miroir et la pièce se retournerait.
pub(crate) fn decompose(m: &[f32; 16]) -> Trs {
    let column = |c: usize| [m[c * 4], m[c * 4 + 1], m[c * 4 + 2]];
    let length = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let (x, y, z) = (column(0), column(1), column(2));
    let mut scale = [length(x), length(y), length(z)];
    if determinant(m) < 0.0 {
        scale[0] = -scale[0];
    }
    let safe = |v: [f32; 3], s: f32| {
        if s.abs() <= f32::EPSILON {
            [0.0, 0.0, 0.0]
        } else {
            [v[0] / s, v[1] / s, v[2] / s]
        }
    };
    let (x, y, z) = (safe(x, scale[0]), safe(y, scale[1]), safe(z, scale[2]));
    Trs {
        translation: [m[12], m[13], m[14]],
        rotation: quaternion(&[x, y, z]),
        scale,
    }
}

fn determinant(m: &[f32; 16]) -> f32 {
    m[0] * (m[5] * m[10] - m[6] * m[9]) - m[4] * (m[1] * m[10] - m[2] * m[9]) + m[8] * (m[1] * m[6] - m[2] * m[5])
}

/// Quaternion `xyzw` d'une base orthonormée donnée par ses trois colonnes.
///
/// Méthode de Shepperd : on part de la plus grande des quatre composantes, ce
/// qui évite la division par un nombre proche de zéro que la formule directe
/// produit sur les demi-tours.
fn quaternion(basis: &[[f32; 3]; 3]) -> [f32; 4] {
    let r = |row: usize, col: usize| basis[col][row];
    let trace = r(0, 0) + r(1, 1) + r(2, 2);
    if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        return normalize4([
            (r(2, 1) - r(1, 2)) / s,
            (r(0, 2) - r(2, 0)) / s,
            (r(1, 0) - r(0, 1)) / s,
            0.25 * s,
        ]);
    }
    if r(0, 0) > r(1, 1) && r(0, 0) > r(2, 2) {
        let s = (1.0 + r(0, 0) - r(1, 1) - r(2, 2)).sqrt() * 2.0;
        return normalize4([
            0.25 * s,
            (r(0, 1) + r(1, 0)) / s,
            (r(0, 2) + r(2, 0)) / s,
            (r(2, 1) - r(1, 2)) / s,
        ]);
    }
    if r(1, 1) > r(2, 2) {
        let s = (1.0 + r(1, 1) - r(0, 0) - r(2, 2)).sqrt() * 2.0;
        return normalize4([
            (r(0, 1) + r(1, 0)) / s,
            0.25 * s,
            (r(1, 2) + r(2, 1)) / s,
            (r(0, 2) - r(2, 0)) / s,
        ]);
    }
    let s = (1.0 + r(2, 2) - r(0, 0) - r(1, 1)).sqrt() * 2.0;
    normalize4([
        (r(0, 2) + r(2, 0)) / s,
        (r(1, 2) + r(2, 1)) / s,
        0.25 * s,
        (r(1, 0) - r(0, 1)) / s,
    ])
}

fn normalize4(q: [f32; 4]) -> [f32; 4] {
    let length = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if length <= f32::EPSILON {
        return [0.0, 0.0, 0.0, 1.0];
    }
    [q[0] / length, q[1] / length, q[2] / length, q[3] / length]
}

#[cfg(test)]
mod tests {
    use super::*;

    // Règle : une transformation décomposée puis recomposée est la même. C'est
    // la seule garantie qui compte — glTF n'accepte que du TRS sur un nœud
    // animé, et une décomposition fausse déplace un membre sans rien signaler.
    #[test]
    fn decomposing_a_transform_loses_nothing() {
        // Rotation d'un tiers de tour autour de Y, échelle 2 en X, translation.
        let (s, c) = (std::f32::consts::FRAC_PI_3).sin_cos();
        let m = [
            2.0 * c,
            0.0,
            2.0 * -s,
            0.0,
            0.0,
            1.0,
            0.0,
            0.0,
            s,
            0.0,
            c,
            0.0,
            0.4,
            0.9,
            -0.2,
            1.0,
        ];
        let trs = decompose(&m);
        assert_eq!(trs.translation, [0.4, 0.9, -0.2], "la translation est lue telle quelle");
        assert!(
            (trs.scale[0] - 2.0).abs() < 1e-5,
            "l'échelle sort de la longueur des colonnes"
        );
        assert!((trs.scale[1] - 1.0).abs() < 1e-5);
        assert!((trs.scale[2] - 1.0).abs() < 1e-5);
        let back = compose(&trs);
        for (index, (a, b)) in m.iter().zip(back.iter()).enumerate() {
            assert!((a - b).abs() < 1e-4, "composante {index} : {a} contre {b}");
        }
    }

    // Règle : un demi-tour est le cas qui casse la formule directe d'extraction
    // du quaternion (division par une trace nulle). Shepperd le passe.
    #[test]
    fn a_half_turn_survives_the_decomposition() {
        let m = [
            -1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, -1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ];
        let trs = decompose(&m);
        let back = compose(&trs);
        for (a, b) in m.iter().zip(back.iter()) {
            assert!((a - b).abs() < 1e-4, "{a} contre {b}");
        }
    }

    /// Recompose une matrice à partir de son TRS, pour vérifier la
    /// décomposition. Uniquement en test : la conversion, elle, n'a jamais
    /// besoin de refaire le chemin inverse.
    fn compose(trs: &Trs) -> [f32; 16] {
        let [x, y, z, w] = trs.rotation;
        let r = [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y + z * w),
            2.0 * (x * z - y * w),
            2.0 * (x * y - z * w),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z + x * w),
            2.0 * (x * z + y * w),
            2.0 * (y * z - x * w),
            1.0 - 2.0 * (x * x + y * y),
        ];
        let mut m = [0.0f32; 16];
        for column in 0..3 {
            for row in 0..3 {
                m[column * 4 + row] = r[column * 3 + row] * trs.scale[column];
            }
        }
        m[12] = trs.translation[0];
        m[13] = trs.translation[1];
        m[14] = trs.translation[2];
        m[15] = 1.0;
        m
    }

    /// Où sont les **extrêmes** d'une animation de braquage dans son clip.
    ///
    /// `pose::frame_for` suppose que la première image est la butée d'un côté,
    /// la dernière celle de l'autre, et celle du milieu le volant droit. Mesuré
    /// au banc sur `lotus_evora_gtc`, c'est faux : les images 0, 50 et 99 y
    /// donnent toutes le volant droit, et les extrêmes sont aux quarts. Cette
    /// mesure dit si le cas est général.
    ///
    /// Pour chaque animation, on prend l'os dont la rotation varie le plus et
    /// on regarde à quelle fraction du clip elle s'écarte le plus de l'image du
    /// milieu.
    ///
    /// ```text
    /// PITBOX_CARS_ROOT=... cargo test -p kn5-gltf -- --ignored --nocapture where_the_extremes
    /// ```
    #[test]
    #[ignore = "needs a real corpus of cars; measurement, not a check"]
    fn where_the_extremes_of_a_steering_animation_are() {
        let Ok(root) = std::env::var("PITBOX_CARS_ROOT") else {
            eprintln!("PITBOX_CARS_ROOT unset, skipping");
            return;
        };
        let mut buckets: std::collections::BTreeMap<i32, usize> = std::collections::BTreeMap::new();
        let mut total = 0;
        for entry in std::fs::read_dir(&root).expect("read the corpus root").flatten() {
            let car = entry.path();
            if !car.is_dir() {
                continue;
            }
            let version = std::fs::read_dir(&car)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .find(|p| p.is_dir())
                .unwrap_or(car.clone());
            let path = version.join("animations").join("steer.ksanim");
            let Ok(bytes) = std::fs::read(&path) else { continue };
            let Ok(animation) = kn5::parse_animation(&bytes) else {
                continue;
            };
            let frames = animation.frame_count();
            if frames < 8 {
                continue;
            }
            total += 1;
            // L'os qui bouge le plus, et l'image où il s'écarte le plus de
            // celle du milieu.
            let middle = frames / 2;
            let mut best = (0.0f32, 0usize);
            for node in &animation.nodes {
                let Some(reference) = node.frames.get(middle) else {
                    continue;
                };
                for (index, frame) in node.frames.iter().enumerate() {
                    let spread: f32 = frame.iter().zip(reference.iter()).map(|(a, b)| (a - b).abs()).sum();
                    if spread > best.0 {
                        best = (spread, index);
                    }
                }
            }
            let fraction = ((best.1 as f32 / (frames - 1) as f32) * 20.0).round() as i32 * 5;
            *buckets.entry(fraction).or_default() += 1;
        }
        eprintln!("{total} animations ; fraction du clip où l'écart au milieu est maximal :");
        for (fraction, count) in &buckets {
            eprintln!("  {fraction:3} % : {count}");
        }
    }
}
