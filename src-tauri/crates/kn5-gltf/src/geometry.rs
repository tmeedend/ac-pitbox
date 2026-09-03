//! Node tree → flat list of meshes in glTF space (spec §3.4, §3.5, §4.4).
//!
//! Three things happen here, and each one has its own way of going wrong:
//! flattening the hierarchy, changing handedness, and dropping the nodes that
//! are not meant to be drawn.

use std::collections::BTreeMap;

use kn5::{Kn5Material, Kn5Mesh, Kn5Model, Kn5Node, Kn5NodeKind, Kn5SkinBinding};

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
    /// Repère tangent, `xyz` normalisé et `w` la latéralité — le `TANGENT` de
    /// glTF. Voir [`convert_mesh`] pour ce qu'il coûte de ne pas l'écrire.
    pub tangents: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    /// Kept for reporting: a mesh flagged transparent needs the render-order
    /// treatment of §8.2 on the viewer side.
    pub transparent: bool,
}

#[derive(Debug, Clone)]
pub struct GeometryOptions {
    /// Node name patterns that mark helpers rather than geometry. Kept as
    /// configuration rather than hard-coded in the middle of the walk (§3.5),
    /// because mods invent their own conventions. A match drops the node
    /// **and everything under it**: the telling name is on the group, not on
    /// the meshes inside it.
    pub excluded_name_patterns: Vec<String>,
    /// Node name prefixes with the same effect, but on that node alone — see
    /// the note in `classify`.
    pub excluded_name_prefixes: Vec<String>,
    /// Drop meshes that only appear beyond a distance, i.e. lower LODs stored
    /// inside the same file.
    pub skip_distant_lods: bool,
    /// Drop `COCKPIT_LR` when `COCKPIT_HR` is there too — see [`flatten`].
    pub skip_low_res_cockpit: bool,
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
            skip_low_res_cockpit: true,
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
    /// Meshes of the duplicate low-resolution interior — see [`flatten`].
    pub skipped_low_res_cockpit: usize,
    /// Meshes whose accumulated transform mirrors space, and whose winding was
    /// flipped a second time as a result.
    pub mirrored: usize,
    /// Meshes actually written, once those sharing a material have been merged
    /// — i.e. the number of draw calls the viewer will make. Always ≤ `kept`.
    pub merged: usize,
}

/// Nom des deux habitacles qu'AC range dans le même fichier.
const HIGH_RES_COCKPIT: &str = "COCKPIT_HR";
const LOW_RES_COCKPIT: &str = "COCKPIT_LR";

/// Walks the tree, accumulates transforms and returns every mesh worth drawing.
///
/// **AC livre deux habitacles superposés**, et n'en montre qu'un à la fois :
/// `COCKPIT_HR` depuis le poste de pilotage, `COCKPIT_LR` — une coque grossière
/// de quelques milliers de triangles — vu de l'extérieur. Les dessiner tous les
/// deux les fait se disputer le tampon de profondeur, et le tableau de bord se
/// couvre de taches claires à bords francs qui **scintillent dès que la caméra
/// bouge**. C'est ce scintillement qui a identifié le défaut : une erreur
/// d'éclairage ne bouge pas avec la caméra, un z-fighting si.
///
/// On garde le détaillé, parce que la vue peut s'approcher et regarder dedans.
/// Mesuré sur 30 voitures : 27 portent les deux, 3 n'ont que `COCKPIT_HR`, et
/// **aucune n'a que `COCKPIT_LR`** — d'où la garde, qui ne retire la coque que
/// lorsque le détaillé est bien là.
pub fn flatten(model: &Kn5Model, options: &GeometryOptions) -> (Vec<FlatMesh>, GeometryStats) {
    let mut meshes = Vec::new();
    let mut stats = GeometryStats::default();
    let drop_low_res_cockpit =
        options.skip_low_res_cockpit && has_node(model, HIGH_RES_COCKPIT) && has_node(model, LOW_RES_COCKPIT);
    // Calculé en amont, et seulement s'il y a de quoi s'en servir : c'est un
    // second parcours complet de l'arbre, que la quasi-totalité des modèles
    // n'a aucune raison de payer — un maillage skinné sur une voiture, ça
    // n'existe pas. Le pilote greffé en apporte deux (§4.6bis).
    let bones = if has_skinned_mesh(model) {
        node_world_matrices(model)
    } else {
        BTreeMap::new()
    };
    walk(
        &model.root,
        &IDENTITY,
        &model.materials,
        options,
        drop_low_res_cockpit,
        &bones,
        &mut meshes,
        &mut stats,
    );
    let meshes = merge_by_material(meshes);
    stats.merged = meshes.len();
    (meshes, stats)
}

