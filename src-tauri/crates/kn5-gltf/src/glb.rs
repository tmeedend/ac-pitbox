//! GLB writer — glTF 2.0 binary container.
//!
//! Written against the schema directly rather than through `gltf-json`
//! (suggested by spec §5.2): the subset used here is small and fixed, the
//! container itself is a 12-byte header plus two chunks, and the acceptance
//! test is empirical anyway — the file has to open in Blender *and* in a web
//! viewer, which catches schema mistakes far better than a type wrapper would.

use serde_json::{json, Map, Value};

use crate::geometry::FlatMesh;
use crate::material::GltfMaterial;
use crate::texture::TextureSet;

// glTF component and target constants, spelled out so the JSON below reads as
// the specification does.
const COMPONENT_F32: u32 = 5126;
const COMPONENT_U16: u32 = 5123;
const COMPONENT_U32: u32 = 5125;
const TARGET_ARRAY_BUFFER: u32 = 34962;
const TARGET_ELEMENT_ARRAY_BUFFER: u32 = 34963;

/// Assembles the whole preview into a single self-contained `.glb`.
pub fn write_glb(
    meshes: &[FlatMesh],
    rig: Option<&crate::rig::Rig>,
    materials: &[GltfMaterial],
    textures: &TextureSet,
) -> Result<Vec<u8>, String> {
    if meshes.is_empty() {
        return Err("no drawable mesh left after filtering".to_string());
    }

    let mut bin: Vec<u8> = Vec::new();
    let mut buffer_views: Vec<Value> = Vec::new();
    let mut accessors: Vec<Value> = Vec::new();

    // Only the materials actually used by a surviving mesh are emitted, and
    // only their textures get embedded. Colliders, damage meshes and lower
    // LODs drag whole texture sets behind them otherwise.
    let mut material_remap: Vec<Option<usize>> = vec![None; materials.len()];
    let mut used_materials: Vec<&GltfMaterial> = Vec::new();
    // Les maillages du mannequin comptent comme les autres : leurs matériaux
    // doivent survivre au tri, sinon le pilote sort sans texture.
    let rig_meshes: Vec<&FlatMesh> = rig
        .into_iter()
        .flat_map(|r| {
            r.skinned
                .iter()
                .map(|s| &s.mesh)
                .chain(r.attached.iter().map(|a| &a.mesh))
        })
        .collect();
    for mesh in meshes.iter().chain(rig_meshes.iter().copied()) {
        let source = mesh.material_id as usize;
        if let Some(slot) = material_remap.get_mut(source) {
            if slot.is_none() {
                *slot = Some(used_materials.len());
                used_materials.push(&materials[source]);
            }
        }
    }

    // Images, deduplicated by texture name: one embedded blob however many
    // materials point at it (§5.4).
    let mut images: Vec<Value> = Vec::new();
    let mut gltf_textures: Vec<Value> = Vec::new();
    let mut texture_index: Map<String, Value> = Map::new();
    for material in &used_materials {
        for name in [
            &material.base_color_texture,
            &material.normal_texture,
            &material.roughness_texture,
        ]
        .into_iter()
        .flatten()
        {
            if texture_index.contains_key(name) {
                continue;
            }
            let Some(prepared) = textures.get(name) else {
                continue;
            };
            let view = push_view(&mut bin, &mut buffer_views, &prepared.bytes, None);
            images.push(json!({ "bufferView": view, "mimeType": prepared.mime, "name": name }));
            gltf_textures.push(json!({ "sampler": 0, "source": images.len() - 1 }));
            texture_index.insert(name.clone(), json!(gltf_textures.len() - 1));
        }
    }

    let mut gltf_meshes: Vec<Value> = Vec::new();
    let mut nodes: Vec<Value> = Vec::new();
    let mut skins: Vec<Value> = Vec::new();
    let mut animations: Vec<Value> = Vec::new();
    // Les nœuds que la scène liste directement. Tout n'en est plus un : les os
    // du mannequin et ce qui leur est accroché pendent de leur parent.
    let mut roots: Vec<usize> = Vec::new();
    for mesh in meshes {
        let mesh_index = push_mesh(
            &mut bin,
            &mut buffer_views,
            &mut accessors,
            &mut gltf_meshes,
            mesh,
            &material_remap,
            materials,
            None,
        );
        let mut node = json!({ "name": mesh.name, "mesh": mesh_index });
        // **Un maillage braqué garde son pivot et son axe**, et ses sommets
        // sont déjà relatifs au pivot (voir `geometry::walk`). La vue n'a donc
        // qu'une rotation à écrire sur le nœud pour tourner une roue, sans rien
        // reconvertir — c'est ce qui sort l'angle de braquage de la clé de
        // cache. `extras` et non une extension : rien ici ne demande à être
        // compris par un autre lecteur glTF, et un lecteur qui l'ignore voit la
        // voiture roues droites, ce qui est le repli qu'on veut.
        if let Some(steer) = mesh.steer {
            node["translation"] = json!(steer.pivot);
            let mut described = json!({ "axis": steer.axis, "gain": steer.gain });
            // Absente quand rien n'arrête ce nœud : c'est le cas des roues,
            // que l'aperçu laisse aller jusqu'où le réglage le demande.
            if let Some(limit) = steer.limit {
                described["limit"] = json!(limit);
            }
            node["extras"] = json!({ "pitboxSteer": described });
        }
        nodes.push(node);
        roots.push(nodes.len() - 1);
    }

    // --- Le mannequin, en squelette vivant (voir `rig`) ---------------------
    //
    // Écrit après la voiture pour que les index de nœud de celle-ci ne bougent
    // pas, et parce que rien de tout ceci n'existe sur la plupart des modèles.
    if let Some(rig) = rig {
        let first_joint = nodes.len();
        // Les os d'abord, à plat : leurs enfants sont recousus juste après,
        // une fois que tous les index existent.
        for joint in &rig.joints {
            nodes.push(json!({
                "name": joint.name,
                "translation": joint.rest.translation,
                "rotation": joint.rest.rotation,
                "scale": joint.rest.scale,
            }));
        }
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); rig.joints.len()];
        for (index, joint) in rig.joints.iter().enumerate() {
            if let Some(parent) = joint.parent {
                children[parent].push(first_joint + index);
            }
        }

        // Un maillage rigide accroché à un os devient **enfant de cet os**.
        // Sans ça il resterait en arrière dès que l'os bouge : c'est le cas du
        // casque en cinq pièces de `rh_schuberth_helmet_driver_19`.
        for attached in &rig.attached {
            let mesh_index = push_mesh(
                &mut bin,
                &mut buffer_views,
                &mut accessors,
                &mut gltf_meshes,
                &attached.mesh,
                &material_remap,
                materials,
                None,
            );
            nodes.push(json!({ "name": attached.mesh.name, "mesh": mesh_index }));
            children[attached.joint].push(nodes.len() - 1);
        }
        for (index, list) in children.into_iter().enumerate() {
            if !list.is_empty() {
                nodes[first_joint + index]["children"] = json!(list);
            }
        }

        // **Une peau par maillage skinné**, et non une pour tout le mannequin :
        // les indices d'os d'un maillage désignent les entrées de *sa* peau,
        // et chaque maillage n'utilise qu'un sous-ensemble des os.
        let mut skinned_roots: Vec<usize> = Vec::new();
        for skinned in &rig.skinned {
            let joints_accessor = push_accessor_joints(&mut bin, &mut buffer_views, &mut accessors, &skinned.joints);
            let weights_accessor = push_accessor_vec4(&mut bin, &mut buffer_views, &mut accessors, &skinned.weights);
            let mesh_index = push_mesh(
                &mut bin,
                &mut buffer_views,
                &mut accessors,
                &mut gltf_meshes,
                &skinned.mesh,
                &material_remap,
                materials,
                Some((joints_accessor, weights_accessor)),
            );
            let binds = push_accessor_mat4(&mut bin, &mut buffer_views, &mut accessors, &skinned.inverse_binds);
            skins.push(json!({
                "inverseBindMatrices": binds,
                "joints": skinned.bones.iter().map(|b| first_joint + b).collect::<Vec<_>>(),
            }));
            // **À la racine de la scène**, pas sous le squelette : glTF ignore
            // la transformation d'un nœud skinné, et l'y ranger inviterait un
            // lecteur à l'appliquer deux fois.
            nodes.push(json!({
                "name": skinned.mesh.name,
                "mesh": mesh_index,
                "skin": skins.len() - 1,
            }));
            skinned_roots.push(nodes.len() - 1);
        }

        // L'animation de braquage : une entrée par image, et par os animé une
        // piste de translation, une de rotation, une d'échelle.
        if !rig.tracks.is_empty() {
            let times: Vec<f32> = (0..rig.frames).map(|f| f as f32).collect();
            let input = push_accessor_scalar(&mut bin, &mut buffer_views, &mut accessors, &times);
            let mut samplers: Vec<Value> = Vec::new();
            let mut channels: Vec<Value> = Vec::new();
            for track in &rig.tracks {
                let translations: Vec<[f32; 3]> = track.frames.iter().map(|f| f.translation).collect();
                let rotations: Vec<[f32; 4]> = track.frames.iter().map(|f| f.rotation).collect();
                let scales: Vec<[f32; 3]> = track.frames.iter().map(|f| f.scale).collect();
                for (path, output) in [
                    (
                        "translation",
                        push_accessor_vec3(&mut bin, &mut buffer_views, &mut accessors, &translations, false),
                    ),
                    (
                        "rotation",
                        push_accessor_vec4(&mut bin, &mut buffer_views, &mut accessors, &rotations),
                    ),
                    (
                        "scale",
                        push_accessor_vec3(&mut bin, &mut buffer_views, &mut accessors, &scales, false),
                    ),
                ] {
                    samplers.push(json!({ "input": input, "output": output, "interpolation": "LINEAR" }));
                    channels.push(json!({
                        "sampler": samplers.len() - 1,
                        "target": { "node": first_joint + track.joint, "path": path },
                    }));
                }
            }
            animations.push(json!({
                "name": "steer",
                "samplers": samplers,
                "channels": channels,
                // Ce qu'il faut à la vue pour choisir une image à partir de
                // l'angle réglé — qui est celui des **roues**. Les trois images
                // sont mesurées sur l'animation, pas déduites de sa longueur :
                // ses bouts ne sont pas des butées (voir `Rig::extremes`).
                "extras": {
                    "pitboxDriver": {
                        "frames": rig.frames,
                        "low": rig.extremes.0,
                        "centre": rig.extremes.1,
                        "high": rig.extremes.2,
                        "wheelLimit": rig.wheel_limit,
                    }
                },
            }));
        }
        roots.extend(
            rig.joints
                .iter()
                .enumerate()
                .filter(|(_, joint)| joint.parent.is_none())
                .map(|(index, _)| first_joint + index),
        );
        roots.extend(skinned_roots);
    }

    let json_materials: Vec<Value> = used_materials
        .iter()
        .map(|m| material_json(m, &texture_index))
        .collect();

    let mut document = json!({
        "asset": { "version": "2.0", "generator": "Pit Box kn5-gltf" },
        "scene": 0,
        "scenes": [ { "nodes": roots } ],
        "nodes": nodes,
        "meshes": gltf_meshes,
        "accessors": accessors,
        "bufferViews": buffer_views,
        "buffers": [ { "byteLength": bin.len() } ],
    });

    // Déclaration obligatoire : un lecteur qui ne connaît pas une extension
    // doit pouvoir le dire. `extensionsUsed` (et non `extensionsRequired`) :
    // le modèle reste lisible sans elles, le verre y perd seulement son reflet.
    let extensions: Vec<&str> = used_materials
        .iter()
        .flat_map(|m| {
            [
                (m.transmission > 0.0).then_some("KHR_materials_transmission"),
                m.ior.map(|_| "KHR_materials_ior"),
                (m.clearcoat > 0.0).then_some("KHR_materials_clearcoat"),
            ]
        })
        .flatten()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    if !extensions.is_empty() {
        document["extensionsUsed"] = json!(extensions);
    }

    if !skins.is_empty() {
        document["skins"] = json!(skins);
    }
    if !animations.is_empty() {
        document["animations"] = json!(animations);
    }

    if !json_materials.is_empty() {
        document["materials"] = json!(json_materials);
    }
    if !images.is_empty() {
        document["images"] = json!(images);
        document["textures"] = json!(gltf_textures);
        // A single sampler for everything: repeat on both axes, trilinear.
        // AC relies on wrapping for tyre treads and detail maps.
        document["samplers"] = json!([ { "magFilter": 9729, "minFilter": 9987, "wrapS": 10497, "wrapT": 10497 } ]);
    }

    Ok(container(&document, &bin))
}

