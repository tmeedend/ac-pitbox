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

/// Ce que le pipeline de textures apprend d'un matériau et que la conversion
/// ne peut pas deviner seule.
#[derive(Debug, Clone, Copy, Default)]
pub struct MaterialTextures {
    /// La texture diffuse porte-t-elle un alpha qu'un matériau exploite ?
    pub diffuse_has_alpha: bool,
    /// Couleur moyenne de la carte de détail, quand le matériau en a une.
    pub detail_average: Option<[f32; 3]>,
}

pub fn convert(material: &Kn5Material, textures: MaterialTextures) -> GltfMaterial {
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

    let (alpha_mode, alpha_cutoff) = alpha_mode_of(material);

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
    let base_color_texture = base_color_map(material);
    let texture_carries_alpha = base_color_texture.is_some() && textures.diffuse_has_alpha;
    let mut base_color = match alpha_mode {
        AlphaMode::Blend if !texture_carries_alpha => [1.0, 1.0, 1.0, glass_opacity(material)],
        _ => [1.0, 1.0, 1.0, 1.0],
    };

    // Teinte apportée par la carte de détail (§6.2, approximation assumée).
    //
    // Beaucoup de peintures AC ont une diffuse en **niveaux de gris** et
    // tiennent leur couleur du `txDetail` du skin, que le shader
    // `ksPerPixelMultiMap` multiplie par-dessus avec un fort facteur de
    // répétition (`detailUVMultiplier = 25` sur la Supra). glTF ne sait pas
    // multiplier deux textures de couleur, et le motif est de toute façon trop
    // fin pour compter à la résolution d'un aperçu : ce qu'il en reste à l'œil
    // est une **teinte**. On applique donc la couleur moyenne de la carte,
    // ramenée à luminance constante pour ne teinter que la nuance — une carte
    // neutre ne change alors rien, et aucune voiture ne s'assombrit.
    //
    // Reste une approximation : `ks_toyota_supra_mkiv` / `dark_green_pearl_met`
    // ressort en vert clair, pas en vert foncé nacré. Reproduire vraiment la
    // peinture demande le pipeline multi-map, encore non documenté (§12 q3).
    if let Some(detail) = textures
        .detail_average
        .filter(|_| material.property("useDetail").unwrap_or(0.0) > 0.0)
    {
        if let Some(tint) = detail_tint(detail) {
            for (channel, factor) in base_color.iter_mut().zip(tint.iter()) {
                *channel *= factor;
            }
        }
    }

    GltfMaterial {
        name: material.name.clone(),
        shader: material.shader.clone(),
        base_color_texture,
        normal_texture: normal_map(material),
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

/// Amplification de la teinte tirée de la carte de détail.
///
/// **Calibré à l'œil, et c'est assumé.** La carte de détail d'un skin AC est
/// très peu saturée — celle du vert de `ks_toyota_supra_mkiv` est un vert
/// d'eau — alors que la voiture rendue par le jeu est franchement verte. Le
/// shader d'AC amplifie donc l'écart d'une façon qui n'est pas documentée
/// (§12 q3, toujours ouverte). Sans ce facteur, la teinte est juste mais si
/// pâle qu'elle se lit comme du blanc.
const DETAIL_TINT_BOOST: f32 = 3.0;

/// Teinte à appliquer, à partir de la couleur moyenne d'une carte de détail.
///
/// Le calcul se fait en **linéaire**, parce que `baseColorFactor` est linéaire
/// et que faire le rapport entre canaux en sRGB écrase les écarts. La teinte
/// est ramenée à luminance constante : une carte neutre ne change alors rien
/// et aucune voiture ne s'assombrit.
fn detail_tint(average_srgb: [f32; 3]) -> Option<[f32; 3]> {
    let linear = average_srgb.map(|c| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    });
    let luminance = 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
    if luminance <= 0.02 {
        return None;
    }
    Some(linear.map(|c| (1.0 + (c / luminance - 1.0) * DETAIL_TINT_BOOST).clamp(0.0, 1.0)))
}

/// Texture de couleur d'un matériau, **sauf sur un pare-brise**.
///
/// `ksWindscreen` n'utilise pas sa `txDiffuse` comme une couleur : c'est une
/// carte de **rayures et de poussière**, qu'AC ne mélange qu'à proportion de
/// la saleté du pare-brise — nulle sur une voiture propre. Posée telle quelle,
/// elle donne un vitrage constellé de taches. Même mécanisme que la carte de
/// dégâts sur la carrosserie (voir [`normal_map`]), et même remède.
///
/// Vérifié sur trois voitures : `ks_mazda_mx5_cup` et `abarth500`
/// (`INTERNAL_glass.dds`, une texture de rayures grises), `ks_toyota_supra_mkiv`
/// (`Interior_windscreen_diff.dds`). Sans texture, le matériau retombe sur
/// l'opacité tirée de `ksDiffuse`, qui est le bon comportement pour du verre.
fn base_color_map(material: &Kn5Material) -> Option<String> {
    if material.shader.contains("ksWindscreen") {
        return None;
    }
    material
        .texture_for("txDiffuse")
        .filter(|t| !t.is_empty())
        .map(str::to_string)
}

/// Normal map d'un matériau, **sauf quand c'en est une de dégâts**.
///
/// Les shaders de la famille `*_damage*` réservent `txNormal` à la déformation
/// des tôles, qu'AC ne mélange qu'à proportion des dégâts subis — nulle sur une
/// voiture intacte. Appliquée à pleine intensité, elle donne une carrosserie
/// définitivement froissée : c'est exactement le défaut remonté par
/// l'utilisateur après essai réel.
///
/// Vérifié sur quatre voitures, sans exception : `ks_toyota_supra_mkiv`
/// (`exterior_damage_NM.dds`), `ks_mazda_mx5_cup` (`Damage_NM.dds`),
/// `abarth500` (`NORMAL MAP DAMAGE_TEMP.dds`), `ks_ford_gt40` (`damageNM.dds`).
fn normal_map(material: &Kn5Material) -> Option<String> {
    if material.shader.to_ascii_lowercase().contains("damage") {
        return None;
    }
    material
        .texture_for("txNormal")
        .filter(|t| !t.is_empty())
        .map(str::to_string)
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
        let converted = convert(
            &material("ksPerPixelAT", 0, true, &[("ksAlphaRef", 0.3)]),
            MaterialTextures::default(),
        );
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
            convert(
                &material("ksPerPixelReflection", 1, false, &[]),
                MaterialTextures::default()
            )
            .alpha_mode,
            AlphaMode::Blend,
            "blend mode 1 is enough"
        );
        assert_eq!(
            convert(&material("ksWindscreen", 0, false, &[]), MaterialTextures::default()).alpha_mode,
            AlphaMode::Blend,
            "shader name is enough"
        );
    }

    // Rule: a high specular exponent means a smooth surface, so roughness must
    // move the opposite way. A sign error here makes every car look chalky.
    #[test]
    fn roughness_decreases_as_specular_exponent_rises() {
        let dull = convert(
            &material("ksPerPixel", 0, false, &[("ksSpecularEXP", 5.0)]),
            MaterialTextures::default(),
        )
        .roughness;
        let shiny = convert(
            &material("ksPerPixel", 0, false, &[("ksSpecularEXP", 200.0)]),
            MaterialTextures::default(),
        )
        .roughness;
        assert!(shiny < dull, "shinier material is less rough: {shiny} < {dull}");
        assert_eq!(
            convert(
                &material("ksTyres", 0, false, &[("ksSpecularEXP", 200.0)]),
                MaterialTextures::default()
            )
            .roughness,
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
            convert(
                &decal,
                MaterialTextures {
                    diffuse_has_alpha: true,
                    ..Default::default()
                }
            )
            .base_color[3],
            1.0,
            "la texture porte la découpe, on n'y touche pas"
        );
        assert!(
            convert(&decal, MaterialTextures::default()).base_color[3] < 1.0,
            "sans alpha exploitable, la transparence vient du shader"
        );
    }

    // Règle : la normal map d'un shader de dégâts n'est pas exportée. Bug réel
    // remonté par l'utilisateur : la carrosserie paraissait cabossée en
    // permanence, parce qu'AC ne mélange cette carte qu'à proportion des dégâts.
    #[test]
    fn damage_shaders_do_not_export_their_normal_map() {
        let mut damaged = material("ksPerPixelMultiMap_damage_dirt", 0, false, &[]);
        damaged.samplers.push(kn5::Kn5Sampler {
            name: "txNormal".to_string(),
            slot: 1,
            texture: "damage_NM.dds".to_string(),
        });
        assert_eq!(
            convert(&damaged, MaterialTextures::default()).normal_texture,
            None,
            "une carte de dégâts ne doit pas déformer une voiture intacte"
        );

        let mut plain = material("ksPerPixelNM", 0, false, &[]);
        plain.samplers.push(kn5::Kn5Sampler {
            name: "txNormal".to_string(),
            slot: 1,
            texture: "body_NM.dds".to_string(),
        });
        assert_eq!(
            convert(&plain, MaterialTextures::default()).normal_texture.as_deref(),
            Some("body_NM.dds"),
            "une vraie normal map reste exportée"
        );
    }

    // Règle : la carte de détail ne fait que teinter, jamais assombrir. Sa
    // couleur moyenne est ramenée à luminance constante, donc une carte neutre
    // ne change rien.
    #[test]
    fn detail_map_tints_without_darkening() {
        let painted = material("ksPerPixelMultiMap", 0, false, &[("useDetail", 1.0)]);
        let neutral = convert(
            &painted,
            MaterialTextures {
                detail_average: Some([0.5, 0.5, 0.5]),
                ..Default::default()
            },
        );
        for channel in 0..3 {
            assert!(
                (neutral.base_color[channel] - 1.0).abs() < 1e-5,
                "une carte de détail neutre laisse la couleur intacte"
            );
        }

        // Valeurs réelles mesurées sur `metal_detail.dds` du skin
        // `01_dark_green_pearl_met` : une carte de paillettes très peu saturée.
        let green = convert(
            &painted,
            MaterialTextures {
                detail_average: Some([0.84, 0.91, 0.86]),
                ..Default::default()
            },
        );
        assert!(
            green.base_color[1] > green.base_color[0] && green.base_color[1] > green.base_color[2],
            "la nuance verte ressort, et domine"
        );
        assert!(
            green.base_color.iter().take(3).all(|c| *c > 0.3),
            "sur une carte réelle, aucun canal ne s'effondre"
        );
        assert!(
            green.base_color[1] >= 1.0,
            "le canal dominant reste à pleine intensité : la teinte n'assombrit pas"
        );
    }

    // Règle : sans `useDetail`, la carte de détail est ignorée.
    #[test]
    fn detail_map_ignored_when_the_material_does_not_use_it() {
        let plain = material("ksPerPixelMultiMap", 0, false, &[]);
        let converted = convert(
            &plain,
            MaterialTextures {
                detail_average: Some([0.2, 0.9, 0.2]),
                ..Default::default()
            },
        );
        assert_eq!(
            converted.base_color,
            [1.0, 1.0, 1.0, 1.0],
            "détail non déclaré, non appliqué"
        );
    }

    // Règle : un pare-brise n'emprunte pas sa couleur à sa texture. Bug réel
    // remonté par l'utilisateur : le vitrage paraissait constellé de taches,
    // parce que `ksWindscreen` réserve sa `txDiffuse` aux rayures et à la
    // poussière, qu'AC ne mélange qu'à proportion de la saleté.
    #[test]
    fn windscreen_does_not_use_its_dirt_map_as_colour() {
        let windscreen = material("ksWindscreen", 1, false, &[("ksDiffuse", 0.45)]);
        let converted = convert(&windscreen, MaterialTextures::default());
        assert_eq!(
            converted.base_color_texture, None,
            "la carte de rayures ne sert pas de couleur"
        );
        assert!(
            converted.base_color[3] < 1.0,
            "sans texture, l'opacité vient de ksDiffuse — du verre, pas une vitre pleine"
        );

        let plain_glass = material("ksPerPixelReflection", 1, false, &[]);
        assert!(
            convert(&plain_glass, MaterialTextures::default())
                .base_color_texture
                .is_some(),
            "les autres matériaux gardent leur texture"
        );
    }

    // Rule: metallic stays at zero until §12 q3 is answered — a wrong
    // metallic surface looks far worse than a merely diffuse one (§6.2).
    #[test]
    fn metallic_stays_neutral() {
        assert_eq!(
            convert(
                &material("ksPerPixelMultiMap", 0, false, &[("ksSpecular", 1.0)]),
                MaterialTextures::default()
            )
            .metallic,
            0.0,
            "no metallic guess before the txMaps semantics are documented"
        );
    }
}
