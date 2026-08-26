//! Texture pipeline: decode → resize → re-encode, with skin overrides.
//!
//! Turns the blobs embedded in a KN5 (plus the loose files a skin folder puts
//! on top of them) into images a glTF viewer can actually display. AC ships
//! block-compressed DDS, which WebGL could technically consume, but only via
//! `WEBGL_compressed_texture_s3tc` and never for BC7 — decoding to plain RGBA8
//! here and re-encoding to PNG/JPEG is what makes the result portable
//! (spec §5.4).
//!
//! ## Why JPEG rather than WebP
//!
//! §5.4 asks for WebP quality 85 on colour maps. Two things argue against it,
//! and they come from the spec itself:
//!
//! - core glTF 2.0 only allows `image/png` and `image/jpeg`; WebP needs the
//!   `EXT_texture_webp` extension. Lot 3's acceptance criterion is that the
//!   produced `.glb` opens **in Blender and in an online glTF viewer** — an
//!   extension is exactly what breaks that;
//! - the `image` crate dropped its WebP encoder in 0.25, so it would mean a
//!   third-party binding to libwebp, i.e. a C toolchain in CI.
//!
//! JPEG at the same quality lands within ~30 % of WebP's size, and the resize
//! step already divides the payload by 4 to 16. Correctness of the acceptance
//! test wins over the last 30 %.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use image::{ImageEncoder, RgbaImage};
use kn5::{ImageFormat, Kn5Model};
use rayon::prelude::*;

/// What a texture is used for, which decides both its size cap and whether it
/// may be encoded lossily.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureRole {
    /// Bound to `txDiffuse`: the visible colour of the surface, sRGB.
    Color,
    /// Everything else — normal maps, `txMaps`, masks, detail maps. Never
    /// encoded lossily: JPEG artefacts on a normal map show up as visible
    /// ripples across a car body panel.
    Data,
}

impl TextureRole {
    fn max_size(self, options: &TextureOptions) -> u32 {
        match self {
            Self::Color => options.max_color_size,
            Self::Data => options.max_data_size,
        }
    }
}

/// Where the pixels came from — reported so that a skin that fails to apply is
/// visible rather than silently ignored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextureOrigin {
    /// The blob stored inside the KN5.
    Embedded,
    /// A file of the selected skin folder, which takes priority (§4.3).
    Skin(PathBuf),
}

#[derive(Debug, Clone)]
pub struct TextureOptions {
    /// Longest side of a colour map. A preview panel is a few hundred pixels
    /// wide; 4K liveries buy nothing and cost seconds of conversion (§5.4).
    pub max_color_size: u32,
    /// Longest side of every other map.
    pub max_data_size: u32,
    pub jpeg_quality: u8,
}