fn material_json(material: &GltfMaterial, texture_index: &Map<String, Value>) -> Value {
    let mut pbr = json!({
        "metallicFactor": material.metallic,
        "roughnessFactor": material.roughness,
        "baseColorFactor": material.base_color,
    });
    if let Some(index) = material.base_color_texture.as_ref().and_then(|n| texture_index.get(n)) {
        pbr["baseColorTexture"] = json!({ "index": index });
    }
    if let Some(index) = material.roughness_texture.as_ref().and_then(|n| texture_index.get(n)) {
        pbr["metallicRoughnessTexture"] = json!({ "index": index });
    }

    let mut value = json!({
        "name": material.name,
        "pbrMetallicRoughness": pbr,
        "alphaMode": material.alpha_mode.as_str(),
        "doubleSided": material.double_sided,
        "emissiveFactor": material.emissive,
    });
    if material.alpha_mode == crate::material::AlphaMode::Mask {
        value["alphaCutoff"] = json!(material.alpha_cutoff);
    }

    let mut material_extensions = json!({});
    if material.transmission > 0.0 {
        material_extensions["KHR_materials_transmission"] = json!({ "transmissionFactor": material.transmission });
    }
    if let Some(ior) = material.ior {
        material_extensions["KHR_materials_ior"] = json!({ "ior": ior });
    }
    if material.clearcoat > 0.0 {
        material_extensions["KHR_materials_clearcoat"] = json!({
            "clearcoatFactor": material.clearcoat,
            "clearcoatRoughnessFactor": material.clearcoat_roughness,
        });
    }
    if material_extensions.as_object().is_some_and(|o| !o.is_empty()) {
        value["extensions"] = material_extensions;
    }
    if let Some(index) = material.normal_texture.as_ref().and_then(|n| texture_index.get(n)) {
        value["normalTexture"] = json!({ "index": index, "scale": material.normal_scale });
    }
    value
}

