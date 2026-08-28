//! CSP model replacements — `extension/ext_config.ini` (docs/kn5-format.md).
//!
//! Many tuning mods ship a KN5 that is **deliberately incomplete**: the parts
//! that differ from one skin to the next are kept in separate KN5 files and
//! grafted onto the tree at load time by Custom Shaders Patch. Rendering the
//! main file alone then shows a car with holes in it, which is not a bug in
//! the parser — the geometry is simply somewhere else.
//!
//! Measured on `ks_toyota_ae86_tuned` + its tuning layer: the `WHEEL_*` nodes
//! hold the tyre and nothing else (no rim at all), the front light bar lives
//! in `extension/TOYOTA_HALOGEN.kn5`, and the rims live in the *skin* folder,
//! one wheel per file, instanced four times.
//!
//! # What this module implements, and what it deliberately does not
//!
//! `ext_config.ini` is not an INI file: it is a template language with
//! expressions (`$" ... "`), generators, and includes that reach into
//! `assettocorsa/extension/config/cars/common/`. Implementing it is out of
//! the question. But every template ultimately expands to one primitive —
//! `[MODEL_REPLACEMENT_*]` — and that primitive is small. So:
//!
//! - literal `[MODEL_REPLACEMENT_*]` sections are honoured;
//! - `[ReplaceRims]`, by far the most common template and the one that costs
//!   a car its wheels, is expanded by hand in [`expand_replace_rims`];
//! - everything else (material and shader overrides, lights, animations) is
//!   ignored. Those change how a surface *looks*, not whether it exists.
//!
//! # Wildcards
//!
//! `?` stands for **any run of characters, including none** — not a single
//! character. The evidence is local and decisive: the AE86 config filters on
//! `SKINS = ?07_topaz?` and the skin folder is named exactly `07_topaz`, so
//! the pattern has to be able to match nothing on both sides. The CSP wiki
//! only shows examples (`RIM_?`, `red?`) without stating the rule.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use kn5::{Kn5Model, Kn5Node, Kn5NodeKind};

/// Name given to every wrapper node this module grafts in.
///
/// Not decoration: the insertion walks skip anything carrying it, so a pattern
/// can never match *inside* a part that was just inserted and send the walk
/// round in circles.
const GRAFT_MARKER: &str = "CSP_INSERT";

/// Materials CSP turns into **physical glass**, with the index of refraction
/// that drives their reflectance.
///
/// `[Material_Glass]` is not a tweak of the stock shader: it swaps in `smGlass`,
/// a PBR glass whose transparency comes from an IOR and a thickness, never from
/// an alpha channel. Reading the mod's own declaration is therefore the only
/// way to know that a material is a windowpane rather than a translucent
/// sticker — and it is the author of CSP who wrote the rule, in
/// `<AC>/extension/config/cars/common/materials_glass.ini`.
#[derive(Debug, Clone, Default)]
pub struct GlassOverrides {
    /// Material name patterns (CSP wildcards) → IOR.
    materials: Vec<(String, f32)>,
    /// Mesh name patterns → IOR. Rarer, but `ks_toyota_ae86_tuned` uses it.
    meshes: Vec<(String, f32)>,
}

impl GlassOverrides {
    pub fn is_empty(&self) -> bool {
        self.materials.is_empty() && self.meshes.is_empty()
    }

    /// Resolves the declarations against a model: material index → IOR.
    ///
    /// Mesh-targeted declarations are folded in here, where the node tree is
    /// available to say which material a mesh actually carries.
    pub fn resolve(&self, model: &Kn5Model) -> BTreeMap<usize, f32> {
        let mut out = BTreeMap::new();
        for (index, material) in model.materials.iter().enumerate() {
            if let Some((_, ior)) = self.materials.iter().find(|(p, _)| glob_match(p, &material.name)) {
                out.insert(index, *ior);
            }
        }
        if !self.meshes.is_empty() {
            model.visit_nodes(&mut |node| {
                let Some(mesh) = node.mesh() else { return };
                if let Some((_, ior)) = self.meshes.iter().find(|(p, _)| glob_match(p, &node.name)) {
                    out.insert(mesh.material_id as usize, *ior);
                }
            });
        }
        out
    }
}

/// Indice de réfraction du verre par défaut, tel que `materials_glass.ini` le
/// fixe. `FilmIOR` le remplace quand le mod veut plus de reflet.
const DEFAULT_GLASS_IOR: f32 = 1.5;

/// Reads the glass declarations of a car — `[Material_Glass]` and its variants,
/// plus the `ExteriorGlass*` shorthands that `materials_glass.ini` defines.
pub fn glass_overrides(car_dir: &Path, skin_dir: Option<&Path>) -> GlassOverrides {
    let mut out = GlassOverrides::default();
    let mut sources: Vec<PathBuf> = vec![car_dir.join("extension").join("ext_config.ini")];
    if let Some(skin) = skin_dir {
        sources.push(skin.join("ext_config.ini"));
    }
    // Les mods rangent souvent leurs matériaux dans un fichier à part, tiré par
    // un `[INCLUDE: materials.ini]` — que l'on ne suit pas, mais dont le nom est
    // assez stable pour être lu directement.
    sources.push(car_dir.join("extension").join("materials.ini"));

    for source in sources {
        let Ok(text) = std::fs::read_to_string(&source) else {
            continue;
        };
        collect_glass(&text, &mut out);
    }
    out
}

