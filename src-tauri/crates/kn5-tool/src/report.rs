//! Statistics gathered over one or many models.
//!
//! The same accumulator serves `inspect` (one file) and `scan` (a whole
//! `content/cars`). Scanning a real library is how the open questions of spec
//! §12 get answered — the distribution of `blend_mode` values or of mesh flag
//! combinations over two hundred cars says more than any single file.

use std::collections::BTreeMap;

use kn5::{ImageFormat, Kn5Model, Kn5NodeKind};

#[derive(Default)]
pub struct Stats {
    pub models: usize,
    pub versions: BTreeMap<u32, usize>,

    pub textures: usize,
    pub texture_bytes: u64,
    pub texture_formats: BTreeMap<&'static str, usize>,
    /// Type-0 entries: declared but holding nothing (§3.2).
    pub texture_placeholders: usize,
    /// Blobs whose real container contradicts the extension in their name.
    /// The number that justifies sniffing rather than trusting the filename.
    pub texture_extension_mismatch: usize,

    pub materials: usize,
    pub shaders: BTreeMap<String, usize>,
    /// `(blend_mode, alpha_tested)` — the two bytes of the material's `i16`.
    pub blend_modes: BTreeMap<(u8, bool), usize>,
    /// Shaders used by meshes whose third flag byte is set. If that byte
    /// really is `is_transparent` (§12, q1) this table holds glass and decals
    /// and nothing else.
    pub transparent_mesh_shaders: BTreeMap<String, usize>,
    pub property_names: BTreeMap<String, usize>,
    pub sampler_names: BTreeMap<String, usize>,
    /// Non-zero values of the 36 trailing bytes of a property (§12, q5): if
    /// they are always zero we can stop wondering what they hold.
    pub properties_with_extra: usize,

    pub nodes: usize,
    pub dummies: usize,
    pub meshes: usize,
    pub skinned_meshes: usize,
    pub vertices: usize,
    pub triangles: usize,
    /// `(cast_shadows, is_visible, is_transparent)` as parsed — feeds §12, q1.
    pub mesh_flags: BTreeMap<(bool, bool, bool), usize>,
    pub not_renderable: usize,

    /// Bounding box over raw vertex positions, node transforms *not* applied.
    /// A rough scale check only: flattening the hierarchy is Lot 3.
    pub bounds: Option<([f32; 3], [f32; 3])>,
}

impl Stats {
    pub fn add(&mut self, model: &Kn5Model) {
        self.models += 1;
        *self.versions.entry(model.version).or_default() += 1;

        for texture in &model.textures {
            if !texture.has_data() {
                self.texture_placeholders += 1;
                continue;
            }
            self.textures += 1;
            self.texture_bytes += texture.data.len() as u64;
            let format = ImageFormat::sniff(&texture.data);
            *self.texture_formats.entry(format.as_str()).or_default() += 1;
            let extension = texture
                .name
                .rsplit_once('.')
                .map(|(_, ext)| ext.to_ascii_lowercase())
                .unwrap_or_default();
            let announced = match extension.as_str() {
                "dds" => ImageFormat::Dds,
                "png" => ImageFormat::Png,
                "jpg" | "jpeg" => ImageFormat::Jpeg,
                _ => ImageFormat::Unknown,
            };
            if announced != format {
                self.texture_extension_mismatch += 1;
            }
        }

        for material in &model.materials {
            self.materials += 1;
            *self.shaders.entry(material.shader.clone()).or_default() += 1;
            *self
                .blend_modes
                .entry((material.blend_mode, material.alpha_tested))
                .or_default() += 1;
            for property in &material.properties {
                *self.property_names.entry(property.name.clone()).or_default() += 1;
                if property.extra.iter().any(|v| *v != 0.0) {
                    self.properties_with_extra += 1;
                }
            }
            for sampler in &material.samplers {
                *self.sampler_names.entry(sampler.name.clone()).or_default() += 1;
            }
        }

        model.visit_nodes(&mut |node| {
            self.nodes += 1;
            match &node.kind {
                Kn5NodeKind::Dummy { .. } => self.dummies += 1,
                Kn5NodeKind::Mesh(_) => self.meshes += 1,
                Kn5NodeKind::SkinnedMesh(_) => self.skinned_meshes += 1,
            }
            if let Some(mesh) = node.mesh() {
                self.vertices += mesh.vertices.len();
                self.triangles += mesh.indices.len() / 3;
                *self
                    .mesh_flags
                    .entry((mesh.cast_shadows, mesh.is_visible, mesh.is_transparent))
                    .or_default() += 1;
                if !mesh.is_renderable {
                    self.not_renderable += 1;
                }
                if mesh.is_transparent {
                    if let Some(material) = model.materials.get(mesh.material_id as usize) {
                        *self
                            .transparent_mesh_shaders
                            .entry(material.shader.clone())
                            .or_default() += 1;
                    }
                }
                for vertex in &mesh.vertices {
                    self.extend_bounds(vertex.position);
                }
            }
        });
    }

    fn extend_bounds(&mut self, p: [f32; 3]) {
        match &mut self.bounds {
            None => self.bounds = Some((p, p)),
            Some((min, max)) => {
                for axis in 0..3 {
                    min[axis] = min[axis].min(p[axis]);
                    max[axis] = max[axis].max(p[axis]);
                }
            }
        }
    }

    /// Longest side of the bounding box, in metres. A car should land between
    /// 1 and 8 m — the cheapest detector of a scale or coordinate mistake
    /// (spec §11).
    pub fn size(&self) -> Option<[f32; 3]> {
        self.bounds
            .map(|(min, max)| [max[0] - min[0], max[1] - min[1], max[2] - min[2]])
    }
}

/// Sorts a `name -> count` map by descending count, for readable output.
pub fn by_count(map: &BTreeMap<String, usize>) -> Vec<(&str, usize)> {
    let mut entries: Vec<(&str, usize)> = map.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    entries
}

pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