/// Appends bytes to the binary chunk and registers a buffer view over them.
///
/// Every view starts on a four-byte boundary. glTF only requires alignment to
/// the component size, but four satisfies every type used here and costs three
/// padding bytes at worst.
/// Écrit un maillage et sa primitive, et rend l'index du `mesh` glTF.
///
/// Partagé par la voiture et le mannequin : les deux écrivent les mêmes
/// attributs, seul le mannequin ajoute sa peau. En faire deux copies aurait
/// suffi à ce qu'un jour l'une écrive les tangentes et pas l'autre.
#[allow(clippy::too_many_arguments)]
fn push_mesh(
    bin: &mut Vec<u8>,
    views: &mut Vec<Value>,
    accessors: &mut Vec<Value>,
    gltf_meshes: &mut Vec<Value>,
    mesh: &FlatMesh,
    material_remap: &[Option<usize>],
    materials: &[GltfMaterial],
    skin: Option<(usize, usize)>,
) -> usize {
    let positions = push_accessor_vec3(bin, views, accessors, &mesh.positions, true);
    let normals = push_accessor_vec3(bin, views, accessors, &mesh.normals, false);
    let uvs = push_accessor_vec2(bin, views, accessors, &mesh.uvs);
    let indices = push_accessor_indices(bin, views, accessors, &mesh.indices);

    let mut primitive = json!({
        "attributes": { "POSITION": positions, "NORMAL": normals, "TEXCOORD_0": uvs },
        "indices": indices,
        "mode": 4,
    });
    if let Some(Some(index)) = material_remap.get(mesh.material_id as usize) {
        primitive["material"] = json!(index);
    }
    // **Le repère tangent n'est écrit que là où il sert.** Sans carte de
    // normales il ne change rien au rendu, et il coûte seize octets par
    // sommet — sur une voiture de 500 000 sommets, huit mégaoctets pour
    // rien. Avec une carte, en revanche, il est ce qui évite au lecteur de
    // reconstruire le repère à l'écran et d'exploser sur les UV dégénérés
    // (voir `geometry::convert_mesh`).
    let needs_tangents = materials
        .get(mesh.material_id as usize)
        .is_some_and(|m| m.normal_texture.is_some());
    if needs_tangents && mesh.tangents.len() == mesh.positions.len() {
        let tangents = push_accessor_vec4(bin, views, accessors, &mesh.tangents);
        primitive["attributes"]["TANGENT"] = json!(tangents);
    }
    if let Some((joints, weights)) = skin {
        primitive["attributes"]["JOINTS_0"] = json!(joints);
        primitive["attributes"]["WEIGHTS_0"] = json!(weights);
    }
    gltf_meshes.push(json!({ "name": mesh.name, "primitives": [primitive] }));
    gltf_meshes.len() - 1
}

