//! Roughness read from `txMaps` (spec §6.2, §12 q3 — partly answered).
//!
//! `txMaps` is AC's per-pixel surface map. Measured across the reference
//! library, its **green channel is glossiness**, and that is the only channel
//! whose meaning holds up:
//!
//! | Texture | surface | G |
//! | --- | --- | --- |
//! | `EXT_Chrome_MAP` (`ks_mazda_mx5_cup`) | chrome | 255 |
//! | `exterior_body_map` (`ks_toyota_supra_mkiv`) | peinture | 255 |
//! | `EXT_Rims_MAP` | jantes | 223 |
//! | `exterior_metal_map` | métal nu | 148 |
//! | `exterior_plastic_map` | plastique mat | 64 |
//! | `INT_LR_map` | cuir, plastique de planche de bord | 25 |
//!
//! It matters because `ksSpecularEXP` alone — what the roughness used to be
//! guessed from — does not separate those surfaces at all: the MX-5's chrome
//! and its interior leather both sit at `EXP ≈ 100`, with G at 255 against 25.
//! A car body came out at roughness 0.55, a satin finish rather than paint.
//!
//! **R and B carry nothing usable, and that is now measured rather than
//! suspected** (docs/kn5-format.md, écart n°7). On Kunos content the two are
//! the *same data*: correlation 1.00 in the median, pixel-identical on 49 % of
//! the maps. Neither tracks the gloss — the median correlation with green is
//! 0.06, and the figure swings from −0.97 to +1.00 from one car to the next,
//! which is the signature of a channel nobody authors deliberately. Mods write
//! something else again (correlation 0.00 in the median) without looking broken
//! in game, so the shader does not read them either. They stay unused.
//!
//! The metallicity they were expected to provide comes from `fresnelC`
//! instead — see [`crate::material`].
//!
//! The conversion is per-pixel and not a scalar because an atlas is routinely
//! shared between surfaces of different finishes — G varies by a standard
//! deviation of 70 within `INT_Cockpit_OCC_Map`, which one number could only
//! average away.

use std::collections::BTreeMap;

use image::RgbaImage;
use kn5::{Kn5Material, Kn5Model};

use crate::texture::{TextureRole, TextureSet};

/// Rugosité minimale. Un miroir parfait n'existe pas sur une voiture, et une
/// rugosité nulle rend un reflet d'environnement dur et faux.
const MIN_ROUGHNESS: f32 = 0.08;

/// Suffixe du nom de la texture dérivée. La transformation n'a pas de
/// paramètre : une carte source donne une seule variante.
const SUFFIX: &str = "#rough";

pub(crate) fn variant_name(maps: &str) -> String {
    format!("{maps}{SUFFIX}")
}

/// Carte de rugosité par matériau, et cartes à cuire.
#[derive(Debug, Default)]
pub(crate) struct Plan {
    per_material: Vec<Option<String>>,
    /// Variante → texture source.
    variants: BTreeMap<String, String>,
}

