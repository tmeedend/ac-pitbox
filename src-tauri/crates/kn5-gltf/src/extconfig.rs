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

/// Where a car's CSP configuration lives.
///
/// Three places, and they are **not** interchangeable — CSP reads all of them
/// and the more specific wins:
///
/// - the selected skin's `ext_config.ini`, which is how a mod varies one skin;
/// - the car's own `extension/ext_config.ini` (plus the `materials.ini` it
///   usually includes);
/// - **`<AC>/extension/config/cars/loaded/<car_id>.ini`**, the config CSP
///   itself ships. This is the only one that exists for a Kunos car: 195 of
///   them are described there and nowhere else, which is why their glass and
///   their paint were invisible to us until we read it.
#[derive(Debug, Clone, Default)]
pub struct CspConfig {
    sources: Vec<PathBuf>,
}

impl CspConfig {
    /// `ac_install` is the Assetto Corsa root; `car_id` names the folder under
    /// `content/cars`, which is **not** always the name of `car_dir` — in the
    /// library a car lives under `<library>/cars/<car_id>/<version>`.
    pub fn locate(car_dir: &Path, skin_dir: Option<&Path>, ac_install: Option<&Path>, car_id: &str) -> Self {
        // Ordre délibéré : du plus spécifique au plus général, parce que la
        // résolution garde la **première** correspondance trouvée.
        let mut sources = Vec::new();
        if let Some(skin) = skin_dir {
            sources.push(skin.join("ext_config.ini"));
        }
        sources.push(car_dir.join("extension").join("ext_config.ini"));
        // Les mods rangent souvent leurs matériaux à part, tirés par un
        // `[INCLUDE: materials.ini]`. Le nom est assez stable pour être listé
        // ici même quand le fichier ne l'inclut pas ; les autres arrivent par
        // [`with_includes`].
        sources.push(car_dir.join("extension").join("materials.ini"));
        if let (Some(ac), false) = (ac_install, car_id.is_empty()) {
            sources.push(
                ac.join("extension")
                    .join("config")
                    .join("cars")
                    .join("loaded")
                    .join(format!("{car_id}.ini")),
            );
        }
        let mut expanded = Vec::new();
        for source in sources {
            with_includes(source, &mut expanded);
        }
        Self { sources: expanded }
    }

    /// Every file that may carry a declaration, most specific first. Files that
    /// do not exist are included: the caller stamps them into its cache key, and
    /// one appearing later has to invalidate it.
    pub fn sources(&self) -> &[PathBuf] {
        &self.sources
    }
}

/// Plafond de fichiers lus pour une voiture, cycles d'inclusion compris.
const MAX_CONFIG_SOURCES: usize = 32;

/// Ajoute un fichier de configuration, puis ceux qu'il inclut, dans cet ordre.
///
/// **Un mod range rarement tout dans `ext_config.ini`** : il découpe en
/// `materials.ini`, `pbr.ini`, `lights.ini`, `refraction.ini`… et les rappelle
/// par `[INCLUDE: …]`. Seul `materials.ini` était lu, par son nom ; tout le
/// reste était invisible. Bug réel sur `rj_honda_civic_eg6_tuned`, dont les
/// phares sortaient en aplat noir : ce qui les rend transparents est une
/// section `[REFRACTING_HEADLIGHT_…]` de `refraction.ini`, fichier que rien
/// n'ouvrait.
///
/// **Seuls les fichiers frères sont suivis.** Un `[INCLUDE: common/…]` ne
/// désigne pas un fichier de la voiture mais un template de CSP, résolu contre
/// son propre dossier — et ces templates sont précisément ce que ce module
/// renonce à interpréter (voir la documentation du module). D'où le filtre sur
/// le séparateur de chemin, qui écarte aussi tout échappement hors du dossier.
///
/// L'ordre compte : la résolution garde la **première** correspondance, donc
/// un fichier inclus se range juste derrière celui qui l'inclut, et devant les
/// sources plus générales. Le doublon est écarté (`materials.ini` est à la
/// fois listé d'office et inclus par la plupart des mods), ce qui suffit aussi
/// à rompre un cycle.
fn with_includes(path: PathBuf, out: &mut Vec<PathBuf>) {
    if out.len() >= MAX_CONFIG_SOURCES || out.contains(&path) {
        return;
    }
    let text = std::fs::read_to_string(&path).ok();
    let dir = path.parent().map(Path::to_path_buf);
    out.push(path);
    let (Some(dir), Some(text)) = (dir, text) else {
        return;
    };
    for name in includes(&text) {
        with_includes(dir.join(name), out);
    }
}

/// Les fichiers frères qu'un texte de configuration inclut.
fn includes(text: &str) -> Vec<String> {
    parse_sections(text)
        .into_iter()
        .filter_map(|section| {
            let (keyword, name) = section.name.split_once(':')?;
            if !keyword.trim().eq_ignore_ascii_case("INCLUDE") {
                return None;
            }
            let name = name.trim();
            (!name.is_empty() && !name.contains(['/', '\\'])).then(|| name.to_string())
        })
        .collect()
}

/// What CSP's material templates say a surface is.
///
/// Every field is optional, and `None` means **the mod said nothing** — the
/// conversion then keeps whatever it derived from the KN5 itself. That
/// distinction is the whole point: applying a template wholesale is worse than
/// applying nothing, because several of them only make sense together with a
/// texture we do not load (see [`SURFACE_TEMPLATES`]).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SurfaceOverride {
    pub metallic: Option<f32>,
    pub roughness: Option<f32>,
    /// Clear coat intensity and roughness — `KHR_materials_clearcoat`. A
    /// varnish over the surface, which is what carbon fibre and car paint are.
    pub clearcoat: Option<(f32, f32)>,
    /// Physical glass: index of refraction, transmission coming with it.
    pub glass_ior: Option<f32>,
    /// Ce verre-là ne remplace la conversion que là où elle produirait une
    /// surface **opaque** — voir la section `REFRACTING_HEADLIGHT` de
    /// [`collect_materials`].
    pub glass_only_if_opaque: bool,
}

impl SurfaceOverride {
    fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Everything a car's CSP configuration says about its surfaces, by name.
#[derive(Debug, Clone, Default)]
pub struct MaterialOverrides {
    materials: Vec<(String, SurfaceOverride)>,
    meshes: Vec<(String, SurfaceOverride)>,
}

impl MaterialOverrides {
    pub fn is_empty(&self) -> bool {
        self.materials.is_empty() && self.meshes.is_empty()
    }