/// Regroupe en un seul maillage tous ceux qui partagent un matériau.
///
/// **Ce que ça achète** : un appel de dessin par matériau au lieu d'un par
/// nœud. Mesuré sur cinq voitures, 133 à 208 primitives tombent à 32 à 53,
/// soit 3,3× à 5,3× moins — et sur Windows, où WebGL passe par la traduction
/// D3D11, un appel de dessin n'est pas gratuit. Le panneau rend en continu
/// (plateau tournant), donc ce coût est payé soixante fois par seconde.
///
/// **Pourquoi c'est une simple concaténation** : les sommets sont déjà en
/// espace monde à ce stade — la transformation accumulée du nœud leur a été
/// appliquée par [`convert_mesh`] — et les nœuds glTF écrits ensuite ne
/// portent aucune matrice. Il n'y a donc rien à recomposer, seulement des
/// index à décaler.
///
/// **La clé inclut la transparence**, et ce n'est pas de la prudence
/// décorative : le drapeau est porté par le *maillage* (`mesh.is_transparent`)
/// et non par le matériau, donc deux maillages du même matériau peuvent ne pas
/// s'accorder. Les fusionner mélangerait un objet que la vue doit rendre après
/// l'opaque, sans écriture de profondeur (§8.2), avec un objet ordinaire.
fn merge_by_material(meshes: Vec<FlatMesh>) -> Vec<FlatMesh> {
    let mut merged: Vec<FlatMesh> = Vec::new();
    // L'ordre de première apparition est conservé : deux conversions du même
    // modèle doivent produire le même fichier, octet pour octet, sinon le
    // cache disque perd son sens.
    let mut slot_of: std::collections::BTreeMap<(u32, bool), usize> = std::collections::BTreeMap::new();
    for mesh in meshes {
        match slot_of.get(&(mesh.material_id, mesh.transparent)) {
            Some(&slot) => {
                let target: &mut FlatMesh = &mut merged[slot];
                let offset = target.positions.len() as u32;
                target.positions.extend(mesh.positions);
                target.normals.extend(mesh.normals);
                target.uvs.extend(mesh.uvs);
                target.tangents.extend(mesh.tangents);
                target.indices.extend(mesh.indices.iter().map(|index| index + offset));
            }
            None => {
                slot_of.insert((mesh.material_id, mesh.transparent), merged.len());
                merged.push(mesh);
            }
        }
    }
    merged
}

const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, //
    0.0, 0.0, 0.0, 1.0,
];

/// Le modèle porte-t-il un nœud de ce nom ?
fn has_node(model: &Kn5Model, name: &str) -> bool {
    let mut found = false;
    model.visit_nodes(&mut |node| found |= node.name.eq_ignore_ascii_case(name));
    found
}

/// Nombre de maillages non vides sous ce nœud, lui compris.
fn mesh_count(node: &Kn5Node) -> usize {
    let mut count = 0;
    node.visit(&mut |n| {
        if n.mesh().is_some_and(|m| !m.vertices.is_empty()) {
            count += 1;
        }
    });
    count
}