impl Default for TextureOptions {
    fn default() -> Self {
        Self {
            max_color_size: 2048,
            max_data_size: 1024,
            jpeg_quality: 85,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreparedTexture {
    /// Name as referenced by the materials — the key the glTF writer will use.
    pub name: String,
    /// `image/png` or `image/jpeg`.
    pub mime: &'static str,
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub role: TextureRole,
    pub origin: TextureOrigin,
    /// Size of the source blob, for the compression ratio the tool reports.
    pub source_bytes: usize,
    /// L'image encodée porte-t-elle un canal alpha exploitable ?
    pub has_alpha: bool,
    /// Cet alpha **varie-t-il** d'un pixel à l'autre ?
    ///
    /// La distinction décide de la transparence d'un matériau en fondu. Un
    /// alpha qui varie **découpe** : décalcomanie, flou de jante, couture — on
    /// n'y touche pas. Un alpha constant ne découpe rien, c'est une opacité
    /// uniforme, et la traiter comme une découpe rendait le vitrage d'AC
    /// invisible : le `glass.dds` de `ks_toyota_supra_mkiv` vaut 13/255 partout,
    /// soit 5 % d'opacité, là où l'opacité du shader en donne le triple.
    pub alpha_varies: bool,
    /// Couleur moyenne, en linéaire approché [0,1]. Sert à teinter un matériau
    /// depuis sa carte de détail (voir `material::convert`).
    pub average: [f32; 3],
}

/// A texture that could not be prepared. Never fatal: a car with one broken
/// texture is still worth previewing (§6.3).
#[derive(Debug, Clone)]
pub struct TextureWarning {
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Default)]
pub struct TextureSet {
    /// Par matériau (index dans `model.materials`) : ce que vaut l'alpha de sa
    /// diffuse **là où ce matériau l'échantillonne**. Voir [`FootprintAlpha`].
    pub footprint_alpha: BTreeMap<usize, FootprintAlpha>,
    pub textures: Vec<PreparedTexture>,
    pub warnings: Vec<TextureWarning>,
    /// Embedded blobs no material ever binds. Left untouched — transcoding
    /// them would be pure waste, and they are common in mods assembled from
    /// several sources.
    pub unreferenced: Vec<String>,
}

impl TextureSet {
    pub fn get(&self, name: &str) -> Option<&PreparedTexture> {
        self.textures.iter().find(|t| t.name == name)
    }

    pub fn total_bytes(&self) -> usize {
        self.textures.iter().map(|t| t.bytes.len()).sum()
    }

    pub fn source_bytes(&self) -> usize {
        self.textures.iter().map(|t| t.source_bytes).sum()
    }
}

/// Proportion de pixels à alpha nul et couleur moyenne sous ces pixels.
///
/// Sert au diagnostic : si le RVB est plein là où l'alpha est nul, le canal
/// alpha ne décrit pas une transparence et le conserver produit un rendu faux.
pub fn alpha_stats(texture: &PreparedTexture) -> Option<(usize, usize, [u8; 3])> {
    let decoded = image::load_from_memory(&texture.bytes).ok()?.to_rgba8();
    let mut zero = 0usize;
    let mut sum = [0u64; 3];
    for pixel in decoded.pixels() {
        if pixel.0[3] == 0 {
            zero += 1;
            for (channel, value) in sum.iter_mut().zip(pixel.0.iter()) {
                *channel += *value as u64;
            }
        }
    }
    let total = decoded.pixels().len();
    let mean = if zero == 0 {
        [0, 0, 0]
    } else {
        [
            (sum[0] / zero as u64) as u8,
            (sum[1] / zero as u64) as u8,
            (sum[2] / zero as u64) as u8,
        ]
    };
    Some((zero, total, mean))
}

/// Role of every texture referenced by a material, keyed by texture name.
///
/// A texture bound to `txDiffuse` anywhere is treated as colour everywhere:
/// mods do reuse the same file across slots, and getting it wrong in the
/// lossy direction (a normal map encoded as JPEG) is far worse than the
/// opposite.
fn roles(model: &Kn5Model) -> BTreeMap<String, TextureUse> {
    let mut roles: BTreeMap<String, TextureUse> = BTreeMap::new();
    for material in &model.materials {
        let consumes_alpha = !matches!(
            crate::material::alpha_mode_of(material).0,
            crate::material::AlphaMode::Opaque
        );
        for sampler in &material.samplers {
            if sampler.texture.is_empty() {
                continue;
            }
            let role = if sampler.name == "txDiffuse" {
                TextureRole::Color
            } else {
                TextureRole::Data
            };
            let entry = roles.entry(sampler.texture.clone()).or_insert(TextureUse {
                role,
                keep_alpha: false,
            });
            if role == TextureRole::Color {
                entry.role = TextureRole::Color;
                // L'alpha n'est conservé que si un matériau s'en sert
                // réellement. Voir `strip_alpha` : chez AC il transporte
                // le plus souvent tout autre chose.
                entry.keep_alpha |= consumes_alpha;
            } else {
                // Une carte de données (normales, masques) garde son alpha :
                // il y encode une donnée, pas une découpe.
                entry.keep_alpha = true;
            }
        }
    }
    roles
}

/// Ce qu'on sait d'une texture avant de la décoder.
#[derive(Debug, Clone, Copy)]
struct TextureUse {
    role: TextureRole,
    keep_alpha: bool,
}

/// Decodes, resizes and re-encodes every texture the materials actually use.
///
/// `skin_dir` is the folder of the selected skin, when there is one. Each
/// texture is deduplicated by name, so a file shared by twenty materials is
/// decoded once (§5.4).
pub fn prepare_textures(model: &Kn5Model, skin_dir: Option<&Path>, options: &TextureOptions) -> TextureSet {
    let roles = roles(model);
    let footprints = diffuse_footprints(model);
    let referenced: BTreeSet<&str> = roles.keys().map(String::as_str).collect();

    let mut set = TextureSet::default();
    for texture in &model.textures {
        if texture.has_data() && !referenced.contains(texture.name.as_str()) {
            set.unreferenced.push(texture.name.clone());
        }
    }

    // Decoding, resizing and re-encoding are pure CPU work, independent from
    // one texture to the next. `par_iter` over an ordered vector keeps the
    // result deterministic, which matters: the cache key must not depend on
    // how threads happened to be scheduled.
    let work: Vec<(&String, &TextureUse)> = roles.iter().collect();
    type Prepared = (PreparedTexture, Vec<(usize, FootprintAlpha)>);
    let prepared: Vec<Result<Prepared, TextureWarning>> = work
        .par_iter()
        .map(|(name, role)| {
            let embedded = model.texture(name);
            let Some((blob, origin)) = load_source(name, skin_dir, embedded.map(|t| t.data.as_slice())) else {
                return Err(TextureWarning {
                    name: (*name).clone(),
                    reason: "referenced by a material but neither embedded nor present in the skin folder".to_string(),
                });
            };
            let users = footprints.get(*name).map(Vec::as_slice).unwrap_or(&[]);
            prepare_one(name, &blob, origin, **role, users, options).map_err(|reason| TextureWarning {
                name: (*name).clone(),
                reason,
            })
        })
        .collect();

    for outcome in prepared {
        match outcome {
            Ok((texture, verdicts)) => {
                set.textures.push(texture);
                set.footprint_alpha.extend(verdicts);
            }
            Err(warning) => set.warnings.push(warning),
        }
    }

    set
}

/// Ce que l'alpha d'une diffuse vaut **là où un matériau l'échantillonne**.
///
/// Mesuré sur des **points** — un par sommet, un par centre de triangle — et
/// non sur le rectangle englobant des UV. La différence est décisive : sur
/// `vrc_erc_1999_renoir_csp`, le rectangle du pare-brise couvre 27 % de
/// l'atlas de carrosserie, où il attrape forcément de l'opaque comme du
/// transparent — ce qui ne dit rien de la vitre elle-même.
#[derive(Debug, Clone, Copy)]
pub struct FootprintAlpha {
    pub min: u8,
    pub max: u8,
    /// Nombre de points mesurés. Zéro quand l'alpha a été retiré à l'encodage,
    /// auquel cas il n'y a rien à conclure.
    pub samples: usize,
}

impl FootprintAlpha {
    /// Ce matériau n'échantillonne **que** des texels transparents : lu comme
    /// une découpe, cet alpha ne le découperait pas, il l'effacerait.
    ///
    /// Un auteur ne modélise pas une pièce pour qu'elle soit invisible. Quand
    /// ça arrive, c'est que l'alpha n'était pas une transparence — sur
    /// `vrc_erc_1999_renoir_csp`, `MAIN_WINDSCREEN` partage l'atlas
    /// `MAIN_BODY.dds` du matériau `MAIN_BODY`, dont l'alpha est un **masque
    /// de peinture** (écart n°5) : 0 y veut dire « peins ici », pas « perce
    /// ici ». Le pare-brise disparaissait purement et simplement.
    ///
    /// Volontairement étroit : le cas « uniformément opaque » ne déclenche
    /// rien, pour ne pas changer le rendu de tout ce qui marchait déjà.
    pub fn is_blank(&self) -> bool {
        self.samples > 0 && self.max <= BLANK_ALPHA
    }
}

/// Au-dessus, il reste quelque chose à voir : on ne touche à rien.
const BLANK_ALPHA: u8 = 8;

/// Au-delà, un matériau en dit déjà bien assez sur son empreinte, et payer
/// plus ne changerait aucun verdict.
const MAX_FOOTPRINT_SAMPLES: usize = 20_000;

/// Pour chaque texture servant de `txDiffuse`, les matériaux qui l'utilisent et
/// les points UV où ils l'échantillonnent.
fn diffuse_footprints(model: &Kn5Model) -> BTreeMap<String, Vec<DiffuseUser>> {
    let mut samples: BTreeMap<usize, Vec<[f32; 2]>> = BTreeMap::new();
    model.visit_nodes(&mut |node| {
        let Some(mesh) = node.mesh() else { return };
        // Un maillage que le rendu écartera de toute façon ne doit pas peser
        // sur l'empreinte d'un matériau qu'il partage avec du visible.
        if !mesh.is_visible || !mesh.is_renderable {
            return;
        }
        let points = samples.entry(mesh.material_id as usize).or_default();
        if points.len() >= MAX_FOOTPRINT_SAMPLES {
            return;
        }
        points.extend(mesh.vertices.iter().map(|vertex| vertex.uv));
        // Les sommets ne décrivent que le contour des triangles. Le centre
        // coûte trois additions et évite qu'un quadrilatère dont les quatre
        // coins tombent sur des texels identiques passe pour uniforme alors
        // que son intérieur est découpé.
        for triangle in mesh.indices.chunks_exact(3) {
            let corners: Vec<[f32; 2]> = triangle
                .iter()
                .filter_map(|i| mesh.vertices.get(*i as usize).map(|v| v.uv))
                .collect();
            if let [a, b, c] = corners[..] {
                points.push([(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0]);
            }
        }
    });

    let mut out: BTreeMap<String, Vec<DiffuseUser>> = BTreeMap::new();
    for (index, material) in model.materials.iter().enumerate() {
        let Some(texture) = material.texture_for("txDiffuse").filter(|name| !name.is_empty()) else {
            continue;
        };
        let Some(points) = samples.get(&index) else {
            continue;
        };
        out.entry(texture.to_string()).or_default().push(DiffuseUser {
            material: index,
            alpha_mode: crate::material::alpha_mode_of(material).0,
            points: points.clone(),
        });
    }
    out
}

/// Un matériau qui échantillonne une diffuse, et ce qu'il attend de son alpha.
struct DiffuseUser {
    material: usize,
    alpha_mode: crate::material::AlphaMode,
    points: Vec<[f32; 2]>,
}

/// Alpha minimal et maximal **aux points** où un matériau échantillonne.
///
/// Les UV ne sont pas normalisés dans `[0, 1]` : `vrc_erc_1999_renoir_csp`
/// range tout son V dans `[-1, 0]`. Le rendu s'en moque, l'échantillonnage
/// boucle — donc on boucle aussi, au lieu de rogner et de lire le mauvais
/// texel.
fn alpha_range_at(image: &RgbaImage, points: &[[f32; 2]]) -> (u8, u8) {
    let (mut min, mut max) = (u8::MAX, u8::MIN);
    for uv in points {
        let alpha = image
            .get_pixel(wrap(uv[0], image.width()), wrap(uv[1], image.height()))
            .0[3];
        min = min.min(alpha);
        max = max.max(alpha);
    }
    if min > max {
        return (0, 0);
    }
    (min, max)
}

/// Une coordonnée de texture en index de pixel, en répétant comme le rendu.
fn wrap(coordinate: f32, size: u32) -> u32 {
    if !coordinate.is_finite() {
        return 0;
    }
    ((coordinate - coordinate.floor()) * size as f32) as u32 % size
}

/// Bakes the paint the plan asks for, and adds the results to the set.
///
/// A second decoding pass over a handful of liveries, and it is deliberate: the
/// paint colour is the *average of another texture*, so it cannot be known
/// before the first pass has decoded the detail maps. Only the diffuse textures
/// a plan actually names are read again.
///
/// Variants that turn out to change nothing are dropped from the plan, so their
/// materials fall back on the plain texture instead of shipping a copy of it.
pub(crate) fn bake_paint(
    set: &mut TextureSet,
    plan: &mut crate::paint::Plan,
    model: &Kn5Model,
    skin_dir: Option<&Path>,
    options: &TextureOptions,
) {
    let variants = plan.variants();
    let baked: Vec<(String, Result<Option<PreparedTexture>, TextureWarning>)> = variants
        .par_iter()
        .map(|variant| {
            let embedded = model.texture(&variant.source);
            let Some((blob, origin)) = load_source(&variant.source, skin_dir, embedded.map(|t| t.data.as_slice()))
            else {
                // The plain texture was prepared, so its source was there a
                // moment ago: only a race with the filesystem gets here.
                return (
                    variant.name.clone(),
                    Err(TextureWarning {
                        name: variant.name.clone(),
                        reason: "paint source disappeared between the two passes".to_string(),
                    }),
                );
            };
            let outcome = paint_one(variant, &blob, origin, options).map_err(|reason| TextureWarning {
                name: variant.name.clone(),
                reason,
            });
            (variant.name.clone(), outcome)
        })
        .collect();

    for (name, outcome) in baked {
        match outcome {
            Ok(Some(texture)) => set.textures.push(texture),
            Ok(None) => plan.forget(&name),
            Err(warning) => {
                plan.forget(&name);
                set.warnings.push(warning);
            }
        }
    }
}

/// Bakes the metallic-roughness textures the plan asks for (see
/// [`crate::roughness`]), and adds them to the set.
///
/// Same two-pass shape as [`bake_paint`], and for the same reason: which
/// `txMaps` textures are surface maps rather than recycled colour textures is
/// only known once the first pass has assigned every texture its role.
pub(crate) fn bake_roughness(
    set: &mut TextureSet,
    plan: &mut crate::roughness::Plan,
    model: &Kn5Model,
    skin_dir: Option<&Path>,
    options: &TextureOptions,
) {
    let variants = plan.variants();
    let baked: Vec<(String, Result<Option<PreparedTexture>, TextureWarning>)> = variants
        .par_iter()
        .map(|(name, source)| {
            let embedded = model.texture(source);
            let Some((blob, origin)) = load_source(source, skin_dir, embedded.map(|t| t.data.as_slice())) else {
                return (
                    name.clone(),
                    Err(TextureWarning {
                        name: name.clone(),
                        reason: "surface map disappeared between the two passes".to_string(),
                    }),
                );
            };
            let outcome = roughness_one(name, &blob, origin, options).map_err(|reason| TextureWarning {
                name: name.clone(),
                reason,
            });
            (name.clone(), outcome)
        })
        .collect();

    for (name, outcome) in baked {
        match outcome {
            Ok(Some(texture)) => set.textures.push(texture),
            Ok(None) => plan.forget(&name),
            Err(warning) => {
                plan.forget(&name);
                set.warnings.push(warning);
            }
        }
    }
}

/// `None` sur une carte qui ne dit rien : le matériau garde alors la rugosité
/// tirée de `ksSpecularEXP` (voir [`crate::roughness::apply`]).
fn roughness_one(
    name: &str,
    blob: &[u8],
    origin: TextureOrigin,
    options: &TextureOptions,
) -> Result<Option<PreparedTexture>, String> {
    let source_bytes = blob.len();
    let decoded = decode(blob).map_err(|e| format!("decode failed: {e}"))?;
    // Rôle `Data` : jamais de JPEG sur une carte de rugosité, ses artefacts se
    // liraient comme des variations de brillance sur une carrosserie.
    let mut resized = downscale(decoded, TextureRole::Data.max_size(options));
    if !crate::roughness::apply(&mut resized) {
        return Ok(None);
    }
    let (bytes, mime) = encode(&resized, TextureRole::Data, options)?;

    Ok(Some(PreparedTexture {
        name: name.to_string(),
        mime,
        bytes,
        width: resized.width(),
        height: resized.height(),
        role: TextureRole::Data,
        origin,
        source_bytes,
        has_alpha: false,
        alpha_varies: false,
        average: average_color(&resized),
    }))
}

/// Decodes a diffuse texture again and paints it. `None` when its alpha mask
/// protects every pixel, i.e. the variant would duplicate its source.
fn paint_one(
    variant: &crate::paint::Variant,
    blob: &[u8],
    origin: TextureOrigin,
    options: &TextureOptions,
) -> Result<Option<PreparedTexture>, String> {
    let source_bytes = blob.len();
    let decoded = decode(blob).map_err(|e| format!("decode failed: {e}"))?;
    let mut resized = downscale(decoded, TextureRole::Color.max_size(options));
    if !crate::paint::apply(&mut resized, variant.factor) {
        return Ok(None);
    }
    let (bytes, mime) = encode(&resized, TextureRole::Color, options)?;

    Ok(Some(PreparedTexture {
        name: variant.name.clone(),
        mime,
        bytes,
        width: resized.width(),
        height: resized.height(),
        role: TextureRole::Color,
        origin,
        source_bytes,
        // `paint::apply` consumes the mask and leaves the image opaque.
        has_alpha: false,
        alpha_varies: false,
        average: average_color(&resized),
    }))
}

/// Skin file first, embedded blob second (§4.3).
fn load_source(name: &str, skin_dir: Option<&Path>, embedded: Option<&[u8]>) -> Option<(Vec<u8>, TextureOrigin)> {
    if let Some(dir) = skin_dir {
        let path = dir.join(name);
        match std::fs::read(&path) {
            Ok(bytes) => return Some((bytes, TextureOrigin::Skin(path))),
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                log::warn!("kn5-gltf: skin override {} unreadable: {e}", path.display());
            }
            Err(_) => {}
        }
    }
    embedded.map(|bytes| (bytes.to_vec(), TextureOrigin::Embedded))
}

fn prepare_one(
    name: &str,
    blob: &[u8],
    origin: TextureOrigin,
    usage: TextureUse,
    users: &[DiffuseUser],
    options: &TextureOptions,
) -> Result<(PreparedTexture, Vec<(usize, FootprintAlpha)>), String> {
    let source_bytes = blob.len();
    let decoded = decode(blob).map_err(|e| format!("decode failed: {e}"))?;
    let mut resized = downscale(decoded, usage.role.max_size(options));

    // Mesuré **avant** tout retrait d'alpha, et c'est tout l'objet de l'ordre
    // choisi ici : c'est cette mesure qui décide si l'alpha doit survivre.
    let verdicts: Vec<(usize, FootprintAlpha)> = users
        .iter()
        .map(|user| {
            let (min, max) = alpha_range_at(&resized, &user.points);
            (
                user.material,
                FootprintAlpha {
                    min,
                    max,
                    samples: if usage.keep_alpha { user.points.len() } else { 0 },
                },
            )
        })
        .collect();

    // **Un alpha dont personne ne découpe doit disparaître de l'image.**
    //
    // glTF multiplie `baseColorFactor.a` par l'alpha de la texture. Tant que
    // celui-ci reste dans le PNG, l'opacité qu'on calcule pour une vitre ne
    // s'ajoute pas : elle se **multiplie** à un alpha qui vaut déjà presque
    // zéro. Sur `ks_toyota_ae86_tuned`, `glass.dds` vaut 13/255 partout et le
    // matériau porte 0,15 — soit 0,76 % d'opacité finale, une vitre
    // rigoureusement invisible. C'est le défaut que l'écart n°9 croyait avoir
    // corrigé : le plancher d'opacité était bien posé, mais il ne pouvait rien
    // tant que la texture continuait de multiplier par-dessus.
    //
    // On ne retire donc l'alpha que si **aucun** de ses utilisateurs n'en fait
    // une découpe : un matériau alpha-testé (grille, jante ajourée) en vit,
    // et un matériau en fondu dont l'alpha varie dans son empreinte découpe
    // vraiment (décalcomanie, autocollant).
    let cuts_out = users.iter().any(|user| match user.alpha_mode {
        crate::material::AlphaMode::Mask => true,
        crate::material::AlphaMode::Blend => verdicts
            .iter()
            .find(|(index, _)| *index == user.material)
            .is_some_and(|(_, footprint)| footprint.min != footprint.max),
        crate::material::AlphaMode::Opaque => false,
    });
    let keep_alpha = usage.keep_alpha && cuts_out;
    if !keep_alpha {
        strip_alpha(&mut resized);
    }
    let (bytes, mime) = encode(&resized, usage.role, options)?;

    Ok((
        PreparedTexture {
            name: name.to_string(),
            mime,
            bytes,
            width: resized.width(),
            height: resized.height(),
            role: usage.role,
            origin,
            source_bytes,
            has_alpha: keep_alpha && resized.pixels().any(|p| p.0[3] != u8::MAX),
            alpha_varies: keep_alpha && alpha_varies(&resized),
            average: average_color(&resized),
        },
        verdicts,
    ))
}

/// L'alpha de cette image varie-t-il d'un pixel à l'autre ?
fn alpha_varies(image: &RgbaImage) -> bool {
    let mut pixels = image.pixels();
    let Some(first) = pixels.next().map(|p| p.0[3]) else {
        return false;
    };
    pixels.any(|p| p.0[3] != first)
}

/// Couleur moyenne d'une image, canaux dans [0,1].
fn average_color(image: &RgbaImage) -> [f32; 3] {
    let mut sum = [0u64; 3];
    for pixel in image.pixels() {
        for (channel, value) in sum.iter_mut().zip(pixel.0.iter()) {
            *channel += *value as u64;
        }
    }
    let count = (image.pixels().len() as u64).max(1);
    [
        (sum[0] / count) as f32 / 255.0,
        (sum[1] / count) as f32 / 255.0,
        (sum[2] / count) as f32 / 255.0,
    ]
}

/// Force le canal alpha à l'opacité totale.
///
/// **Le canal alpha d'une texture diffuse d'Assetto Corsa ne décrit
/// généralement pas une transparence.** Mesuré sur `abarth500` :
/// `SkinBase_DEFAULT.dds` a 82,5 % de ses pixels à alpha nul, alors que le RVB
/// sous ces pixels vaut [163, 159, 159] — la peinture de la carrosserie. Les
/// shaders d'AC y lisent autre chose (masque de spécularité selon les
/// matériaux). Conservé tel quel, le navigateur prémultiplie le RVB par cet
/// alpha à l'envoi vers le GPU : la carrosserie est effacée par son propre
/// canal alpha, et la voiture paraît transparente.
///
/// N'est donc appliqué qu'aux textures dont **aucun** matériau n'exploite
/// l'alpha (voir `TextureUse::keep_alpha`). Bénéfice secondaire : elles
/// repassent en JPEG, bien plus léger.
fn strip_alpha(image: &mut RgbaImage) {
    for pixel in image.pixels_mut() {
        pixel.0[3] = u8::MAX;
    }
}

/// Decodes whatever the blob actually is — the filename is never consulted
/// (§3.2, and one texture in a hundred contradicts its own extension).
pub(crate) fn decode(blob: &[u8]) -> Result<RgbaImage, String> {
    match ImageFormat::sniff(blob) {
        ImageFormat::Dds => {
            let dds = image_dds::ddsfile::Dds::read(blob).map_err(|e| format!("not a readable DDS: {e}"))?;
            // Mip 0 only: the preview downscales anyway, and the smaller mips
            // would just be thrown away.
            match image_dds::image_from_dds(&dds, 0) {
                Ok(image) => Ok(image),
                // `image_dds` only knows block-compressed and DXGI-tagged
                // surfaces. Roughly one AC texture in eight is instead a plain
                // uncompressed DDS whose layout is described only by channel
                // bit masks — a legacy form the crate reports as
                // `DdsFormatInfo { dxgi: None, d3d: None, fourcc: None }`.
                // Measured on twelve cars of the reference library: 117 of 938
                // textures, up to 26 % on `ks_ford_gt40`. Far too many to drop.
                Err(compressed_error) => decode_uncompressed_dds(&dds)
                    .map_err(|e| format!("unsupported DDS payload: {compressed_error}; as uncompressed: {e}")),
            }
        }
        ImageFormat::Png | ImageFormat::Jpeg => image::load_from_memory(blob)
            .map(|image| image.to_rgba8())
            .map_err(|e| e.to_string()),
        ImageFormat::Unknown => Err("unrecognised container".to_string()),
    }
}

/// Decodes an uncompressed DDS surface straight from its channel bit masks.
///
/// Covers every layout the `DDS_PIXELFORMAT` block can describe in one go —
/// A8R8G8B8, X8R8G8B8, R8G8B8, A1R5G5B5, R5G6B5, A4R4G4B4, L8, A8L8 and their
/// relatives — rather than enumerating named formats, because the masks *are*
/// the format. Anything with a FourCC (block compression) never reaches here:
/// `image_dds` owns those.
fn decode_uncompressed_dds(dds: &image_dds::ddsfile::Dds) -> Result<RgbaImage, String> {
    use image_dds::ddsfile::PixelFormatFlags;

    let spf = &dds.header.spf;
    if spf.fourcc.is_some() {
        return Err("block-compressed surface".to_string());
    }
    let bit_count = spf.rgb_bit_count.ok_or("no bit count in the pixel format")?;
    if !matches!(bit_count, 8 | 16 | 24 | 32) {
        return Err(format!("unsupported bit count {bit_count}"));
    }
    let bytes_per_pixel = (bit_count / 8) as usize;

    let width = dds.get_width();
    let height = dds.get_height();
    let data = dds.get_data(0).map_err(|e| e.to_string())?;

    // Rows can be padded when the header advertises a pitch; otherwise they
    // are packed tight. Trusting the header blindly would misread the many
    // files that leave `pitch` at zero.
    let tight_row = width as usize * bytes_per_pixel;
    let row_stride = match dds.header.pitch {
        Some(pitch) if pitch as usize >= tight_row => pitch as usize,
        _ => tight_row,
    };
    let needed = row_stride * height as usize;
    if data.len() < needed {
        return Err(format!("surface truncated: {} bytes for {needed} needed", data.len()));
    }

    let luminance = spf.flags.contains(PixelFormatFlags::LUMINANCE);
    let alpha_only = bit_count == 8 && spf.flags.contains(PixelFormatFlags::ALPHA) && !luminance;

    // A single-channel surface names its channel in `r_bit_mask` (luminance)
    // or `a_bit_mask` (alpha); either way the whole byte is the value.
    let r_mask = spf
        .r_bit_mask
        .unwrap_or(if luminance || !alpha_only { 0xFF } else { 0 });
    let g_mask = spf.g_bit_mask.unwrap_or(0);
    let b_mask = spf.b_bit_mask.unwrap_or(0);
    let a_mask = spf.a_bit_mask.unwrap_or(if alpha_only { 0xFF } else { 0 });

    let mut image = RgbaImage::new(width, height);
    for y in 0..height as usize {
        let row = &data[y * row_stride..];
        for x in 0..width as usize {
            let mut raw = 0u32;
            for (i, byte) in row[x * bytes_per_pixel..(x + 1) * bytes_per_pixel].iter().enumerate() {
                raw |= (*byte as u32) << (8 * i);
            }
            let r = channel(raw, r_mask).unwrap_or(0);
            let (g, b) = if luminance || (g_mask == 0 && b_mask == 0) {
                // Luminance replicates across RGB; an alpha-only surface has
                // no colour at all and stays white so that multiplying by it
                // is a no-op.
                let value = if alpha_only { u8::MAX } else { r };
                (value, value)
            } else {
                (channel(raw, g_mask).unwrap_or(0), channel(raw, b_mask).unwrap_or(0))
            };
            let r = if alpha_only { u8::MAX } else { r };
            let a = channel(raw, a_mask).unwrap_or(u8::MAX);
            image.put_pixel(x as u32, y as u32, image::Rgba([r, g, b, a]));
        }
    }
    Ok(image)
}

/// Extracts one channel from a packed pixel and stretches it to 8 bits.
///
/// The stretch matters: a 5-bit channel scaled by a plain `<< 3` tops out at
/// 248 instead of 255, so a white surface comes out visibly grey.
fn channel(raw: u32, mask: u32) -> Option<u8> {
    if mask == 0 {
        return None;
    }
    let shift = mask.trailing_zeros();
    let width = mask.count_ones();
    let max = (1u32 << width) - 1;
    let value = (raw & mask) >> shift;
    Some(((value * 255 + max / 2) / max) as u8)
}

/// Shrinks to fit `max_size` on its longest side, preserving the aspect ratio.
/// Never enlarges: a 64×64 badge stays 64×64.
fn downscale(image: RgbaImage, max_size: u32) -> RgbaImage {
    let (width, height) = (image.width(), image.height());
    let longest = width.max(height);
    if longest <= max_size || longest == 0 {
        return image;
    }
    let scale = max_size as f32 / longest as f32;
    let target_w = ((width as f32 * scale).round() as u32).max(1);
    let target_h = ((height as f32 * scale).round() as u32).max(1);
    // Lanczos3 on a downscale keeps livery lettering readable where a nearest
    // or triangle filter turns it to mush.
    image::imageops::resize(&image, target_w, target_h, image::imageops::FilterType::Lanczos3)
}

fn encode(image: &RgbaImage, role: TextureRole, options: &TextureOptions) -> Result<(Vec<u8>, &'static str), String> {
    // Alpha decides before the role does. A colour map with transparency is
    // exactly what feeds `ksAlphaRef` masking — grilles, ajoured rims — and
    // JPEG would silently drop that channel, filling every opening (§10,
    // "`ksAlphaRef` ignoré").
    let has_alpha = image.pixels().any(|p| p.0[3] != u8::MAX);
    if role == TextureRole::Color && !has_alpha {
        let mut bytes = Vec::new();
        let rgb = image::DynamicImage::ImageRgba8(image.clone()).to_rgb8();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, options.jpeg_quality)
            .encode(rgb.as_raw(), rgb.width(), rgb.height(), image::ExtendedColorType::Rgb8)
            .map_err(|e| format!("jpeg encoding failed: {e}"))?;
        return Ok((bytes, "image/jpeg"));
    }

    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new_with_quality(
        &mut bytes,
        image::codecs::png::CompressionType::Default,
        image::codecs::png::FilterType::Adaptive,
    )
    .write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        image::ExtendedColorType::Rgba8,
    )
    .map_err(|e| format!("png encoding failed: {e}"))?;
    Ok((bytes, "image/png"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, pixel: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(width, height, image::Rgba(pixel))
    }

    // Rule: only shrink, never grow — a small badge must not be blown up to
    // the cap and inflate the payload for nothing.
    #[test]
    fn downscale_only_shrinks() {
        let small = downscale(solid(64, 64, [255; 4]), 1024);
        assert_eq!((small.width(), small.height()), (64, 64), "small image untouched");

        let large = downscale(solid(4096, 2048, [255; 4]), 1024);
        assert_eq!(
            (large.width(), large.height()),
            (1024, 512),
            "longest side capped, aspect ratio preserved"
        );
    }

    // Rule: transparency survives encoding. This is the guard behind alpha
    // masking — a grille whose alpha is dropped renders as a solid panel.
    #[test]
    fn colour_texture_with_alpha_stays_png() {
        let mut image = solid(8, 8, [255; 4]);
        image.put_pixel(0, 0, image::Rgba([255, 0, 0, 0]));
        let options = TextureOptions::default();

        let (_, mime) = encode(&image, TextureRole::Color, &options).expect("encodes");
        assert_eq!(mime, "image/png", "alpha forces a lossless container");

        let (_, opaque_mime) = encode(&solid(8, 8, [255; 4]), TextureRole::Color, &options).expect("encodes");
        assert_eq!(opaque_mime, "image/jpeg", "fully opaque colour map may be lossy");
    }

    // Rule: normal maps and masks are never lossy, whatever their alpha.
    #[test]
    fn data_textures_are_always_lossless() {
        let (_, mime) = encode(
            &solid(8, 8, [128, 128, 255, 255]),
            TextureRole::Data,
            &TextureOptions::default(),
        )
        .expect("encodes");
        assert_eq!(mime, "image/png", "a normal map must not go through JPEG");
    }

    // Rule: the role comes from the sampler slot, and a texture used as a
    // colour map anywhere is colour everywhere.
    #[test]
    fn roles_follow_sampler_slots() {
        use kn5::{Kn5Material, Kn5Sampler};

        let sampler = |name: &str, texture: &str| Kn5Sampler {
            name: name.to_string(),
            slot: 0,
            texture: texture.to_string(),
        };
        let material = |samplers: Vec<Kn5Sampler>| Kn5Material {
            name: "m".to_string(),
            shader: "ksPerPixel".to_string(),
            blend_mode: 0,
            alpha_tested: false,
            reserved: 0,
            properties: Vec::new(),
            samplers,
        };

        let model = Kn5Model {
            version: 6,
            extra: None,
            textures: Vec::new(),
            materials: vec![
                material(vec![
                    sampler("txNormal", "shared.dds"),
                    sampler("txDiffuse", "body.dds"),
                ]),
                material(vec![sampler("txDiffuse", "shared.dds")]),
            ],
            root: kn5::Kn5Node {
                name: "root".to_string(),
                active: true,
                kind: kn5::Kn5NodeKind::Dummy { transform: [0.0; 16] },
                children: Vec::new(),
            },
        };

        let roles = roles(&model);
        assert_eq!(
            roles.get("body.dds").map(|u| u.role),
            Some(TextureRole::Color),
            "txDiffuse is colour"
        );
        assert_eq!(
            roles.get("shared.dds").map(|u| u.role),
            Some(TextureRole::Color),
            "colour wins over data when a texture is bound to both"
        );
        // La texture liée aussi à `txNormal` garde son alpha : une carte de
        // données y encode une valeur, pas une découpe.
        assert_eq!(
            roles.get("shared.dds").map(|u| u.keep_alpha),
            Some(true),
            "un usage en carte de données impose de garder l'alpha"
        );
        assert_eq!(
            roles.get("body.dds").map(|u| u.keep_alpha),
            Some(false),
            "diffuse d'un matériau opaque : l'alpha n'est pas une transparence"
        );
    }

    // Rule: a channel narrower than 8 bits is stretched, not shifted. A plain
    // `<< 3` on a 5-bit channel tops out at 248, so white surfaces come out
    // visibly grey — the kind of bug nobody spots until a car looks dull.
    #[test]
    fn narrow_channels_stretch_to_full_range() {
        assert_eq!(channel(0b11111, 0b11111), Some(255), "5 bits full scale reaches white");
        assert_eq!(channel(0, 0b11111), Some(0), "zero stays zero");
        assert_eq!(channel(0xFF00, 0xFF00), Some(255), "shifted 8-bit channel");
        assert_eq!(channel(0x7F, 0xFF), Some(127), "8-bit channel passes through");
        assert_eq!(channel(0xFFFF, 0), None, "an absent channel has no value");
    }

    /// Minimal uncompressed DDS, built byte by byte: 4-byte magic then the
    /// 124-byte header, whose `DDS_PIXELFORMAT` block describes the layout
    /// through channel masks alone.
    fn uncompressed_dds(width: u32, height: u32, masks: [u32; 4], bit_count: u32, pixels: &[u8]) -> Vec<u8> {
        let mut header = Vec::new();
        header.extend_from_slice(b"DDS ");
        let mut word = |v: u32| header.extend_from_slice(&v.to_le_bytes());
        word(124); // header size
        word(0x1007); // CAPS | HEIGHT | WIDTH | PIXELFORMAT
        word(height);
        word(width);
        word(width * bit_count / 8); // pitch
        word(0); // depth
        word(1); // mip count
        for _ in 0..11 {
            word(0); // reserved
        }
        word(32); // pixel format size
        word(0x41); // ALPHAPIXELS | RGB
        word(0); // fourcc: none, which is what sends us down the mask path
        word(bit_count);
        for mask in masks {
            word(mask);
        }
        word(0x1000); // caps: TEXTURE
        for _ in 0..4 {
            word(0);
        }
        header.extend_from_slice(pixels);
        header
    }

    // Rule: an uncompressed DDS described only by channel masks decodes to the
    // right colours. Roughly one AC texture in eight takes this path, and
    // `image_dds` refuses all of them.
    #[test]
    fn uncompressed_dds_decodes_from_channel_masks() {
        // A8R8G8B8, two pixels: opaque red, then half-transparent blue.
        let pixels = [0x00, 0x00, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x80];
        let blob = uncompressed_dds(2, 1, [0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0xFF00_0000], 32, &pixels);

        let image = decode(&blob).expect("mask-described DDS decodes");
        assert_eq!((image.width(), image.height()), (2, 1), "dimensions from the header");
        assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0, 255], "opaque red");
        assert_eq!(image.get_pixel(1, 0).0, [0, 0, 255, 128], "blue at half alpha");
    }

    // Rule: a blob that decodes to nothing usable is a warning, not a failure
    // of the whole car (§6.3).
    #[test]
    fn undecodable_blob_is_reported_not_fatal() {
        let error = prepare_one(
            "broken.dds",
            b"not an image at all",
            TextureOrigin::Embedded,
            TextureUse {
                role: TextureRole::Color,
                keep_alpha: false,
            },
            &[],
            &TextureOptions::default(),
        )
        .expect_err("garbage must not decode");
        assert!(error.contains("decode failed"), "reason names the failing stage");
    }

    /// A PNG blob of `image`, the shape `prepare_one` expects to be handed.
    fn png(image: &RgbaImage) -> Vec<u8> {
        encode(image, TextureRole::Color, &TextureOptions::default())
            .expect("encoding a test image")
            .0
    }

    fn user(material: usize, alpha_mode: crate::material::AlphaMode, points: Vec<[f32; 2]>) -> DiffuseUser {
        DiffuseUser {
            material,
            alpha_mode,
            points,
        }
    }

    fn prepared(image: &RgbaImage, users: Vec<DiffuseUser>) -> PreparedTexture {
        prepare_one(
            "t.dds",
            &png(image),
            TextureOrigin::Embedded,
            TextureUse {
                role: TextureRole::Color,
                keep_alpha: true,
            },
            &users,
            &TextureOptions::default(),
        )
        .expect("a valid PNG must prepare")
        .0
    }

    // Règle : un alpha dont personne ne découpe est retiré de l'image, sinon
    // il **multiplie** l'opacité calculée pour le matériau au lieu de lui
    // céder la place. Bug réel sur `ks_toyota_ae86_tuned` : `glass.dds` vaut
    // 13/255 partout et le matériau porte 0,15, soit 0,76 % d'opacité finale
    // — une vitre rigoureusement invisible. C'est ce que l'écart n°9 croyait
    // avoir corrigé en posant le plancher d'opacité.
    #[test]
    fn an_alpha_nobody_cuts_with_is_dropped_so_the_shader_opacity_can_apply() {
        let glass = solid(8, 8, [200, 200, 200, 13]);
        let texture = prepared(
            &glass,
            vec![user(0, crate::material::AlphaMode::Blend, vec![[0.5, 0.5]])],
        );
        assert!(
            !texture.has_alpha,
            "l'alpha constant d'une vitre ne doit pas survivre à l'encodage"
        );
        assert_eq!(texture.mime, "image/jpeg", "sans alpha, le codec de couleur suffit");
    }

    // Règle : une découpe, elle, survit. C'est le garde-fou de la grille et de
    // la jante ajourée — leur alpha retiré, elles deviennent des panneaux
    // pleins.
    #[test]
    fn a_real_cutout_keeps_its_alpha() {
        let mut grille = solid(8, 8, [255; 4]);
        grille.put_pixel(2, 2, image::Rgba([255, 255, 255, 0]));

        let masked = prepared(
            &grille,
            vec![user(0, crate::material::AlphaMode::Mask, vec![[0.5, 0.5]])],
        );
        assert!(masked.has_alpha, "un matériau alpha-testé vit de son alpha");

        // En fondu, c'est la mesure de l'empreinte qui tranche : l'alpha varie
        // là où ce matériau échantillonne, donc il découpe vraiment.
        let decal = prepared(
            &grille,
            // Un point sur le trou (pixel 2,2), un point sur la matière.
            vec![user(0, crate::material::AlphaMode::Blend, vec![[0.3, 0.3], [0.9, 0.9]])],
        );
        assert!(decal.has_alpha, "un alpha qui varie dans l'empreinte est une découpe");
    }

    // Règle : l'empreinte se mesure là où le matériau regarde, pas sur l'atlas
    // entier. Bug réel sur `vrc_erc_1999_renoir_csp`, dont le pare-brise
    // partage l'atlas de carrosserie : celui-ci porte un masque de peinture
    // (écart n°5), donc son alpha varie forcément quelque part.
    #[test]
    fn the_footprint_is_measured_where_the_material_samples() {
        let mut atlas = solid(8, 8, [255; 4]);
        // Moitié gauche opaque (les décalcomanies), moitié droite à zéro (la
        // zone que le shader repeindra).
        for y in 0..8 {
            for x in 4..8 {
                atlas.put_pixel(x, y, image::Rgba([255, 255, 255, 0]));
            }
        }
        let texture = prepared(
            &atlas,
            // Un seul matériau, qui n'échantillonne que la moitié droite.
            vec![user(
                0,
                crate::material::AlphaMode::Blend,
                vec![[0.7, 0.2], [0.8, 0.5], [0.9, 0.8]],
            )],
        );
        assert!(
            !texture.has_alpha,
            "uniforme dans son empreinte, donc pas une découpe — même si l'atlas varie ailleurs"
        );
    }

    // Règle : deux matériaux se partagent une image, l'un découpe et l'autre
    // non — l'alpha reste, parce qu'un seul utilisateur suffit à en avoir
    // besoin. C'est le sens de « on ne retire que si personne ne découpe ».
    #[test]
    fn one_user_that_cuts_is_enough_to_keep_the_alpha_for_everyone() {
        let mut atlas = solid(8, 8, [255; 4]);
        for y in 0..8 {
            for x in 4..8 {
                atlas.put_pixel(x, y, image::Rgba([255, 255, 255, 0]));
            }
        }
        let texture = prepared(
            &atlas,
            vec![
                // La vitre, uniforme dans son coin.
                user(0, crate::material::AlphaMode::Blend, vec![[0.7, 0.5]]),
                // La décalcomanie, qui traverse la frontière.
                user(1, crate::material::AlphaMode::Blend, vec![[0.1, 0.5], [0.7, 0.5]]),
            ],
        );
        assert!(texture.has_alpha, "la découpe de l'un protège l'alpha de l'image");
    }
}