/// `JOINTS_0` : quatre indices d'os par sommet, en entiers courts non signés.
fn push_accessor_joints(
    bin: &mut Vec<u8>,
    views: &mut Vec<Value>,
    accessors: &mut Vec<Value>,
    data: &[[u16; 4]],
) -> usize {
    let mut bytes = Vec::with_capacity(data.len() * 8);
    for value in data {
        for component in value {
            bytes.extend_from_slice(&component.to_le_bytes());
        }
    }
    let view = push_view(bin, views, &bytes, Some(TARGET_ARRAY_BUFFER));
    accessors.push(json!({
        "bufferView": view,
        "componentType": COMPONENT_U16,
        "count": data.len(),
        "type": "VEC4",
    }));
    accessors.len() - 1
}

/// `inverseBindMatrices` : les matrices telles que le KN5 les range, sans
/// transposition — les deux conventions se croisent et s'annulent (voir
/// l'en-tête de [`crate::rig`]).
fn push_accessor_mat4(
    bin: &mut Vec<u8>,
    views: &mut Vec<Value>,
    accessors: &mut Vec<Value>,
    data: &[[f32; 16]],
) -> usize {
    let mut bytes = Vec::with_capacity(data.len() * 64);
    for value in data {
        for component in value {
            bytes.extend_from_slice(&component.to_le_bytes());
        }
    }
    // Pas de `target` : ce n'est ni un tampon de sommets ni un tampon d'index.
    let view = push_view(bin, views, &bytes, None);
    accessors.push(json!({
        "bufferView": view,
        "componentType": COMPONENT_F32,
        "count": data.len(),
        "type": "MAT4",
    }));
    accessors.len() - 1
}

