//! Node tree → flat list of meshes in glTF space (spec §3.4, §3.5, §4.4).
//!
//! Three things happen here, and each one has its own way of going wrong:
//! flattening the hierarchy, changing handedness, and dropping the nodes that
//! are not meant to be drawn.

use kn5::{Kn5Material, Kn5Mesh, Kn5Model, Kn5Node};

// **Aucune conversion de repère, et aucune inversion de la coordonnée V.**
// Le §4.4 de la spec demande les deux ; les deux sont fausses, et il a fallu
// deux erreurs successives pour l'établir.
//
// Deux mesures numériques disaient dès le départ « identité » :
//  - les noms de roues `WHEEL_LF`/`WHEEL_RF`, lus dans le repère droitier ;
//  - l'accord entre l'enroulement des triangles et les normales stockées,
//    à 100 % sur 1,3 million de triangles.
//
// Elles ont été écartées au lot 3 sur la foi d'un rendu de `ks_mazda_mx5_cup`,
// où la livrée semblait à l'endroit avec négation de X **et** inversion de V.
// C'était une coïncidence, et une coïncidence double :
//  - l'îlot UV du flanc est tourné à 90°, donc l'inversion de V agit
//    horizontalement sur la voiture et **annule** le miroir géométrique : le
//    texte redevient lisible alors que le modèle est bien en miroir ;
//  - l'atlas de cette voiture range ses deux flancs côte à côte et
//    quasi identiques, donc échantillonner l'un pour l'autre ne se voit pas.
//
// `abarth500` a cassé les deux illusions d'un coup : son atlas place la photo
// du compartiment moteur là où l'inversion de V envoie la portière. Un lancer
// de rayon sur la portière donnait `uv=(0.515, 1.619)` — 62 % de la hauteur de
// l'atlas, en plein moteur — contre 38 % sans inversion, soit le panneau.
//
// Explication de fond, cohérente avec la mesure : DirectX **et** glTF placent
// tous deux l'origine des textures en haut à gauche. L'inversion de V est
// nécessaire vers OpenGL, pas vers glTF.
//
// Leçon : valider une conversion sur une voiture dont l'atlas est symétrique
// ne prouve rien. Le test doit porter sur du texte **et** sur une zone d'atlas
// que la transformation fautive déplacerait visiblement.

/// One drawable mesh, already in glTF space and independent of any parent.
#[derive(Debug, Clone)]
pub struct FlatMesh {
    pub name: String,
    pub material_id: u32,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u16>,
    /// Kept for reporting: a mesh flagged transparent needs the render-order
    /// treatment of §8.2 on the viewer side.
    pub transparent: bool,
}

#[derive(Debug, Clone)]
pub struct GeometryOptions {
    /// Node name patterns that mark helpers rather than geometry. Kept as
    /// configuration rather than hard-coded in the middle of the walk (§3.5),
    /// because mods invent their own conventions.
    pub excluded_name_patterns: Vec<String>,
    /// Node name prefixes with the same effect.
    pub excluded_name_prefixes: Vec<String>,
    /// Drop meshes that only appear beyond a distance, i.e. lower LODs stored
    /// inside the same file.
    pub skip_distant_lods: bool,
}

impl Default for GeometryOptions {
    fn default() -> Self {
        Self {
            // `BLUR` : les jantes floutées d'AC (`GEO_rimblur1`, `RIM_BLUR_LF`),
            // qui ne remplacent la vraie jante qu'au-delà d'une certaine vitesse
            // de rotation. À l'arrêt elles se superposent à elle et brouillent
            // la roue. Motif sans souligné : les deux conventions de nommage
            // coexistent selon les voitures.
            excluded_name_patterns: ["COLLIDER", "_SHADOW", "AC_CRASH", "DAMAGE_GLASS", "BLUR"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            // `AC_` prefixes the game's helper dummies: `AC_POS_0`, `AC_PIT_0`,
            // `AC_START_0`… They carry no geometry worth showing.
            excluded_name_prefixes: vec!["AC_".to_string()],
            skip_distant_lods: true,
        }
    }
}

#[derive(Debug, Default)]
pub struct GeometryStats {
    pub kept: usize,
    pub skipped_hidden: usize,
    pub skipped_by_name: usize,
    pub skipped_empty: usize,
    pub skipped_distant_lod: usize,
    /// Meshes carrying the car's *broken* glass — see [`Skip::BrokenGlass`].
    pub skipped_broken_glass: usize,
    /// Meshes whose accumulated transform mirrors space, and whose winding was
    /// flipped a second time as a result.
    pub mirrored: usize,
}

/// Walks the tree, accumulates transforms and returns every mesh worth drawing.
pub fn flatten(model: &Kn5Model, options: &GeometryOptions) -> (Vec<FlatMesh>, GeometryStats) {
    let mut meshes = Vec::new();
    let mut stats = GeometryStats::default();
    walk(
        &model.root,
        &IDENTITY,
        &model.materials,
        options,
        &mut meshes,
        &mut stats,
    );
    (meshes, stats)
}

const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, //
    0.0, 0.0, 0.0, 1.0,
];

