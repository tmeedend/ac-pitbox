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
    /// Carte métallique-rugosité dérivée de `txMaps`, quand le matériau en a
    /// une exploitable (voir [`crate::roughness`]).
    pub roughness_texture: Option<String>,
    pub normal_scale: f32,
    pub emissive: [f32; 3],
    /// Facteur de rugosité. glTF le **multiplie** par la carte ci-dessus, donc
    /// il vaut 1 dès qu'une carte est là : c'est elle qui décide.
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
#[derive(Debug, Clone, Default)]
pub struct MaterialTextures {
    /// La texture diffuse porte-t-elle un alpha qu'un matériau exploite ?
    pub diffuse_has_alpha: bool,
    /// Variante peinte de la texture diffuse, quand la carte de détail du
    /// matériau porte une couleur de peinture (voir [`crate::paint`]).
    pub painted_diffuse: Option<String>,
    /// Carte métallique-rugosité tirée de `txMaps` (voir [`crate::roughness`]).
    pub roughness_texture: Option<String>,
}

pub fn convert(material: &Kn5Material, textures: MaterialTextures) -> GltfMaterial {
    let shader = material.shader.as_str();

    // §6.1: an approximation to calibrate by eye, not an exact conversion.
    // `ksSpecularEXP` is a Blinn-Phong exponent; the square root maps its very
    // non-linear range onto something roughness-shaped.
    //
    // Ce n'est qu'un **repli** : quand le matériau a une carte de surface
    // exploitable, c'est elle qui dit la rugosité, pixel par pixel, et
    // `ksSpecularEXP` ne sépare de toute façon pas les finitions (le chrome et
    // le cuir de la MX-5 sont tous deux à 100 — voir [`crate::roughness`]).
    let roughness = match material.property("ksSpecularEXP") {
        Some(exp) if exp > 0.0 => (1.0 - (exp / 250.0).sqrt()).clamp(0.05, 1.0),
        _ => 0.6,
    };
    // Tyres are the one surface where a generic guess is plainly wrong: rubber
    // is almost fully rough, and reading it off `ksSpecularEXP` gives a
    // plastic sheen (§6.3).
    let roughness = if shader.contains("ksTyres") { 0.9 } else { roughness };
    // Glass is the other one, in the opposite direction. `ksWindscreen`
    // announces `ksSpecular = 0` and `ksSpecularEXP = 10` — its shader does not
    // use them the way the others do — which the formula above turns into a
    // roughness of 0.8, i.e. **frosted** glass. That is the "dirty windscreen"
    // the user reported once the white veil was gone. A pane is smooth,
    // whatever its material says; the exterior glass of the same car already
    // lands here on its own (`ksSpecularEXP = 500`).
    let roughness = if GLASS_MARKERS.iter().any(|marker| shader.contains(marker)) {
        roughness.min(0.08)
    } else {
        roughness
    };
    // glTF multiplie facteur et carte : avec une carte, le facteur doit valoir
    // 1, sinon il assombrirait une rugosité déjà juste.
    let roughness = if textures.roughness_texture.is_some() {
        1.0
    } else {
        roughness
    };

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
    // La peinture du skin ne passe pas par `baseColorFactor` mais par une
    // variante de la texture diffuse : elle est masquée pixel par pixel par
    // l'alpha de cette texture, et un facteur global peindrait les
    // décalcomanies avec la carrosserie (voir [`crate::paint`]).
    let base_color_texture = textures.painted_diffuse.clone().or_else(|| base_color_map(material));
    let texture_carries_alpha = base_color_texture.is_some() && textures.diffuse_has_alpha;
    let base_color = match alpha_mode {
        AlphaMode::Blend if !texture_carries_alpha => [1.0, 1.0, 1.0, glass_opacity(material)],
        _ => [1.0, 1.0, 1.0, 1.0],
    };

    GltfMaterial {
        name: material.name.clone(),
        shader: material.shader.clone(),
        base_color_texture,
        normal_texture: normal_map(material),
        roughness_texture: textures.roughness_texture.clone(),
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
pub(crate) fn base_color_map(material: &Kn5Material) -> Option<String> {
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
///
/// **`ksWindscreen` is the exception, and reading it like the others was a
/// real bug.** Its `ksDiffuse` is not a per-car opacity but a constant of the
/// shader family: 0.45 on `ks_toyota_supra_mkiv`, `ks_mazda_mx5_cup`,
/// `ks_ford_gt40` and `abarth500` alike, 0.75 on `ks_ferrari_488_gt3`. Taken
/// as opacity it lays a **white pane at 45 % over the whole cabin** — the
/// haze the user reported through the Supra's windscreen. The material is the
/// reflection layer of the glass (`INT_Glass_REFLEX`, `INT_Vetro`,
/// `Windshield`), so the pane itself is clear and what should show is the
/// environment reflected in it.
fn glass_opacity(material: &Kn5Material) -> f32 {
    if material.shader.contains("ksWindscreen") {
        return WINDSCREEN_OPACITY;
    }
    material.property("ksDiffuse").unwrap_or(0.3).clamp(0.08, 0.6)
}

/// Ce qu'il reste d'une vitre propre : un voile, pas une teinte. Assez pour
/// que la vitre attrape un reflet du studio et ne disparaisse pas, trop peu
/// pour laver ce qu'il y a derrière.
const WINDSCREEN_OPACITY: f32 = 0.1;

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

    // Règle : la peinture du skin passe par la variante peinte de la livrée,
    // pas par `baseColorFactor`. Un facteur global peindrait aussi les
    // décalcomanies, et glTF le borne à 1 — il ne saurait pas éclaircir.
    #[test]
    fn paint_comes_from_the_painted_texture_not_the_base_colour() {
        let body = material("ksPerPixelMultiMap", 0, false, &[("useDetail", 1.0)]);
        let converted = convert(
            &body,
            MaterialTextures {
                painted_diffuse: Some("body.dds#paint-001026".to_string()),
                ..Default::default()
            },
        );
        assert_eq!(
            converted.base_color_texture.as_deref(),
            Some("body.dds#paint-001026"),
            "le matériau pointe sur la livrée peinte"
        );
        assert_eq!(
            converted.base_color,
            [1.0, 1.0, 1.0, 1.0],
            "et la couleur de base reste neutre"
        );

        let plain = convert(&body, MaterialTextures::default());
        assert_eq!(
            plain.base_color_texture.as_deref(),
            Some("body.dds"),
            "sans peinture, la livrée d'origine"
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

    // Règle : une vitre est lisse, quoi qu'annonce son `ksSpecularEXP`. Bug
    // réel : `ksWindscreen` déclare `ksSpecular = 0` et `ksSpecularEXP = 10`,
    // dont la formule générale tirait une rugosité de 0,8 — du verre dépoli,
    // vu comme un pare-brise sale.
    #[test]
    fn glass_is_smooth_whatever_its_exponent_says() {
        let windscreen = convert(
            &material("ksWindscreen", 1, false, &[("ksSpecularEXP", 10.0)]),
            MaterialTextures::default(),
        );
        assert!(
            windscreen.roughness <= 0.1,
            "une vitre reste lisse (obtenu {})",
            windscreen.roughness
        );

        let plastic = convert(
            &material("ksPerPixel", 0, false, &[("ksSpecularEXP", 10.0)]),
            MaterialTextures::default(),
        );
        assert!(
            plastic.roughness > 0.5,
            "et le même exposant sur du plastique reste mat"
        );
    }

    // Règle : sur un `ksWindscreen`, `ksDiffuse` n'est pas une opacité. Bug
    // réel remonté par l'utilisateur : un voile blanc sur tout l'habitacle de
    // `ks_toyota_supra_mkiv`. La valeur vaut 0,45 sur quatre voitures mesurées
    // et 0,75 sur une cinquième — c'est une constante de famille de shaders,
    // pas un réglage de vitre.
    #[test]
    fn a_windscreen_stays_clear_whatever_its_ksdiffuse_says() {
        let windscreen = convert(
            &material("ksWindscreen", 1, false, &[("ksDiffuse", 0.45)]),
            MaterialTextures::default(),
        );
        assert!(
            windscreen.base_color[3] <= 0.15,
            "une vitre propre laisse passer, elle ne lave pas l'habitacle (obtenu {})",
            windscreen.base_color[3]
        );

        // La règle ne déborde pas sur le reste du vitrage, où `ksDiffuse`
        // ordonne bien les épaisseurs.
        let side = convert(
            &material("ksPerPixelReflection", 1, false, &[("ksDiffuse", 0.45)]),
            MaterialTextures::default(),
        );
        assert!(
            side.base_color[3] > windscreen.base_color[3],
            "les autres vitres gardent leur opacité tirée de ksDiffuse"
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
