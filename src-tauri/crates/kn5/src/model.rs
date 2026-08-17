//! In-memory representation of a parsed KN5 (spec §3).
//!
//! Deliberately close to the file layout: no filtering, no coordinate change,
//! no material interpretation. Those belong to the conversion stage, so that
//! `kn5-tool inspect` shows what is *actually* in the file — which is the
//! whole point of having an inspectable intermediate (§1).

use std::collections::BTreeSet;

/// A whole KN5 file.
#[derive(Debug, Clone)]
pub struct Kn5Model {
    pub version: u32,
    /// Extra header word present when `version > 5`, meaning unknown (§3.1).
    pub extra: Option<u32>,
    pub textures: Vec<Kn5Texture>,
    pub materials: Vec<Kn5Material>,
    /// Single root; the whole scene hangs from it.
    pub root: Kn5Node,
}

impl Kn5Model {
    /// Depth-first walk over every node, root included.
    pub fn visit_nodes(&self, f: &mut impl FnMut(&Kn5Node)) {
        self.root.visit(f);
    }

    pub fn node_count(&self) -> usize {
        let mut n = 0;
        self.visit_nodes(&mut |_| n += 1);
        n
    }

    pub fn mesh_count(&self) -> usize {
        let mut n = 0;
        self.visit_nodes(&mut |node| {
            if node.mesh().is_some() {
                n += 1;
            }
        });
        n
    }

    /// Triangles across every mesh, LODs and helpers included — this counts
    /// what the file contains, not what the preview will end up drawing.
    pub fn triangle_count(&self) -> usize {
        let mut n = 0;
        self.visit_nodes(&mut |node| {
            if let Some(mesh) = node.mesh() {
                n += mesh.indices.len() / 3;
            }
        });
        n
    }

    /// Distinct shader names, sorted. Feeds the material mapping work (§6):
    /// the list of shaders actually met in the wild is what tells us which
    /// ones deserve a special case.
    pub fn shaders(&self) -> BTreeSet<&str> {
        self.materials.iter().map(|m| m.shader.as_str()).collect()
    }

    /// Embedded texture by name. Placeholder entries (§3.2, type 0) never
    /// match: they have no name and no blob.
    pub fn texture(&self, name: &str) -> Option<&Kn5Texture> {
        self.textures.iter().find(|t| t.has_data() && t.name == name)
    }
}

/// An embedded texture. The blob is stored verbatim: sniff it, do not trust
/// the extension in `name` (§3.2).
#[derive(Debug, Clone)]
pub struct Kn5Texture {
    /// `1` = a real entry. `0` is a placeholder: the file stores nothing else
    /// for it, so `name` and `data` are empty (§3.2).
    pub kind: i32,
    pub name: String,
    pub data: Vec<u8>,
}