#[allow(clippy::too_many_arguments)]
fn walk(
    node: &Kn5Node,
    parent_world: &[f32; 16],
    materials: &[Kn5Material],
    options: &GeometryOptions,
    drop_low_res_cockpit: bool,
    bones: &BTreeMap<String, [f32; 16]>,
    meshes: &mut Vec<FlatMesh>,
    stats: &mut GeometryStats,
) {
    // Écarté **avec tout son sous-arbre** : c'est un nœud de transformation,
    // pas un maillage, donc le filtrage par nom de `classify` ne le verrait
    // jamais et ses enfants passeraient quand même.
    if drop_low_res_cockpit && node.name.eq_ignore_ascii_case(LOW_RES_COCKPIT) {
        stats.skipped_low_res_cockpit += mesh_count(node);
        return;
    }
    // Même raison, pour les motifs de nom : le nom qui désigne un accessoire
    // est porté par le **groupe**, pas par les maillages dedans. `RIM_BLUR_LF`
    // contient `Object190` et `Object193` — deux noms qui ne disent rien —,
    // et tester le seul nom du maillage laissait donc passer la jante floutée
    // par-dessus la vraie. Mesuré sur 134 voitures de la bibliothèque :
    // 33 en portaient au moins un, toujours sous `RIM_BLUR_*` ou
    // `DAMAGE_GLASS_*`, jamais sous autre chose — aucun vrai morceau de
    // voiture ne se perd à couper au groupe.
    if matches_excluded_pattern(&node.name, options) {
        stats.skipped_by_name += mesh_count(node);
        return;
    }
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
                // Un maillage skinné n'est pas placé par son nœud mais par ses
                // os : la transformation accumulée ne le concerne pas. Repli
                // sur le traitement rigide si un seul os manque à l'appel — en
                // pose de repos les deux donnent le même résultat, ce qui est
                // exactement ce qu'il faut quand on ne sait pas poser.
                let flat = match skinning_matrices(&node.kind, bones) {
                    Some(matrices) => convert_skinned_mesh(&node.name, &node.kind, &matrices),
                    None => convert_mesh(&node.name, mesh, &world),
                };
                if determinant3(&world) < 0.0 {
                    stats.mirrored += 1;
                }
                stats.kept += 1;
                meshes.push(flat);
            }
        }
    }

    for child in &node.children {
        walk(
            child,
            &world,
            materials,
            options,
            drop_low_res_cockpit,
            bones,
            meshes,
            stats,
        );
    }
}

/// Le modèle porte-t-il au moins un maillage skinné ?
fn has_skinned_mesh(model: &Kn5Model) -> bool {
    let mut found = false;
    model.visit_nodes(&mut |node| found |= matches!(node.kind, Kn5NodeKind::SkinnedMesh(_)));
    found
}

/// Transformation monde de chaque nœud, par nom — ce dont les os ont besoin,
/// puisqu'un os désigne un nœud de l'arbre par son nom et rien d'autre.
///
/// Premier nom gagnant en cas d'homonymie. Le cas existe dans les modèles
/// (un maillage porte souvent le nom du dummy qui le tient), mais pas sur un
/// os : les noms de rig sont uniques.
fn node_world_matrices(model: &Kn5Model) -> BTreeMap<String, [f32; 16]> {
    let mut out = BTreeMap::new();
    collect_matrices(&model.root, &IDENTITY, &mut out);
    out
}

fn collect_matrices(node: &Kn5Node, parent_world: &[f32; 16], out: &mut BTreeMap<String, [f32; 16]>) {
    let world = match node.transform() {
        Some(local) => multiply(local, parent_world),
        None => *parent_world,
    };
    out.entry(node.name.clone()).or_insert(world);
    for child in &node.children {
        collect_matrices(child, &world, out);
    }
}

/// Une matrice par os : inverse de la pose de liaison, puis transformation
/// monde courante — dans cet ordre, convention vecteur-ligne, donc un sommet
/// la traverse de gauche à droite.
///
/// `None` dès qu'un seul os manque. Un skinning partiel ne se rattrape pas :
/// les sommets rattachés à l'os absent partiraient à l'origine, ce qui étire
/// le maillage entier en éventail. La pose de repos entière vaut mieux.
fn skinning_matrices(kind: &Kn5NodeKind, bones: &BTreeMap<String, [f32; 16]>) -> Option<Vec<[f32; 16]>> {
    let Kn5NodeKind::SkinnedMesh(skinned) = kind else {
        return None;
    };
    if skinned.skin.is_empty() || skinned.bones.is_empty() {
        return None;
    }
    skinned
        .bones
        .iter()
        .map(|bone| {
            bones
                .get(&bone.name)
                .map(|world| multiply(&bone.inverse_bind_matrix, world))
        })
        .collect()
}

