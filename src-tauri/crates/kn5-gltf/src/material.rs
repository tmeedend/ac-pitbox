//! KN5 material → glTF PBR (spec §6).
//!
//! An approximation, and an assumed one: AC's shaders are not
//! metallic/roughness, so there is no exact conversion. The rule followed here
//! is the spec's — a plausible diffuse result beats a wrong metallic one
//! (§6.2), so `metallicFactor` stays at zero until the semantics of `txMaps`
//! are actually documented rather than guessed.

use kn5::Kn5Material;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaMode {
    Opaque,
    /// Alpha testing: the fragment is either fully there or gone. Grilles,
    /// ajoured rims, mesh fabrics.
    Mask,
    /// Real transparency: glass.
    Blend,
}

impl AlphaMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Opaque => "OPAQUE",
            Self::Mask => "MASK",
            Self::Blend => "BLEND",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GltfMaterial {
    pub name: String,
    /// Source shader, kept for the debug panel and for the warning log.
    pub shader: String,
    pub base_color_texture: Option<String>,
    pub normal_texture: Option<String>,
    pub normal_scale: f32,
    pub emissive: [f32; 3],
    pub roughness: f32,
    pub metallic: f32,
    pub alpha_mode: AlphaMode,
    pub alpha_cutoff: f32,
    pub double_sided: bool,
    /// Base colour, alpha included. Only ever below 1 on blended materials
    /// that carry no texture of their own — see [`glass_opacity`].
    pub base_color: [f32; 4],
}

/// Shaders whose name alone says the surface is glass.
const GLASS_MARKERS: [&str; 3] = ["Glass", "Windscreen", "windscreen"];

/// Mode de transparence d'un matériau, et son seuil de découpe.
///
/// Extrait de [`convert`] parce que le pipeline de textures a besoin de la
/// même réponse : c'est lui qui décide de conserver ou non le canal alpha
/// d'une texture, et il ne peut le faire qu'en sachant si un matériau
/// s'en sert vraiment.
pub(crate) fn alpha_mode_of(material: &Kn5Material) -> (AlphaMode, f32) {
    let shader = material.shader.as_str();
    let is_glass = GLASS_MARKERS.iter().any(|marker| shader.contains(marker));
    if material.blend_mode == 1 || is_glass {
        (AlphaMode::Blend, 0.5)
    } else if material.alpha_tested {
        // Le drapeau décodé est le signal fiable ici, plus que `ksAlphaRef > 0`
        // — voir docs/kn5-format.md, §12 q2.
        (
            AlphaMode::Mask,
            material.property("ksAlphaRef").unwrap_or(0.5).clamp(0.0, 1.0),
        )
    } else {
        (AlphaMode::Opaque, 0.5)
    }
}

pub fn convert(material: &Kn5Material, diffuse_has_alpha: bool) -> GltfMaterial {
    let shader = material.shader.as_str();

    // §6.1: an approximation to calibrate by eye, not an exact conversion.
    // `ksSpecularEXP` is a Blinn-Phong exponent; the square root maps its very
    // non-linear range onto something roughness-shaped.
    let roughness = match material.property("ksSpecularEXP") {
        Some(exp) if exp > 0.0 => (1.0 - (exp / 250.0).sqrt()).clamp(0.05, 1.0),
        _ => 0.6,
    };
    // Tyres are the one surface where a generic guess is plainly wrong: rubber
    // is almost fully rough, and reading it off `ksSpecularEXP` gives a
    // plastic sheen (§6.3).
    let roughness = if shader.contains("ksTyres") { 0.9 } else { roughness };

    let emissive = match material.property("ksEmissive") {
        Some(value) if value > 0.0 => {
            let v = value.clamp(0.0, 1.0);
            [v, v, v]
        }
        _ => [0.0, 0.0, 0.0],
    };

    let is_glass = GLASS_MARKERS.iter().any(|marker| shader.contains(marker));
    let (alpha_mode, alpha_cutoff) = if material.blend_mode == 1 || is_glass {
        (AlphaMode::Blend, 0.5)
    } else if material.alpha_tested {
        // The decoded `alpha_tested` byte is the reliable signal here, more so
        // than `ksAlphaRef > 0` — see docs/kn5-format.md, §12 q2.
        (
            AlphaMode::Mask,
            material.property("ksAlphaRef").unwrap_or(0.5).clamp(0.0, 1.0),
        )
    } else {
        (AlphaMode::Opaque, 0.5)
    };

    if !shader.starts_with("ks") {
        // Never a failure (§6.3), but worth collecting: the list of shaders met
        // in the wild is what drives the next round of material work.
        log::warn!(
            "kn5-gltf: unknown shader `{shader}` on material `{}`, using defaults",
            material.name
        );
    }

    // Un matériau en fondu tire sa transparence de deux endroits possibles, et
    // il faut savoir lequel — sans se fier au nom, qui est dans la langue du
    // moddeur (le verre de l'Abarth s'appelle `CAR_Vetro`).
    //
    // - Sa texture porte un alpha utile : décalcomanie, flou de jante,
    //   couture. C'est l'alpha qui découpe, on n'y touche pas.
    // - Sa texture n'en porte pas, ou il n'y a pas de texture : la
    //   transparence vient alors du shader, comme pour toutes les vitres d'AC.
    //   C'est là, et seulement là, qu'on l'approxime depuis `ksDiffuse`.
    //
    // Appliquer l'opacité à tout matériau en fondu rendait translucides des
    // pièces qui ne devaient pas l'être — bug remonté sur `abarth500`.
    let base_color = match alpha_mode {
        AlphaMode::Blend if !diffuse_has_alpha => [1.0, 1.0, 1.0, glass_opacity(material)],
        _ => [1.0, 1.0, 1.0, 1.0],
    };

    GltfMaterial {
        name: material.name.clone(),
        shader: material.shader.clone(),
        base_color_texture: material
            .texture_for("txDiffuse")
            .filter(|t| !t.is_empty())
            .map(str::to_string),
        normal_texture: material
            .texture_for("txNormal")
            .filter(|t| !t.is_empty())
            .map(str::to_string),
        normal_scale: material.property("normalMult").filter(|v| *v > 0.0).unwrap_or(1.0),
        emissive,
        roughness,
        // Held at zero on purpose: `txMaps` channel semantics are still
        // undocumented (§6.2, §12 q3).
        metallic: 0.0,
        alpha_mode,
        // Glass rendered double-sided shows the inside of the far pane through
        // the near one; §6.1 asks for single-sided there specifically.
        double_sided: false,
        alpha_cutoff,
        base_color,
    }
}

