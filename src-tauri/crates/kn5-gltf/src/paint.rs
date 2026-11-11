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
//!
//! **A detail map at the diffuse's own scale is a drawing, not a swatch.**
//! With `detailUVMultiplier = 1` it is laid over the diffuse texel for texel,
//! and some mods put everything there: the traffic trucks of
//! `traffic_isuzu_tanker` keep only ambient occlusion in their diffuse, and the
//! cab, its grille, the hazard stripes of the tank all live in a 2048² detail
//! map. Reduced to its average, it painted the truck one flat colour (reported
//! on screen). Such a map is baked pixel by pixel ([`apply_detail`]); on a flat
//! swatch that gives the very same result as its average. A tiled one (a
//! carbon weave repeated 800 times) cannot be baked into the diffuse at all,
//! and keeps its average.

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
    /// Baked pixel by pixel instead of `factor`, when there is one.
    pub detail: Option<Detail>,
}

/// A detail map laid over the diffuse at its own scale — see the head of this
/// module.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Detail {
    pub texture: String,
    pub use_detail: f32,
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
    if let Some(detail) = same_scale_detail(material) {
        textures.get(&detail.texture)?;
        return Some(Variant {
            // The blend amount in the name too: the same pair at another
            // amount is another image.
            name: format!("{diffuse}#detail-{}-{:.2}", detail.texture, detail.use_detail),
            source: diffuse,
            factor: [1.0; 3],
            detail: Some(detail),
        });
    }
    let factor = factor(material, average(material, textures)?)?;
    Some(Variant {
        name: variant_name(&diffuse, factor),
        source: diffuse,
        factor,
        detail: None,
    })
}

/// The detail map of a material, when it is laid over the diffuse at the same
/// scale. Only an explicit `detailUVMultiplier = 1`: an absent one says
/// nothing about the scale, and such materials keep the average.
fn same_scale_detail(material: &Kn5Material) -> Option<Detail> {
    let use_detail = material.property("useDetail").unwrap_or(0.0).clamp(0.0, 1.0);
    if use_detail <= 0.0 || material.property("detailUVMultiplier") != Some(1.0) {
        return None;
    }
    let texture = material.texture_for("txDetail").filter(|t| !t.is_empty())?;
    Some(Detail {
        texture: texture.to_string(),
        use_detail,
    })
}

/// The repeated detail map of a material, for the viewer to lay over the
/// colour ([`crate::material::DetailLayer`]): a `detailUVMultiplier` above 1,
/// and a map that carries a pattern. A flat swatch says nothing its average
/// has not already said in the diffuse.
pub(crate) fn tiled_detail(material: &Kn5Material, textures: &TextureSet) -> Option<crate::material::DetailLayer> {
    let use_detail = material.property("useDetail").unwrap_or(0.0).clamp(0.0, 1.0);
    let scale = material.property("detailUVMultiplier")?;
    if use_detail <= 0.0 || !(scale > 1.0 && scale.is_finite()) {
        return None;
    }
    let texture = material.texture_for("txDetail").filter(|t| !t.is_empty())?;
    let prepared = textures.get(texture)?;
    if prepared.flat {
        return None;
    }
    Some(crate::material::DetailLayer {
        texture: texture.to_string(),
        scale,
        use_detail,
        average: prepared.average,
    })
}

/// How far a texel may stray from the average of its map for the map to still
/// count as a flat swatch, in levels out of 255. Swatches are flat colours
/// saved through a lossy format; a drawing strays by far more.
const FLAT_TOLERANCE: u8 = 8;

/// Whether a detail map is a flat swatch, which then paints through its average
/// exactly as before ([`factor`]) — Kunos skins do not move.
pub(crate) fn is_flat(detail: &RgbaImage, average: [f32; 3]) -> bool {
    let average = average.map(|c| (c * 255.0).round() as i16);
    detail
        .pixels()
        .all(|p| (0..3).all(|c| (p.0[c] as i16 - average[c]).unsigned_abs() <= FLAT_TOLERANCE as u16))
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
    factor_for(material.property("useDetail").unwrap_or(0.0), detail_average)
}

/// [`factor`], from the blend amount itself.
pub(crate) fn factor_for(use_detail: f32, detail_average: [f32; 3]) -> Option<[f32; 3]> {
    let use_detail = use_detail.clamp(0.0, 1.0);
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
        painted |= paint_pixel(&mut pixel.0, factor);
    }
    painted
}

/// Paints a decoded diffuse texture with a detail map of the same size, pixel
/// by pixel — the `MODULATE2X` of [`factor`], one texel at a time.
pub(crate) fn apply_detail(image: &mut RgbaImage, detail: &RgbaImage, use_detail: f32) -> bool {
    debug_assert_eq!(image.dimensions(), detail.dimensions(), "detail resized to the diffuse");
    let use_detail = use_detail.clamp(0.0, 1.0);
    let mut painted = false;
    for (pixel, texel) in image.pixels_mut().zip(detail.pixels()) {
        let factor = [0, 1, 2].map(|c| 1.0 + (texel.0[c] as f32 / 255.0 * DETAIL_MODULATE - 1.0) * use_detail);
        painted |= paint_pixel(&mut pixel.0, factor);
    }
    painted
}

