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
    /// Variante → plancher de rugosité, tiré des shaders qui la consomment
    /// (voir [`floor_for`]). Le **plus haut** l'emporte quand une même texture
    /// sert deux familles : ne pas transformer une peau en miroir prime sur
    /// laisser une carrosserie briller, et le cas ne se rencontre pas — un
    /// mannequin n'a que des matériaux de mannequin.
    floors: BTreeMap<String, f32>,
}

impl Plan {
    pub(crate) fn variants(&self) -> Vec<(String, String)> {
        self.variants.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    pub(crate) fn floor_of(&self, variant: &str) -> f32 {
        self.floors.get(variant).copied().unwrap_or(MIN_ROUGHNESS)
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

pub(crate) fn plan(model: &Kn5Model, textures: &TextureSet, mannequin: bool) -> Plan {
    let mut plan = Plan::default();
    for material in &model.materials {
        let variant = maps_texture(material, textures).map(|source| {
            let name = variant_name(&source);
            plan.variants.entry(name.clone()).or_insert(source);
            let floor = floor_for(&material.shader, mannequin);
            plan.floors
                .entry(name.clone())
                .and_modify(|kept| *kept = kept.max(floor))
                .or_insert(floor);
            name
        });
        plan.per_material.push(variant);
    }
    plan
}

/// Plancher de rugosité selon la famille de shader.
///
/// **Un mannequin n'est ni chromé ni mouillé.** Le `txMaps` d'un matériau de
/// mannequin n'est presque jamais une carte de surface : mesuré sur 62
/// voitures, 98 % des `txMaps` réellement exploités appartiennent à la famille
/// `ksPerPixelMultiMap` — celle dont le nom dit qu'elle lit plusieurs cartes.
/// Sur `ksSkinnedMesh*`, les moddeurs y rangent ce qui leur passe par la
/// main : une carte d'éclairage (`Rinoa_Skin_L.dds`, vert moyen 235), une
/// normale (`Rinoa_Necklace_N.dds`), une diffuse (`hairsh_d.png`). Lu comme
/// une brillance, ce vert clair donne la rugosité plancher — une peau en
/// miroir, signalée par l'utilisateur comme « ils transpirent ».
///
/// **Pourquoi un plancher et non l'abandon de la carte** : certaines *sont*
/// de vraies cartes spéculaires (`legs_s.dds` de `jill_re3`, vert moyen 15 —
/// donc mate, et juste), et le repli sur `ksSpecularEXP` serait pire pour
/// elles : ce mannequin écrit 1000, une valeur que la formule rend
/// mirifiquement lisse. Le plancher garde ce que la carte a de bon et coupe
/// seulement ce qu'elle a d'impossible.
fn floor_for(shader: &str, mannequin: bool) -> f32 {
    if mannequin || shader.contains("ksSkinnedMesh") {
        SKIN_MIN_ROUGHNESS
    } else {
        MIN_ROUGHNESS
    }
}

/// Rugosité minimale d'une peau ou d'un tissu. Ni l'une ni l'autre ne renvoie
/// d'image nette : au-dessous, on ne rend pas une matière, on rend un reflet.
const SKIN_MIN_ROUGHNESS: f32 = 0.5;

/// The `txMaps` texture of a material, when it really is a surface map.
///
/// **Guard that earns its place**: plenty of materials point `txMaps` at a
/// texture that is also somebody's `txDiffuse` — `Grey.dds` on the Supra's
/// radiator, its own cockpit atlas on the Abarth's interior. The green channel
/// of a colour texture is a colour, not a glossiness, and reading it as one
/// turns a green seat into a mirror.
fn maps_texture(material: &Kn5Material, textures: &TextureSet) -> Option<String> {
    if says_it_is_rough(material) {
        return None;
    }
    let name = material.texture_for("txMaps").filter(|t| !t.is_empty())?;
    let prepared = textures.get(name)?;
    (prepared.role == TextureRole::Data).then(|| name.to_string())
}

/// L'auteur a-t-il écrit noir sur blanc que cette surface est mate ?
///
/// **Second garde-fou, pour un cas que le premier ne voit pas** : une carte
/// qui *est* une carte de surface, mais dont le vert est plat et haut, dit
/// « brillant partout » sans rien apporter pixel par pixel. Quand le matériau
/// annonce par ailleurs l'exposant le plus mat possible, les deux se
/// contredisent totalement, et entre deux constantes c'est celle que l'auteur
/// a écrite dans le matériau qui gagne.
///
/// Bug réel : les cheveux de `lm_mai_shiranui` (mod de pilote) pointent
/// `txMaps` sur `hair.dds`, vert constant à 235 sur 255 — rugosité 0,08, le
/// plancher, donc des cheveux en miroir — alors que leur `ksSpecularEXP` vaut
/// 0,2, c'est-à-dire mat au maximum.
///
/// **Étroit à dessein, et mesuré** : sur 62 voitures (1 371 matériaux à
/// `txMaps`), sept seulement ont un exposant sous 1, et **tous** l'ont à zéro
/// — c'est-à-dire « non réglé » et non « mat », le même piège que `ksAlphaRef`
/// (kn5-format.md, écart n°12). Exiger une valeur strictement positive laisse
/// donc le corpus voiture intact, et ne retient que l'auteur qui a vraiment
/// posé une valeur basse.
fn says_it_is_rough(material: &Kn5Material) -> bool {
    material
        .property("ksSpecularEXP")
        .is_some_and(|exp| exp > 0.0 && exp < ROUGH_EXPONENT)
}

/// En dessous, l'exposant ne décrit plus un reflet : la surface est mate.
const ROUGH_EXPONENT: f32 = 1.0;

/// Turns a surface map into a glTF metallic-roughness texture.
///
/// glTF reads roughness from **G** and metallic from **B** of the same image,
/// so the transformation is one inversion and one channel opened: AC's
/// glossiness is the complement of a roughness, and the blue is held wide open
/// so that the material's `metallicFactor` reaches the surface untouched.
///
/// Returns `false` on a map that says nothing — see [`is_placeholder`] — so the
/// caller can drop it and leave the material on its `ksSpecularEXP` fallback.
pub(crate) fn apply(image: &mut RgbaImage, floor: f32) -> bool {
    if is_placeholder(image) {
        return false;
    }
    // **Une carte qui varie vraiment garde la main, plancher ou pas.** Sans
    // cette échappatoire, le plancher des mannequins mattifierait aussi ce
    // qui, sur une personne, brille pour de bon : le casque de Kunos
    // (`HELMET_2012_map.dds`) distingue coque peinte, aérations et sangles —
    // vert moyen 199, **écart-type 97**. Les cartes fautives, elles, disent la
    // même chose partout : peau de `rinoa` 235 ± 26, mains de `jill_re3`
    // 199 ± 56. Une carte qui ne varie pas ne mesure rien ; celle qui varie
    // décrit des finitions différentes, et c'est exactement ce qu'on lui
    // demande.
    let floor = if green_spread(image) >= REAL_MAP_SPREAD {
        MIN_ROUGHNESS
    } else {
        floor
    };
    for pixel in image.pixels_mut() {
        let roughness = (1.0 - pixel.0[1] as f32 / 255.0).max(floor);
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

/// Écart-type du canal vert, l'unité dans laquelle se mesure « cette carte
/// distingue-t-elle des finitions ? ».
fn green_spread(image: &RgbaImage) -> f32 {
    let count = image.pixels().len() as f32;
    if count == 0.0 {
        return 0.0;
    }
    let mean = image.pixels().map(|p| p.0[1] as f32).sum::<f32>() / count;
    let variance = image.pixels().map(|p| (p.0[1] as f32 - mean).powi(2)).sum::<f32>() / count;
    variance.sqrt()
}

/// Au-dessus, la carte décrit bien plusieurs finitions et personne ne la
/// corrige — voir la mesure dans [`apply`]. Posé entre les cartes fautives
/// (26 à 56) et la seule vraie mesurée sur un mannequin (97).
const REAL_MAP_SPREAD: f32 = 75.0;

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

    fn with_exponent(value: Option<f32>) -> Kn5Material {
        Kn5Material {
            name: "m".to_string(),
            shader: "ksSkinnedMesh".to_string(),
            blend_mode: 0,
            alpha_tested: false,
            reserved: 0,
            properties: value
                .into_iter()
                .map(|value| kn5::Kn5MaterialProperty {
                    name: "ksSpecularEXP".to_string(),
                    value,
                    extra: [0.0; 9],
                })
                .collect(),
            samplers: Vec::new(),
        }
    }

    /// Règle : une carte de surface ne rend jamais une peau ni un tissu
    /// spéculaires — signalé par l'utilisateur sur `jill_re3` et `rinoa`, dont
    /// les mannequins « transpiraient » — **sauf** si elle distingue vraiment
    /// des finitions, comme le fait la carte de casque de Kunos.
    #[test]
    fn a_mannequin_never_gets_a_mirror_finish() {
        // Une carte plate et claire : la peau de `rinoa`.
        let mut skin = RgbaImage::new(4, 1);
        for x in 0..4 {
            skin.put_pixel(x, 0, Rgba([0, 235, 0, 255]));
        }
        assert!(apply(&mut skin, floor_for("ksSkinnedMesh_NMDetaill", false)));
        assert!(
            skin.get_pixel(0, 0).0[1] >= (SKIN_MIN_ROUGHNESS * 255.0) as u8,
            "la peau garde un plancher de rugosité (obtenu {})",
            skin.get_pixel(0, 0).0[1]
        );

        let mut paint = RgbaImage::new(4, 1);
        for x in 0..4 {
            paint.put_pixel(x, 0, Rgba([0, 235, 0, 255]));
        }
        assert!(apply(&mut paint, floor_for("ksPerPixelMultiMap", false)));
        assert!(
            paint.get_pixel(0, 0).0[1] < (SKIN_MIN_ROUGHNESS * 255.0) as u8,
            "une carrosserie, elle, a le droit de briller"
        );

        // Une carte qui distingue coque peinte, aérations et sangles : le
        // casque garde son vernis, même porté par un mannequin.
        let mut helmet = RgbaImage::new(4, 1);
        for (x, green) in [255u8, 40, 250, 30].into_iter().enumerate() {
            helmet.put_pixel(x as u32, 0, Rgba([0, green, 0, 255]));
        }
        assert!(apply(&mut helmet, SKIN_MIN_ROUGHNESS));
        assert!(
            helmet.get_pixel(0, 0).0[1] < (SKIN_MIN_ROUGHNESS * 255.0) as u8,
            "une carte qui varie vraiment garde la main sur le plancher"
        );
    }

    /// Règle : un exposant spéculaire **explicitement** très bas prime sur la
    /// carte de surface — cheveux en miroir de `lm_mai_shiranui`. Un zéro, lui,
    /// veut dire « non réglé » et ne prime sur rien (mesuré : les sept
    /// matériaux du corpus voiture sous 1 sont tous à zéro).
    #[test]
    fn an_explicit_matte_exponent_wins_over_the_surface_map() {
        assert!(
            says_it_is_rough(&with_exponent(Some(0.2))),
            "0,2 est posé à la main : l'auteur dit mat"
        );
        assert!(
            !says_it_is_rough(&with_exponent(Some(0.0))),
            "zéro veut dire « non réglé », pas « mat »"
        );
        assert!(
            !says_it_is_rough(&with_exponent(Some(50.0))),
            "un exposant ordinaire laisse la carte décider"
        );
        assert!(
            !says_it_is_rough(&with_exponent(None)),
            "et un matériau sans exposant ne dit rien"
        );
    }

    // Règle : le vert de `txMaps` est une brillance, donc son complément est la
    // rugosité — et c'est bien dans le canal vert que glTF va la lire.
    #[test]
    fn glossy_green_becomes_low_roughness() {
        let mut image = RgbaImage::new(3, 1);
        image.put_pixel(0, 0, Rgba([24, 255, 239, 255])); // peinture, chrome
        image.put_pixel(1, 0, Rgba([34, 25, 34, 255])); // cuir, plastique mat
        image.put_pixel(2, 0, Rgba([46, 148, 46, 255])); // métal nu
        assert!(
            apply(&mut image, MIN_ROUGHNESS),
            "une vraie carte de surface est exploitée"
        );

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
        assert!(
            apply(&mut image, MIN_ROUGHNESS),
            "une carte franchement brillante reste une carte"
        );
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
        assert!(
            !apply(&mut placeholder, MIN_ROUGHNESS),
            "une carte blanche partout ne dit rien"
        );
        assert_eq!(
            placeholder.get_pixel(0, 0).0,
            [255, 255, 255, 255],
            "et elle repart intacte, le matériau garde son repli"
        );

        // Une carte uniformément brillante mais réelle porte des valeurs
        // ailleurs (carrosserie de `ks_abarth500_assetto_corse` : R=24, B=239).
        let mut glossy = RgbaImage::from_pixel(4, 4, Rgba([24, 255, 239, 255]));
        assert!(
            apply(&mut glossy, MIN_ROUGHNESS),
            "celle-là est bien une carte de surface"
        );
    }
}