/// Skinning linéaire : chaque sommet suit la moyenne pondérée des matrices de
/// ses quatre os.
///
/// **En pose de liaison, ce calcul rend l'identité** — c'est ce qui rend son
/// absence invisible tant qu'on n'anime rien, et c'est aussi pourquoi il n'est
/// pas optionnel dès qu'on anime : sans lui, la combinaison et les gants
/// resteraient en arrière pendant que le casque, simple enfant de l'os de
/// tête, suivrait le rig. Le pilote se démembrerait à l'écran.
///
/// Les normales passent par l'inverse transposée de la matrice mélangée, comme
/// dans le cas rigide : un mélange de matrices n'est plus une rotation, même
/// quand chacune en était une.
fn convert_skinned_mesh(name: &str, kind: &Kn5NodeKind, matrices: &[[f32; 16]]) -> FlatMesh {
    let Kn5NodeKind::SkinnedMesh(skinned) = kind else {
        unreachable!("skinning_matrices ne répond que pour un maillage skinné")
    };
    let mesh = &skinned.mesh;

    let mut positions = Vec::with_capacity(mesh.vertices.len());
    let mut normals = Vec::with_capacity(mesh.vertices.len());
    let mut uvs = Vec::with_capacity(mesh.vertices.len());
    let mut tangents = Vec::with_capacity(mesh.vertices.len());
    let uv_handedness = uv_handedness(mesh);

    for (index, vertex) in mesh.vertices.iter().enumerate() {
        let blended = match skinned.skin.get(index) {
            Some(binding) => blend(binding, matrices),
            None => IDENTITY,
        };
        positions.push(transform_point(&blended, vertex.position));
        let normal = normalize(transform_direction(&inverse_transpose3(&blended), vertex.normal));
        normals.push(normal);
        uvs.push(vertex.uv);
        let tangent = transform_direction(&upper3(&blended), vertex.tangent);
        let tangent = normalize(reject(tangent, normal));
        let w = uv_handedness.get(index).copied().unwrap_or(1.0);
        tangents.push([tangent[0], tangent[1], tangent[2], w]);
    }

    FlatMesh {
        name: name.to_string(),
        material_id: mesh.material_id,
        positions,
        normals,
        uvs,
        tangents,
        // Aucune inversion d'enroulement : un mélange de matrices d'os ne met
        // pas l'espace en miroir, et aucun mannequin de l'échantillon ne porte
        // d'os à échelle négative.
        indices: mesh.indices.iter().map(|i| u32::from(*i)).collect(),
        transparent: mesh.is_transparent,
    }
}

