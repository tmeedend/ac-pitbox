//! Allocation caps for untrusted files (spec §5.2, "sécurité du parser").
//!
//! Every count read from the file is checked against one of these *before* any
//! `Vec::with_capacity`. The values are deliberately generous — they are there
//! to stop an absurd `i32` from reserving gigabytes, not to enforce a quality
//! bar on mods. A legitimate LOD A car model sits two orders of magnitude
//! below each of them.

#[derive(Debug, Clone)]
pub struct Limits {
    /// Embedded textures declared in the texture section.
    pub max_textures: usize,
    /// Byte size of a single embedded texture blob.
    pub max_texture_bytes: usize,
    /// Materials declared in the material section.
    pub max_materials: usize,
    /// Scalar properties on a single material.
    pub max_material_properties: usize,
    /// Texture samplers on a single material.
    pub max_material_samplers: usize,
    /// Byte length of a single length-prefixed string.
    pub max_string_bytes: usize,
    /// Direct children of a single node.
    pub max_children: usize,
    /// Bones of a single skinned mesh.
    pub max_bones: usize,
    /// Vertices of a single mesh.
    pub max_vertices: usize,
    /// Indices of a single mesh.
    pub max_indices: usize,
    /// Nesting depth of the node tree — guards the recursion (stack overflow
    /// is not recoverable, so this cap is the only defence).
    pub max_depth: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_textures: 4_096,
            max_texture_bytes: 128 * 1024 * 1024,
            max_materials: 4_096,
            max_material_properties: 512,
            max_material_samplers: 64,
            max_string_bytes: 4_096,
            max_children: 65_536,
            max_bones: 1_024,
            max_vertices: 8_000_000,
            max_indices: 32_000_000,
            max_depth: 256,
        }
    }
}
