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
const COMPONENT_I8: u32 = 5120;
const COMPONENT_U8: u32 = 5121;
const COMPONENT_I16: u32 = 5122;
const COMPONENT_U16: u32 = 5123;
const COMPONENT_U32: u32 = 5125;
/// La seule extension que le document exige, plutôt que de simplement s'en
/// servir (voir la déclaration dans `write_glb`).
const MESH_QUANTIZATION: &str = "KHR_mesh_quantization";

const TARGET_ARRAY_BUFFER: u32 = 34962;
const TARGET_ELEMENT_ARRAY_BUFFER: u32 = 34963;

/// Où vont les images du document.
///
/// **Deux rangements pour un seul document**, et la raison est le cache, pas
/// le format. Un `.glb` est autonome — c'est ce qu'il faut à `kn5-tool`, dont
/// la sortie doit s'ouvrir telle quelle dans Blender. Mais dans le cache, deux
/// skins d'une même voiture écrivent alors deux fois la même géométrie et les
/// mêmes textures : mesuré sur trois skins, **une seule variante de géométrie
/// pour les trois**, et les images partagées aux deux tiers. Éclatées, elles
/// s'adressent par leur contenu et ne s'écrivent qu'une fois (§15.0quater).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Layout {
    /// Tout dans le tampon binaire : le `.glb` autonome. C'est le défaut :
    /// écrire un fichier est le cas courant, seul le cache a intérêt à éclater.
    #[default]
    Embedded,
    /// Images sorties du tampon, à ranger et à nommer par l'appelant.
    Split,
}

/// Un document glTF et ses données, avant qu'on décide comment les ranger.
pub struct Document {
    /// Le document lui-même. En `Split`, `buffers[0].uri` et chaque
    /// `images[].uri` valent la chaîne vide : c'est à l'appelant de les
    /// remplir, puisque c'est lui qui nomme les blobs.
    pub json: Value,
    /// Géométrie seule en `Split`, géométrie et images en `Embedded`.
    pub buffer: Vec<u8>,
    /// Les images, dans l'ordre de `json["images"]`. Vide en `Embedded`.
    pub images: Vec<Vec<u8>>,
}

/// Assembles the whole preview into a single self-contained `.glb`.
pub fn write_glb(
    meshes: &[FlatMesh],
    rig: Option<&crate::rig::Rig>,
    materials: &[GltfMaterial],
    textures: &TextureSet,
) -> Result<Vec<u8>, String> {
    let document = build(meshes, rig, materials, textures, Layout::Embedded)?;
    Ok(container(&document.json, &document.buffer))
}

/// Le même document, images sorties du tampon (voir [`Layout`]).
pub fn build(
    meshes: &[FlatMesh],
    rig: Option<&crate::rig::Rig>,
    materials: &[GltfMaterial],
    textures: &TextureSet,
    layout: Layout,
) -> Result<Document, String> {
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
    let mut image_blobs: Vec<Vec<u8>> = Vec::new();
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
            let mut image = json!({ "mimeType": prepared.mime, "name": name });
            match layout {
                Layout::Embedded => {
                    let view = push_view(&mut bin, &mut buffer_views, &prepared.bytes, None);
                    image["bufferView"] = json!(view);
                }
                // L'URI reste vide jusqu'à ce que l'appelant ait haché le blob :
                // c'est lui qui décide du nom, donc de l'adresse.
                Layout::Split => {
                    image["uri"] = json!("");
                    image_blobs.push(prepared.bytes.clone());
                }
            }
            images.push(image);
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
            let weights_accessor = push_accessor_weights(&mut bin, &mut buffer_views, &mut accessors, &skinned.weights);
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

    // Déclaration obligatoire : un lecteur qui ne connaît pas une extension doit
    // pouvoir le dire. Celles des matériaux ne vont qu'en `extensionsUsed` — le
    // modèle reste lisible sans elles, le verre y perd seulement son reflet.
    let mut extensions: std::collections::BTreeSet<&str> = used_materials
        .iter()
        .flat_map(|m| {
            [
                (m.transmission > 0.0).then_some("KHR_materials_transmission"),
                m.ior.map(|_| "KHR_materials_ior"),
                (m.clearcoat > 0.0).then_some("KHR_materials_clearcoat"),
                (!m.uv_scale.is_identity()).then_some("KHR_texture_transform"),
            ]
        })
        .flatten()
        .collect();

    // **`KHR_mesh_quantization` fait exception : elle est EXIGÉE**, et c'est la
    // spec de l'extension qui l'impose autant que le bon sens. Les autres
    // décrivent un raffinement qu'on peut ignorer ; celle-ci change le TYPE des
    // octets d'un attribut. Un lecteur qui la passerait sous silence relirait
    // des entiers courts comme des flottants — pas un modèle terne, un tas de
    // triangles. Mieux vaut un refus franc.
    //
    // Toujours présente : il y a au moins un maillage (`write_glb` refuse le
    // contraire) et tout maillage écrit ses normales quantifiées.
    extensions.insert(MESH_QUANTIZATION);
    document["extensionsUsed"] = json!(extensions.iter().collect::<Vec<_>>());
    document["extensionsRequired"] = json!([MESH_QUANTIZATION]);

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

    if layout == Layout::Split {
        document["buffers"][0]["uri"] = json!("");
    }

    Ok(Document {
        json: document,
        buffer: bin,
        images: image_blobs,
    })
}

