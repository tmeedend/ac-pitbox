//! Car paint: the colour AC keeps in a detail map, masked by the diffuse alpha.
//!
//! A car body in Assetto Corsa is almost never coloured by its diffuse texture.
//! The diffuse holds a **neutral grey** panel layout plus the decals, and the
//! paint comes from the tiny `txDetail` map the skin folder overrides — a flat
//! colour swatch on most skins. The shader combines them the way D3D's
//! `MODULATE2X` did, so that mid-grey is neutral:
//!
//! ```text
//! diffuse.rgb *= lerp(1, detail.rgb * 2, useDetail * (1 - diffuse.a))
//! ```
//!
//! The `1 - diffuse.a` term is the part that is easy to miss: **the diffuse
//! alpha is a paint mask**, not transparency. Alpha 0 means "this is bodywork,
//! paint it", alpha 255 means "this is a decal, leave it alone" — which is why
//! a racing number stays white on a navy car.
//!
//! Measured, and consistent on every skin checked (see docs/kn5-format.md,
//! écart n°5):
//!
//! | Skin | detail swatch | rendered car |
//! | --- | --- | --- |
//! | `ks_abarth500_assetto_corse` / `dark_blue` | (0, 16, 38) | navy |
//! | … / `red_yellow` | (239, 0, 0) | red |
//! | … / `white_grey` | (238, 238, 238) | white |
//! | `ks_toyota_supra_mkiv` / `01_dark_green_pearl_met` | (5, 105, 36) | dark green |
//! | … / `05_blue_pearl_met` | (35, 57, 161) | blue |
//!
//! Because the mask is per-pixel, the paint is baked into a **variant of the
//! diffuse texture** rather than into `baseColorFactor`. A factor could not
//! spare the decals, and glTF clamps it to 1, so it could never carry the
//! brightening half of a `MODULATE2X` (a white swatch asks for ×1.87).

use std::collections::BTreeMap;

use image::RgbaImage;
use kn5::{Kn5Material, Kn5Model};

use crate::material::{alpha_mode_of, base_color_map, AlphaMode};
use crate::texture::TextureSet;

/// The `MODULATE2X` factor: a mid-grey detail map is neutral.
///
/// Kunos' own "official" swatches sit at 148–156 out of 255 — just above the
/// 128 this convention makes neutral, which is what a stock body colour should
/// be. Nothing here is calibrated by eye.
pub(crate) const DETAIL_MODULATE: f32 = 2.0;

/// Below this deviation from neutral, baking is invisible and only costs a
/// second copy of a 2048² livery in the GLB.
const NEUTRAL_TOLERANCE: f32 = 0.05;

/// One diffuse texture, painted with one colour.
#[derive(Debug, Clone)]
pub(crate) struct Variant {
    /// Name the glTF will reference, derived from the source and the colour so
    /// that two materials asking for the same paint share one image.
    pub name: String,
    /// Texture the pixels come from.
    pub source: String,
    pub factor: [f32; 3],
}

/// Which material gets which painted texture, and what has to be baked for it.
#[derive(Debug, Default)]
pub(crate) struct Plan {
    /// Painted texture name per material index, `None` when the material keeps
    /// its diffuse as-is.
    per_material: Vec<Option<String>>,
    variants: BTreeMap<String, Variant>,
}

impl Plan {
    pub(crate) fn variants(&self) -> Vec<Variant> {
        self.variants.values().cloned().collect()
    }

    pub(crate) fn painted_diffuse(&self, material: usize) -> Option<String> {
        self.per_material.get(material).cloned().flatten()
    }

    /// Drops a variant that turned out to change nothing, so its materials fall
    /// back on the plain texture instead of carrying a duplicate of it.
    pub(crate) fn forget(&mut self, name: &str) {
        self.variants.remove(name);
        for slot in &mut self.per_material {
            if slot.as_deref() == Some(name) {
                *slot = None;
            }
        }
    }
}