/// Les clés raccourcies de `materials_glass.ini`, qui évitent au moddeur
/// d'écrire une section entière. Chacune a sa variante `…Meshes`.
const GLASS_SHORTHANDS: [&str; 5] = [
    "ExteriorGlassMaterials",
    "ExteriorGlassTintedMaterials",
    "ExteriorGlassFilmedMaterials",
    "ExteriorGlassHeadlightsMaterials",
    "ExteriorGlassPhotoelasticMaterials",
];

fn collect_glass(text: &str, out: &mut GlassOverrides) {
    for section in parse_sections(text) {
        // `Material_Glass`, `Material_GlassSide`, `Material_MultiEmissiveGlass`,
        // `Material_PhotoelasticGlass` — tous héritent du même `smGlass`.
        let is_glass_template = section.name.starts_with("Material_") && section.name.contains("Glass");
        let ior = section
            .get("FilmIOR")
            .or_else(|| section.get("IOR"))
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| *v > 1.0)
            .unwrap_or(DEFAULT_GLASS_IOR);

        if is_glass_template {
            for name in section.list("Materials").unwrap_or_default() {
                out.materials.push((name, ior));
            }
            for name in section.list("Meshes").unwrap_or_default() {
                out.meshes.push((name, ior));
            }
        }

        // Les raccourcis peuvent apparaître dans n'importe quelle section — ils
        // sont posés juste sous le `[INCLUDE: common/materials_glass.ini]`.
        for shorthand in GLASS_SHORTHANDS {
            for name in section.list(shorthand).unwrap_or_default() {
                out.materials.push((name, DEFAULT_GLASS_IOR));
            }
            let meshes = shorthand.replace("Materials", "Meshes");
            for name in section.list(&meshes).unwrap_or_default() {
                out.meshes.push((name, DEFAULT_GLASS_IOR));
            }
        }
    }
}