fn walk(
    node: &Kn5Node,
    parent_world: &[f32; 16],
    materials: &[Kn5Material],
    options: &GeometryOptions,
    meshes: &mut Vec<FlatMesh>,
    stats: &mut GeometryStats,
) {
    // Row-vector convention: `world = local × parent` (§3.4). Getting this
    // order backwards leaves the hierarchy intact but scatters every part.
    let world = match node.transform() {
        Some(local) => multiply(local, parent_world),
        None => *parent_world,
    };

    if let Some(mesh) = node.mesh() {
        match classify(node, mesh, materials, options) {
            Keep::No(reason) => match reason {
                Skip::Hidden => stats.skipped_hidden += 1,
                Skip::Name => stats.skipped_by_name += 1,
                Skip::Empty => stats.skipped_empty += 1,
                Skip::DistantLod => stats.skipped_distant_lod += 1,
                Skip::BrokenGlass => stats.skipped_broken_glass += 1,
            },
            Keep::Yes => {
                let flat = convert_mesh(&node.name, mesh, &world);
                if determinant3(&world) < 0.0 {
                    stats.mirrored += 1;
                }
                stats.kept += 1;
                meshes.push(flat);
            }
        }
    }

    for child in &node.children {
        walk(child, &world, materials, options, meshes, stats);
    }
}

enum Keep {
    Yes,
    No(Skip),
}

enum Skip {
    Hidden,
    Name,
    Empty,
    DistantLod,
    /// The shattered version of the car's glass. AC keeps it in the model at
    /// all times and only shows it once the pane has actually been broken —
    /// same mechanism as the damage normal map (docs/kn5-format.md, écart n°4),
    /// on a whole mesh rather than a texture slot. Drawn as it stands, it lays
    /// a grey pane and a web of cracks over the windscreen: the "dirty glass"
    /// the user kept reporting on `ks_toyota_supra_mkiv`.
    BrokenGlass,
}

fn classify(node: &Kn5Node, mesh: &Kn5Mesh, materials: &[Kn5Material], options: &GeometryOptions) -> Keep {
    if mesh.vertices.is_empty() || mesh.indices.len() < 3 {
        return Keep::No(Skip::Empty);
    }
    if !mesh.is_renderable || !mesh.is_visible {
        return Keep::No(Skip::Hidden);
    }
    if materials
        .get(mesh.material_id as usize)
        .is_some_and(|m| m.shader.contains("ksBrokenGlass"))
    {
        return Keep::No(Skip::BrokenGlass);
    }
    let name = node.name.to_ascii_uppercase();
    if options.excluded_name_prefixes.iter().any(|p| name.starts_with(p))
        || options.excluded_name_patterns.iter().any(|p| name.contains(p))
    {
        return Keep::No(Skip::Name);
    }
    // `lod_in` is the distance at which a mesh *starts* being drawn: anything
    // above zero belongs to a lower level of detail stored in the same file.
    if options.skip_distant_lods && mesh.lod_in > 0.0 {
        return Keep::No(Skip::DistantLod);
    }
    Keep::Yes
}

fn convert_mesh(name: &str, mesh: &Kn5Mesh, world: &[f32; 16]) -> FlatMesh {
    let normal_matrix = inverse_transpose3(world);
    let mut positions = Vec::with_capacity(mesh.vertices.len());
    let mut normals = Vec::with_capacity(mesh.vertices.len());
    let mut uvs = Vec::with_capacity(mesh.vertices.len());

    for vertex in &mesh.vertices {
        positions.push(transform_point(world, vertex.position));
        normals.push(normalize(transform_direction(&normal_matrix, vertex.normal)));
        uvs.push(vertex.uv);
    }

    let mut indices = mesh.indices.clone();
    // Le repère ne change pas, mais un nœud dont la transformation est en
    // miroir — la façon habituelle de ne modéliser qu'une moitié symétrique —
    // inverse l'orientation de ses triangles. glTF veut des faces avant en
    // sens antihoraire, donc on la rétablit là et seulement là (§10).
    if determinant3(world) < 0.0 {
        // `as_chunks_mut` plutôt que `chunks_exact_mut(3)` : la taille étant
        // constante, il rend des `[u32; 3]` au lieu de tranches de longueur
        // vérifiée à l'exécution. Clippy l'exige depuis Rust 1.98 — plus récent
        // que la chaîne locale, donc invisible ici et vu seulement en CI.
        for triangle in indices.as_chunks_mut::<3>().0 {
            triangle.swap(1, 2);
        }
    }

    FlatMesh {
        name: name.to_string(),
        material_id: mesh.material_id,
        positions,
        normals,
        uvs,
        indices,
        transparent: mesh.is_transparent,
    }
}