/// Moyenne pondérée des matrices des quatre os d'un sommet.
///
/// Les poids sont **renormalisés** : ils somment à 1 dans les fichiers vus,
/// mais un fichier de mod qui sommerait à 0,9 rétrécirait son maillage de 10 %
/// en silence, et un qui sommerait à 0 l'enverrait à l'origine.
fn blend(binding: &Kn5SkinBinding, matrices: &[[f32; 16]]) -> [f32; 16] {
    let mut out = [0.0f32; 16];
    let mut total = 0.0f32;
    for (weight, index) in binding.weights.iter().zip(binding.bone_indices) {
        if *weight <= 0.0 {
            continue;
        }
        // L'indice est stocké en flottant (§3.4) ; négatif ou hors bornes, il
        // ne désigne rien, et l'os est ignoré plutôt que de faire paniquer.
        let matrix = if index >= 0.0 {
            matrices.get(index as usize)
        } else {
            None
        };
        let Some(matrix) = matrix else { continue };
        for (slot, value) in out.iter_mut().zip(matrix) {
            *slot += weight * value;
        }
        total += weight;
    }
    if total <= f32::EPSILON {
        return IDENTITY;
    }
    for slot in &mut out {
        *slot /= total;
    }
    out
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

/// Le nom de ce nœud désigne-t-il un accessoire dont **rien** n'est à garder ?
fn matches_excluded_pattern(name: &str, options: &GeometryOptions) -> bool {
    let name = name.to_ascii_uppercase();
    options.excluded_name_patterns.iter().any(|p| name.contains(p))
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
    // Les motifs, eux, sont traités un cran plus haut, sur le groupe entier
    // (voir `walk`). Ne reste ici que le préfixe, volontairement laissé au
    // maillage seul : mesuré sur la bibliothèque, le seul nœud de voiture
    // préfixé `AC_` est `ac_black_metal`, un groupe nommé d'après son matériau
    // par un export Blender, au milieu de `plastic_interior_plastic.*` — le
    // couper avec son sous-arbre retirerait de la vraie garniture.
    let name = node.name.to_ascii_uppercase();
    if options.excluded_name_prefixes.iter().any(|p| name.starts_with(p)) {
        return Keep::No(Skip::Name);
    }
    // `lod_in` is the distance at which a mesh *starts* being drawn: anything
    // above zero belongs to a lower level of detail stored in the same file.
    if options.skip_distant_lods && mesh.lod_in > 0.0 {
        return Keep::No(Skip::DistantLod);
    }
    Keep::Yes
}

/// **Le KN5 porte un repère tangent par sommet, et il faut l'écrire.**
///
/// Une carte de normales s'exprime en espace tangent : sans repère, un lecteur
/// glTF doit le reconstruire **par pixel**, à partir des dérivées écran de la
/// position et de l'UV. Ce repli s'effondre là où la dérivée UV est nulle — un
/// panneau entier plaqué sur un texel uniforme d'atlas, c'est-à-dire la façon
/// ordinaire de déplier un intérieur de voiture — et le repère explose en
/// stries blanches sur les surfaces sombres et brillantes. Défaut signalé par
/// l'utilisateur sur l'Aventador, la RX-7 et l'Alfa GTA.
///
/// Mesuré (`kn5-tool inspect --tangents`) : **100 % des sommets** de toutes les
/// voitures de l'échantillon portent une tangente utilisable, tandis que 0,2 %
/// à 30,8 % de leurs triangles ont des UV dégénérés. La donnée était lue par le
/// parseur depuis le début et jetée ici.
fn convert_mesh(name: &str, mesh: &Kn5Mesh, world: &[f32; 16]) -> FlatMesh {
    let normal_matrix = inverse_transpose3(world);
    let mut positions = Vec::with_capacity(mesh.vertices.len());
    let mut normals = Vec::with_capacity(mesh.vertices.len());
    let mut uvs = Vec::with_capacity(mesh.vertices.len());
    let mut tangents = Vec::with_capacity(mesh.vertices.len());

    // La latéralité ne se lit pas dans le KN5 : elle se déduit de l'orientation
    // des UV, et un îlot en miroir la retourne. Une transformation en miroir la
    // retourne une seconde fois, d'où le facteur global.
    let handedness_flip = if determinant3(world) < 0.0 { -1.0 } else { 1.0 };
    let uv_handedness = uv_handedness(mesh);

    for (index, vertex) in mesh.vertices.iter().enumerate() {
        positions.push(transform_point(world, vertex.position));
        let normal = normalize(transform_direction(&normal_matrix, vertex.normal));
        normals.push(normal);
        uvs.push(vertex.uv);
        // La tangente suit la matrice du modèle, pas son inverse transposée :
        // c'est une direction *dans* la surface, pas une normale.
        let tangent = transform_direction(&upper3(world), vertex.tangent);
        // Gram-Schmidt : glTF veut la tangente orthogonale à la normale, et
        // l'interpolation entre sommets ne la garde pas telle quelle.
        let tangent = normalize(reject(tangent, normal));
        let w = uv_handedness.get(index).copied().unwrap_or(1.0) * handedness_flip;
        tangents.push([tangent[0], tangent[1], tangent[2], w]);
    }

    let mut indices: Vec<u32> = mesh.indices.iter().map(|i| u32::from(*i)).collect();
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
        tangents,
        indices,
        transparent: mesh.is_transparent,
    }
}