/// Reads the whole model and decides what to paint.
pub(crate) fn plan(model: &Kn5Model, textures: &TextureSet) -> Plan {
    let mut plan = Plan::default();
    for material in &model.materials {
        let variant = variant_for(material, textures);
        plan.per_material.push(variant.as_ref().map(|v| v.name.clone()));
        if let Some(variant) = variant {
            // `or_insert` and not `insert`: several materials legitimately ask
            // for the same paint on the same livery, and they must share it.
            plan.variants.entry(variant.name.clone()).or_insert(variant);
        }
    }
    plan
}

/// The painted texture a material needs, when it needs one.
fn variant_for(material: &Kn5Material, textures: &TextureSet) -> Option<Variant> {
    // A blended or masked material reads its own alpha as a cut-out, and the
    // paint consumes that alpha. Bodywork is always opaque, so nothing of value
    // is lost by staying away from the others.
    if alpha_mode_of(material).0 != AlphaMode::Opaque {
        return None;
    }
    let diffuse = base_color_map(material)?;
    textures.get(&diffuse)?;
    let factor = factor(material, average(material, textures)?)?;
    Some(Variant {
        name: variant_name(&diffuse, factor),
        source: diffuse,
        factor,
    })
}

/// Average colour of the material's detail map, once prepared.
fn average(material: &Kn5Material, textures: &TextureSet) -> Option<[f32; 3]> {
    let detail = material.texture_for("txDetail").filter(|t| !t.is_empty())?;
    textures.get(detail).map(|t| t.average)
}

/// How much a detail map multiplies the diffuse, `None` when it is neutral.
///
/// `useDetail` is the blend amount the shader applies — 0 or 1 on every
/// material met so far, but honoured as a ratio since that is what it is.
pub(crate) fn factor(material: &Kn5Material, detail_average: [f32; 3]) -> Option<[f32; 3]> {
    let use_detail = material.property("useDetail").unwrap_or(0.0).clamp(0.0, 1.0);
    if use_detail <= 0.0 {
        return None;
    }
    let factor = detail_average.map(|c| 1.0 + (c * DETAIL_MODULATE - 1.0) * use_detail);
    factor
        .iter()
        .any(|c| (c - 1.0).abs() > NEUTRAL_TOLERANCE)
        .then_some(factor)
}

/// Stable name for a painted texture: source plus the swatch it was painted
/// with, so the same paint on the same livery is baked once.
pub(crate) fn variant_name(diffuse: &str, factor: [f32; 3]) -> String {
    let byte = |c: f32| (c / DETAIL_MODULATE * 255.0).round().clamp(0.0, 255.0) as u8;
    format!(
        "{diffuse}#paint-{:02x}{:02x}{:02x}",
        byte(factor[0]),
        byte(factor[1]),
        byte(factor[2])
    )
}