fn material_json(material: &GltfMaterial, texture_index: &Map<String, Value>) -> Value {
    let mut pbr = json!({
        "metallicFactor": material.metallic,
        "roughnessFactor": material.roughness,
        "baseColorFactor": material.base_color,
    });
    if let Some(index) = material.base_color_texture.as_ref().and_then(|n| texture_index.get(n)) {
        pbr["baseColorTexture"] = texture_info(index, material.uv_scale.diffuse);
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
        value["normalTexture"] = texture_info(index, material.uv_scale.normal);
    }
    value
}

/// Une référence de texture, avec sa répétition d'UV quand il y en a une.
///
/// `KHR_texture_transform` et non des UV recalculés dans le maillage : la
/// répétition appartient au matériau, pas à la géométrie, et un même maillage
/// peut porter une diffuse répétée quarante fois sous une normale répétée
/// autrement. L'échantillonneur du document répète déjà sur les deux axes.
fn texture_info(index: &Value, scale: f32) -> Value {
    let mut info = json!({ "index": index });
    if (scale - 1.0).abs() > f32::EPSILON {
        info["extensions"] = json!({ "KHR_texture_transform": { "scale": [scale, scale] } });
    }
    info
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
    let normals = push_accessor_normals(bin, views, accessors, &mesh.normals);
    let uvs = push_accessor_uvs(bin, views, accessors, &mesh.uvs);
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
        let tangents = push_accessor_tangents(bin, views, accessors, &mesh.tangents);
        primitive["attributes"]["TANGENT"] = json!(tangents);
    }
    if let Some((joints, weights)) = skin {
        primitive["attributes"]["JOINTS_0"] = json!(joints);
        primitive["attributes"]["WEIGHTS_0"] = json!(weights);
    }
    gltf_meshes.push(json!({ "name": mesh.name, "primitives": [primitive] }));
    gltf_meshes.len() - 1
}

// --- Attributs quantifiés (`KHR_mesh_quantization`) -------------------------
//
// Un aperçu de voiture, c'est 60 % de géométrie et 40 % d'images — mesuré sur
// le cache : 133 Mo contre 92 sur onze entrées. Le poste le plus lourd n'était
// donc pas les textures mais quatre attributs écrits en `f32` alors qu'aucun
// n'a besoin de 32 bits de précision :
//
//   NORMAL      12 o → 8 (short normalisé + complément)
//   TANGENT     16 o → 4 (byte normalisé)
//   TEXCOORD_0   8 o → 4 (ushort normalisé, quand les UV tiennent dans [0,1])
//   WEIGHTS_0   16 o → 4 (ubyte normalisé)
//
// **Pourquoi `short` pour les normales et `byte` pour les tangentes**, et pas
// `byte` partout (qui rapporterait quatre octets de plus par sommet) : la
// normale est ce que TOUT calcul d'éclairage consomme directement, et 0,5°
// d'erreur — ce que donne un byte — se voit d'abord en bandes dans un reflet
// net qui balaie une grande surface courbe, exactement le capot d'une voiture.
// La tangente, elle, ne fait qu'orienter une carte de normales dont le bruit
// de texel dépasse largement cet écart. Passer les normales en byte reste une
// ligne à changer si la place manque plus que la finesse.
//
// `POSITION` reste en `f32` : le quantifier demande de rendre l'échelle par la
// transformation du nœud, or celle d'un maillage skinné est ignorée par la
// spec — il faudrait la cuire dans les matrices de liaison du mannequin. C'est
// le lot suivant, pas celui-ci.