/// L'entrée d'une animation : le temps de chaque image. glTF exige que
/// l'accesseur d'entrée porte ses bornes, faute de quoi un lecteur ne sait pas
/// quelle est la durée du clip.
fn push_accessor_scalar(bin: &mut Vec<u8>, views: &mut Vec<Value>, accessors: &mut Vec<Value>, data: &[f32]) -> usize {
    let mut bytes = Vec::with_capacity(data.len() * 4);
    for value in data {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let view = push_view(bin, views, &bytes, None);
    let min = data.iter().copied().fold(f32::INFINITY, f32::min);
    let max = data.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    accessors.push(json!({
        "bufferView": view,
        "componentType": COMPONENT_F32,
        "count": data.len(),
        "type": "SCALAR",
        "min": [min],
        "max": [max],
    }));
    accessors.len() - 1
}

fn push_view(bin: &mut Vec<u8>, views: &mut Vec<Value>, data: &[u8], target: Option<u32>) -> usize {
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let offset = bin.len();
    bin.extend_from_slice(data);
    let mut view = json!({ "buffer": 0, "byteOffset": offset, "byteLength": data.len() });
    if let Some(target) = target {
        view["target"] = json!(target);
    }
    views.push(view);
    views.len() - 1
}

fn push_accessor_vec4(
    bin: &mut Vec<u8>,
    views: &mut Vec<Value>,
    accessors: &mut Vec<Value>,
    data: &[[f32; 4]],
) -> usize {
    let mut bytes = Vec::with_capacity(data.len() * 16);
    for value in data {
        for component in value {
            bytes.extend_from_slice(&component.to_le_bytes());
        }
    }
    let view = push_view(bin, views, &bytes, Some(TARGET_ARRAY_BUFFER));
    accessors.push(json!({
        "bufferView": view,
        "componentType": COMPONENT_F32,
        "count": data.len(),
        "type": "VEC4",
    }));
    accessors.len() - 1
}

fn push_accessor_vec3(
    bin: &mut Vec<u8>,
    views: &mut Vec<Value>,
    accessors: &mut Vec<Value>,
    data: &[[f32; 3]],
    with_bounds: bool,
) -> usize {
    let mut bytes = Vec::with_capacity(data.len() * 12);
    for value in data {
        for component in value {
            bytes.extend_from_slice(&component.to_le_bytes());
        }
    }
    let view = push_view(bin, views, &bytes, Some(TARGET_ARRAY_BUFFER));
    let mut accessor = json!({
        "bufferView": view,
        "componentType": COMPONENT_F32,
        "count": data.len(),
        "type": "VEC3",
    });
    if with_bounds {
        // `min`/`max` are mandatory on POSITION: viewers use them for frustum
        // culling and for framing the camera, and some refuse the file
        // outright without them.
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for value in data {
            for axis in 0..3 {
                min[axis] = min[axis].min(value[axis]);
                max[axis] = max[axis].max(value[axis]);
            }
        }
        accessor["min"] = json!(min);
        accessor["max"] = json!(max);
    }
    accessors.push(accessor);
    accessors.len() - 1
}

fn push_accessor_vec2(
    bin: &mut Vec<u8>,
    views: &mut Vec<Value>,
    accessors: &mut Vec<Value>,
    data: &[[f32; 2]],
) -> usize {
    let mut bytes = Vec::with_capacity(data.len() * 8);
    for value in data {
        for component in value {
            bytes.extend_from_slice(&component.to_le_bytes());
        }
    }
    let view = push_view(bin, views, &bytes, Some(TARGET_ARRAY_BUFFER));
    accessors.push(json!({
        "bufferView": view,
        "componentType": COMPONENT_F32,
        "count": data.len(),
        "type": "VEC2",
    }));
    accessors.len() - 1
}

/// Index, dans le plus étroit des deux types que WebGL 2 accepte.
///
/// La fusion par matériau (voir `geometry::merge_by_material`) fait dépasser
/// 65 535 sommets à une carrosserie entière, ce qu'un `u16` ne peut plus
/// adresser — mais la plupart des maillages fusionnés restent bien en deçà, et
/// les écrire tous en 32 bits gonflait le `.glb` de 10 % pour rien. Le cache
/// disque a une taille réglée par l'utilisateur : la gaspiller sur des zéros
/// de poids fort serait un mauvais échange.
fn push_accessor_indices(bin: &mut Vec<u8>, views: &mut Vec<Value>, accessors: &mut Vec<Value>, data: &[u32]) -> usize {
    let wide = data.iter().any(|index| *index > u32::from(u16::MAX));
    let mut bytes = Vec::with_capacity(data.len() * if wide { 4 } else { 2 });
    for value in data {
        if wide {
            bytes.extend_from_slice(&value.to_le_bytes());
        } else {
            bytes.extend_from_slice(&(*value as u16).to_le_bytes());
        }
    }
    let view = push_view(bin, views, &bytes, Some(TARGET_ELEMENT_ARRAY_BUFFER));
    accessors.push(json!({
        "bufferView": view,
        "componentType": if wide { COMPONENT_U32 } else { COMPONENT_U16 },
        "count": data.len(),
        "type": "SCALAR",
    }));
    accessors.len() - 1
}

/// Wraps the document and its binary payload in the GLB container: a 12-byte
/// header then two length-prefixed chunks, each padded to four bytes — the
/// JSON one with spaces, the binary one with zeros, as the specification
/// requires.
fn container(document: &Value, bin: &[u8]) -> Vec<u8> {
    let mut json_chunk = serde_json::to_vec(document).unwrap_or_default();
    while !json_chunk.len().is_multiple_of(4) {
        json_chunk.push(b' ');
    }
    let mut bin_chunk = bin.to_vec();
    while !bin_chunk.len().is_multiple_of(4) {
        bin_chunk.push(0);
    }

    let total = 12 + 8 + json_chunk.len() + 8 + bin_chunk.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());

    out.extend_from_slice(&(json_chunk.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json_chunk);

    out.extend_from_slice(&(bin_chunk.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin_chunk);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::AlphaMode;

    fn sample_mesh() -> FlatMesh {
        FlatMesh {
            name: "BODY".to_string(),
            material_id: 0,
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 2.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            tangents: vec![[1.0, 0.0, 0.0, 1.0]; 3],
            indices: vec![0, 1, 2],
            transparent: false,
            steer: None,
        }
    }

    fn sample_material() -> GltfMaterial {
        GltfMaterial {
            name: "carpaint".to_string(),
            shader: "ksPerPixel".to_string(),
            base_color_texture: None,
            normal_texture: None,
            roughness_texture: None,
            normal_scale: 1.0,
            emissive: [0.0; 3],
            roughness: 0.5,
            metallic: 0.0,
            alpha_mode: AlphaMode::Opaque,
            alpha_cutoff: 0.5,
            double_sided: false,
            base_color: [1.0, 1.0, 1.0, 1.0],
            transmission: 0.0,
            ior: None,
            clearcoat: 0.0,
            clearcoat_roughness: 0.0,
        }
    }

    // Règle : le repère tangent n'est écrit **que** là où une carte de normales
    // s'en sert. Ailleurs il ne change rien au rendu et coûte seize octets par
    // sommet — huit mégaoctets sur une voiture de 500 000 sommets.
    #[test]
    fn tangents_are_written_only_where_a_normal_map_uses_them() {
        let plain = write_glb(&[sample_mesh()], None, &[sample_material()], &TextureSet::default()).expect("writes");
        assert!(
            parse(&plain)["meshes"][0]["primitives"][0]["attributes"]["TANGENT"].is_null(),
            "sans carte de normales, rien à écrire"
        );

        let mut mapped = sample_material();
        mapped.normal_texture = Some("nm.dds".to_string());
        let with_map = write_glb(&[sample_mesh()], None, &[mapped], &TextureSet::default()).expect("writes");
        let document = parse(&with_map);
        let accessor = document["meshes"][0]["primitives"][0]["attributes"]["TANGENT"]
            .as_u64()
            .expect("TANGENT présent");
        assert_eq!(
            document["accessors"][accessor as usize]["type"], "VEC4",
            "glTF range la latéralité dans un quatrième composant"
        );
    }

    // Règle : le verre physique sort avec ses deux extensions, et le document
    // les déclare. Un lecteur qui les ignore doit pouvoir le dire — d'où
    // `extensionsUsed`, jamais `extensionsRequired` : sans elles le modèle
    // reste lisible, la vitre y perd seulement son reflet.
    #[test]
    fn physical_glass_carries_its_extensions_and_declares_them() {
        let mut glass = sample_material();
        glass.transmission = 1.0;
        glass.ior = Some(1.8);
        let glb = write_glb(&[sample_mesh()], None, &[glass], &TextureSet::default()).expect("writes");
        let document = parse(&glb);

        let used = document["extensionsUsed"].as_array().expect("extensionsUsed present");
        assert!(
            used.iter().any(|v| v == "KHR_materials_transmission") && used.iter().any(|v| v == "KHR_materials_ior"),
            "les deux extensions sont déclarées, got {used:?}"
        );
        assert!(
            document["extensionsRequired"].is_null(),
            "le modèle doit rester lisible sans elles"
        );

        let extensions = &document["materials"][0]["extensions"];
        assert_eq!(
            extensions["KHR_materials_transmission"]["transmissionFactor"], 1.0,
            "la transmission porte la transparence"
        );
        // Tolérance : la valeur traverse un `f32`, elle ressort en `1.7999999…`.
        let ior = extensions["KHR_materials_ior"]["ior"].as_f64().expect("un nombre");
        assert!((ior - 1.8).abs() < 1e-6, "l'IOR déclaré par le mod, got {ior}");
    }

    // Règle : un matériau ordinaire n'écrit ni extension ni déclaration. Le
    // contraire ferait payer à chaque voiture le coût d'une passe de rendu
    // supplémentaire pour rien.
    #[test]
    fn an_ordinary_material_declares_no_extension() {
        let glb = write_glb(&[sample_mesh()], None, &[sample_material()], &TextureSet::default()).expect("writes");
        let document = parse(&glb);
        assert!(document["extensionsUsed"].is_null(), "rien à déclarer");
        assert!(document["materials"][0]["extensions"].is_null(), "rien à porter");
    }

    fn parse(glb: &[u8]) -> Value {
        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        serde_json::from_slice(&glb[20..20 + json_len]).expect("JSON chunk parses")
    }

    // Rule: the container layout is exactly what the spec describes — magic,
    // version, total length, then two padded chunks. A viewer that refuses the
    // file usually does so here, before ever looking at the scene.
    #[test]
    fn container_header_and_chunks_are_well_formed() {
        let glb = write_glb(&[sample_mesh()], None, &[sample_material()], &TextureSet::default()).expect("writes");

        assert_eq!(&glb[0..4], b"glTF", "magic");
        assert_eq!(u32::from_le_bytes(glb[4..8].try_into().unwrap()), 2, "version 2");
        assert_eq!(
            u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize,
            glb.len(),
            "declared length matches the file"
        );

        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        assert_eq!(&glb[16..20], b"JSON", "first chunk is the document");
        assert!(json_len.is_multiple_of(4), "JSON chunk padded to four bytes");
        assert_eq!(
            &glb[20 + json_len + 4..20 + json_len + 8],
            b"BIN\0",
            "second chunk is binary"
        );
    }

    // Rule: POSITION carries min/max. Some viewers reject the file without
    // them, and those that accept it cannot frame the model.
    #[test]
    fn position_accessor_declares_its_bounds() {
        let glb = write_glb(&[sample_mesh()], None, &[sample_material()], &TextureSet::default()).expect("writes");
        let document = parse(&glb);
        let position = &document["accessors"][0];
        assert_eq!(position["min"], json!([0.0, 0.0, 0.0]), "min over the three vertices");
        assert_eq!(position["max"], json!([1.0, 2.0, 0.0]), "max over the three vertices");
    }

    // Rule: only materials a surviving mesh actually uses are emitted. The
    // discarded ones would otherwise drag their textures into the payload.
    #[test]
    fn unused_materials_are_left_out() {
        let unused = GltfMaterial {
            name: "collider".to_string(),
            ..sample_material()
        };
        let glb = write_glb(
            &[sample_mesh()],
            None,
            &[sample_material(), unused],
            &TextureSet::default(),
        )
        .expect("writes");
        let document = parse(&glb);
        assert_eq!(
            document["materials"].as_array().map(Vec::len),
            Some(1),
            "one material kept"
        );
        assert_eq!(document["materials"][0]["name"], "carpaint", "the used one");
    }

    // Rule: every buffer view starts on a four-byte boundary, whatever the
    // size of what came before. Misaligned accessors are the classic cause of
    // a file that loads but renders as noise.
    #[test]
    fn buffer_views_stay_aligned() {
        // An odd-length index buffer forces padding before the next view.
        let mut mesh = sample_mesh();
        mesh.indices = vec![0, 1, 2, 0, 2, 1, 1, 2, 0];
        let glb = write_glb(
            &[mesh, sample_mesh()],
            None,
            &[sample_material()],
            &TextureSet::default(),
        )
        .expect("writes");
        let document = parse(&glb);
        for view in document["bufferViews"].as_array().expect("views") {
            let offset = view["byteOffset"].as_u64().expect("offset");
            assert!(offset.is_multiple_of(4), "buffer view aligned, got {offset}");
        }
    }
}