/// Paints a decoded diffuse texture, and reports whether anything was painted.
///
/// Returns `false` when the image is fully opaque: the mask then says "decal
/// everywhere", the result would be identical to the source, and the caller
/// drops the variant rather than shipping a copy of a livery for nothing.
pub(crate) fn apply(image: &mut RgbaImage, factor: [f32; 3]) -> bool {
    let mut painted = false;
    for pixel in image.pixels_mut() {
        let mask = 1.0 - pixel.0[3] as f32 / 255.0;
        if mask > 0.0 {
            painted = true;
            // `zip` stops after the three colour channels, leaving the alpha to
            // the line below.
            for (channel, factor) in pixel.0.iter_mut().zip(factor.iter()) {
                let scale = 1.0 + (factor - 1.0) * mask;
                *channel = (*channel as f32 * scale).round().clamp(0.0, 255.0) as u8;
            }
        }
        // The mask has done its job; kept, it would be read as transparency and
        // erase the bodywork (see `texture::strip_alpha`).
        pixel.0[3] = u8::MAX;
    }
    painted
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;
    use kn5::{Kn5MaterialProperty, Kn5Sampler};

    fn material(properties: &[(&str, f32)]) -> Kn5Material {
        Kn5Material {
            name: "body".to_string(),
            shader: "ksPerPixelMultiMap".to_string(),
            blend_mode: 0,
            alpha_tested: false,
            reserved: 0,
            properties: properties
                .iter()
                .map(|(name, value)| Kn5MaterialProperty {
                    name: name.to_string(),
                    value: *value,
                    extra: [0.0; 9],
                })
                .collect(),
            samplers: vec![
                Kn5Sampler {
                    name: "txDiffuse".to_string(),
                    slot: 0,
                    texture: "body.dds".to_string(),
                },
                Kn5Sampler {
                    name: "txDetail".to_string(),
                    slot: 3,
                    texture: "detail.dds".to_string(),
                },
            ],
        }
    }

    // Règle : la carte de détail se multiplie ×2, donc un gris moyen est neutre
    // et une carte sombre **assombrit**. Bug réel : la version précédente
    // ramenait la teinte à luminance constante, si bien qu'aucune voiture ne
    // pouvait être foncée — `ks_abarth500_assetto_corse` / `dark_blue`
    // ressortait blanche alors que sa carte de détail vaut (0, 16, 38).
    #[test]
    fn dark_detail_map_darkens_instead_of_only_tinting() {
        let navy = factor(&material(&[("useDetail", 1.0)]), [0.0, 16.0 / 255.0, 38.0 / 255.0])
            .expect("une carte sombre peint");
        assert!(navy[2] > navy[1] && navy[1] > navy[0], "la nuance bleue ressort");
        assert!(
            navy.iter().all(|c| *c < 0.35),
            "et surtout elle assombrit : {navy:?} devrait rester bien en dessous de 1"
        );

        let neutral = factor(&material(&[("useDetail", 1.0)]), [0.5, 0.5, 0.5]);
        assert!(neutral.is_none(), "un gris moyen est neutre, rien à peindre");
    }

    // Règle : sans `useDetail`, la carte de détail est ignorée.
    #[test]
    fn detail_map_ignored_when_the_material_does_not_use_it() {
        assert!(
            factor(&material(&[]), [0.2, 0.9, 0.2]).is_none(),
            "détail non déclaré, non appliqué"
        );
    }

    // Règle : l'alpha de la diffuse est un masque de peinture. Les pixels à
    // alpha nul sont de la carrosserie et se peignent ; ceux à alpha plein sont
    // des décalcomanies et ne bougent pas — c'est ce qui laisse un numéro de
    // course blanc sur une voiture bleu nuit.
    #[test]
    fn paint_spares_the_pixels_the_alpha_mask_protects() {
        let mut image = RgbaImage::new(2, 1);
        image.put_pixel(0, 0, Rgba([200, 200, 200, 0])); // carrosserie
        image.put_pixel(1, 0, Rgba([200, 200, 200, 255])); // décalcomanie

        assert!(apply(&mut image, [0.0, 0.25, 0.5]), "quelque chose a été peint");
        assert_eq!(
            image.get_pixel(0, 0).0,
            [0, 50, 100, 255],
            "la carrosserie prend la couleur du skin"
        );
        assert_eq!(
            image.get_pixel(1, 0).0,
            [200, 200, 200, 255],
            "la décalcomanie garde la sienne"
        );
    }

    // Règle : une diffuse entièrement opaque n'a rien à peindre, et la variante
    // ne doit pas être écrite — ce serait une copie exacte d'une livrée de
    // 2048² dans le GLB.
    #[test]
    fn a_fully_opaque_diffuse_produces_no_variant() {
        let mut image = RgbaImage::from_pixel(2, 2, Rgba([120, 120, 120, 255]));
        assert!(!apply(&mut image, [0.0, 0.5, 1.0]), "rien à peindre");
        assert_eq!(image.get_pixel(0, 0).0, [120, 120, 120, 255], "image intacte");
    }

    // Règle : deux matériaux qui demandent la même peinture sur la même livrée
    // partagent une seule image.
    #[test]
    fn the_same_paint_on_the_same_livery_is_named_once() {
        let name = variant_name("livery.dds", [0.5, 1.0, 1.5]);
        assert_eq!(name, variant_name("livery.dds", [0.5, 1.0, 1.5]), "nom déterministe");
        assert_ne!(name, variant_name("livery.dds", [1.5, 1.0, 0.5]), "couleur distincte");
        assert_ne!(name, variant_name("other.dds", [0.5, 1.0, 1.5]), "source distincte");
    }
}