/// Vers un entier court signé normalisé (`-32767..=32767` pour `-1..=1`).
fn quantize_i16(value: f32) -> i16 {
    (value.clamp(-1.0, 1.0) * 32767.0).round() as i16
}

/// Vers un octet signé normalisé (`-127..=127` pour `-1..=1`).
fn quantize_i8(value: f32) -> i8 {
    (value.clamp(-1.0, 1.0) * 127.0).round() as i8
}

/// Un vecteur ramené à la longueur 1, ou `None` s'il est nul.
///
/// La quantification normalisée suppose des composantes dans `[-1,1]` : une
/// normale d'un poil plus longue que 1 — ce que produit une interpolation —
/// serait écrêtée composante par composante, donc **déviée**, alors que la
/// renormaliser ne coûte rien.
fn unit(value: [f32; 3]) -> Option<[f32; 3]> {
    let length = (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt();
    (length > 1e-6).then(|| [value[0] / length, value[1] / length, value[2] / length])
}

/// `NORMAL` : trois entiers courts signés normalisés, complétés à huit octets.
fn push_accessor_normals(
    bin: &mut Vec<u8>,
    views: &mut Vec<Value>,
    accessors: &mut Vec<Value>,
    data: &[[f32; 3]],
) -> usize {
    let mut bytes = Vec::with_capacity(data.len() * 8);
    for value in data {
        let normal = unit(*value).unwrap_or([0.0, 0.0, 0.0]);
        for component in normal {
            bytes.extend_from_slice(&quantize_i16(component).to_le_bytes());
        }
        // Le complément à quatre octets exigé par la spec (voir
        // `push_view_strided`).
        bytes.extend_from_slice(&[0, 0]);
    }
    let view = push_view_strided(bin, views, &bytes, Some(TARGET_ARRAY_BUFFER), Some(8));
    accessors.push(json!({
        "bufferView": view,
        "componentType": COMPONENT_I16,
        "normalized": true,
        "count": data.len(),
        "type": "VEC3",
    }));
    accessors.len() - 1
}

/// `TANGENT` : le repère en octets signés normalisés, latéralité comprise.
///
/// Le quatrième composant vaut ±1 et se quantifie comme les autres : ±127
/// redonne exactement ±1 une fois déquantifié, la latéralité ne s'abîme pas.
fn push_accessor_tangents(
    bin: &mut Vec<u8>,
    views: &mut Vec<Value>,
    accessors: &mut Vec<Value>,
    data: &[[f32; 4]],
) -> usize {
    let mut bytes = Vec::with_capacity(data.len() * 4);
    for value in data {
        let axis = unit([value[0], value[1], value[2]]).unwrap_or([1.0, 0.0, 0.0]);
        for component in axis {
            bytes.push(quantize_i8(component) as u8);
        }
        bytes.push(quantize_i8(if value[3] < 0.0 { -1.0 } else { 1.0 }) as u8);
    }
    let view = push_view(bin, views, &bytes, Some(TARGET_ARRAY_BUFFER));
    accessors.push(json!({
        "bufferView": view,
        "componentType": COMPONENT_I8,
        "normalized": true,
        "count": data.len(),
        "type": "VEC4",
    }));
    accessors.len() - 1
}

/// `TEXCOORD_0` : entiers courts normalisés quand l'atlas tient dans la plage
/// d'un normalisé, flottants sinon.
///
/// Un accesseur normalisé ne sait couvrir que `[0,1]` (non signé) ou `[-1,1]`
/// (signé) : au-delà, il écrête, et une texture posée de travers ne se
/// signalerait nulle part. D'où un test primitive par primitive, et le repli en
/// flottants pour ce qui déborde.
///
/// **Les deux plages, et pas seulement la première** — c'est la mesure qui l'a
/// imposé. Le carré unité seul ne servait à rien : sur quatre voitures de
/// référence, **2 primitives sur 301** y tenaient, soit 0 Mo gagné sur 4,1. Les
/// îlots d'UV d'AC débordent presque toujours un peu, sans pour autant répéter.
/// En ouvrant à `[-1,1]`, **242 primitives passent, soit 3,6 Mo sur 4,1** ; il
/// ne reste que 46 primitives qui répètent vraiment (au-delà de 1) et 11 qui
/// débordent d'un côté seulement. Le coût est le même — quatre octets — et la
/// perte vaut 1/32767 d'atlas, six centièmes de texel sur une texture de 2048.
///
/// La fusion par matériau (`geometry::merge_by_material`) explique la sévérité
/// du premier test : une primitive couvre TOUT un matériau, donc un seul
/// détail qui répète y entraînait la carrosserie entière.
fn push_accessor_uvs(
    bin: &mut Vec<u8>,
    views: &mut Vec<Value>,
    accessors: &mut Vec<Value>,
    data: &[[f32; 2]],
) -> usize {
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for value in data {
        for component in value {
            min = min.min(*component);
            max = max.max(*component);
        }
    }
    // Comparaisons fausses pour un NaN, qui part donc en flottants : c'est le
    // bon défaut, on ne quantifie pas ce qu'on ne comprend pas.
    let unsigned = min >= 0.0 && max <= 1.0;
    let signed = min >= -1.0 && max <= 1.0;
    if !unsigned && !signed {
        return push_accessor_vec2(bin, views, accessors, data);
    }

    let mut bytes = Vec::with_capacity(data.len() * 4);
    for value in data {
        for component in value {
            if unsigned {
                bytes.extend_from_slice(&((component.clamp(0.0, 1.0) * 65535.0).round() as u16).to_le_bytes());
            } else {
                bytes.extend_from_slice(&quantize_i16(*component).to_le_bytes());
            }
        }
    }
    let view = push_view(bin, views, &bytes, Some(TARGET_ARRAY_BUFFER));
    accessors.push(json!({
        "bufferView": view,
        "componentType": if unsigned { COMPONENT_U16 } else { COMPONENT_I16 },
        "normalized": true,
        "count": data.len(),
        "type": "VEC2",
    }));
    accessors.len() - 1
}

/// `WEIGHTS_0` : quatre octets non signés normalisés, de somme exactement 255.
///
/// La somme est corrigée et pas seulement arrondie : glTF exige des poids de
/// somme 1, et quatre arrondis indépendants la manquent d'une unité ou deux
/// une fois sur deux. L'écart est infime, mais il se produit **à chaque
/// sommet** d'un mannequin entier, et c'est le genre de dérive qui fait fondre
/// un doigt sans qu'on sache d'où elle vient. Le reste va aux plus grandes
/// parties fractionnaires — la règle du plus fort reste.
fn push_accessor_weights(
    bin: &mut Vec<u8>,
    views: &mut Vec<Value>,
    accessors: &mut Vec<Value>,
    data: &[[f32; 4]],
) -> usize {
    let mut bytes = Vec::with_capacity(data.len() * 4);
    for value in data {
        let total: f32 = value.iter().map(|w| w.max(0.0)).sum();
        let mut quantized = [0u8; 4];
        if total > 1e-6 {
            let exact = value.map(|w| w.max(0.0) / total * 255.0);
            let mut sum = 0u32;
            for slot in 0..4 {
                quantized[slot] = exact[slot].floor() as u8;
                sum += u32::from(quantized[slot]);
            }
            let fract = |i: usize| exact[i] - exact[i].floor();
            let mut order = [0usize, 1, 2, 3];
            order.sort_by(|a, b| fract(*b).total_cmp(&fract(*a)));
            for slot in order {
                if sum >= 255 {
                    break;
                }
                if quantized[slot] < u8::MAX {
                    quantized[slot] += 1;
                    sum += 1;
                }
            }
        }
        bytes.extend_from_slice(&quantized);
    }
    let view = push_view(bin, views, &bytes, Some(TARGET_ARRAY_BUFFER));
    accessors.push(json!({
        "bufferView": view,
        "componentType": COMPONENT_U8,
        "normalized": true,
        "count": data.len(),
        "type": "VEC4",
    }));
    accessors.len() - 1
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
    push_view_strided(bin, views, data, target, None)
}

/// Idem, mais en déclarant le pas entre deux éléments.
///
/// **Obligatoire dès qu'un élément est complété.** glTF impose qu'un attribut
/// de sommet commence sur une frontière de quatre octets, or un `VEC3` de
/// `SHORT` en fait six : on écrit donc huit octets par normale, et sans
/// `byteStride` le lecteur relirait la donnée avec un pas de six — c'est-à-dire
/// n'importe quoi dès le deuxième sommet.
fn push_view_strided(
    bin: &mut Vec<u8>,
    views: &mut Vec<Value>,
    data: &[u8],
    target: Option<u32>,
    stride: Option<usize>,
) -> usize {
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let offset = bin.len();
    bin.extend_from_slice(data);
    let mut view = json!({ "buffer": 0, "byteOffset": offset, "byteLength": data.len() });
    if let Some(target) = target {
        view["target"] = json!(target);
    }
    if let Some(stride) = stride {
        view["byteStride"] = json!(stride);
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
pub(crate) fn container(document: &Value, bin: &[u8]) -> Vec<u8> {
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
            uv_scale: crate::material::UvScale::default(),
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
        let required = document["extensionsRequired"]
            .as_array()
            .expect("extensionsRequired present");
        assert!(
            !required.iter().any(|v| v == "KHR_materials_transmission")
                && !required.iter().any(|v| v == "KHR_materials_ior"),
            "le modèle doit rester lisible sans elles, got {required:?}"
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
    // supplémentaire pour rien. (La quantification, elle, est toujours
    // déclarée : elle porte sur la géométrie, pas sur les matériaux.)
    #[test]
    fn an_ordinary_material_declares_no_extension() {
        let glb = write_glb(&[sample_mesh()], None, &[sample_material()], &TextureSet::default()).expect("writes");
        let document = parse(&glb);
        let used = document["extensionsUsed"].as_array().expect("extensionsUsed present");
        assert_eq!(used, &[Value::from(MESH_QUANTIZATION)], "rien à déclarer de plus");
        assert!(document["materials"][0]["extensions"].is_null(), "rien à porter");
    }

    // Règle : les attributs quantifiés se déclarent en `extensionsRequired`, et
    // pas seulement en `extensionsUsed`. Un lecteur qui ignorerait l'extension
    // relirait des entiers courts comme des flottants — un tas de triangles,
    // pas un modèle terne. Le refus franc vaut mieux.
    #[test]
    fn quantized_attributes_declare_the_extension_as_required() {
        let glb = write_glb(&[sample_mesh()], None, &[sample_material()], &TextureSet::default()).expect("writes");
        let document = parse(&glb);

        let used = document["extensionsUsed"].as_array().expect("extensionsUsed present");
        assert!(used.iter().any(|v| v == MESH_QUANTIZATION), "déclarée comme employée");
        let required = document["extensionsRequired"]
            .as_array()
            .expect("extensionsRequired present");
        assert!(required.iter().any(|v| v == MESH_QUANTIZATION), "et comme exigée");

        let attributes = &document["meshes"][0]["primitives"][0]["attributes"];
        let normals = attributes["NORMAL"].as_u64().expect("NORMAL présent") as usize;
        let accessor = &document["accessors"][normals];
        assert_eq!(accessor["componentType"], COMPONENT_I16, "normales en entiers courts");
        assert_eq!(accessor["normalized"], true, "lues comme des réels de [-1,1]");

        // Le pas est ce qui rend la donnée relisible : un `VEC3` de `SHORT` fait
        // six octets, la spec en exige huit, et sans `byteStride` le lecteur
        // relirait tout de travers dès le deuxième sommet.
        let view = accessor["bufferView"].as_u64().expect("une vue") as usize;
        assert_eq!(
            document["bufferViews"][view]["byteStride"], 8,
            "le complément à quatre octets doit être annoncé"
        );
    }

    // Règle : une coordonnée de texture se quantifie tant qu'elle tient dans la
    // plage d'un normalisé — `[0,1]` non signé, `[-1,1]` signé — et retombe en
    // flottants dès qu'elle répète. Un normalisé écrêterait, et la texture se
    // poserait de travers sans un mot. Les trois cas, parce que n'en tester que
    // le premier avait laissé la quantification sans effet sur 299 primitives
    // de référence sur 301 (voir `push_accessor_uvs`).
    #[test]
    fn uvs_quantise_within_a_normalised_range_and_fall_back_when_they_tile() {
        let component_of = |mesh: FlatMesh| {
            let glb = write_glb(&[mesh], None, &[sample_material()], &TextureSet::default()).expect("writes");
            let document = parse(&glb);
            let uvs = document["meshes"][0]["primitives"][0]["attributes"]["TEXCOORD_0"]
                .as_u64()
                .expect("TEXCOORD_0 présent") as usize;
            document["accessors"][uvs]["componentType"].clone()
        };

        assert_eq!(component_of(sample_mesh()), COMPONENT_U16, "dans [0,1], non signé");

        let mut spilling = sample_mesh();
        spilling.uvs = vec![[-0.4, 0.0], [1.0, 0.0], [0.0, 0.8]];
        assert_eq!(
            component_of(spilling),
            COMPONENT_I16,
            "un îlot qui déborde un peu reste quantifiable en signé"
        );

        let mut tiled = sample_mesh();
        tiled.uvs = vec![[0.0, 0.0], [4.0, 0.0], [0.0, 4.0]];
        assert_eq!(
            component_of(tiled),
            COMPONENT_F32,
            "une UV qui répète vraiment reste en flottants"
        );
    }

    // Règle : les poids d'un sommet somment exactement à 255 après
    // quantification. Quatre arrondis indépendants manquent la cible une fois
    // sur deux, et l'écart se produirait à chaque sommet du mannequin.
    #[test]
    fn skin_weights_quantise_to_an_exact_sum() {
        let mut bin = Vec::new();
        let mut views = Vec::new();
        let mut accessors = Vec::new();
        // Un tiers/tiers/tiers ne tombe pas juste, et des poids qui ne somment
        // pas à 1 doivent être ramenés avant d'être arrondis.
        let data = [
            [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0, 0.0],
            [0.5, 0.25, 0.15, 0.10],
            [0.8, 0.8, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0],
        ];
        push_accessor_weights(&mut bin, &mut views, &mut accessors, &data);

        let offset = views[0]["byteOffset"].as_u64().expect("un décalage") as usize;
        for (index, vertex) in bin[offset..offset + data.len() * 4].chunks(4).enumerate() {
            let sum: u32 = vertex.iter().map(|w| u32::from(*w)).sum();
            let expected = if index == 3 { 0 } else { 255 };
            assert_eq!(
                sum, expected,
                "sommet {index} : poids de somme {expected}, got {vertex:?}"
            );
        }
    }

    // Règle : une normale non unitaire est RENORMALISÉE avant d'être
    // quantifiée, jamais écrêtée composante par composante — l'écrêtage la
    // ferait dévier, ce qui se verrait dans un reflet.
    #[test]
    fn an_overlong_normal_is_renormalised_not_clipped() {
        let mut long = sample_mesh();
        // Même direction que [1,1,0] normalisé, mais deux fois trop longue.
        long.normals = vec![[1.4, 1.4, 0.0]; 3];
        let glb = write_glb(&[long], None, &[sample_material()], &TextureSet::default()).expect("writes");
        let document = parse(&glb);
        let normals = document["meshes"][0]["primitives"][0]["attributes"]["NORMAL"]
            .as_u64()
            .expect("NORMAL présent") as usize;
        let view = document["accessors"][normals]["bufferView"].as_u64().expect("une vue") as usize;
        let offset = document["bufferViews"][view]["byteOffset"]
            .as_u64()
            .expect("un décalage") as usize;

        let bin = bin_chunk(&glb);
        let x = i16::from_le_bytes(bin[offset..offset + 2].try_into().unwrap());
        let y = i16::from_le_bytes(bin[offset + 2..offset + 4].try_into().unwrap());
        let expected = (std::f32::consts::FRAC_1_SQRT_2 * 32767.0).round() as i16;
        assert_eq!(
            (x, y),
            (expected, expected),
            "la direction est gardée, la longueur ramenée à 1"
        );
    }

    /// Le contenu du second chunk (`BIN`), là où vivent les accesseurs.
    fn bin_chunk(glb: &[u8]) -> &[u8] {
        let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let start = 20 + json_len;
        let bin_len = u32::from_le_bytes(glb[start..start + 4].try_into().unwrap()) as usize;
        &glb[start + 8..start + 8 + bin_len]
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