/// Latéralité du repère tangent de chaque sommet, `+1` ou `-1`.
///
/// glTF range dans le `w` de `TANGENT` le signe qui donne la bitangente :
/// `B = cross(N, T) * w`. Le KN5 n'écrit qu'une tangente à trois composantes,
/// donc le signe se retrouve à partir de l'orientation du triangle dans
/// l'espace UV — et il **change** d'un îlot à l'autre dès qu'une moitié du
/// modèle est dépliée en miroir, ce qui est la règle sur une carrosserie.
/// Le supposer constant remettrait des reliefs inversés là où on vient de
/// corriger des stries.
fn uv_handedness(mesh: &Kn5Mesh) -> Vec<f32> {
    let mut out = vec![0.0f32; mesh.vertices.len()];
    for corners in mesh.indices.as_chunks::<3>().0 {
        let vertices = corners.map(|i| mesh.vertices.get(i as usize));
        let ([Some(a), Some(b), Some(c)], true) = (vertices, true) else {
            continue;
        };
        // Aire signée dans l'espace UV : son signe est celui de la bitangente.
        let area = (b.uv[0] - a.uv[0]) * (c.uv[1] - a.uv[1]) - (c.uv[0] - a.uv[0]) * (b.uv[1] - a.uv[1]);
        if area == 0.0 {
            continue;
        }
        let sign = if area < 0.0 { -1.0 } else { 1.0 };
        for index in corners {
            // Premier triangle rencontré : un sommet partagé entre deux îlots
            // de latéralités opposées devrait être dédoublé, ce qu'AC fait déjà
            // — il écrit une tangente par sommet, donc il a scindé ses coutures.
            if let Some(slot) = out.get_mut(*index as usize) {
                if *slot == 0.0 {
                    *slot = sign;
                }
            }
        }
    }
    for slot in &mut out {
        if *slot == 0.0 {
            *slot = 1.0;
        }
    }
    out
}

/// Composante `v - n * dot(v, n)` : la part de `v` orthogonale à `n`.
fn reject(v: [f32; 3], n: [f32; 3]) -> [f32; 3] {
    let dot = v[0] * n[0] + v[1] * n[1] + v[2] * n[2];
    [v[0] - n[0] * dot, v[1] - n[1] * dot, v[2] - n[2] * dot]
}