/// Row-major 4x4 product, row-vector convention.
fn multiply(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut out = [0.0f32; 16];
    for row in 0..4 {
        for col in 0..4 {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += a[row * 4 + k] * b[k * 4 + col];
            }
            out[row * 4 + col] = sum;
        }
    }
    out
}

/// `p × M` with `p` a row vector — the translation therefore comes from the
/// last row, not the last column.
fn transform_point(m: &[f32; 16], p: [f32; 3]) -> [f32; 3] {
    [
        p[0] * m[0] + p[1] * m[4] + p[2] * m[8] + m[12],
        p[0] * m[1] + p[1] * m[5] + p[2] * m[9] + m[13],
        p[0] * m[2] + p[1] * m[6] + p[2] * m[10] + m[14],
    ]
}

/// Same, without the translation — for normals and other directions.
fn transform_direction(m: &[f32; 9], v: [f32; 3]) -> [f32; 3] {
    [
        v[0] * m[0] + v[1] * m[3] + v[2] * m[6],
        v[0] * m[1] + v[1] * m[4] + v[2] * m[7],
        v[0] * m[2] + v[1] * m[5] + v[2] * m[8],
    ]
}

/// Inverse transpose of the upper 3x3, which is what normals need whenever the
/// transform carries a non-uniform scale — and car models are full of those:
/// mirrored wheels and mirrors are shipped as a `-1` scale on one axis. Using
/// the plain matrix there tilts every normal and lights the part wrongly.
/// Falls back to the plain 3x3 when the matrix is singular, so a degenerate
/// node still renders instead of vanishing.
fn inverse_transpose3(m: &[f32; 16]) -> [f32; 9] {
    let a = [m[0], m[1], m[2], m[4], m[5], m[6], m[8], m[9], m[10]];
    let det =
        a[0] * (a[4] * a[8] - a[5] * a[7]) - a[1] * (a[3] * a[8] - a[5] * a[6]) + a[2] * (a[3] * a[7] - a[4] * a[6]);
    if det.abs() < 1e-12 {
        return a;
    }
    let inv = 1.0 / det;
    // Cofactor matrix scaled by 1/det gives the inverse transpose directly:
    // cofactors are already laid out transposed relative to the adjugate.
    [
        (a[4] * a[8] - a[5] * a[7]) * inv,
        (a[5] * a[6] - a[3] * a[8]) * inv,
        (a[3] * a[7] - a[4] * a[6]) * inv,
        (a[2] * a[7] - a[1] * a[8]) * inv,
        (a[0] * a[8] - a[2] * a[6]) * inv,
        (a[1] * a[6] - a[0] * a[7]) * inv,
        (a[1] * a[5] - a[2] * a[4]) * inv,
        (a[2] * a[3] - a[0] * a[5]) * inv,
        (a[0] * a[4] - a[1] * a[3]) * inv,
    ]
}

fn determinant3(m: &[f32; 16]) -> f32 {
    m[0] * (m[5] * m[10] - m[6] * m[9]) - m[1] * (m[4] * m[10] - m[6] * m[8]) + m[2] * (m[4] * m[9] - m[5] * m[8])
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if length < 1e-8 {
        // A zero normal would make the shading undefined; pointing up is the
        // least surprising fallback.
        return [0.0, 1.0, 0.0];
    }
    [v[0] / length, v[1] / length, v[2] / length]
}