/// Opacity of a blended material.
///
/// **A calibration, not a conversion.** `ksDiffuse` scales how much diffuse
/// light a surface returns, and AC's glass materials set it low precisely
/// because most of what reaches the eye passes through — 0.1 on interior
/// glass, 0.3 on exterior, 0.45 on a windscreen, on the reference car. Reusing
/// it as opacity reproduces that ordering, which is what the eye reads as
/// glass. The clamp keeps a missing or extreme value from producing either an
/// invisible pane or an opaque one.
fn glass_opacity(material: &Kn5Material) -> f32 {
    material.property("ksDiffuse").unwrap_or(0.3).clamp(0.08, 0.6)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kn5::{Kn5MaterialProperty, Kn5Sampler};

    fn material(shader: &str, blend_mode: u8, alpha_tested: bool, properties: &[(&str, f32)]) -> Kn5Material {
        Kn5Material {
            name: "m".to_string(),
            shader: shader.to_string(),
            blend_mode,
            alpha_tested,
            reserved: 0,
            properties: properties
                .iter()
                .map(|(name, value)| Kn5MaterialProperty {
                    name: name.to_string(),
                    value: *value,
                    extra: [0.0; 9],
                })
                .collect(),
            samplers: vec![Kn5Sampler {
                name: "txDiffuse".to_string(),
                slot: 0,
                texture: "body.dds".to_string(),
            }],
        }
    }

    // Rule: the alpha-tested byte drives MASK. Ignoring it renders grilles and
    // ajoured rims as solid panels (§10).
    #[test]
    fn alpha_tested_material_becomes_a_mask() {
        let converted = convert(&material("ksPerPixelAT", 0, true, &[("ksAlphaRef", 0.3)]), false);
        assert_eq!(converted.alpha_mode, AlphaMode::Mask, "alpha tested maps to MASK");
        assert!(
            (converted.alpha_cutoff - 0.3).abs() < 1e-6,
            "cutoff taken from ksAlphaRef"
        );
    }

    // Rule: glass blends, whether it says so through its blend mode or only
    // through its shader name.
    #[test]
    fn glass_blends_either_way() {
        assert_eq!(
            convert(&material("ksPerPixelReflection", 1, false, &[]), false).alpha_mode,
            AlphaMode::Blend,
            "blend mode 1 is enough"
        );
        assert_eq!(
            convert(&material("ksWindscreen", 0, false, &[]), false).alpha_mode,
            AlphaMode::Blend,
            "shader name is enough"
        );
    }

    // Rule: a high specular exponent means a smooth surface, so roughness must
    // move the opposite way. A sign error here makes every car look chalky.
    #[test]
    fn roughness_decreases_as_specular_exponent_rises() {
        let dull = convert(&material("ksPerPixel", 0, false, &[("ksSpecularEXP", 5.0)]), false).roughness;
        let shiny = convert(&material("ksPerPixel", 0, false, &[("ksSpecularEXP", 200.0)]), false).roughness;
        assert!(shiny < dull, "shinier material is less rough: {shiny} < {dull}");
        assert_eq!(
            convert(&material("ksTyres", 0, false, &[("ksSpecularEXP", 200.0)]), false).roughness,
            0.9,
            "rubber overrides the formula"
        );
    }

    // Règle : un matériau en fondu dont la texture porte un alpha exploitable
    // garde son opacité pleine — c'est l'alpha qui découpe. Sans texture
    // utile, l'opacité vient de `ksDiffuse`, comme pour le verre.
    //
    // Bug réel remonté sur `abarth500` : appliquer l'opacité à tout matériau
    // en fondu rendait translucides le logo d'airbag, les coutures et le flou
    // de jante. Et le verre y est nommé `CAR_Vetro`, donc aucun critère fondé
    // sur le nom n'aurait pu trancher.
    #[test]
    fn blended_material_trusts_its_texture_alpha_when_there_is_one() {
        let decal = material("ksPerPixelNM", 1, false, &[("ksDiffuse", 0.3)]);
        assert_eq!(
            convert(&decal, true).base_color[3],
            1.0,
            "la texture porte la découpe, on n'y touche pas"
        );
        assert!(
            convert(&decal, false).base_color[3] < 1.0,
            "sans alpha exploitable, la transparence vient du shader"
        );
    }

    // Rule: metallic stays at zero until §12 q3 is answered — a wrong
    // metallic surface looks far worse than a merely diffuse one (§6.2).
    #[test]
    fn metallic_stays_neutral() {
        assert_eq!(
            convert(&material("ksPerPixelMultiMap", 0, false, &[("ksSpecular", 1.0)]), false).metallic,
            0.0,
            "no metallic guess before the txMaps semantics are documented"
        );
    }
}