    /// Resolves the declarations against a model: material index → override.
    ///
    /// Mesh-targeted declarations are folded in here, where the node tree is
    /// available to say which material a mesh actually carries. First match
    /// wins, and [`CspConfig`] orders its sources so that the most specific
    /// file is read first.
    pub fn resolve(&self, model: &Kn5Model) -> BTreeMap<usize, SurfaceOverride> {
        let mut out = BTreeMap::new();
        for (index, material) in model.materials.iter().enumerate() {
            if let Some((_, over)) = self.materials.iter().find(|(p, _)| glob_match(p, &material.name)) {
                out.insert(index, *over);
            }
        }
        if !self.meshes.is_empty() {
            model.visit_nodes(&mut |node| {
                let Some(mesh) = node.mesh() else { return };
                if let Some((_, over)) = self.meshes.iter().find(|(p, _)| glob_match(p, &node.name)) {
                    out.entry(mesh.material_id as usize).or_insert(*over);
                }
            });
        }
        out
    }
}

/// Indice de réfraction du verre par défaut, tel que `materials_glass.ini` le
/// fixe.
const DEFAULT_GLASS_IOR: f32 = 1.5;

/// Indice de réfraction d'une section de verre — **`IOR`, jamais `FilmIOR`**.
///
/// Les deux clés se ressemblent et ne désignent pas la même chose, ce que
/// `materials_glass.ini` dit sans ambiguïté dès qu'on regarde ce qu'il passe
/// à son shader :
///
/// ```ini
/// IOR = 1.5       ; index of refraction for glass, usualy, 1.5
/// FilmIOR = $IOR  ; redefine IOR for external film layer to increase reflections
/// PROP_0_EXTIOR = extIOR, $IOR              ; ← le shader reçoit IOR
/// FresnelC = $" _PBR_EstimateF0($FilmIOR or $IOR) "  ; ← FilmIOR ne pilote que le reflet
/// ```
///
/// `FilmIOR` décrit une **fine couche** posée par-dessus, qui n'affecte que la
/// réflectance ; `IOR` est celui du volume. Or le `KHR_materials_ior` de glTF
/// pilote la réflectance de tout le volume, par la même formule de Schlick.
/// Y écrire `FilmIOR` multiplie donc le reflet du verre entier :
///
/// | valeur posée | F0 obtenu |
/// | --- | --- |
/// | `IOR` 1,5 | 0,040 |
/// | `FilmIOR` 2,2 (phares) | 0,141 |
/// | `FilmIOR` 2,4 (filmé) | 0,170 |
/// | `FilmIOR` 3,2 (teinté) | **0,274** |
///
/// Bug réel, et de mon fait : les vitres de `ks_lamborghini_aventador_sv`
/// déclarent elles-mêmes `fresnelC = 0.1` et recevaient 0,274, soit près de
/// trois fois trop — un miroir à la place d'une vitre.
///
/// La couche fine n'est pas modélisée : glTF ne sait pas augmenter la
/// réflectance d'un diélectrique (`KHR_materials_specular` ne fait que la
/// réduire), et la rendre par un vernis rajouterait un lobe spéculaire là où on
/// vient d'en retirer un de trop.
fn glass_ior_of(section: &Section) -> f32 {
    section.number("IOR").filter(|v| *v > 1.0).unwrap_or(DEFAULT_GLASS_IOR)
}

/// Rugosité du vernis d'une peinture de carrosserie.
///
/// **Un choix, pas une transcription** : CSP n'exprime pas la rugosité de son
/// vernis par une valeur qu'on puisse reprendre. Zéro — le défaut de glTF —
/// donnerait un miroir parfait qui scintillerait au moindre mouvement, et le
/// plancher anti-scintillement du viewer ne s'applique qu'à la couche de base,
/// pas au vernis. Une valeur faible mais non nulle est le compromis ; à revoir
/// à l'œil si la carrosserie paraît trop ou trop peu laquée.
const CLEARCOAT_ROUGHNESS: f32 = 0.05;

/// Ce qu'un template de CSP pose comme valeurs, avant surcharge par la section.
#[derive(Debug, Clone, Copy)]
struct TemplateValues {
    metalness: Option<f32>,
    smoothness: Option<f32>,
    clearcoat: Option<(f32, f32)>,
}

/// Les templates de surface de CSP, **transcrits** de
/// `<AC>/extension/config/cars/common/materials_interior.ini`.
///
/// **`smoothness` vaut `None` sur toute la famille `_v2`**, et ce n'est pas un
/// oubli : ces templates étendent `Material_InteriorPBRDetail`, qui pose
/// `Smoothness = 0` *avant* mise à l'échelle par une texture PBR de détail
/// carrelée (`common/pbr_metal.dds` et consorts) qu'on ne charge pas.
/// Transcrire ce zéro rendrait tous les chromes et tous les alliages
/// parfaitement mats — strictement pire que de laisser notre propre estimation
/// tirée de `ksSpecularEXP`. Leur métallicité, elle, est une constante
/// ordinaire et se prend sans réserve.
///
/// `Reflectance` est lue par CSP mais n'est pas reprise ici : sur un métal la
/// métallicité la porte déjà, et sur un diélectrique glTF fixe F0 à 0,04 —
/// exactement la valeur que la plupart de ces templates déclarent. La rendre
/// demanderait `KHR_materials_specular` pour un écart invisible.
const SURFACE_TEMPLATES: [(&str, TemplateValues); 20] = [
    // Famille autonome : `Smoothness` y est une vraie constante.
    (
        "Material_InteriorPBR",
        TemplateValues {
            metalness: Some(0.0),
            smoothness: Some(0.9),
            clearcoat: None,
        },
    ),
    (
        "Material_Chrome",
        TemplateValues {
            metalness: Some(0.85),
            smoothness: Some(0.95),
            clearcoat: None,
        },
    ),
    (
        "Material_Carbon",
        TemplateValues {
            metalness: Some(0.0),
            smoothness: Some(0.5),
            clearcoat: Some((1.0, 0.1)),
        },
    ),
    (
        "Material_Leather",
        TemplateValues {
            metalness: Some(0.0),
            smoothness: Some(0.65),
            clearcoat: None,
        },
    ),
    (
        "Material_LeatherDetailed",
        TemplateValues {
            metalness: Some(0.0),
            smoothness: Some(0.65),
            clearcoat: None,
        },
    ),
    (
        "Material_DashboardLeather",
        TemplateValues {
            metalness: Some(0.0),
            smoothness: Some(0.01),
            clearcoat: None,
        },
    ),
    (
        "Material_Plastic",
        TemplateValues {
            metalness: Some(0.0),
            smoothness: Some(0.7),
            clearcoat: None,
        },
    ),
    (
        "Material_Fabric",
        TemplateValues {
            metalness: Some(0.0),
            smoothness: Some(0.0),
            clearcoat: None,
        },
    ),
    (
        "Material_Carpet",
        TemplateValues {
            metalness: Some(0.0),
            smoothness: Some(0.0),
            clearcoat: None,
        },
    ),
    (
        "Material_Velvet",
        TemplateValues {
            metalness: Some(0.0),
            smoothness: Some(0.0),
            clearcoat: None,
        },
    ),
    // Peinture de carrosserie : **le vernis, et rien d'autre.**
    //
    // `materials_carpaint.ini` pose `ClearCoatIntensity = 1.0` sur toute la
    // famille, et un vernis est littéralement ce qu'est une peinture de
    // carrosserie : glTF le rend par une couche lisse de F0 0,04 par-dessus la
    // base, ce qui est la description physique de la chose.
    //
    // **Ce qui est laissé de côté, et pourquoi.** Ces templates posent aussi un
    // `FresnelC` de 0,08 à 0,16. Le traduire en `KHR_materials_ior` referait
    // exactement l'erreur du `FilmIOR` (voir [`glass_ior_of`]) : un F0 de 0,16
    // demande un indice de 2,33 et transformerait la carrosserie en miroir,
    // alors que la conversion tire déjà sa réflectance du `fresnelC` que le
    // KN5 déclare lui-même. `FlakesK`, `ColoredSpecular` et `PearlescentSpecular`
    // n'ont pas d'équivalent glTF du tout.
    //
    // **La peinture mate n'a pas de vernis** : elle écrit `SpecularSun = 0, 1`,
    // c'est-à-dire pas de reflet solaire. Lui en poser un serait le contraire
    // de ce qu'elle demande, d'où son absence de cette table — elle retombe
    // alors sur le traitement ordinaire, ce qui est le bon défaut.
    (
        "Material_CarPaint",
        TemplateValues {
            metalness: None,
            smoothness: None,
            clearcoat: Some((1.0, CLEARCOAT_ROUGHNESS)),
        },
    ),
    (
        "Material_CarPaint_Metallic",
        TemplateValues {
            metalness: None,
            smoothness: None,
            clearcoat: Some((1.0, CLEARCOAT_ROUGHNESS)),
        },
    ),
    (
        "Material_CarPaint_Solid",
        TemplateValues {
            metalness: None,
            smoothness: None,
            clearcoat: Some((1.0, CLEARCOAT_ROUGHNESS)),
        },
    ),
    (
        "Material_CarPaint_Pearl",
        TemplateValues {
            metalness: None,
            smoothness: None,
            clearcoat: Some((1.0, CLEARCOAT_ROUGHNESS)),
        },
    ),
    // Famille `_v2` : métallicité seulement, brillance laissée au KN5.
    (
        "Material_Metal_v2",
        TemplateValues {
            metalness: Some(0.8),
            smoothness: None,
            clearcoat: None,
        },
    ),
    (
        "Material_MetalOld_v2",
        TemplateValues {
            metalness: Some(0.8),
            smoothness: None,
            clearcoat: None,
        },
    ),
    (
        "Material_Aluminium_v2",
        TemplateValues {
            metalness: Some(0.4),
            smoothness: None,
            clearcoat: None,
        },
    ),
    (
        "Material_AluminiumOld_v2",
        TemplateValues {
            metalness: Some(0.2),
            smoothness: None,
            clearcoat: None,
        },
    ),
    (
        "Material_Plastic_v2",
        TemplateValues {
            metalness: Some(0.0),
            smoothness: None,
            clearcoat: None,
        },
    ),
    (
        "Material_Leather_v2",
        TemplateValues {
            metalness: Some(0.0),
            smoothness: None,
            clearcoat: None,
        },
    ),
];

/// Matériau que vise une section de peinture qui ne nomme pas sa cible.
///
/// `materials_carpaint.ini` le pose dans ses `[DEFAULTS]` :
/// `CarPaintMaterial = Carpaint`. **128 des 195 configs livrées par CSP s'en
/// remettent à ce défaut** et n'écrivent aucune ligne `Materials` — les
/// ignorer revenait à ne rien appliquer à la carrosserie de la plupart des
/// voitures.
const DEFAULT_CAR_PAINT_MATERIAL: &str = "Carpaint";

/// Reads everything a car's configuration declares about its surfaces.
///
/// `skin_id` filtre les sections qui portent un `Skins` : un mod décrit
/// couramment plusieurs peintures dans le même fichier et laisse le skin
/// choisir — l'Aventador réserve ainsi sa peinture mate aux skins `?matt?`.
pub fn material_overrides(config: &CspConfig, skin_id: &str) -> MaterialOverrides {
    let mut out = MaterialOverrides::default();
    let mut car_paint = vec![DEFAULT_CAR_PAINT_MATERIAL.to_string()];
    let mut splits = Vec::new();
    for source in config.sources() {
        if let Ok(text) = std::fs::read_to_string(source) {
            collect_materials(&text, skin_id, &mut car_paint, &mut out, &mut splits);
        }
    }
    // En dernier, pas au fil des sections : rien ne garantit qu'un
    // `[MESH_SPLIT_…]` précède le `[REFRACTING_HEADLIGHT_…]` qui vise le
    // morceau qu'il découpe, ni même qu'ils soient dans le même fichier.
    resolve_split_names(&mut out, &splits);
    out
}

/// Un maillage que CSP **fabrique** en découpant un autre.
///
/// `[MESH_SPLIT_…]` coupe un maillage en deux le long d'un axe et suffixe le
/// morceau (`SPLIT_POSTFIX = _CUT`). Le nom ainsi créé n'existe nulle part
/// dans le KN5, et une section qui le vise ne trouve donc rien chez nous.
#[derive(Debug, Clone)]
struct MeshSplit {
    meshes: Vec<String>,
    postfix: String,
}

/// Fait pointer sur le maillage d'origine les déclarations qui visent un
/// morceau découpé.
///
/// **Bug réel sur `ks_toyota_supra_mkiv_drift`** : sa vitre de phare est
/// déclarée en optique par `SURFACE = SUPRA_LIGHTS_GLASS_CUT`, un nom que CSP
/// obtient en découpant `SUPRA_LIGHTS_GLASS`. Faute de faire la découpe, la
/// déclaration ne s'appliquait à rien et la vitre sortait en panneau noir
/// strié — voir `docs/kn5-format.md`, écart n°26.
///
/// **On ne découpe pas pour autant** : la moitié qui reste est du verre elle
/// aussi, et le morceau retiré est justement celui que CSP redessine. Viser le
/// maillage entier est donc plus juste que de ne rien viser du tout, et c'est
/// tout ce que cette fonction fait. Portée mesurée sur les 310 voitures
/// installées : 695 `SURFACE` se résolvent directement, **4 passent par ici**
/// (les trois Supra MkIV et `ddm_mugen_civic_aero_ek9`), 57 restent
/// introuvables pour d'autres raisons — des expressions de filtre CSP
/// (`{ lod:A & Head_Light_Glass_L }`) et des maillages qui vivent dans un LOD.
fn resolve_split_names(out: &mut MaterialOverrides, splits: &[MeshSplit]) {
    let mut extra = Vec::new();
    for (name, over) in &out.meshes {
        for split in splits {
            let Some(base) = name.strip_suffix(&split.postfix) else {
                continue;
            };
            if split.meshes.iter().any(|m| m == base) {
                // Ajouté, pas substitué : si le KN5 porte réellement un
                // maillage à ce nom, il garde la main.
                extra.push((base.to_string(), *over));
            }
        }
    }
    out.meshes.extend(extra);
}

/// Les clés raccourcies de `materials_glass.ini`, et leur variante `…Meshes`.
///
/// Elles ne portent **pas** d'indice de réfraction propre : ce qui les
/// distingue dans le fichier de CSP est un `FilmIOR` et un `ThicknessMult`,
/// dont ni l'un ni l'autre n'est l'IOR du verre — voir [`glass_ior_of`].
const GLASS_SHORTHANDS: [&str; 5] = [
    "ExteriorGlassMaterials",
    "ExteriorGlassTintedMaterials",
    "ExteriorGlassFilmedMaterials",
    "ExteriorGlassHeadlightsMaterials",
    "ExteriorGlassPhotoelasticMaterials",
];

fn collect_materials(
    text: &str,
    skin_id: &str,
    car_paint: &mut Vec<String>,
    out: &mut MaterialOverrides,
    splits: &mut Vec<MeshSplit>,
) {
    for section in parse_sections(text) {
        if section.name.to_ascii_uppercase().starts_with("MESH_SPLIT") {
            let meshes = section.list("MESHES").unwrap_or_default();
            let postfix = section.get("SPLIT_POSTFIX").unwrap_or_default().trim().to_string();
            if !meshes.is_empty() && !postfix.is_empty() {
                splits.push(MeshSplit { meshes, postfix });
            }
            continue;
        }
        // Le raccourci se redéfinit en cours de fichier, et vaut pour tout ce
        // qui suit — `ks_toyota_ae86_tuned` y liste ses onze pièces de
        // carrosserie d'un coup.
        if let Some(names) = section.list("CarPaintMaterial") {
            if !names.is_empty() {
                *car_paint = names;
            }
        }

        // Un mod décrit plusieurs peintures dans le même fichier et laisse le
        // skin choisir. Sans ce filtre, la peinture mate de l'Aventador
        // s'appliquerait à ses skins brillants.
        if let Some(skins) = section.list("Skins") {
            if !skins.iter().any(|pattern| glob_match(pattern, skin_id)) {
                continue;
            }
        }

        // **Un phare est du verre que rien d'autre ne déclare comme tel.**
        //
        // `[REFRACTING_HEADLIGHT_…]` n'est pas un template de matériau : c'est
        // l'optique entière que CSP reconstruit — la vitre, ce qu'il y a
        // derrière (`INSIDE`), les ampoules, le miroir du réflecteur. On n'en
        // retient que la vitre, et seulement le fait qu'elle est du verre
        // d'indice `IOR`.
        //
        // Sans ça, la vitre est prise au mot de sa diffuse, qui n'est pas une
        // couleur : `ext_headlight_glass` de `rj_honda_civic_eg6_tuned` est un
        // `ksPerPixelNM` opaque dont la texture (`glass.dds`, 64×64) vaut
        // (32,32,32) et (0,0,0) — un phare bouché en noir, alors que le jeu
        // montre le réflecteur au travers. Même famille d'écart que les neuf
        // de `docs/kn5-format.md` : AC remplit un champ standard d'une valeur
        // que son shader n'utilise pas comme une couleur.
        //
        // `SURFACE` nomme des **maillages**, vérifié sur les deux voitures :
        // le maillage `83` de la Civic porte `ext_headlight_glass`, et
        // `tailights_glass_red` d'`amy_ek_cup` porte `glass_taillight`.
        //
        // **Et la déclaration ne vaut que là où la conversion échoue** :
        // `glass_only_if_opaque`. Mesuré sur la bibliothèque — 74 voitures
        // portent ces sections, pour 655 surfaces retrouvées dans leur
        // modèle. **571 sont déjà en fondu ou en découpe** : leur verre rouge
        // ou orange est rendu correctement, et le traitement CSP, qui jette la
        // texture diffuse au motif que `smGlass` ne s'en sert pas comme d'une
        // couleur, décolorerait tous les feux arrière de la bibliothèque.
        // **84 sortent opaques, sur 27 voitures** — 81 en `ksPerPixelNM` —, et
        // celles-là sont bouchées faute de savoir qu'il s'agit d'une optique.
        // C'est exactement la population à corriger, `amy_ek_cup` et
        // `rj_honda_civic_eg6_tuned` comprises.
        if section.name.to_ascii_uppercase().starts_with("REFRACTING_HEADLIGHT") {
            let glass = SurfaceOverride {
                glass_ior: Some(glass_ior_of(&section)),
                glass_only_if_opaque: true,
                ..SurfaceOverride::default()
            };
            for name in section.list("SURFACE").unwrap_or_default() {
                out.meshes.push((name, glass));
            }
            continue;
        }

        let over = section_override(&section);
        if !over.is_empty() {
            let named = section.list("Materials").unwrap_or_default();
            let meshes = section.list("Meshes").unwrap_or_default();
            // Une section de peinture qui ne nomme personne vise le raccourci.
            let named = if named.is_empty() && meshes.is_empty() && is_car_paint(&section.name) {
                car_paint.clone()
            } else {
                named
            };
            for name in named {
                out.materials.push((name, over));
            }
            for name in meshes {
                out.meshes.push((name, over));
            }
        }

        // Les raccourcis peuvent apparaître dans n'importe quelle section — ils
        // sont posés juste sous le `[INCLUDE: common/materials_glass.ini]`.
        for shorthand in GLASS_SHORTHANDS {
            let glass = SurfaceOverride {
                glass_ior: Some(DEFAULT_GLASS_IOR),
                ..SurfaceOverride::default()
            };
            for name in section.list(shorthand).unwrap_or_default() {
                out.materials.push((name, glass));
            }
            let meshes = shorthand.replace("Materials", "Meshes");
            for name in section.list(&meshes).unwrap_or_default() {
                out.meshes.push((name, glass));
            }
        }
    }
}

/// Cette section décrit-elle une peinture de carrosserie ?
fn is_car_paint(name: &str) -> bool {
    name.starts_with("Material_CarPaint")
}

/// Ce qu'une section déclare : les valeurs de son template, surchargées par ce
/// qu'elle écrit elle-même.
fn section_override(section: &Section) -> SurfaceOverride {
    if !section.name.starts_with("Material_") {
        return SurfaceOverride::default();
    }

    // `Material_Glass`, `Material_GlassSide`, `Material_MultiEmissiveGlass`,
    // `Material_PhotoelasticGlass` — tous héritent du même `smGlass`.
    if section.name.contains("Glass") {
        return SurfaceOverride {
            glass_ior: Some(glass_ior_of(section)),
            ..SurfaceOverride::default()
        };
    }

    let template = SURFACE_TEMPLATES
        .iter()
        .find(|(name, _)| section.name.eq_ignore_ascii_case(name))
        .map(|(_, values)| *values);
    let Some(template) = template else {
        return SurfaceOverride::default();
    };

    // Une section peut redéfinir ce que son template pose — c'est même l'usage
    // courant, `[Material_Chrome] Smoothness = 0.8` par exemple.
    let metallic = section.number("Metalness").or(template.metalness);
    let smoothness = section.number("Smoothness").or(template.smoothness);
    let clearcoat = match section.number("UseClearCoat") {
        Some(v) if v <= 0.0 => None,
        _ => {
            let (intensity, smooth) = template.clearcoat.unwrap_or((1.0, 0.1));
            let intensity = section.number("ClearCoatIntensity").unwrap_or(intensity);
            let roughness = section
                .number("ClearCoatSmoothness")
                .map(|s| (1.0 - s).clamp(0.0, 1.0))
                .unwrap_or(smooth);
            (section.number("UseClearCoat").is_some() || template.clearcoat.is_some())
                .then_some((intensity.clamp(0.0, 1.0), roughness))
        }
    };

    SurfaceOverride {
        metallic: metallic.map(|v| v.clamp(0.0, 1.0)),
        roughness: smoothness.map(|s| (1.0 - s).clamp(0.0, 1.0)),
        clearcoat,
        glass_ior: None,
        glass_only_if_opaque: false,
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
    /// Propriétés de matériau surchargées par la config.
    pub overridden_properties: usize,
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
    model_path: &Path,
    skin_dir: Option<&Path>,
    config: &CspConfig,
) -> ExtConfigStats {
    let mut stats = ExtConfigStats::default();
    let model_file = file_name_of(model_path);
    let skin_id = skin_dir.map(file_name_of).unwrap_or_default();

    // Les greffes s'additionnent, elles ne se remplacent pas : l'ordre importe
    // peu ici, contrairement aux déclarations de matériau.
    for source in config.sources() {
        let Ok(text) = std::fs::read_to_string(source) else {
            continue;
        };
        // `INSERT` nomme un KN5 posé **à côté de la config qui le nomme** —
        // d'où la résolution relative à ce fichier-là et pas au dossier de la
        // voiture.
        let Some(dir) = source.parent() else { continue };
        for replacement in replacements_of(&text, dir, &model_file, &skin_id) {
            stats.applied += 1;
            apply_one(model, &replacement, &mut stats);
        }
    }

    // Les surcharges de propriété viennent **après** les greffes, pour couvrir
    // aussi les matériaux que celles-ci ont amenés.
    stats.overridden_properties = apply_property_overrides(model, config, &skin_id);
    stats
}

/// Applique les propriétés que la configuration impose aux matériaux.
///
/// **Rien à traduire, et c'est ce qui rend l'opération sûre** : un
/// `[SHADER_REPLACEMENT_...]` écrit `PROP_0 = fresnelC, 0.02`, c'est-à-dire la
/// propriété qu'AC porte déjà, sous le même nom et avec le même sens. On la
/// pose dans le matériau **avant** la conversion, et tout ce qui en dérive
/// suit sans qu'il y ait une ligne à changer en aval : `fresnelC` et
/// `fresnelMaxLevel` décident de la métallicité, `ksSpecularEXP` de la
/// rugosité, `ksDiffuse` de l'opacité d'une vitre.
///
/// Mesuré sur la bibliothèque de référence : **4 549 surcharges, dont 2 056
/// (45 %) portent sur une propriété qu'on interprète**. Ce n'est pas un
/// réglage marginal — c'est un moddeur qui écrit, deux mille fois, qu'il veut
/// autre chose que ce que son propre KN5 déclare.
///
/// **Le nom du shader n'est pas repris**, lui. CSP y met les siens (`smGlass`,
/// `smCarPaint`), inconnus de la conversion, et les adopter ferait perdre les
/// cas particuliers accrochés aux noms d'AC — `ksWindscreen` et sa texture de
/// saleté (écart n°6), `ksTyres` et sa gomme, `ksBrokenGlass` et sa vitre
/// brisée (écart n°8).
fn apply_property_overrides(model: &mut Kn5Model, config: &CspConfig, skin_id: &str) -> usize {
    // Quels maillages portent quel matériau : les sélecteurs `MESHES` visent la
    // géométrie, les propriétés vivent sur le matériau.
    let mut mesh_names: Vec<(String, usize)> = Vec::new();
    model.visit_nodes(&mut |node| {
        if let Some(mesh) = node.mesh() {
            mesh_names.push((node.name.clone(), mesh.material_id as usize));
        }
    });

    // Première source servie, première valeur retenue : `CspConfig` classe ses
    // fichiers du plus spécifique au plus général, et un skin doit pouvoir
    // contredire la config de la voiture.
    let mut settled: HashMap<(usize, String), ()> = HashMap::new();
    let mut count = 0usize;

    for source in config.sources() {
        let Ok(text) = std::fs::read_to_string(source) else {
            continue;
        };
        for section in parse_sections(&text) {
            count += apply_section_properties(model, &mesh_names, &section, skin_id, &mut settled);
        }
    }
    count
}

/// Applique les propriétés d'une seule section. Renvoie le nombre posé.
fn apply_section_properties(
    model: &mut Kn5Model,
    mesh_names: &[(String, usize)],
    section: &Section,
    skin_id: &str,
    settled: &mut HashMap<(usize, String), ()>,
) -> usize {
    let properties = section.properties();
    if properties.is_empty() || section.get("ACTIVE").is_some_and(|v| v.trim() == "0") {
        return 0;
    }
    if let Some(skins) = section.list("Skins") {
        if !skins.iter().any(|pattern| glob_match(pattern, skin_id)) {
            return 0;
        }
    }

    let mut targets: Vec<usize> = Vec::new();
    for pattern in section.list("Materials").unwrap_or_default() {
        for (index, material) in model.materials.iter().enumerate() {
            if glob_match(&pattern, &material.name) {
                targets.push(index);
            }
        }
    }
    for pattern in section.list("Meshes").unwrap_or_default() {
        for (name, material) in mesh_names {
            if glob_match(&pattern, name) {
                targets.push(*material);
            }
        }
    }
    targets.sort_unstable();
    targets.dedup();

    let mut count = 0;
    for index in targets {
        for (name, value) in &properties {
            if settled.insert((index, name.clone()), ()).is_some() {
                continue;
            }
            let Some(material) = model.materials.get_mut(index) else {
                continue;
            };
            count += 1;
            match material.properties.iter_mut().find(|p| p.name == *name) {
                Some(existing) => existing.value = *value,
                // Une propriété absente du KN5 est **ajoutée** : un matériau
                // sans `fresnelC` que le mod déclare réfléchissant doit le
                // devenir, sans quoi la surcharge ne dirait rien.
                None => material.properties.push(kn5::Kn5MaterialProperty {
                    name: name.clone(),
                    value: *value,
                    extra: [0.0; 9],
                }),
            }
        }
    }
    count
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
/// not decrypt (SPEC-preview-3d-kn5 PREVIEW§4.2). Absent an explicit `Radius`/`Width`
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
    let graft = merge_assets(
        model,
        inserted,
        GRAFT_MARKER,
        compose(replacement.scale, replacement.rotation, replacement.offset),
    );
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
/// the grafted subtree's material indices, and wraps it in a dummy named
/// `wrapper` carrying `transform`.
///
/// Shared with `driver.rs`, which grafts a whole mannequin the same way — the
/// asset merge below is the part that has nothing to do with CSP, and writing
/// it twice would mean two answers to the texture clash question.
///
/// Textures are keyed **by name** in a KN5, so two files can disagree about
/// what `black.dds` contains. An identical blob is shared; a genuine clash is
/// renamed and the inserted materials are pointed at the new name. Renaming
/// costs that one texture its skin override, which is a far smaller price than
/// showing the wrong image.
pub(crate) fn merge_assets(
    host: &mut Kn5Model,
    mut inserted: Kn5Model,
    wrapper: &str,
    transform: [f32; 16],
) -> Kn5Node {
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
        name: wrapper.to_string(),
        active: true,
        kind: Kn5NodeKind::Dummy { transform },
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
pub(crate) fn compose(scale: [f32; 3], rotation_deg: [f32; 3], offset: [f32; 3]) -> [f32; 16] {
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

    /// Les propriétés de matériau que la section impose.
    ///
    /// Deux écritures, documentées par le wiki CSP et toutes deux rencontrées :
    /// `PROP_<n> = nom, valeur` d'un seul tenant, ou la paire `KEY_<n>` /
    /// `VALUE_<n>`. Le `<n>` est souvent `...`, que CSP remplit tout seul.
    fn properties(&self) -> Vec<(String, f32)> {
        let mut out: Vec<(String, f32)> = Vec::new();
        for (key, value) in &self.entries {
            let upper = key.to_ascii_uppercase();
            if let Some(rest) = upper.strip_prefix("PROP_") {
                // `PROP_0_KSAMBIENT = ksAmbient, 0.4` : le suffixe est
                // décoratif, seule la valeur compte.
                let _ = rest;
                let mut parts = value.split(',');
                let (Some(name), Some(number)) = (parts.next(), parts.next()) else {
                    continue;
                };
                if let Ok(number) = number.trim().parse::<f32>() {
                    out.push((name.trim().to_string(), number));
                }
            } else if let Some(index) = upper.strip_prefix("KEY_") {
                let name = value.trim().to_string();
                if name.is_empty() {
                    continue;
                }
                if let Some(number) = self
                    .get(&format!("VALUE_{index}"))
                    .and_then(|v| v.trim().parse::<f32>().ok())
                {
                    out.push((name, number));
                }
            }
        }
        out
    }

    /// Valeur numérique d'une clé, quand elle en est une.
    fn number(&self, key: &str) -> Option<f32> {
        self.get(key).and_then(|v| v.trim().parse().ok())
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

    // Règle : les configs sont consultées du plus spécifique au plus général —
    // skin, voiture, puis celle que CSP livre pour la voiture. La résolution
    // gardant la première correspondance, c'est cet ordre qui décide qui gagne.
    #[test]
    fn config_sources_run_from_the_skin_down_to_the_one_csp_ships() {
        let car = Path::new("D:/lib/cars/abarth500/v1.2");
        let skin = car.join("skins").join("red");
        let ac = Path::new("D:/AC");

        let config = CspConfig::locate(car, Some(&skin), Some(ac), "abarth500");
        assert_eq!(
            config.sources(),
            [
                skin.join("ext_config.ini"),
                car.join("extension").join("ext_config.ini"),
                car.join("extension").join("materials.ini"),
                ac.join("extension")
                    .join("config")
                    .join("cars")
                    .join("loaded")
                    .join("abarth500.ini"),
            ],
            "du plus spécifique au plus général"
        );

        // **L'identifiant n'est pas le nom du dossier** en bibliothèque : une
        // voiture y vit sous `<car_id>/<version>`, donc le déduire du chemin
        // pointerait sur `v1.2.ini` et ne trouverait jamais rien.
        assert!(
            config.sources().last().unwrap().ends_with("abarth500.ini"),
            "la config livrée est nommée par l'identifiant, pas par la version"
        );

        // Sans install AC connue, on ne peut que lire le dossier de la voiture.
        let alone = CspConfig::locate(car, None, None, "abarth500");
        assert_eq!(alone.sources().len(), 2, "pas de skin, pas d'install : deux fichiers");
    }

    // Règle : un `[INCLUDE: …]` qui nomme un fichier frère est suivi, un
    // `common/…` ne l'est pas — celui-là désigne un template de CSP, résolu
    // contre son propre dossier, et ce module ne les interprète pas.
    //
    // Bug réel sur `rj_honda_civic_eg6_tuned` : seul `materials.ini` était lu,
    // par son nom, et les phares se déclarent dans `refraction.ini`.
    #[test]
    fn a_sibling_include_is_followed_and_a_template_one_is_not() {
        let base = crate::testutil::temp_dir("includes");
        let extension = base.join("extension");
        std::fs::create_dir_all(&extension).unwrap();
        std::fs::write(
            extension.join("ext_config.ini"),
            "[INCLUDE: common/materials_glass.ini]\n[INCLUDE: refraction.ini]\n[INCLUDE: absent.ini]\n",
        )
        .unwrap();
        std::fs::write(extension.join("refraction.ini"), "[INCLUDE: deeper.ini]\n").unwrap();
        std::fs::write(extension.join("deeper.ini"), "").unwrap();

        let sources = CspConfig::locate(&base, None, None, "").sources().to_vec();
        assert!(
            sources.contains(&extension.join("refraction.ini")),
            "le fichier frère inclus est lu"
        );
        assert!(
            sources.contains(&extension.join("deeper.ini")),
            "et ce qu'il inclut à son tour aussi"
        );
        assert!(
            sources.contains(&extension.join("absent.ini")),
            "un inclus absent reste listé : le voir apparaître doit invalider le cache"
        );
        assert!(
            !sources.iter().any(|p| p.ends_with("materials_glass.ini")),
            "un template de CSP n'est pas un fichier de la voiture"
        );
        assert!(
            sources.iter().position(|p| p.ends_with("refraction.ini"))
                > sources.iter().position(|p| p.ends_with("ext_config.ini")),
            "un fichier inclus se range derrière celui qui l'inclut"
        );
    }

    // Règle : une `SURFACE` qui vise un morceau découpé par `[MESH_SPLIT_…]`
    // retombe sur le maillage d'origine — sans quoi elle ne vise rien.
    //
    // Bug réel sur `ks_toyota_supra_mkiv_drift` : sa vitre de phare sortait en
    // panneau noir strié parce que la seule déclaration qui la dit optique
    // nomme `SUPRA_LIGHTS_GLASS_CUT`, un maillage que CSP fabrique.
    #[test]
    fn a_surface_naming_a_split_off_piece_falls_back_on_the_whole_mesh() {
        let over = collected(
            "[MESH_SPLIT_...]
MESHES = SUPRA_LIGHTS_GLASS
SPLIT_AXIS = 0, 1, 0
SPLIT_POSTFIX = _CUT

[REFRACTING_HEADLIGHT_...]
SURFACE = SUPRA_LIGHTS_GLASS_CUT
IOR = 2
",
        );
        let names: Vec<&str> = over.meshes.iter().map(|(n, _)| n.as_str()).collect();
        assert!(
            names.contains(&"SUPRA_LIGHTS_GLASS"),
            "le maillage d'origine est visé, lui existe : {names:?}"
        );
        assert!(
            over.meshes
                .iter()
                .all(|(_, o)| o.glass_ior == Some(2.0) && o.glass_only_if_opaque),
            "et il hérite de l'optique déclarée, garde comprise"
        );

        // Un suffixe qui ne correspond à aucun découpage ne fabrique rien.
        let alone = collected(
            "[REFRACTING_HEADLIGHT_...]
SURFACE = SOMETHING_CUT
",
        );
        assert_eq!(
            alone.meshes.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
            vec!["SOMETHING_CUT"],
            "rien à retomber sur"
        );
    }

    // Règle : `[REFRACTING_HEADLIGHT_…]` déclare que le maillage de son
    // `SURFACE` est une optique, et rien d'autre de la section n'est retenu.
    #[test]
    fn a_refracting_headlight_declares_its_surface_to_be_glass() {
        let over = collected(
            "[REFRACTING_HEADLIGHT_...]
SURFACE = 83
INSIDE = 55_T
IOR = 1.73
GLASS_COLOR = 0.25,0.25,0.25
",
        );
        assert!(over.materials.is_empty(), "la section ne nomme pas de matériau");
        assert_eq!(
            over.meshes,
            vec![(
                "83".to_string(),
                SurfaceOverride {
                    glass_ior: Some(1.73),
                    glass_only_if_opaque: true,
                    ..SurfaceOverride::default()
                }
            )],
            "le maillage de la vitre devient du verre, et seulement là où la conversion le rendrait opaque"
        );

        // `INSIDE` nomme le réflecteur et les ampoules : ils restent opaques.
        assert!(
            !over.meshes.iter().any(|(name, _)| name == "55_T"),
            "ce qu'il y a derrière la vitre n'est pas de la vitre"
        );
    }

    fn collected(text: &str) -> MaterialOverrides {
        collected_for(text, "")
    }

    fn collected_for(text: &str, skin: &str) -> MaterialOverrides {
        let mut out = MaterialOverrides::default();
        let mut car_paint = vec![DEFAULT_CAR_PAINT_MATERIAL.to_string()];
        let mut splits = Vec::new();
        collect_materials(text, skin, &mut car_paint, &mut out, &mut splits);
        resolve_split_names(&mut out, &splits);
        out
    }

    fn glass(ior: f32) -> SurfaceOverride {
        SurfaceOverride {
            glass_ior: Some(ior),
            ..SurfaceOverride::default()
        }
    }

    // Règle : une section de peinture qui ne nomme personne vise le raccourci
    // `CarPaintMaterial`, dont `materials_carpaint.ini` fixe le défaut à
    // « Carpaint ». 128 des 195 configs livrées par CSP s'en remettent à ce
    // défaut : les ignorer ne toucherait la carrosserie de presque aucune
    // voiture.
    #[test]
    fn a_paint_section_without_a_target_uses_the_shorthand() {
        let over = collected(
            "[INCLUDE: common/materials_carpaint.ini]
[Material_CarPaint_Metallic]
",
        );
        assert_eq!(
            over.materials.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
            vec!["Carpaint"],
            "la cible par défaut"
        );
        assert!(over.materials[0].1.clearcoat.is_some(), "et elle reçoit son vernis");

        // Redéfini, le raccourci vaut pour tout ce qui suit — l'AE86 y liste
        // ses onze pièces de carrosserie d'un coup.
        let renamed = collected(
            "[INCLUDE: common/materials_carpaint.ini]
CarPaintMaterial = coupebody, body_1
[Material_CarPaint_Solid]
",
        );
        assert_eq!(
            renamed.materials.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
            vec!["coupebody", "body_1"],
        );
    }

    // Règle : une section filtrée par `Skins` ne vaut que pour ces skins. Sans
    // ce filtre, la peinture mate de l'Aventador — qui n'a pas de vernis —
    // s'appliquerait aussi à ses skins brillants, et inversement.
    #[test]
    fn a_material_section_can_be_limited_to_some_skins() {
        let text = "[Material_CarPaint_Metallic]
Materials = body
Skins = ?rosso?
";
        assert_eq!(collected_for(text, "rosso_corsa").materials.len(), 1, "le skin visé");
        assert!(
            collected_for(text, "bianco").materials.is_empty(),
            "un autre skin n'est pas concerné"
        );
    }

    // Règle : la peinture **mate** ne reçoit pas de vernis. Elle écrit
    // `SpecularSun = 0, 1`, c'est-à-dire pas de reflet solaire ; lui en poser
    // un serait le contraire de ce qu'elle demande.
    #[test]
    fn matte_paint_gets_no_clear_coat() {
        let matte = collected(
            "[Material_CarPaint_Matte]
Materials = body
",
        );
        assert!(matte.is_empty(), "rien appliqué, donc traitement ordinaire");

        let glossy = collected(
            "[Material_CarPaint_Pearl]
Materials = body
",
        );
        assert_eq!(
            glossy.materials[0].1.clearcoat,
            Some((1.0, CLEARCOAT_ROUGHNESS)),
            "les peintures brillantes, elles, sont vernies"
        );
    }

    /// Un modèle d'un matériau, pour les surcharges de propriété.
    fn model_with_material(name: &str, properties: &[(&str, f32)]) -> Kn5Model {
        Kn5Model {
            version: 6,
            extra: None,
            textures: Vec::new(),
            materials: vec![kn5::Kn5Material {
                name: name.to_string(),
                shader: "ksPerPixel".to_string(),
                blend_mode: 0,
                alpha_tested: false,
                reserved: 0,
                properties: properties
                    .iter()
                    .map(|(n, v)| kn5::Kn5MaterialProperty {
                        name: n.to_string(),
                        value: *v,
                        extra: [0.0; 9],
                    })
                    .collect(),
                samplers: Vec::new(),
            }],
            root: Kn5Node {
                name: "root".to_string(),
                active: true,
                kind: Kn5NodeKind::Dummy { transform: [0.0; 16] },
                children: Vec::new(),
            },
        }
    }

    fn apply(model: &mut Kn5Model, text: &str) -> usize {
        let mut settled = HashMap::new();
        parse_sections(text)
            .iter()
            .map(|section| apply_section_properties(model, &[], section, "", &mut settled))
            .sum()
    }

    fn value_of(model: &Kn5Model, name: &str) -> Option<f32> {
        model.materials[0].property(name)
    }

    // Règle : les deux écritures de `PROP_` du wiki CSP sont lues — d'un seul
    // tenant, ou en paire `KEY_`/`VALUE_`. Le suffixe est décoratif, CSP
    // remplissant lui-même les `...`.
    #[test]
    fn both_property_syntaxes_are_read() {
        let sections = parse_sections(
            "[SHADER_REPLACEMENT_...]\nPROP_0_KSAMBIENT = ksAmbient, 0.4\nPROP_... = fresnelC,0.5\nKEY_3 = ksDiffuse\nVALUE_3 = 0.2\n",
        );
        assert_eq!(
            sections[0].properties(),
            vec![
                ("ksAmbient".to_string(), 0.4),
                ("fresnelC".to_string(), 0.5),
                ("ksDiffuse".to_string(), 0.2),
            ],
        );
    }

    // Règle : une surcharge remplace ce que le KN5 déclare, et **ajoute** ce
    // qu'il ne déclare pas. Bug réel évité : `INT_Logo_Cambio` de l'abarth500
    // déclare `fresnelC = 0.05`, la config impose 0,5 — sans la surcharge le
    // logo du pommeau reste du plastique au lieu de devenir un métal.
    #[test]
    fn an_override_replaces_what_the_kn5_says_and_adds_what_it_omits() {
        let mut model = model_with_material("logo", &[("fresnelC", 0.05)]);
        let posed = apply(
            &mut model,
            "[SHADER_REPLACEMENT_...]\nMATERIALS = logo\nPROP_... = fresnelC, 0.5\nPROP_... = fresnelMaxLevel, 1\n",
        );
        assert_eq!(posed, 2, "les deux propriétés sont posées");
        assert_eq!(value_of(&model, "fresnelC"), Some(0.5), "la déclarée est remplacée");
        assert_eq!(value_of(&model, "fresnelMaxLevel"), Some(1.0), "l'absente est ajoutée");
    }

    // Règle : première source servie, première valeur retenue. `CspConfig`
    // classe ses fichiers du plus spécifique au plus général, donc un skin doit
    // pouvoir contredire la config de la voiture, jamais l'inverse.
    #[test]
    fn the_first_source_to_set_a_property_keeps_it() {
        let mut model = model_with_material("body", &[]);
        let mut settled = HashMap::new();
        let specific = parse_sections("[SHADER_REPLACEMENT_...]\nMATERIALS = body\nPROP_... = ksDiffuse, 0.9\n");
        let general = parse_sections("[SHADER_REPLACEMENT_...]\nMATERIALS = body\nPROP_... = ksDiffuse, 0.1\n");
        apply_section_properties(&mut model, &[], &specific[0], "", &mut settled);
        apply_section_properties(&mut model, &[], &general[0], "", &mut settled);
        assert_eq!(value_of(&model, "ksDiffuse"), Some(0.9), "le plus spécifique gagne");
    }

    // Règle : une section qui ne vise personne ne pose rien, et un `ACTIVE = 0`
    // la désactive — mêmes filtres que pour les remplacements de modèle.
    #[test]
    fn a_property_section_without_a_target_or_disabled_poses_nothing() {
        let mut model = model_with_material("body", &[]);
        assert_eq!(
            apply(&mut model, "[SHADER_REPLACEMENT_...]\nPROP_... = ksDiffuse, 0.9\n"),
            0,
            "aucun sélecteur, aucune cible"
        );
        assert_eq!(
            apply(
                &mut model,
                "[SHADER_REPLACEMENT_...]\nACTIVE = 0\nMATERIALS = body\nPROP_... = ksDiffuse, 0.9\n"
            ),
            0,
            "désactivée"
        );
        assert_eq!(value_of(&model, "ksDiffuse"), None, "rien posé");
    }

    // Règle : **`IOR`, jamais `FilmIOR`.** Les deux clés se ressemblent et ne
    // désignent pas la même chose : `materials_glass.ini` passe `IOR` à son
    // shader (`extIOR`) et ne se sert de `FilmIOR` que pour une fine couche de
    // reflet. Le `KHR_materials_ior` de glTF, lui, pilote la réflectance de
    // tout le volume — y écrire un `FilmIOR` de 3,2 donne un F0 de 0,274 là où
    // la vitre en déclare 0,1, soit un miroir à la place d'une vitre.
    #[test]
    fn glass_takes_its_bulk_ior_never_the_film_one() {
        let filmed = collected(
            "[Material_Glass]
Materials = pane
FilmIOR = 3.2
",
        );
        assert_eq!(
            filmed.materials[0].1.glass_ior,
            Some(1.5),
            "la couche fine ne devient pas l'indice du volume"
        );

        let bulk = collected(
            "[Material_Glass]
Materials = pane
IOR = 1.7
FilmIOR = 3.2
",
        );
        assert_eq!(bulk.materials[0].1.glass_ior, Some(1.7), "IOR est pris tel quel");
    }

    // Règle : les raccourcis `ExteriorGlass*` déclarent du verre, et rien de
    // plus — ce qui les distingue chez CSP est un `FilmIOR` et un
    // `ThicknessMult`, dont aucun n'est un indice de réfraction.
    #[test]
    fn the_exterior_glass_shorthands_all_declare_plain_glass() {
        let over = collected(
            "[INCLUDE: common/materials_glass.ini]
ExteriorGlassFilmedMaterials=CAR_Vetro
ExteriorGlassHeadlightsMaterials=CAR_Vetro_Fanali
ExteriorGlassMaterials=plain
",
        );
        assert_eq!(
            over.materials,
            vec![
                ("plain".to_string(), glass(1.5)),
                ("CAR_Vetro".to_string(), glass(1.5)),
                ("CAR_Vetro_Fanali".to_string(), glass(1.5)),
            ],
            "tous du verre ordinaire"
        );
    }

    // Règle : le verre se lit dans la déclaration du mod, pas dans le KN5.
    // `[Material_Glass]` remplace le shader par `smGlass`, dont la
    // transparence vient d'un IOR — c'est la seule façon de savoir qu'un
    // matériau est une vitre et non un autocollant translucide.
    #[test]
    fn glass_is_read_from_the_mods_own_declaration() {
        let over = collected(
            "\
[Material_Glass]
Materials = MAIN_GLASS
FilmIOR = 1.8

[Material_PhotoelasticGlass]
Meshes = REAR_KOUKI_GLASS.004
IOR = 4

[Material_CarPaint_Metallic]
Materials = MAIN_BODY
",
        );
        assert_eq!(
            over.materials[0],
            ("MAIN_GLASS".to_string(), glass(1.5)),
            "le FilmIOR de la section ne devient pas l'indice du volume"
        );
        assert_eq!(
            over.meshes,
            vec![("REAR_KOUKI_GLASS.004".to_string(), glass(4.0))],
            "les variantes de Material_*Glass* comptent aussi, y compris par maillage"
        );
    }

    // Règle : les templates de surface sont **transcrits** de CSP, pas devinés.
    // `Smoothness` y est l'inverse de la rugosité de glTF.
    #[test]
    fn surface_templates_are_transcribed_from_csp() {
        let over = collected("[Material_Chrome]\nMaterials = chrome_trim\n");
        let (name, chrome) = &over.materials[0];
        assert_eq!(name, "chrome_trim", "la section nomme sa cible");
        assert_eq!(chrome.metallic, Some(0.85), "Metalness de materials_interior.ini");
        // `1 - 0.95` ne tombe pas rond en `f32`, d'où la tolérance.
        let roughness = chrome.roughness.expect("le chrome donne sa brillance");
        assert!(
            (roughness - 0.05).abs() < 1e-5,
            "Smoothness 0,95 devient une rugosité de 0,05, got {roughness}"
        );

        // Le carbone porte un vernis, et c'est ce qui le distingue d'un
        // plastique sombre.
        let carbon = collected("[Material_Carbon]\nMaterials = weave\n");
        assert_eq!(
            carbon.materials[0].1.clearcoat,
            Some((1.0, 0.1)),
            "UseClearCoat = 1, ClearCoatSmoothness = 0.9"
        );
    }

    // Règle : **la famille `_v2` ne donne pas sa brillance.** Ces templates
    // posent `Smoothness = 0` avant mise à l'échelle par une texture PBR de
    // détail qu'on ne charge pas ; transcrire ce zéro rendrait tous les
    // chromes parfaitement mats, strictement pire que notre propre estimation.
    // Leur métallicité, elle, est une constante ordinaire.
    #[test]
    fn the_detail_driven_family_only_gives_its_metalness() {
        let over = collected("[Material_Metal_v2]\nMaterials = trim\n");
        let value = over.materials[0].1;
        assert_eq!(value.metallic, Some(0.8), "la métallicité est une constante");
        assert_eq!(
            value.roughness, None,
            "la brillance vient d'une texture qu'on ne charge pas : on n'y touche pas"
        );
    }

    // Règle : ce que la section écrit l'emporte sur ce que son template pose.
    // C'est l'usage courant — `ks_toyota_ae86_tuned` ramène son métal de 0,8 à
    // 0,25 de cette façon.
    #[test]
    fn a_section_overrides_the_template_it_uses() {
        let over = collected("[Material_Metal_v2]\nMaterials = int_metal\nMetalness=0.25\n");
        assert_eq!(over.materials[0].1.metallic, Some(0.25), "0,25 et non 0,8");

        let smooth = collected("[Material_Leather]\nMaterials = seat\nSmoothness=0.2\n");
        assert_eq!(
            smooth.materials[0].1.roughness,
            Some(0.8),
            "Smoothness explicite, converti en rugosité"
        );
    }

    // Règle : une déclaration qu'on ne sait pas traduire ne produit rien — le
    // traitement habituel reste en place, et rien n'est payé pour rien.
    #[test]
    fn an_unknown_template_changes_nothing() {
        assert!(
            collected("[Material_Fur]\nMaterials = rug\n").is_empty(),
            "un template qu'on ne sait pas traduire : on n'invente pas"
        );
        assert!(
            collected("[LIGHTING]\nMaterials = body\n").is_empty(),
            "une section qui n'est pas un matériau ne déclare rien"
        );
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