/// Fraction of triangles whose winding agrees with their stored normals, under
/// the right-handed cross-product rule, on the **raw** data before any axis is
/// negated.
///
/// This is a chirality measurement, independent of any naming convention: glTF
/// wants front faces wound counter-clockwise in a right-handed frame, so
/// `(p1 - p0) × (p2 - p0)` must point the same way as the surface normal. If
/// the source data were left-handed, or wound the other way, the dot product
/// would come out negative essentially everywhere. Returns `(agreeing, total)`.
pub fn winding_consistency(model: &Kn5Model) -> (usize, usize) {
    let mut agreeing = 0;
    let mut total = 0;
    model.visit_nodes(&mut |node| {
        let Some(mesh) = node.mesh() else { return };
        if !mesh.is_renderable || !mesh.is_visible {
            return;
        }
        for triangle in mesh.indices.as_chunks::<3>().0 {
            let Some(v) = triangle
                .iter()
                .map(|i| mesh.vertices.get(*i as usize))
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            let e1 = sub(v[1].position, v[0].position);
            let e2 = sub(v[2].position, v[0].position);
            let face = cross(e1, e2);
            let n = v[0].normal;
            let dot = face[0] * n[0] + face[1] * n[1] + face[2] * n[2];
            // Degenerate triangles say nothing about chirality.
            if dot.abs() < 1e-12 {
                continue;
            }
            total += 1;
            if dot > 0.0 {
                agreeing += 1;
            }
        }
    });
    (agreeing, total)
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Where a node sits once flattened and converted, by name — the wheel test of
/// §4.4 needs this, and nothing else does.
pub fn node_world_centers(model: &Kn5Model) -> Vec<(String, [f32; 3])> {
    let mut out = Vec::new();
    collect_centers(&model.root, &IDENTITY, &mut out);
    out
}

fn collect_centers(node: &Kn5Node, parent_world: &[f32; 16], out: &mut Vec<(String, [f32; 3])>) {
    let world = match node.transform() {
        Some(local) => multiply(local, parent_world),
        None => *parent_world,
    };
    // A dummy is located by its own translation — which is exactly what the
    // wheel check needs, since `WHEEL_LF` and friends are dummies holding the
    // rim and tyre below them, not meshes themselves.
    let center = match node.mesh() {
        None => Some([world[12], world[13], world[14]]),
        Some(mesh) if mesh.vertices.is_empty() => None,
        Some(mesh) => {
            let mut sum = [0.0f64; 3];
            for vertex in &mesh.vertices {
                let p = transform_point(&world, vertex.position);
                for axis in 0..3 {
                    sum[axis] += p[axis] as f64;
                }
            }
            let count = mesh.vertices.len() as f64;
            Some([
                (sum[0] / count) as f32,
                (sum[1] / count) as f32,
                (sum[2] / count) as f32,
            ])
        }
    };
    if let Some(center) = center {
        out.push((node.name.clone(), center));
    }
    for child in &node.children {
        collect_centers(child, &world, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Rule: row-vector convention — the translation lives in the last row, and
    // `world = local × parent`. A transposed read puts parts in the wrong
    // place while leaving the hierarchy plausible, which is why this is
    // checked against a transform worked out by hand (§10).
    #[test]
    fn point_transform_uses_the_last_row_for_translation() {
        let mut m = IDENTITY;
        m[12] = 10.0;
        m[13] = 20.0;
        m[14] = 30.0;
        assert_eq!(
            transform_point(&m, [1.0, 2.0, 3.0]),
            [11.0, 22.0, 33.0],
            "translation read from the last row"
        );
    }

    // Rule: composing a child onto its parent applies the parent second. The
    // check uses a rotation and a translation, whose order is not commutative,
    // so an inverted product cannot pass by accident.
    #[test]
    fn composition_applies_the_parent_after_the_child() {
        // Local: translate +1 along X. Parent: quarter turn around Y, which
        // maps +X onto -Z in this convention.
        let mut local = IDENTITY;
        local[12] = 1.0;
        let parent = [
            0.0, 0.0, -1.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            1.0, 0.0, 0.0, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ];

        let world = multiply(&local, &parent);
        let p = transform_point(&world, [0.0, 0.0, 0.0]);
        assert!(
            (p[0]).abs() < 1e-6 && (p[2] + 1.0).abs() < 1e-6,
            "child offset rotated by the parent, got {p:?}"
        );
    }

    // Rule: a mirroring transform has its winding flipped back. Without this,
    // the symmetric half of a car — mirrors, wheels — renders inside out while
    // the rest looks fine.
    #[test]
    fn mirrored_node_keeps_its_winding() {
        let mut mirror = IDENTITY;
        mirror[0] = -1.0;
        assert!(determinant3(&mirror) < 0.0, "negative scale detected");

        let mesh = Kn5Mesh {
            cast_shadows: true,
            is_visible: true,
            is_transparent: false,
            vertices: vec![
                kn5::Kn5Vertex {
                    position: [0.0, 0.0, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 0.0],
                    tangent: [1.0, 0.0, 0.0],
                };
                3
            ],
            indices: vec![0, 1, 2],
            material_id: 0,
            layer: 0,
            lod_in: 0.0,
            lod_out: 0.0,
            bounding_sphere_center: [0.0; 3],
            bounding_sphere_radius: 0.0,
            is_renderable: true,
        };

        let straight = convert_mesh("m", &mesh, &IDENTITY);
        let mirrored = convert_mesh("m", &mesh, &mirror);
        assert_eq!(
            straight.indices,
            vec![0, 1, 2],
            "sans conversion de repère, un mesh ordinaire garde son enroulement"
        );
        assert_eq!(
            mirrored.indices,
            vec![0, 2, 1],
            "un nœud en miroir voit le sien rétabli en antihoraire"
        );
    }

    // Règle : les UV sont reprises telles quelles. Bug réel sur `abarth500` —
    // inverser V envoyait la portière échantillonner la photo du compartiment
    // moteur, à l'autre bout de l'atlas.
    #[test]
    fn uv_are_taken_as_they_are() {
        let mesh = Kn5Mesh {
            cast_shadows: true,
            is_visible: true,
            is_transparent: false,
            vertices: vec![kn5::Kn5Vertex {
                position: [0.0, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.25, 0.75],
                tangent: [1.0, 0.0, 0.0],
            }],
            indices: vec![0, 0, 0],
            material_id: 0,
            layer: 0,
            lod_in: 0.0,
            lod_out: 0.0,
            bounding_sphere_center: [0.0; 3],
            bounding_sphere_radius: 0.0,
            is_renderable: true,
        };
        let flat = convert_mesh("m", &mesh, &IDENTITY);
        assert_eq!(
            flat.uvs[0],
            [0.25, 0.75],
            "UV reprises telles quelles : DirectX et glTF ont la même origine de texture"
        );
    }

    // Rule: the inverse transpose is what normals go through. Under a
    // non-uniform scale the plain matrix tilts them; here a flattening scale
    // must leave a horizontal normal horizontal.
    // Règle : la vitre **brisée** ne se dessine pas. AC la garde en permanence
    // dans le modèle et ne l'affiche qu'après un choc — même mécanisme que la
    // carte de dégâts (kn5-format.md, écart n°4), sur un maillage entier au
    // lieu d'un slot de texture. Bug réel remonté trois fois par
    // l'utilisateur : un pare-brise « sale » sur `ks_toyota_supra_mkiv`, en
    // fait un réseau de fissures posé par-dessus.
    #[test]
    fn broken_glass_is_never_drawn() {
        fn glass_mesh(material_id: u32) -> Kn5Mesh {
            Kn5Mesh {
                cast_shadows: false,
                is_visible: true,
                is_transparent: true,
                vertices: vec![
                    kn5::Kn5Vertex {
                        position: [0.0, 0.0, 0.0],
                        normal: [0.0, 1.0, 0.0],
                        uv: [0.0, 0.0],
                        tangent: [1.0, 0.0, 0.0],
                    };
                    3
                ],
                indices: vec![0, 1, 2],
                material_id,
                layer: 0,
                lod_in: 0.0,
                lod_out: 0.0,
                bounding_sphere_center: [0.0; 3],
                bounding_sphere_radius: 0.0,
                is_renderable: true,
            }
        }
        fn material(shader: &str) -> Kn5Material {
            Kn5Material {
                name: shader.to_string(),
                shader: shader.to_string(),
                blend_mode: 1,
                alpha_tested: false,
                reserved: 0,
                properties: Vec::new(),
                samplers: Vec::new(),
            }
        }

        let materials = [material("ksBrokenGlass"), material("ksPerPixelReflection")];
        let options = GeometryOptions::default();

        let broken = glass_mesh(0);
        let intact = glass_mesh(1);
        let node = Kn5Node {
            name: "GLASS".to_string(),
            active: true,
            kind: kn5::Kn5NodeKind::Mesh(intact.clone()),
            children: Vec::new(),
        };

        assert!(
            matches!(
                classify(&node, &broken, &materials, &options),
                Keep::No(Skip::BrokenGlass)
            ),
            "la vitre brisée est écartée"
        );
        assert!(
            matches!(classify(&node, &intact, &materials, &options), Keep::Yes),
            "la vitre intacte reste, elle"
        );
    }

    #[test]
    fn normals_survive_non_uniform_scale() {
        let mut squash = IDENTITY;
        squash[0] = 4.0; // stretched along X
        squash[5] = 1.0;
        let normal_matrix = inverse_transpose3(&squash);
        let n = normalize(transform_direction(&normal_matrix, normalize([1.0, 1.0, 0.0])));
        // Stretching X by 4 must tilt the normal *towards* Y, not away.
        assert!(n[1] > n[0], "normal tilted the right way, got {n:?}");
    }
}