impl Kn5Texture {
    pub fn has_data(&self) -> bool {
        !self.data.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct Kn5Material {
    pub name: String,
    /// AC shader name, e.g. `ksPerPixelMultiMap`. Drives the glTF mapping.
    pub shader: String,
    /// `0` opaque, `1` alpha blend. Low byte of the `i16` the file stores;
    /// see [`Self::alpha_tested`] and docs/kn5-format.md (§12, question 2).
    pub blend_mode: u8,
    /// Alpha testing, i.e. the shader discards fragments below `ksAlphaRef`
    /// instead of blending them. High byte of that same `i16`.
    pub alpha_tested: bool,
    /// Word that follows `blend_mode` when `version > 4`; observed at 0.
    pub reserved: i32,
    /// *All* scalar properties, not just the known ones (§3.3) — CSP mods add
    /// their own and we want to be able to surface them later.
    pub properties: Vec<Kn5MaterialProperty>,
    pub samplers: Vec<Kn5Sampler>,
}

impl Kn5Material {
    pub fn property(&self, name: &str) -> Option<f32> {
        self.properties.iter().find(|p| p.name == name).map(|p| p.value)
    }

    /// Texture bound to a sampler slot, e.g. `txDiffuse`.
    pub fn texture_for(&self, sampler: &str) -> Option<&str> {
        self.samplers
            .iter()
            .find(|s| s.name == sampler)
            .map(|s| s.texture.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct Kn5MaterialProperty {
    pub name: String,
    pub value: f32,
    /// The 36 bytes that follow every property, read as 9 floats. Unused in
    /// v1 but kept so the open question about their meaning (§12, question 5)
    /// can be answered from real files rather than from assumption.
    pub extra: [f32; 9],
}

#[derive(Debug, Clone)]
pub struct Kn5Sampler {
    /// Slot name: `txDiffuse`, `txNormal`, `txMaps`…
    pub name: String,
    pub slot: i32,
    /// Key into [`Kn5Model::textures`]. May be empty, and may name a texture
    /// that is not embedded (skin overrides live on disk — §4.3).
    pub texture: String,
}

#[derive(Debug, Clone)]
pub struct Kn5Node {
    pub name: String,
    /// The file's own `is_active` flag. Not a rendering decision by itself.
    pub active: bool,
    pub kind: Kn5NodeKind,
    pub children: Vec<Kn5Node>,
}

impl Kn5Node {
    pub fn visit(&self, f: &mut impl FnMut(&Kn5Node)) {
        f(self);
        for child in &self.children {
            child.visit(f);
        }
    }

    /// Geometry of this node, whether rigid or skinned — the two share every
    /// field the preview cares about.
    pub fn mesh(&self) -> Option<&Kn5Mesh> {
        match &self.kind {
            Kn5NodeKind::Dummy { .. } => None,
            Kn5NodeKind::Mesh(mesh) => Some(mesh),
            Kn5NodeKind::SkinnedMesh(skinned) => Some(&skinned.mesh),
        }
    }

    /// Local transform. Only dummy nodes carry one: meshes inherit their
    /// parent's (§3.4).
    pub fn transform(&self) -> Option<&[f32; 16]> {
        match &self.kind {
            Kn5NodeKind::Dummy { transform } => Some(transform),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Kn5NodeKind {
    /// Type 1 — pure transform, the only kind that carries a matrix.
    Dummy {
        /// Row-vector 4x4 (DirectX convention): translation sits in
        /// `[12..15]`, and world = local × parent (§3.4).
        transform: [f32; 16],
    },
    /// Type 2 — rigid mesh.
    Mesh(Kn5Mesh),
    /// Type 3 — skinned mesh. Rare on cars outside driver models.
    SkinnedMesh(Kn5SkinnedMesh),
}

#[derive(Debug, Clone)]
pub struct Kn5Mesh {
    /// Order of the three flags is not confirmed — §12, open question 1.
    pub cast_shadows: bool,
    pub is_visible: bool,
    pub is_transparent: bool,
    pub vertices: Vec<Kn5Vertex>,
    /// Triangle list.
    pub indices: Vec<u16>,
    /// Index into [`Kn5Model::materials`], validated at parse time.
    pub material_id: u32,
    pub layer: u32,
    pub lod_in: f32,
    pub lod_out: f32,
    pub bounding_sphere_center: [f32; 3],
    pub bounding_sphere_radius: f32,
    /// Absent on skinned meshes, where the field does not exist — defaults to
    /// `true` there rather than hiding the geometry.
    pub is_renderable: bool,
}

#[derive(Debug, Clone)]
pub struct Kn5SkinnedMesh {
    pub mesh: Kn5Mesh,
    pub bones: Vec<Kn5Bone>,
    /// Parallel to `mesh.vertices`: four weights and four bone indices each.
    pub skin: Vec<Kn5SkinBinding>,
}

#[derive(Debug, Clone)]
pub struct Kn5Bone {
    pub name: String,
    pub inverse_bind_matrix: [f32; 16],
}

#[derive(Debug, Clone, Copy)]
pub struct Kn5SkinBinding {
    pub weights: [f32; 4],
    /// Stored as floats in the file, not integers (§3.4).
    pub bone_indices: [f32; 4],
}

#[derive(Debug, Clone, Copy)]
pub struct Kn5Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    /// V is flipped relative to the glTF convention — corrected at conversion
    /// time, not here (§4.4).
    pub uv: [f32; 2],
    pub tangent: [f32; 3],
}