/// Bloc 3x3 supérieur gauche d'une matrice 4x4.
fn upper3(m: &[f32; 16]) -> [f32; 9] {
    [m[0], m[1], m[2], m[4], m[5], m[6], m[8], m[9], m[10]]
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

    /// Un modèle minimal : pour chaque paire, un groupe et le maillage qu'il
    /// contient, nommés indépendamment l'un de l'autre.
    fn model_with_meshes(nodes: &[(&str, &str)]) -> kn5::Kn5Model {
        let mesh = |name: &str| kn5::Kn5Node {
            name: name.to_string(),
            active: true,
            kind: kn5::Kn5NodeKind::Mesh(kn5::Kn5Mesh {
                cast_shadows: true,
                is_visible: true,
                is_transparent: false,
                vertices: vec![
                    kn5::Kn5Vertex {
                        position: [0.0; 3],
                        normal: [0.0, 0.0, 1.0],
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
                bounding_sphere_radius: 1.0,
                is_renderable: true,
            }),
            children: Vec::new(),
        };
        kn5::Kn5Model {
            version: 6,
            extra: None,
            textures: Vec::new(),
            materials: vec![kn5::Kn5Material {
                name: "m".to_string(),
                shader: "ksPerPixel".to_string(),
                blend_mode: 0,
                alpha_tested: false,
                reserved: 0,
                properties: Vec::new(),
                samplers: Vec::new(),
            }],
            root: kn5::Kn5Node {
                name: "root".to_string(),
                active: true,
                kind: kn5::Kn5NodeKind::Dummy { transform: IDENTITY },
                children: nodes
                    .iter()
                    .map(|(group, inside)| kn5::Kn5Node {
                        name: group.to_string(),
                        active: true,
                        kind: kn5::Kn5NodeKind::Dummy { transform: IDENTITY },
                        children: vec![mesh(inside)],
                    })
                    .collect(),
            },
        }
    }

    /// Un modèle minimal portant les nœuds nommés, chacun avec un maillage qui
    /// porte le même nom préfixé — le cas ordinaire.
    fn model_with(nodes: &[&str]) -> kn5::Kn5Model {
        let pairs: Vec<(String, String)> = nodes.iter().map(|n| (n.to_string(), format!("g_{n}"))).collect();
        model_with_meshes(&pairs.iter().map(|(g, m)| (g.as_str(), m.as_str())).collect::<Vec<_>>())
    }

    // Règle : un motif de nom écarte le nœud **et tout son sous-arbre**, parce
    // que le nom qui trahit l'accessoire est sur le groupe, pas sur ce qu'il
    // contient. Bug réel : `a3dr_viper_rt10` montrait une jante floutée
    // par-dessus la vraie, ses maillages s'appelant `Object190`/`Object193`
    // sous un `RIM_BLUR_LF`. 33 voitures sur 134 dans le même cas.
    #[test]
    fn an_excluded_group_takes_its_meshes_with_it() {
        let model = model_with_meshes(&[("RIM_BLUR_LF", "Object190"), ("RIM_LF", "Object051")]);
        let (meshes, stats) = flatten(&model, &GeometryOptions::default());
        assert_eq!(stats.skipped_by_name, 1, "le maillage sous le groupe flouté est écarté");
        assert_eq!(meshes.len(), 1, "il ne reste que la vraie jante");
        assert_eq!(meshes[0].name, "Object051", "et c'est bien elle");
    }

    // Règle : AC livre deux habitacles superposés et n'en montre qu'un ; les
    // dessiner tous les deux les fait se disputer le tampon de profondeur, et
    // le tableau de bord scintille. On garde le détaillé.
    #[test]
    fn the_low_resolution_cockpit_is_dropped_when_the_detailed_one_is_there() {
        let options = GeometryOptions::default();
        let (meshes, stats) = flatten(&model_with(&["COCKPIT_HR", "COCKPIT_LR"]), &options);
        assert_eq!(stats.skipped_low_res_cockpit, 1, "la coque grossière est écartée");
        assert_eq!(meshes.len(), 1, "il reste l'habitacle détaillé");
        assert_eq!(meshes[0].name, "g_COCKPIT_HR", "et c'est bien le détaillé");
    }

    // Règle : et **seulement** quand il est là. Aucune voiture de l'échantillon
    // n'a que la basse résolution, mais si l'une se présente, lui retirer son
    // seul habitacle la viderait.
    #[test]
    fn a_car_with_only_the_low_resolution_cockpit_keeps_it() {
        let (meshes, stats) = flatten(&model_with(&["COCKPIT_LR"]), &GeometryOptions::default());
        assert_eq!(stats.skipped_low_res_cockpit, 0, "rien à écarter");
        assert_eq!(meshes.len(), 1, "son unique habitacle reste");
    }

    // Règle : la latéralité du repère tangent se déduit de l'orientation des
    // UV, et un îlot déplié en miroir la retourne. La supposer constante
    // remettrait des reliefs inversés sur la moitié d'une carrosserie.
    #[test]
    fn tangent_handedness_follows_the_uv_winding() {
        let vertex = |uv: [f32; 2]| kn5::Kn5Vertex {
            position: [0.0; 3],
            normal: [0.0, 0.0, 1.0],
            uv,
            tangent: [1.0, 0.0, 0.0],
        };
        // Deux triangles, l'un déplié à l'endroit, l'autre en miroir.
        let mesh = kn5::Kn5Mesh {
            cast_shadows: true,
            is_visible: true,
            is_transparent: false,
            vertices: vec![
                vertex([0.0, 0.0]),
                vertex([1.0, 0.0]),
                vertex([0.0, 1.0]),
                vertex([0.0, 0.0]),
                vertex([0.0, 1.0]),
                vertex([1.0, 0.0]),
            ],
            indices: vec![0, 1, 2, 3, 4, 5],
            material_id: 0,
            layer: 0,
            lod_in: 0.0,
            lod_out: 0.0,
            bounding_sphere_center: [0.0; 3],
            bounding_sphere_radius: 1.0,
            is_renderable: true,
        };

        let handedness = uv_handedness(&mesh);
        assert_eq!(handedness[0], handedness[1], "un même triangle, une même latéralité");
        assert_eq!(handedness[0], -handedness[3], "l'îlot en miroir prend le signe opposé");
    }

    // Règle : la tangente écrite est orthogonale à la normale et de norme 1,
    // ce que glTF exige — l'interpolation entre sommets ne le garantit pas
    // toute seule, d'où le Gram-Schmidt de `convert_mesh`.
    #[test]
    fn written_tangents_are_orthonormal_to_their_normal() {
        let mesh = kn5::Kn5Mesh {
            cast_shadows: true,
            is_visible: true,
            is_transparent: false,
            vertices: vec![kn5::Kn5Vertex {
                position: [0.0; 3],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 0.0],
                // Volontairement ni normalisée ni orthogonale à la normale.
                tangent: [3.0, 0.0, 4.0],
            }],
            indices: vec![],
            material_id: 0,
            layer: 0,
            lod_in: 0.0,
            lod_out: 0.0,
            bounding_sphere_center: [0.0; 3],
            bounding_sphere_radius: 1.0,
            is_renderable: true,
        };

        let flat = convert_mesh("m", &mesh, &IDENTITY);
        let [x, y, z, w] = flat.tangents[0];
        assert!((x * x + y * y + z * z - 1.0).abs() < 1e-5, "normalisée");
        assert!(z.abs() < 1e-5, "redressée dans le plan de la surface, got z={z}");
        assert!(w == 1.0 || w == -1.0, "la latéralité est un signe, got {w}");
    }

    /// Maillage plat minimal, pour les règles de fusion : trois sommets, un
    /// triangle, tout le reste à zéro — seuls le matériau, la transparence et
    /// le décalage des index comptent ici.
    fn flat(material_id: u32, transparent: bool) -> FlatMesh {
        FlatMesh {
            name: format!("m{material_id}"),
            material_id,
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 1.0, 0.0]; 3],
            uvs: vec![[0.0, 0.0]; 3],
            tangents: vec![[1.0, 0.0, 0.0, 1.0]; 3],
            indices: vec![0, 1, 2],
            transparent,
        }
    }

    // Règle : les maillages d'un même matériau sont fusionnés en un seul, et
    // les index du second sont décalés du nombre de sommets du premier — sans
    // ce décalage, le second triangle irait chercher les sommets du premier.
    #[test]
    fn meshes_sharing_a_material_become_one_and_indices_follow() {
        let merged = merge_by_material(vec![flat(0, false), flat(0, false), flat(1, false)]);

        assert_eq!(merged.len(), 2, "deux matériaux, deux maillages");
        assert_eq!(merged[0].positions.len(), 6, "les sommets sont concaténés");
        assert_eq!(
            merged[0].indices,
            vec![0, 1, 2, 3, 4, 5],
            "le second triangle vise ses propres sommets, pas ceux du premier"
        );
        assert_eq!(merged[1].positions.len(), 3, "l'autre matériau reste seul");
    }

    // Règle : la transparence entre dans la clé de fusion. Elle est portée par
    // le **maillage** et non par le matériau, donc deux maillages du même
    // matériau peuvent ne pas s'accorder — les fusionner mélangerait un objet
    // qui doit passer après l'opaque avec un objet ordinaire (§8.2).
    #[test]
    fn a_transparent_mesh_is_never_merged_into_an_opaque_one() {
        let merged = merge_by_material(vec![flat(0, false), flat(0, true)]);

        assert_eq!(merged.len(), 2, "même matériau, mais pas le même traitement");
        assert!(!merged[0].transparent, "l'opaque reste opaque");
        assert!(merged[1].transparent, "et le transparent reste transparent");
    }

    // Règle : l'ordre de première apparition est conservé. Deux conversions du
    // même modèle doivent produire le même fichier, sans quoi le cache disque
    // servirait des entrées différentes pour une voiture inchangée.
    #[test]
    fn merging_keeps_the_order_materials_first_appeared_in() {
        let merged = merge_by_material(vec![flat(7, false), flat(2, false), flat(7, false)]);

        let materials: Vec<u32> = merged.iter().map(|m| m.material_id).collect();
        assert_eq!(materials, vec![7, 2], "7 est apparu le premier, il reste le premier");
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
