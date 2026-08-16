//! Conversion of a parsed [`kn5::Kn5Model`] into a glTF preview.
//!
//! Split from the `kn5` crate because this half does touch the filesystem
//! (skin overrides live on disk next to the model) and pulls in image codecs,
//! neither of which belong in a parser (spec §5.1).

mod geometry;
mod glb;
mod locate;
mod material;
mod texture;

use std::path::Path;

use kn5::Kn5Model;

pub use geometry::{node_world_centers, winding_consistency, FlatMesh, GeometryOptions, GeometryStats};
pub use locate::{resolve_model, resolve_skin, ModelSource, ResolvedModel};
pub use material::{AlphaMode, GltfMaterial};
pub use texture::{
    prepare_textures, PreparedTexture, TextureOptions, TextureOrigin, TextureRole, TextureSet, TextureWarning,
};

#[derive(Debug, Clone, Default)]
pub struct ConvertOptions {
    pub geometry: GeometryOptions,
    pub textures: TextureOptions,
}

/// Everything the conversion produced, alongside the numbers the caller needs
/// to report — the Tauri command answers with these (§7.1) and `kn5-tool`
/// prints them.
pub struct Conversion {
    pub glb: Vec<u8>,
    pub geometry: GeometryStats,
    pub triangle_count: u32,
    pub material_count: u32,
    pub texture_count: u32,
    pub texture_warnings: Vec<TextureWarning>,
}

/// Stage the conversion has reached, reported as it goes.
///
/// Exists so the application can keep a skeleton alive during the second or
/// two a first conversion takes (§7.3). Transcoding is by far the longest of
/// the three, which is why it is announced before it starts and not after.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertStage {
    Geometry,
    Textures,
    Writing,
}

impl ConvertStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Geometry => "geometry",
            Self::Textures => "textures",
            Self::Writing => "writing",
        }
    }
}

/// Full pipeline: filter and flatten the node tree, transcode the textures the
/// surviving materials need, map the materials, write the GLB.
pub fn convert(
    model: &Kn5Model,
    skin_dir: Option<&Path>,
    options: &ConvertOptions,
    progress: &dyn Fn(ConvertStage),
) -> Result<Conversion, String> {
    // Safety net for the handedness decision documented in `geometry.rs`: the
    // reference library agrees at 100 %, so anything meaningfully below that
    // means a file whose convention we have not met before, and which would
    // otherwise render inside out without a word.
    let (agreeing, total) = geometry::winding_consistency(model);
    if total > 0 && (agreeing as f64) < 0.9 * total as f64 {
        log::warn!(
            "kn5-gltf: only {agreeing}/{total} triangles wind the expected way — this model may render inside out"
        );
    }

    progress(ConvertStage::Geometry);
    let (meshes, geometry) = geometry::flatten(model, &options.geometry);
    let materials: Vec<GltfMaterial> = model.materials.iter().map(material::convert).collect();

    progress(ConvertStage::Textures);
    let textures = prepare_textures(model, skin_dir, &options.textures);

    progress(ConvertStage::Writing);
    let triangle_count = meshes.iter().map(|m| m.indices.len() / 3).sum::<usize>() as u32;
    let glb = glb::write_glb(&meshes, &materials, &textures)?;

    Ok(Conversion {
        glb,
        geometry,
        triangle_count,
        material_count: materials.len() as u32,
        texture_count: textures.textures.len() as u32,
        texture_warnings: textures.warnings,
    })
}