/// One pixel of [`apply`], whose alpha is the paint mask. Whether it was painted.
fn paint_pixel(pixel: &mut [u8; 4], factor: [f32; 3]) -> bool {
    let mask = 1.0 - pixel[3] as f32 / 255.0;
    if mask > 0.0 {
        // `zip` stops after the three colour channels, leaving the alpha to
        // the line below.
        for (channel, factor) in pixel.iter_mut().zip(factor.iter()) {
            let scale = 1.0 + (factor - 1.0) * mask;
            *channel = (*channel as f32 * scale).round().clamp(0.0, 255.0) as u8;
        }
    }
    // The mask has done its job; kept, it would be read as transparency and
    // erase the bodywork (see `texture::strip_alpha`).
    pixel[3] = u8::MAX;
    mask > 0.0
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

    // Règle : une carte de détail à l'échelle de la diffuse est un dessin, et se
    // peint pixel par pixel — les rayures d'une citerne restent des rayures.
    // Bug réel : `traffic_isuzu_tanker` sortait d'une seule couleur, sa carte
    // réduite à sa moyenne.
    #[test]
    fn a_same_scale_detail_map_is_painted_pixel_by_pixel() {
        let mut image = RgbaImage::from_pixel(2, 1, Rgba([128, 128, 128, 0]));
        let mut stripes = RgbaImage::new(2, 1);
        stripes.put_pixel(0, 0, Rgba([255, 200, 0, 255])); // jaune
        stripes.put_pixel(1, 0, Rgba([0, 0, 0, 255])); // noir
        assert!(!is_flat(&stripes, [0.5, 0.39, 0.0]), "des rayures ne sont pas un aplat");

        assert!(apply_detail(&mut image, &stripes, 1.0), "quelque chose a été peint");
        let yellow = image.get_pixel(0, 0).0;
        assert!(
            yellow[0] > 200 && yellow[2] == 0,
            "la bande jaune reste jaune : {yellow:?}"
        );
        assert_eq!(image.get_pixel(1, 0).0, [0, 0, 0, 255], "la bande noire reste noire");
    }

    // Règle : un aplat se reconnaît, et se peint par sa moyenne comme avant —
    // les skins Kunos ne bougent pas. Un aplat passé par un format avec pertes
    // varie de quelques niveaux, pas davantage.
    #[test]
    fn a_flat_swatch_keeps_painting_through_its_average() {
        let mut swatch = RgbaImage::from_pixel(4, 4, Rgba([35, 57, 161, 255]));
        swatch.put_pixel(0, 0, Rgba([38, 55, 165, 255]));
        assert!(
            is_flat(&swatch, [35.0 / 255.0, 57.0 / 255.0, 161.0 / 255.0]),
            "un aplat bruité reste un aplat"
        );

        let average = [35.0 / 255.0, 57.0 / 255.0, 161.0 / 255.0];
        let factor = factor_for(1.0, average).expect("une couleur peint");
        let mut by_average = RgbaImage::from_pixel(2, 2, Rgba([150, 140, 130, 0]));
        let mut by_pixel = by_average.clone();
        apply(&mut by_average, factor);
        apply_detail(
            &mut by_pixel,
            &RgbaImage::from_pixel(2, 2, Rgba([35, 57, 161, 255])),
            1.0,
        );
        assert_eq!(
            by_average, by_pixel,
            "sur un aplat uni, les deux chemins donnent la même image"
        );
    }

    // Règle : seule une échelle de 1 **déclarée** est un dessin. Une carte
    // répétée (le tissage d'un carbone, ×800) ne se reporte pas sur la diffuse,
    // et une échelle absente ne dit rien.
    #[test]
    fn only_a_declared_scale_of_one_is_painted_pixel_by_pixel() {
        let one = material(&[("useDetail", 1.0), ("detailUVMultiplier", 1.0)]);
        assert_eq!(
            same_scale_detail(&one),
            Some(Detail {
                texture: "detail.dds".to_string(),
                use_detail: 1.0
            }),
            "échelle 1"
        );
        let tiled = material(&[("useDetail", 1.0), ("detailUVMultiplier", 800.0)]);
        assert_eq!(same_scale_detail(&tiled), None, "carte répétée : la moyenne");
        let unknown = material(&[("useDetail", 1.0)]);
        assert_eq!(same_scale_detail(&unknown), None, "échelle absente : la moyenne");
        let unused = material(&[("detailUVMultiplier", 1.0)]);
        assert_eq!(same_scale_detail(&unused), None, "détail non utilisé");
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