impl Plan {
    pub(crate) fn variants(&self) -> Vec<(String, String)> {
        self.variants.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    pub(crate) fn roughness_texture(&self, material: usize) -> Option<String> {
        self.per_material.get(material).cloned().flatten()
    }

    pub(crate) fn forget(&mut self, name: &str) {
        self.variants.remove(name);
        for slot in &mut self.per_material {
            if slot.as_deref() == Some(name) {
                *slot = None;
            }
        }
    }
}

pub(crate) fn plan(model: &Kn5Model, textures: &TextureSet) -> Plan {
    let mut plan = Plan::default();
    for material in &model.materials {
        let variant = maps_texture(material, textures).map(|source| {
            let name = variant_name(&source);
            plan.variants.entry(name.clone()).or_insert(source);
            name
        });
        plan.per_material.push(variant);
    }
    plan
}

/// The `txMaps` texture of a material, when it really is a surface map.
///
/// **Guard that earns its place**: plenty of materials point `txMaps` at a
/// texture that is also somebody's `txDiffuse` — `Grey.dds` on the Supra's
/// radiator, its own cockpit atlas on the Abarth's interior. The green channel
/// of a colour texture is a colour, not a glossiness, and reading it as one
/// turns a green seat into a mirror.
fn maps_texture(material: &Kn5Material, textures: &TextureSet) -> Option<String> {
    let name = material.texture_for("txMaps").filter(|t| !t.is_empty())?;
    let prepared = textures.get(name)?;
    (prepared.role == TextureRole::Data).then(|| name.to_string())
}

/// Turns a surface map into a glTF metallic-roughness texture.
///
/// glTF reads roughness from **G** and metallic from **B** of the same image,
/// so the transformation is one inversion and one channel opened: AC's
/// glossiness is the complement of a roughness, and the blue is held wide open
/// so that the material's `metallicFactor` reaches the surface untouched.
///
/// Returns `false` on a map that says nothing — see [`is_placeholder`] — so the
/// caller can drop it and leave the material on its `ksSpecularEXP` fallback.
pub(crate) fn apply(image: &mut RgbaImage) -> bool {
    if is_placeholder(image) {
        return false;
    }
    for pixel in image.pixels_mut() {
        let roughness = (1.0 - pixel.0[1] as f32 / 255.0).max(MIN_ROUGHNESS);
        // Bleu à **255**, et ce n'est pas une valeur neutre choisie au hasard :
        // glTF lit la métallicité dans le bleu de cette même texture et la
        // **multiplie** par `metallicFactor`. Un bleu à zéro annulerait donc en
        // silence la métallicité du matériau — précisément sur les surfaces qui
        // en ont une, puisque ce sont celles qui portent une carte. Le canal
        // dit « laisse passer le facteur », la métallicité restant scalaire :
        // rien dans le KN5 ne la décrit par pixel (voir [`crate::material`]).
        pixel.0 = [0, (roughness * 255.0).round() as u8, u8::MAX, u8::MAX];
    }
    true
}

/// Is this the "no map here" placeholder rather than a surface map?
///
/// AC ships `NULL.dds`, four pixels of pure white, and materials that have
/// nothing to say about their surface point `txMaps` at it — the Supra's fabric
/// seats among them. Read literally it means "glossiness 1", which would turn
/// those seats into a mirror.
///
/// The test is **every channel at maximum**, not just the green one: a real
/// surface map that is uniformly glossy does exist (the Abarth's body map holds
/// G=255 flat) but it carries genuine values in the others (R=24, B=239). A
/// texture saturated everywhere is a placeholder, not a measurement.
fn is_placeholder(image: &RgbaImage) -> bool {
    image.pixels().all(|p| p.0[0] == 255 && p.0[1] == 255 && p.0[2] == 255)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    // Règle : le vert de `txMaps` est une brillance, donc son complément est la
    // rugosité — et c'est bien dans le canal vert que glTF va la lire.
    #[test]
    fn glossy_green_becomes_low_roughness() {
        let mut image = RgbaImage::new(3, 1);
        image.put_pixel(0, 0, Rgba([24, 255, 239, 255])); // peinture, chrome
        image.put_pixel(1, 0, Rgba([34, 25, 34, 255])); // cuir, plastique mat
        image.put_pixel(2, 0, Rgba([46, 148, 46, 255])); // métal nu
        assert!(apply(&mut image), "une vraie carte de surface est exploitée");

        let roughness = |x| image.get_pixel(x, 0).0[1];
        assert!(
            roughness(0) < 30,
            "une surface brillante ressort lisse, pas satinée (obtenu {})",
            roughness(0)
        );
        assert!(roughness(1) > 220, "une surface mate reste rugueuse");
        assert!(
            roughness(2) > roughness(0) && roughness(2) < roughness(1),
            "et le métal nu se range entre les deux"
        );
        for x in 0..3 {
            assert_eq!(
                image.get_pixel(x, 0).0[2],
                255,
                "le canal métallique laisse passer le facteur du matériau, il ne l'annule pas"
            );
        }
    }

    // Règle : pas de miroir parfait. Une rugosité nulle donne un reflet
    // d'environnement dur, qui trahit plus qu'il ne sert sur un aperçu.
    #[test]
    fn a_perfectly_glossy_map_still_keeps_a_floor() {
        let mut image = RgbaImage::from_pixel(1, 1, Rgba([0, 255, 0, 255]));
        assert!(apply(&mut image), "une carte franchement brillante reste une carte");
        assert!(
            image.get_pixel(0, 0).0[1] >= (MIN_ROUGHNESS * 255.0) as u8,
            "la rugosité garde son plancher"
        );
    }

    // Règle : `NULL.dds` (4 pixels blancs) veut dire « rien à dire sur cette
    // surface », pas « brillance maximale ». Bug réel : les sièges en tissu de
    // `ks_toyota_supra_mkiv` pointent dessus, et pris au premier degré ils
    // devenaient des miroirs.
    #[test]
    fn the_null_placeholder_is_not_a_surface_map() {
        let mut placeholder = RgbaImage::from_pixel(4, 4, Rgba([255, 255, 255, 255]));
        assert!(!apply(&mut placeholder), "une carte blanche partout ne dit rien");
        assert_eq!(
            placeholder.get_pixel(0, 0).0,
            [255, 255, 255, 255],
            "et elle repart intacte, le matériau garde son repli"
        );

        // Une carte uniformément brillante mais réelle porte des valeurs
        // ailleurs (carrosserie de `ks_abarth500_assetto_corse` : R=24, B=239).
        let mut glossy = RgbaImage::from_pixel(4, 4, Rgba([24, 255, 239, 255]));
        assert!(apply(&mut glossy), "celle-là est bien une carte de surface");
    }
}