/// One resolved replacement, after templates have been expanded and the
/// skin/file filters have already been applied.
#[derive(Debug, Clone, Default)]
pub struct Replacement {
    /// Node or mesh name patterns to drop from the tree.
    pub hide: Vec<String>,
    /// KN5 to graft in, already resolved to an absolute path.
    pub insert: Option<PathBuf>,
    /// Names of the nodes to insert *into*, as their last child.
    pub insert_in: Vec<String>,
    /// Names of the nodes to insert *after*, as a following sibling.
    pub insert_after: Vec<String>,
    /// Insert at every match rather than only the first one.
    pub multiple: bool,
    /// Metres, along X (left-right), Y (up) and Z (forward).
    pub offset: [f32; 3],
    /// Degrees — **heading, pitch, roll**, i.e. around Y, X and Z. Not XYZ:
    /// reading it as XYZ turns a wheel upside down instead of mirroring it.
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

/// What the pass actually did, for logging and for `kn5-tool`.
#[derive(Debug, Default)]
pub struct ExtConfigStats {
    /// Replacements that survived the `FILE` and `SKINS` filters.
    pub applied: usize,
    pub hidden_nodes: usize,
    pub inserted_models: usize,
    pub inserted_triangles: usize,
    /// Non-fatal problems: a missing KN5, an anchor node that does not exist.
    /// Never a reason to fail the whole preview — a car with one part missing
    /// still beats no preview at all.
    pub failures: Vec<String>,
}

/// Applies the CSP model replacements of a car to a freshly parsed model.
///
/// `model_path` is the KN5 that was actually parsed: replacements name their
/// target in `FILE`, and a section aimed at `..._LOD_B.kn5` must not fire on
/// the main model.
pub fn apply_ext_config(
    model: &mut Kn5Model,
    car_dir: &Path,
    model_path: &Path,
    skin_dir: Option<&Path>,
) -> ExtConfigStats {
    let mut stats = ExtConfigStats::default();
    let model_file = file_name_of(model_path);
    let skin_id = skin_dir.map(file_name_of).unwrap_or_default();

    let mut sources: Vec<PathBuf> = vec![car_dir.join("extension").join("ext_config.ini")];
    // The skin's own config comes second on purpose: it is the more specific
    // of the two, and `[ReplaceRims]` lives there.
    if let Some(skin) = skin_dir {
        sources.push(skin.join("ext_config.ini"));
    }

    for source in sources {
        let Ok(text) = std::fs::read_to_string(&source) else {
            continue;
        };
        let dir = source.parent().unwrap_or(car_dir).to_path_buf();
        for replacement in replacements_of(&text, &dir, &model_file, &skin_id) {
            stats.applied += 1;
            apply_one(model, &replacement, &mut stats);
        }
    }
    stats
}

/// Parses one config and returns the replacements that concern this model and
/// this skin, templates expanded.
fn replacements_of(text: &str, dir: &Path, model_file: &str, skin_id: &str) -> Vec<Replacement> {
    let mut out = Vec::new();
    for section in parse_sections(text) {
        let expanded = if section.name.eq_ignore_ascii_case("ReplaceRims") {
            expand_replace_rims(&section)
        } else if section.name.to_ascii_uppercase().starts_with("MODEL_REPLACEMENT") {
            vec![section.clone()]
        } else {
            continue;
        };
        for section in expanded {
            if !section_applies(&section, model_file, skin_id) {
                continue;
            }
            out.push(section.to_replacement(dir));
        }
    }
    out
}

/// `ACTIVE`, `FILE` and `SKINS` — the three filters, all optional and all
/// meaning "everything" when absent.
///
/// `ACTIVE` is sometimes an expression rather than a flag —
/// `vrc_erc_1999_renoir_csp` gates a section on
/// `$" read('csp/version', 0) >= 2261 "`. Only a literal `0` disables a
/// section here: anything we cannot evaluate is treated as enabled, which is
/// what those version guards evaluate to on any current CSP anyway.
fn section_applies(section: &Section, model_file: &str, skin_id: &str) -> bool {
    if section.get("ACTIVE").is_some_and(|v| v.trim() == "0") {
        return false;
    }
    if let Some(files) = section.list("FILE") {
        if !files.iter().any(|pattern| glob_match(pattern, model_file)) {
            return false;
        }
    }
    if let Some(skins) = section.list("SKINS") {
        if !skins.iter().any(|pattern| glob_match(pattern, skin_id)) {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// `[ReplaceRims]` — the one template expanded by hand
// ---------------------------------------------------------------------------

/// Expands `[ReplaceRims]` into the `MODEL_REPLACEMENT` sections CSP would
/// generate: one that hides the original rims, then one insertion per corner.
///
/// Transcribed from `extension/config/cars/common/custom_rims.ini`, whose
/// generator produces, for each side and each of the two wheels:
///
/// ```text
/// INSERT_IN = 'WHEEL_' .. (left and 'L' or 'R') .. (front and 'F' or 'R')
/// SCALE     = width / model_width, radius / model_radius, radius / model_radius
/// ROTATION  = (right and 180 or 0), 0, 0
/// OFFSET    = (right and -offset or offset), 0, 0
/// ```
///
/// **The one thing we cannot reproduce** is where the target radius and width
/// come from: CSP reads them out of `data/tyres.ini`, which on a packed car
/// lives inside the encrypted `data.acd` that this project deliberately does
/// not decrypt (SPEC-preview-3d-kn5 §4.2). Absent an explicit `Radius`/`Width`
/// in the section, we fall back to the dimensions the rim model declares for
/// itself, i.e. a scale of exactly 1. That is the right answer whenever the
/// modder sized the rim for this car, which is the normal case — the AE86's
/// Watanabe is authored at 0.195 m against a 15" rim radius of 0.1905, a 2 %
/// difference nobody can see on a preview.
fn expand_replace_rims(section: &Section) -> Vec<Section> {
    let Some(model) = section.list("Model") else {
        return Vec::new();
    };
    let Some(rim_file) = model.first().cloned() else {
        return Vec::new();
    };
    // `Model = file.kn5, radius, width`. Missing numbers mean "no scaling",
    // which is also what the fallback below settles on.
    let model_radius = model.get(1).and_then(|v| v.parse::<f32>().ok());
    let model_width = model.get(2).and_then(|v| v.parse::<f32>().ok());

    let front_only = section.get("FrontOnly").is_some_and(|v| v.trim() == "1");
    let rear_only = section.get("RearOnly").is_some_and(|v| v.trim() == "1");

    let mut out = Vec::new();

    // The hiding half of the template. `OriginalRims` is the documented key;
    // several mods write `HIDE` next to it as well, so both are honoured.
    let mut hidden = section.list("OriginalRims").unwrap_or_default();
    hidden.extend(section.list("HIDE").unwrap_or_default());
    if !hidden.is_empty() {
        let mut hide_section = section.filters_only();
        hide_section.entries.push(("HIDE".to_string(), hidden.join(",")));
        out.push(hide_section);
    }

    for front in [true, false] {
        if (front && rear_only) || (!front && front_only) {
            continue;
        }
        // `Offset`, `Radius` and `Width` take one value for both axles or two,
        // front then rear.
        let offset = section.axle_value("Offset", front).unwrap_or(0.0);
        let radius = section.axle_value("Radius", front).or(model_radius);
        let width = section.axle_value("Width", front).or(model_width);
        let scale_y = ratio(radius, model_radius);
        let scale_x = ratio(width, model_width);

        for left in [true, false] {
            let corner = format!(
                "WHEEL_{}{}",
                if left { "L" } else { "R" },
                if front { "F" } else { "R" }
            );
            let mut insert = section.filters_only();
            insert.entries.push(("INSERT".to_string(), rim_file.clone()));
            insert.entries.push(("INSERT_IN".to_string(), corner));
            insert
                .entries
                .push(("SCALE".to_string(), format!("{scale_x},{scale_y},{scale_y}")));
            // Heading 180° on the right-hand side: the rim is modelled once,
            // for the left, and mirrored around the vertical axis.
            let rotation = if left { "0,0,0" } else { "180,0,0" };
            insert.entries.push(("ROTATION".to_string(), rotation.to_string()));
            let x = if left { offset } else { -offset };
            insert.entries.push(("OFFSET".to_string(), format!("{x},0,0")));
            out.push(insert);
        }
    }
    out
}

/// Scale factor for one axis, defaulting to 1 whenever either end of the
/// ratio is unknown — never to 0, which would collapse the part.
fn ratio(target: Option<f32>, source: Option<f32>) -> f32 {
    match (target, source) {
        (Some(t), Some(s)) if s.abs() > f32::EPSILON && t.abs() > f32::EPSILON => t / s,
        _ => 1.0,
    }
}

// ---------------------------------------------------------------------------
// Applying one replacement to the tree
// ---------------------------------------------------------------------------

fn apply_one(model: &mut Kn5Model, replacement: &Replacement, stats: &mut ExtConfigStats) {
    for pattern in &replacement.hide {
        stats.hidden_nodes += hide_matching(&mut model.root, pattern);
    }

    let Some(path) = &replacement.insert else {
        return;
    };
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) => {
            stats.failures.push(format!("{} : {e}", path.display()));
            return;
        }
    };
    let inserted = match kn5::parse(&bytes) {
        Ok(inserted) => inserted,
        Err(e) => {
            stats.failures.push(format!("{} : {e}", path.display()));
            return;
        }
    };

    let triangles = inserted.triangle_count();
    let graft = merge_assets(model, inserted, replacement);
    let anchored = anchor(&mut model.root, graft, replacement);
    if anchored == 0 {
        stats.failures.push(format!(
            "{} : no anchor node found ({:?} / {:?})",
            path.display(),
            replacement.insert_in,
            replacement.insert_after
        ));
        return;
    }
    stats.inserted_models += anchored;
    stats.inserted_triangles += triangles * anchored;
}

/// Moves the inserted model's textures and materials into the host, rewrites
/// the grafted subtree's material indices, and wraps it in a dummy carrying
/// the section's offset/rotation/scale.
///
/// Textures are keyed **by name** in a KN5, so two files can disagree about
/// what `black.dds` contains. An identical blob is shared; a genuine clash is
/// renamed and the inserted materials are pointed at the new name. Renaming
/// costs that one texture its skin override, which is a far smaller price than
/// showing the wrong image.
fn merge_assets(host: &mut Kn5Model, mut inserted: Kn5Model, replacement: &Replacement) -> Kn5Node {
    let mut renamed: HashMap<String, String> = HashMap::new();
    for texture in inserted.textures.drain(..) {
        if !texture.has_data() {
            continue;
        }
        match host.textures.iter().find(|t| t.has_data() && t.name == texture.name) {
            Some(existing) if existing.data == texture.data => continue,
            Some(_) => {
                let fresh = unique_name(&texture.name, &host.textures);
                renamed.insert(texture.name.clone(), fresh.clone());
                host.textures.push(kn5::Kn5Texture {
                    kind: texture.kind,
                    name: fresh,
                    data: texture.data,
                });
            }
            None => host.textures.push(texture),
        }
    }

    let material_base = host.materials.len() as u32;
    for mut material in inserted.materials.drain(..) {
        for sampler in &mut material.samplers {
            if let Some(fresh) = renamed.get(&sampler.texture) {
                sampler.texture = fresh.clone();
            }
        }
        host.materials.push(material);
    }

    let mut root = inserted.root;
    shift_materials(&mut root, material_base);

    Kn5Node {
        name: GRAFT_MARKER.to_string(),
        active: true,
        kind: Kn5NodeKind::Dummy {
            transform: compose(replacement.scale, replacement.rotation, replacement.offset),
        },
        children: vec![root],
    }
}

fn unique_name(name: &str, existing: &[kn5::Kn5Texture]) -> String {
    for n in 1u32.. {
        let candidate = format!("csp{n}__{name}");
        if !existing.iter().any(|t| t.name == candidate) {
            return candidate;
        }
    }
    unreachable!("u32 exhausted while renaming a texture")
}

fn shift_materials(node: &mut Kn5Node, base: u32) {
    match &mut node.kind {
        Kn5NodeKind::Mesh(mesh) => mesh.material_id += base,
        Kn5NodeKind::SkinnedMesh(skinned) => skinned.mesh.material_id += base,
        Kn5NodeKind::Dummy { .. } => {}
    }
    for child in &mut node.children {
        shift_materials(child, base);
    }
}

/// Grafts the prepared subtree at every anchor the section names. Returns how
/// many copies were placed.
fn anchor(root: &mut Kn5Node, graft: Kn5Node, replacement: &Replacement) -> usize {
    let mut placed = 0;
    // `INSERT_IN` first: it is the more precise of the two, and the only one
    // that makes an inserted part follow a moving node (a wheel, a door).
    for pattern in &replacement.insert_in {
        placed += graft_into(root, pattern, &graft, replacement.multiple);
        if placed > 0 && !replacement.multiple {
            return placed;
        }
    }
    for pattern in &replacement.insert_after {
        placed += graft_after(root, pattern, &graft, replacement.multiple);
        if placed > 0 && !replacement.multiple {
            return placed;
        }
    }
    placed
}

/// `INSERT_IN`: the part becomes the last child of the matching node, so it
/// inherits that node's transform — which is what makes a rim follow its
/// wheel.
fn graft_into(node: &mut Kn5Node, pattern: &str, graft: &Kn5Node, multiple: bool) -> usize {
    let mut placed = 0;
    if glob_match(pattern, &node.name) {
        node.children.push(graft.clone());
        placed += 1;
        if !multiple {
            return placed;
        }
    }
    for child in &mut node.children {
        if child.name == GRAFT_MARKER {
            continue;
        }
        placed += graft_into(child, pattern, graft, multiple);
        if placed > 0 && !multiple {
            break;
        }
    }
    placed
}

/// `INSERT_AFTER`: the part becomes the next sibling of the matching node,
/// sharing its parent's transform.
fn graft_after(node: &mut Kn5Node, pattern: &str, graft: &Kn5Node, multiple: bool) -> usize {
    let mut placed = 0;
    let mut at = 0;
    while at < node.children.len() {
        if node.children[at].name != GRAFT_MARKER && glob_match(pattern, &node.children[at].name) {
            node.children.insert(at + 1, graft.clone());
            placed += 1;
            at += 1;
            if !multiple {
                return placed;
            }
        }
        at += 1;
    }
    for child in &mut node.children {
        if child.name == GRAFT_MARKER {
            continue;
        }
        placed += graft_after(child, pattern, graft, multiple);
        if placed > 0 && !multiple {
            break;
        }
    }
    placed
}

/// Drops every node whose name matches, subtree included. Returns how many
/// went.
fn hide_matching(node: &mut Kn5Node, pattern: &str) -> usize {
    let mut removed = 0;
    node.children.retain(|child| {
        if glob_match(pattern, &child.name) {
            removed += 1;
            false
        } else {
            true
        }
    });
    for child in &mut node.children {
        removed += hide_matching(child, pattern);
    }
    removed
}

// ---------------------------------------------------------------------------
// Maths
// ---------------------------------------------------------------------------

/// `scale × rotation × translation`, row-major and row-vector like every other
/// matrix in this crate (`geometry.rs`): a point is `p × M`, so the
/// translation sits in the last **row**.
///
/// Rotation is heading (around Y, up), pitch (around X) and roll (around Z),
/// applied roll → pitch → heading. Right-handed, consistent with the "no
/// handedness conversion" finding in `geometry.rs`.
fn compose(scale: [f32; 3], rotation_deg: [f32; 3], offset: [f32; 3]) -> [f32; 16] {
    let [heading, pitch, roll] = rotation_deg.map(f32::to_radians);
    let (sh, ch) = heading.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    let (sr, cr) = roll.sin_cos();

    let ry = [ch, 0.0, -sh, 0.0, 1.0, 0.0, sh, 0.0, ch];
    let rx = [1.0, 0.0, 0.0, 0.0, cp, sp, 0.0, -sp, cp];
    let rz = [cr, sr, 0.0, -sr, cr, 0.0, 0.0, 0.0, 1.0];
    let r = mul3(&mul3(&rz, &rx), &ry);

    // Scale is diagonal, so it only stretches the rows of the rotation.
    let mut m = [0.0f32; 16];
    for row in 0..3 {
        for col in 0..3 {
            m[row * 4 + col] = scale[row] * r[row * 3 + col];
        }
    }
    m[12] = offset[0];
    m[13] = offset[1];
    m[14] = offset[2];
    m[15] = 1.0;
    m
}

fn mul3(a: &[f32; 9], b: &[f32; 9]) -> [f32; 9] {
    let mut out = [0.0f32; 9];
    for row in 0..3 {
        for col in 0..3 {
            out[row * 3 + col] = (0..3).map(|k| a[row * 3 + k] * b[k * 3 + col]).sum();
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Config parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct Section {
    name: String,
    entries: Vec<(String, String)>,
}

impl Section {
    fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    /// Comma-separated value, empty items dropped. `None` when the key is
    /// absent, `Some(vec![])` when it is present but empty — the difference
    /// matters for `FILE` and `SKINS`, where absent means "all".
    fn list(&self, key: &str) -> Option<Vec<String>> {
        self.get(key).map(|value| {
            value
                .split(',')
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect()
        })
    }

    /// A value written either once for both axles or twice, front then rear.
    fn axle_value(&self, key: &str, front: bool) -> Option<f32> {
        let values = self.list(key)?;
        let raw = if front {
            values.first()
        } else {
            values.get(1).or_else(|| values.first())
        }?;
        raw.parse().ok()
    }

    /// A copy carrying only the filters, so an expanded template inherits the
    /// `FILE`/`SKINS`/`ACTIVE` of the section it came from.
    fn filters_only(&self) -> Section {
        Section {
            name: "MODEL_REPLACEMENT_EXPANDED".to_string(),
            entries: self
                .entries
                .iter()
                .filter(|(k, _)| ["FILE", "SKINS", "ACTIVE"].iter().any(|f| k.eq_ignore_ascii_case(f)))
                .cloned()
                .collect(),
        }
    }

    fn to_replacement(&self, dir: &Path) -> Replacement {
        Replacement {
            hide: self.list("HIDE").unwrap_or_default(),
            insert: self
                .list("INSERT")
                .and_then(|files| files.into_iter().next())
                .map(|file| dir.join(file)),
            insert_in: self.list("INSERT_IN").unwrap_or_default(),
            insert_after: self.list("INSERT_AFTER").unwrap_or_default(),
            multiple: self.get("MULTIPLE").is_some_and(|v| v.trim() == "1"),
            offset: self.triple("OFFSET", 0.0),
            rotation: self.triple("ROTATION", 0.0),
            scale: self.triple("SCALE", 1.0),
        }
    }

    fn triple(&self, key: &str, default: f32) -> [f32; 3] {
        let mut out = [default; 3];
        if let Some(values) = self.list(key) {
            for (slot, raw) in out.iter_mut().zip(values.iter()) {
                if let Ok(value) = raw.parse::<f32>() {
                    *slot = value;
                }
            }
        }
        out
    }
}

/// Splits a CSP config into sections.
///
/// Section names repeat — every replacement is literally called
/// `[MODEL_REPLACEMENT_...]`, ellipsis included — so this cannot be a map.
/// Lines that are neither a header nor `key = value` are skipped, which is
/// what disposes of the `------ HIDE ROLLCAGE ------` banners these files are
/// full of.
fn parse_sections(text: &str) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with("//") {
            continue;
        }
        if let Some(rest) = line.strip_prefix('[') {
            let name = rest.split(']').next().unwrap_or("").trim().to_string();
            sections.push(Section {
                name,
                entries: Vec::new(),
            });
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let Some(section) = sections.last_mut() else {
            continue;
        };
        // Trailing comments are everywhere in these files, including on the
        // very lines we read: `Model = rim.kn5, 0.210, 0.222  ; radius, width`.
        let value = value.split(';').next().unwrap_or("").trim();
        section.entries.push((key.trim().to_string(), value.to_string()));
    }
    sections
}

/// Case-insensitive match where `?` stands for any run of characters,
/// including none (see the module documentation for the evidence).
fn glob_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.trim().to_ascii_lowercase().chars().collect();
    let n: Vec<char> = name.trim().to_ascii_lowercase().chars().collect();
    let (mut pi, mut ni) = (0usize, 0usize);
    let (mut star, mut mark) = (None, 0usize);
    while ni < n.len() {
        if pi < p.len() && p[pi] == '?' {
            star = Some(pi);
            pi += 1;
            mark = ni;
        } else if pi < p.len() && p[pi] == n[ni] {
            pi += 1;
            ni += 1;
        } else if let Some(at) = star {
            pi = at + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '?' {
        pi += 1;
    }
    pi == p.len()
}

fn file_name_of(path: &Path) -> String {
    path.file_name().unwrap_or_default().to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Rule: `?` matches any run of characters, including none — the reading
    // the AE86 config forces (see the module documentation).
    #[test]
    fn wildcard_matches_any_run_including_empty() {
        assert!(
            glob_match("?07_topaz?", "07_topaz"),
            "must match with nothing on either side"
        );
        assert!(glob_match("3_T.00?", "3_T.001"), "trailing run");
        assert!(glob_match("RIM_?", "RIM_LF"), "the wiki's own example");
        assert!(glob_match("toyota_ae86.kn5", "TOYOTA_AE86.KN5"), "case-insensitive");
        assert!(
            !glob_match("3_T.00?", "3_T.1"),
            "a literal prefix still has to be there"
        );
        assert!(
            !glob_match("", "anything"),
            "an empty pattern matches only an empty name"
        );
    }

    // Rule: the banners and trailing comments these files are full of are not
    // entries, and repeated section names are all kept.
    #[test]
    fn parser_survives_csp_config_noise() {
        let text = "\
[EXTRA_FX]
DELAYED_RENDER = 73

--------------- HIDE ROLLCAGE ---------------

[MODEL_REPLACEMENT_...]
ACTIVE = 1        ; set to 0 to disable the whole section
FILE = toyota_ae86.kn5, toyota_ae86_LOD_B.kn5
HIDE = 351
SKINS = ?07_topaz?

[MODEL_REPLACEMENT_...]
INSERT = TOYOTA_HALOGEN.kn5
";
        let sections = parse_sections(text);
        assert_eq!(sections.len(), 3, "banner lines do not open a section");
        assert_eq!(sections[1].get("ACTIVE"), Some("1"), "trailing comment stripped");
        assert_eq!(
            sections[1].list("FILE").unwrap().len(),
            2,
            "FILE is a comma-separated list"
        );
        assert_eq!(sections[2].get("INSERT"), Some("TOYOTA_HALOGEN.kn5"));
    }

    // Rule: a section aimed at another skin or another KN5 must not fire.
    // This is what keeps a per-skin body kit off every other skin.
    #[test]
    fn file_and_skin_filters_select_the_right_sections() {
        let text = "\
[MODEL_REPLACEMENT_...]
FILE = toyota_ae86.kn5
SKINS = ?00_panda?
INSERT = TOYOTA_HALOGEN.kn5
";
        let dir = Path::new("/cars/ae86/extension");
        assert_eq!(
            replacements_of(text, dir, "toyota_ae86.kn5", "00_panda").len(),
            1,
            "right file, right skin"
        );
        assert!(
            replacements_of(text, dir, "toyota_ae86.kn5", "11_advan").is_empty(),
            "another skin must not get this body kit"
        );
        assert!(
            replacements_of(text, dir, "toyota_ae86_LOD_B.kn5", "00_panda").is_empty(),
            "a section naming only the main model must not fire on a LOD"
        );
    }

    // Rule: an absent filter means "everything", which is how most sections
    // are written.
    #[test]
    fn absent_filters_mean_every_file_and_every_skin() {
        let text = "[MODEL_REPLACEMENT_...]\nINSERT = part.kn5\n";
        assert_eq!(
            replacements_of(text, Path::new("/x"), "whatever.kn5", "whatever_skin").len(),
            1,
            "no FILE and no SKINS: applies"
        );
        let inactive = "[MODEL_REPLACEMENT_...]\nACTIVE = 0\nINSERT = part.kn5\n";
        assert!(
            replacements_of(inactive, Path::new("/x"), "a.kn5", "s").is_empty(),
            "ACTIVE = 0 disables the section"
        );
    }

    // Rule: `[ReplaceRims]` expands to one hide plus four insertions, one per
    // corner, each anchored in its own `WHEEL_*` node. This is the template
    // that costs a car its wheels when it is ignored.
    #[test]
    fn replace_rims_expands_to_four_corners() {
        let text = "\
[ReplaceRims]
File = toyota_ae86.kn5
OriginalRims = 3_T.00?, Plane.001
Model = watanabe.kn5, 0.195, 0.165
Offset = 0, -0.0
";
        let out = replacements_of(text, Path::new("/skin"), "toyota_ae86.kn5", "00_panda");
        assert_eq!(out.len(), 5, "one hide + four corners");

        let hidden = &out[0];
        assert_eq!(hidden.hide, vec!["3_T.00?", "Plane.001"], "originals are hidden");
        assert!(hidden.insert.is_none(), "the hiding section inserts nothing");

        let corners: Vec<&String> = out[1..].iter().filter_map(|r| r.insert_in.first()).collect();
        assert_eq!(
            corners,
            vec!["WHEEL_LF", "WHEEL_RF", "WHEEL_LR", "WHEEL_RR"],
            "every corner gets its own rim, anchored inside the wheel node"
        );
        for corner in &out[1..] {
            assert_eq!(
                corner.insert.as_deref(),
                Some(Path::new("/skin").join("watanabe.kn5").as_path()),
                "the rim KN5 is looked up next to the config that names it"
            );
        }
    }

    // Rule: the right-hand rims are mirrored around the **vertical** axis.
    // Reading CSP's `ROTATION` as XYZ instead of heading/pitch/roll would turn
    // them upside down instead, which is exactly the kind of error a static
    // preview hides until someone looks at a wheel closely.
    #[test]
    fn right_hand_rims_are_mirrored_around_the_vertical_axis() {
        let text = "\
[ReplaceRims]
Model = rim.kn5, 0.2, 0.2
OriginalRims = RIM_?
";
        let out = replacements_of(text, Path::new("/skin"), "any.kn5", "any");
        let left = out.iter().find(|r| r.insert_in == ["WHEEL_LF"]).expect("front left");
        let right = out.iter().find(|r| r.insert_in == ["WHEEL_RF"]).expect("front right");
        assert_eq!(left.rotation, [0.0, 0.0, 0.0], "the model is authored for the left");
        assert_eq!(right.rotation, [180.0, 0.0, 0.0], "heading, not pitch");

        let m = compose(right.scale, right.rotation, right.offset);
        // Heading 180° negates X and Z and leaves Y alone: a wheel flipped
        // left-to-right, still the right way up.
        assert!((m[0] + 1.0).abs() < 1e-5, "X mirrored, got {}", m[0]);
        assert!((m[5] - 1.0).abs() < 1e-5, "Y untouched, got {}", m[5]);
        assert!((m[10] + 1.0).abs() < 1e-5, "Z mirrored, got {}", m[10]);
    }

    // Rule: with no `tyres.ini` to read (the data is inside the encrypted
    // `data.acd`, which we do not decrypt), the rim keeps the size its own
    // model declares — a scale of 1, never 0.
    #[test]
    fn rim_scale_falls_back_to_one_without_tyre_data() {
        let text = "[ReplaceRims]\nModel = rim.kn5, 0.195, 0.165\nOriginalRims = RIM_?\n";
        let out = replacements_of(text, Path::new("/skin"), "any.kn5", "any");
        let corner = out.iter().find(|r| r.insert.is_some()).expect("at least one corner");
        assert_eq!(corner.scale, [1.0, 1.0, 1.0], "no data, no scaling");

        // And an explicit Radius/Width is honoured, which is the case CSP
        // itself would have read out of the tyres.
        let sized = "[ReplaceRims]\nModel = rim.kn5, 0.2, 0.2\nRadius = 0.1\nWidth = 0.4\nOriginalRims = RIM_?\n";
        let out = replacements_of(sized, Path::new("/skin"), "any.kn5", "any");
        let corner = out.iter().find(|r| r.insert.is_some()).expect("at least one corner");
        assert_eq!(corner.scale, [2.0, 0.5, 0.5], "width on X, radius on Y and Z");
    }

    fn dummy(name: &str, children: Vec<Kn5Node>) -> Kn5Node {
        Kn5Node {
            name: name.to_string(),
            active: true,
            kind: Kn5NodeKind::Dummy {
                transform: compose([1.0; 3], [0.0; 3], [0.0; 3]),
            },
            children,
        }
    }

    /// A car-shaped skeleton: the two nodes a body kit anchors after, and the
    /// four wheels a rim anchors into.
    fn skeleton() -> Kn5Node {
        dummy(
            "root",
            vec![
                dummy("COCKPIT_LR", vec![dummy("3_T.001", vec![]), dummy("3_T.002", vec![])]),
                dummy("WHEEL_LF", vec![dummy("tyre_lf", vec![])]),
                dummy("WHEEL_RF", vec![dummy("tyre_rf", vec![])]),
            ],
        )
    }

    // Rule: HIDE drops the whole subtree, at any depth, and a pattern that
    // matches nothing is not an error — most sections aim at nodes that only
    // some versions of a mod carry.
    #[test]
    fn hide_removes_matching_subtrees_at_any_depth() {
        let mut root = skeleton();
        assert_eq!(hide_matching(&mut root, "3_T.00?"), 2, "both originals go");
        assert_eq!(hide_matching(&mut root, "3_T.00?"), 0, "nothing left to remove");
        assert_eq!(hide_matching(&mut root, "NO_SUCH_NODE"), 0, "an absent target is fine");
        assert_eq!(
            hide_matching(&mut root, "WHEEL_LF"),
            1,
            "a whole wheel can go, tyre included"
        );
    }

    // Rule: `INSERT_IN` puts the part *under* the anchor, so it inherits that
    // node's transform — this is what makes a rim follow its wheel — while
    // `INSERT_AFTER` puts it *beside* the anchor, sharing the parent's.
    #[test]
    fn graft_targets_land_under_and_beside_their_anchor() {
        let mut root = skeleton();
        let part = dummy("part", vec![]);

        assert_eq!(graft_into(&mut root, "WHEEL_LF", &part, false), 1, "one anchor hit");
        let wheel = root.children.iter().find(|c| c.name == "WHEEL_LF").unwrap();
        assert_eq!(wheel.children.len(), 2, "the rim joins the tyre inside the wheel");

        let mut root = skeleton();
        assert_eq!(graft_after(&mut root, "COCKPIT_LR", &part, false), 1, "one anchor hit");
        assert_eq!(
            root.children.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            vec!["COCKPIT_LR", "part", "WHEEL_LF", "WHEEL_RF"],
            "inserted as the next sibling, not as a child"
        );
    }

    // Rule: without `MULTIPLE`, one anchor is enough; with it, every match
    // gets a copy. A wildcard anchor and no `MULTIPLE` must not scatter the
    // part over the whole car.
    #[test]
    fn multiple_decides_whether_every_anchor_gets_a_copy() {
        let part = dummy("part", vec![]);

        let mut root = skeleton();
        assert_eq!(graft_into(&mut root, "WHEEL_??", &part, false), 1, "first match only");

        let mut root = skeleton();
        assert_eq!(graft_into(&mut root, "WHEEL_??", &part, true), 2, "both wheels");
    }

    // Rule: an inserted subtree is never itself an anchor. Without the guard,
    // a wildcard that matches inside a grafted part would keep matching the
    // copies it just made — a walk that does not terminate.
    #[test]
    fn a_grafted_part_is_never_used_as_an_anchor() {
        let mut root = skeleton();
        let graft = Kn5Node {
            name: GRAFT_MARKER.to_string(),
            active: true,
            kind: Kn5NodeKind::Dummy {
                transform: compose([1.0; 3], [0.0; 3], [0.0; 3]),
            },
            // Deliberately carries a node named like the anchor pattern.
            children: vec![dummy("WHEEL_LF", vec![])],
        };
        assert_eq!(
            graft_into(&mut root, "WHEEL_??", &graft, true),
            2,
            "only the car's own wheels count, not the ones just inserted"
        );
    }

    // Règle : le verre se lit dans la déclaration du mod, pas dans le KN5.
    // `[Material_Glass]` remplace le shader par `smGlass`, dont la
    // transparence vient d'un IOR — c'est la seule façon de savoir qu'un
    // matériau est une vitre et non un autocollant translucide.
    #[test]
    fn glass_is_read_from_the_mods_own_declaration() {
        let text = "[Material_Glass]
Materials = MAIN_GLASS
FilmIOR = 1.8
MaskPass = 1

[Material_PhotoelasticGlass]
Meshes = REAR_KOUKI_GLASS.004
IOR = 4

[Material_CarPaint_Metallic]
Materials = MAIN_BODY
";
        let mut glass = GlassOverrides::default();
        collect_glass(text, &mut glass);

        assert_eq!(
            glass.materials,
            vec![("MAIN_GLASS".to_string(), 1.8)],
            "FilmIOR l'emporte sur le défaut, et la peinture n'est pas du verre"
        );
        assert_eq!(
            glass.meshes,
            vec![("REAR_KOUKI_GLASS.004".to_string(), 4.0)],
            "les variantes de Material_*Glass* comptent aussi, y compris par maillage"
        );
    }

    // Règle : le raccourci `ExteriorGlassMaterials` vaut une section entière.
    // `materials_glass.ini` le définit pour éviter au moddeur de l'écrire, et
    // `ks_toyota_ae86_tuned` s'en sert.
    #[test]
    fn the_exterior_glass_shorthand_counts_as_a_declaration() {
        let text = "[INCLUDE: common/materials_glass.ini]
ExteriorGlassMaterials = glass, 
";
        let mut glass = GlassOverrides::default();
        collect_glass(text, &mut glass);
        assert_eq!(
            glass.materials,
            vec![("glass".to_string(), DEFAULT_GLASS_IOR)],
            "le raccourci déclare du verre au défaut de 1,5"
        );
    }

    // Règle : une voiture sans déclaration ne reçoit rien — le traitement
    // habituel reste en place, et aucune passe de transmission n'est payée.
    #[test]
    fn a_car_without_a_declaration_gets_no_glass() {
        let mut glass = GlassOverrides::default();
        collect_glass(
            "[Material_CarPaint]
Materials = body
",
            &mut glass,
        );
        assert!(glass.is_empty(), "rien de déclaré, rien d'appliqué");
    }

    // Rule: `FrontOnly` / `RearOnly` split the axles, which is how a mod gives
    // a car staggered wheels — two `[ReplaceRims]` sections, one per axle.
    #[test]
    fn front_only_and_rear_only_split_the_axles() {
        let text = "[ReplaceRims]\nModel = rim.kn5\nOriginalRims = RIM_?\nFrontOnly = 1\n";
        let out = replacements_of(text, Path::new("/skin"), "any.kn5", "any");
        let corners: Vec<&String> = out.iter().filter_map(|r| r.insert_in.first()).collect();
        assert_eq!(corners, vec!["WHEEL_LF", "WHEEL_RF"], "front axle only");
    }
}
