//! Conversion of a parsed [`kn5::Kn5Model`] into a glTF preview.
//!
//! Split from the `kn5` crate because this half does touch the filesystem
//! (skin overrides live on disk next to the model) and pulls in image codecs,
//! neither of which belong in a parser (spec §5.1).

mod driver;
mod extconfig;
mod geometry;
mod glb;
mod locate;
mod material;
mod paint;
mod roughness;
mod stats;
#[cfg(test)]
mod testutil;
mod texture;

use std::path::Path;

use kn5::Kn5Model;

pub use driver::{graft as graft_driver, DriverGraft, DriverStats};
pub use extconfig::{
    apply_ext_config, material_overrides, CspConfig, ExtConfigStats, MaterialOverrides, Replacement, SurfaceOverride,
};
pub use geometry::{node_world_centers, winding_consistency, FlatMesh, GeometryOptions, GeometryStats};
pub use locate::{resolve_model, resolve_skin, ModelSource, ResolvedModel};
pub use material::{AlphaMode, GltfMaterial, MaterialTextures};
pub use stats::{channel_stats, ChannelStats};
pub use texture::{
    alpha_stats, prepare_textures, FootprintAlpha, PreparedTexture, TextureOptions, TextureOrigin, TextureRole,
    TextureSet, TextureWarning,
};

#[derive(Debug, Clone, Default)]
pub struct ConvertOptions {
    pub geometry: GeometryOptions,
    pub textures: TextureOptions,
    /// Ce que la configuration CSP de la voiture dit de ses surfaces (voir
    /// [`material_overrides`]). Vide par défaut : une voiture sans config
    /// garde le traitement habituel, tiré du seul KN5.
    pub surfaces: MaterialOverrides,
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

/// Fraction of winding-consistent triangles below which a model is not worth
/// rendering (§4.5bis of the spec).
///
/// Measured, not guessed: every healthy car in the reference library sits
/// between 99.5 % and 100 % (`ferrari_599_gto`, `rss_gtm_lanzo_v8`,
/// `some1_acura_nsx_1992`… eight checked, none below 99.5). The two mods
/// reported broken by a user (`ms_citroen_berlingo_2003_vts`, a blinking blue
/// square; `gmp_w204_c63_c13`, a cloud of small blue triangles) both parse
/// with a perfectly valid KN5 magic — `kn5::parse` succeeds, `NotAKn5File`
/// never fires — yet sit at **exactly 50.0 %/50.1 %**: a coin flip between
/// "front" and "back", not a file using a convention we have not met before.
/// No file in the sample has ever landed between the two clusters. See
/// `docs/kn5-format.md` for the investigation.
pub const WINDING_SANITY_THRESHOLD: f64 = 0.9;

/// True when the model's geometry is coherent enough to be worth converting —
/// see [`WINDING_SANITY_THRESHOLD`]. Split out from `convert()` so a caller
/// that already has `winding_consistency`'s numbers (the app's preview
/// pipeline, §4.5bis) can bail out **before** paying for texture transcoding,
/// instead of converting a model no one will keep.
pub fn is_geometry_sane(agreeing: usize, total: usize) -> bool {
    total == 0 || (agreeing as f64) >= WINDING_SANITY_THRESHOLD * total as f64
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
    // otherwise render inside out without a word. `kn5-tool` calls `convert`
    // directly (never through the app's preview pipeline), so this warning is
    // its only signal — the app itself checks earlier, see
    // `is_geometry_sane`/§4.5bis.
    let (agreeing, total) = geometry::winding_consistency(model);
    if !is_geometry_sane(agreeing, total) {
        log::warn!(
            "kn5-gltf: only {agreeing}/{total} triangles wind the expected way — this model may render inside out"
        );
    }

    progress(ConvertStage::Geometry);
    let (meshes, geometry) = geometry::flatten(model, &options.geometry);

    progress(ConvertStage::Textures);
    let mut textures = prepare_textures(model, skin_dir, &options.textures);

    // La peinture du skin se cuit dans une variante de la livrée, et elle a
    // besoin de la couleur moyenne des cartes de détail — donc après leur
    // décodage, jamais avant (voir `paint`).
    let mut paint = paint::plan(model, &textures);
    texture::bake_paint(&mut textures, &mut paint, model, skin_dir, &options.textures);

    // Même temps, même raison : savoir si un `txMaps` est une vraie carte de
    // surface demande de connaître le rôle de chaque texture (§12 q3).
    let mut roughness = roughness::plan(model, &textures);
    texture::bake_roughness(&mut textures, &mut roughness, model, skin_dir, &options.textures);

    // Les matériaux sont convertis **après** les textures : savoir si la
    // texture diffuse porte un alpha exploitable change la façon de traiter
    // un matériau en fondu (verre contre décalcomanie).
    let surfaces = options.surfaces.resolve(model);
    let materials: Vec<GltfMaterial> = model
        .materials
        .iter()
        .enumerate()
        .map(|(index, m)| {
            material::convert(
                m,
                material::MaterialTextures {
                    diffuse_alpha_varies: m
                        .texture_for("txDiffuse")
                        .and_then(|name| textures.get(name))
                        .is_some_and(|t| t.alpha_varies),
                    // Mesure faite sur les UV du matériau, pas sur l'atlas :
                    // voir `texture::FootprintAlpha`.
                    diffuse_alpha_blank: textures.footprint_alpha.get(&index).is_some_and(|f| f.is_blank()),
                    painted_diffuse: paint.painted_diffuse(index),
                    roughness_texture: roughness.roughness_texture(index),
                    csp: surfaces.get(&index).copied(),
                },
            )
        })
        .collect();

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

#[cfg(test)]
mod tests {
    use super::*;

    // Rule: the threshold used to gate the app's preview pipeline (§4.5bis)
    // is the same one `convert()` warns on — a single constant, not two
    // literals that could drift apart.
    #[test]
    fn geometry_sanity_matches_the_measured_split() {
        assert!(is_geometry_sane(999, 1000), "99.9 % — every healthy car in the sample");
        assert!(is_geometry_sane(900, 1000), "exactly at the threshold");
        assert!(!is_geometry_sane(899, 1000), "just under the threshold");
        assert!(!is_geometry_sane(500, 1000), "50 % — the two broken mods in the sample");
        assert!(is_geometry_sane(0, 0), "no triangles at all: nothing to condemn");
    }
}
