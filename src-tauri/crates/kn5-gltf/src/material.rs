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
    /// Fraction de lumière qui traverse la surface — `KHR_materials_transmission`.
    /// Zéro sur tout ce qui n'est pas du verre déclaré tel quel.
    pub transmission: f32,
    /// `KHR_materials_ior`, quand il y a lieu.
    pub ior: Option<f32>,
    /// Vernis par-dessus la surface — `KHR_materials_clearcoat`. Zéro sur tout
    /// ce qui n'en porte pas.
    pub clearcoat: f32,
    pub clearcoat_roughness: f32,
}

/// Shaders whose name alone says the surface is glass.
const GLASS_MARKERS: [&str; 3] = ["Glass", "Windscreen", "windscreen"];

/// Le nom du shader suffit-il à dire que la surface est du verre ?
///
/// Partagé par [`alpha_mode_of`] (qui décide *si* on fond) et [`convert`]
/// (qui décide *avec quelle opacité*) — les deux doivent s'accorder sur ce
/// qui est vraiment du verre, pas seulement sur ce qui a `blend_mode = 1`
/// (voir la note sur `senna_head` dans [`convert`]).
fn is_glass_shader(shader: &str) -> bool {
    GLASS_MARKERS.iter().any(|marker| shader.contains(marker))
}

/// Seuil de découpe d'un matériau alpha-testé.
///
/// **Un `ksAlphaRef` nul veut dire « non réglé », pas « ne découpe rien ».**
/// En glTF un fragment passe dès que `alpha >= alphaCutoff`, donc un seuil à 0
/// laisse passer jusqu'aux pixels parfaitement transparents — le masque ne
/// masque plus rien. Bug réel sur `j8_mitsubishi_gto_twin_turbo_91`, où huit
/// matériaux écrivent `ksAlphaRef = 0` : les lignes de dégivrage de la lunette
/// arrière (`window_heater_lines.dds`, 87,5 % des pixels à alpha 0 avec de
/// l'orange dessous) se rendaient en panneau orange plein, et tout l'arrière de
/// la voiture avec. Le jeu, lui, découpe bien — sa valeur par défaut n'est pas
/// zéro. Un zéro explicite prend donc le **même** défaut qu'une valeur absente.
///
/// **Et au-delà de 1, ce n'est plus une fraction : c'est un octet.** Mesuré sur
/// 62 voitures et les 52 mannequins de l'installation : côté voiture la
/// propriété tient dans [0, 1] (Kunos écrit 0,1 · 0,2 · 0,5 · 0,9), côté
/// mannequin les mods écrivent 3 · 10 · 20 · 30 · 50 · **100** · 1000 — quatre
/// vingt-dix-sept matériaux, l'échelle 0-255 de l'alpha lui-même. Ramenés à 1,
/// ils ne laissaient passer que les texels parfaitement opaques : les cheveux
/// d'`ada.kn5` (`ksAlphaRef = 100`) se rendaient en mèches déchiquetées
/// laissant voir le crâne au travers, là où le jeu les montre pleins (capture
/// à l'appui). Ramenée sur 255, la même valeur donne 0,39 — un seuil ordinaire.
/// Au-delà de 255, plus aucune lecture ne tient : on reprend le défaut.
fn alpha_cutoff_of(material: &Kn5Material) -> f32 {
    match material.property("ksAlphaRef") {
        Some(reference) if reference > 0.0 && reference <= 1.0 => reference,
        Some(reference) if reference > 1.0 && reference <= 255.0 => reference / 255.0,
        _ => DEFAULT_ALPHA_CUTOFF,
    }
}

/// Seuil de découpe quand le matériau n'en donne pas d'utilisable.
const DEFAULT_ALPHA_CUTOFF: f32 = 0.5;

/// Mode de transparence d'un matériau, et son seuil de découpe.
///
/// Extrait de [`convert`] parce que le pipeline de textures a besoin de la
/// même réponse : c'est lui qui décide de conserver ou non le canal alpha
/// d'une texture, et il ne peut le faire qu'en sachant si un matériau
/// s'en sert vraiment.
pub(crate) fn alpha_mode_of(material: &Kn5Material) -> (AlphaMode, f32) {
    let shader = material.shader.as_str();
    let is_glass = is_glass_shader(shader);
    if material.blend_mode == 1 || is_glass {
        (AlphaMode::Blend, 0.5)
    } else if material.alpha_tested {
        // Le drapeau décodé est le signal fiable ici, plus que `ksAlphaRef > 0`
        // — voir docs/kn5-format.md, §12 q2.
        (AlphaMode::Mask, alpha_cutoff_of(material))
    } else {
        (AlphaMode::Opaque, 0.5)
    }
}

/// Ce que le pipeline de textures apprend d'un matériau et que la conversion
/// ne peut pas deviner seule.
#[derive(Debug, Clone, Default)]
pub struct MaterialTextures {
    /// La texture diffuse porte-t-elle un alpha **qui varie**, donc une
    /// découpe ? Un alpha constant est une opacité, pas un masque — voir
    /// `PreparedTexture::alpha_varies`.
    pub diffuse_alpha_varies: bool,
    /// Ce matériau n'échantillonne-t-il **que** des texels transparents ?
    /// L'alpha le ferait alors disparaître au lieu de le découper — voir
    /// `FootprintAlpha::is_blank`.
    pub diffuse_alpha_blank: bool,
    /// Ce matériau n'échantillonne-t-il **que** des texels opaques ? Un
    /// matériau en fondu (`blend_mode = 1`) qui n'est pas du verre et dont
    /// l'alpha ne dit rien ne devrait pas fondre pour autant — voir
    /// `FootprintAlpha::is_opaque` et la note sur `senna_head` dans
    /// [`convert`].
    pub diffuse_alpha_opaque: bool,
    /// L'alpha de ce matériau **découpe** au lieu de fondre — voir
    /// `FootprintAlpha::is_cutout` et la note sur le numéro de portière de
    /// `rss_gtm_lanzo_v10` dans [`convert`].
    pub diffuse_alpha_cutout: bool,
    /// Variante peinte de la texture diffuse, quand la carte de détail du
    /// matériau porte une couleur de peinture (voir [`crate::paint`]).
    pub painted_diffuse: Option<String>,
    /// Carte métallique-rugosité tirée de `txMaps` (voir [`crate::roughness`]).
    pub roughness_texture: Option<String>,
    /// Ce que la configuration CSP dit de ce matériau (voir
    /// [`crate::SurfaceOverride`]).
    ///
    /// Ce ne sont pas des faits de texture comme les autres champs de cette
    /// structure, mais ils empruntent le même chemin : ce sont des choses que
    /// le pipeline apprend du mod et que la conversion ne peut pas deviner du
    /// seul KN5.
    pub csp: Option<crate::SurfaceOverride>,
}

/// Réflectance à incidence normale à partir de laquelle une surface commence
/// à être traitée comme un métal, puis celle où elle l'est pleinement.
///
/// **Mesurées, pas choisies.** Sur les voitures Kunos — les auteurs des
/// shaders, donc la référence — `fresnelC` couvre 0,02 à 0,40 et ne dépasse
/// **jamais** 0,5 (p99 = 0,40 sur 3 381 matériaux). Les diélectriques y sont
/// groupés bas : cuir 0,00, plastique 0,01, carbone 0,04, peinture et jantes
/// 0,05. Le chrome est seul au-dessus, à 0,20 de médiane et 0,50 de troisième
/// quartile. Le plancher est donc posé au-dessus du groupe diélectrique, et le
/// plafond au sommet de la plage que Kunos s'autorise.
const METALLIC_F0_FLOOR: f32 = 0.10;
const METALLIC_F0_FULL: f32 = 0.40;

/// Au-delà, `fresnelC` ne dit plus rien du tout — voir la mesure dans
/// [`metallic_of`]. Les valeurs relevées au-dessus de 1 sont 1,2 · 1,3 · 1,4 ·
/// 1,5 · 2, puis 3 · 5 · 5,5 · 10 · 12 · 100 : la coupure est posée là où la
/// suite cesse d'être serrée autour du maximum.
const METALLIC_F0_ABSURD: f32 = 2.0;

/// Niveau de réflexion en dessous duquel une surface ne peut pas être un
/// métal, quelle que soit sa `fresnelC`.
///
/// `fresnelMaxLevel` est le plafond du reflet. Sans ce veto, la 250 GTO
/// ressortait avec un **tapis de sol** et des **coutures de cuir** métalliques
/// à 0,33 : leur `fresnelC` vaut bien 0,20, mais leur `fresnelMaxLevel` vaut
/// 0,02, c'est-à-dire « cette surface ne renvoie rien ». Le seuil sépare ce
/// groupe (0,01 à 0,05 : tapis, coutures, plastique, cuir) du chrome et des
/// optiques (0,40 à 1,00), sans toucher à la peinture — elle passe le veto
/// avec 0,50, mais sa `fresnelC` de 0,05 la laisse diélectrique, ce qui est
/// exactement ce qu'est un vernis.
const METALLIC_MIN_REFLECTION: f32 = 0.15;

/// Métallicité d'un matériau, tirée de `fresnelC`.
///
/// `fresnelC` **est** la réflectance à incidence normale — F0 — c'est-à-dire
/// exactement la grandeur qu'encode le modèle métallique-rugosité de glTF :
/// un diélectrique vaut 0,04, un métal la couleur de sa base. C'est la seule
/// donnée du KN5 qui décrive la réflectivité d'une surface, elle est écrite
/// délibérément (82 % des matériaux la portent), et l'ordre qu'elle produit
/// est celui de la physique.
///
/// **Ce qu'elle remplace** : `metallicFactor` était tenu à zéro faute de
/// savoir lire R et B de `txMaps` — question tranchée depuis, par la négative
/// (`docs/kn5-format.md`, écart n°7). Conséquence visible jusqu'ici : le
/// chrome, les jantes et le métal nu rendaient comme de la peinture brillante.
///
/// Deux familles sont exclues quoi qu'annonce leur `fresnelC`. Le **vitrage**,
/// diélectrique très réfléchissant que la métallicité rendrait opaque et
/// teinté — c'est le contraire d'une vitre. Et le **caoutchouc**, pour la même
/// raison qu'au §6.3 : un pneu n'est jamais un miroir, quoi qu'en dise son
/// matériau.
fn metallic_of(material: &Kn5Material, shader: &str) -> f32 {
    if GLASS_MARKERS.iter().any(|marker| shader.contains(marker)) || shader.contains("ksTyres") {
        return 0.0;
    }
    let Some(f0) = material.property("fresnelC") else {
        return 0.0;
    };
    // Le veto du plafond de reflet, avant tout calcul : une surface qui ne
    // renvoie rien n'est pas un métal, même si elle annonce une réflectance
    // franche à incidence normale.
    let ceiling = material.property("fresnelMaxLevel").unwrap_or(0.0);
    if ceiling < METALLIC_MIN_REFLECTION {
        return 0.0;
    }
    // Un plafond de reflet impossible annule le bloc entier, comme le fait
    // plus bas une `fresnelC` aberrante : `fresnelMaxLevel = 100` est un
    // gabarit recopié, présent à l'identique sur cinq mannequins d'auteurs
    // différents (`ada`, `jill_re3`, `rinoa`, `Sienna_Guillory`, `t-800`),
    // toujours avec `fresnelC = 0.5`. Rendus pleinement métalliques, ces
    // matériaux mettent une coque de chrome sur des vêtements et des visages —
    // le haut du corps de `jill_re3` en entier, signalé deux fois à l'écran.
    //
    // **Ce que ça coûte, et pourquoi on l'accepte quand même.** Sur `t-800`,
    // le même gabarit porte le corps d'un endosquelette de Terminator, qui est
    // réellement en chrome : la règle le rend mat. Les deux matériaux sont
    // **identiques propriété par propriété** — `ksAmbient` 0,01, `ksDiffuse` 1,
    // `ksSpecular` 150, `fresnelC` 0,5, `fresnelMaxLevel` 100, même shader,
    // même mode de fusion, seul l'exposant spéculaire diffère (400 contre
    // 1000). Rien ici ne peut les séparer.
    //
    // La seule chose qui les distingue est la **part du modèle** qui porte la
    // signature — 83 % sur `t-800`, 6 à 14 % sur les autres — mais
    // `Sienna_Guillory`, une humaine, est à 56 % : la ligne tiendrait sur deux
    // points, dont un qui deviendrait une femme en chrome. Décision prise avec
    // l'utilisateur : garder les nombreux justes plutôt que le cas unique,
    // faute de critère qui tienne.
    if ceiling > METALLIC_F0_ABSURD {
        return 0.0;
    }
    // Au-delà de 1, ce n'est plus une réflectance — mais tout ce qui dépasse ne
    // se vaut pas, et les traiter pareil donnait des mannequins en or.
    //
    // **Mesuré sur tout le corpus** (14 558 matériaux de voiture, 611 de
    // mannequin) : 1,2 à 2 est le geste d'un moddeur qui pousse le curseur
    // au-delà du maximum — 163 matériaux, toujours des valeurs rondes et
    // proches (1,2 · 1,3 · 1,4 · 1,5 · 2). Au-delà, ce n'est plus un geste :
    // 5, 10, 12, 100 apparaissent **chez Kunos aussi** (`lotus_elise_sc`,
    // `mercedes_sls`, `ks_ford_escort_mk1`), sur un matériau isolé d'une
    // voiture par ailleurs normale. Un studio qui écrit 100 dans un champ dont
    // il a écrit le shader n'exprime pas « miroir parfait » : le shader ignore
    // la valeur, et personne ne l'a jamais vue à l'écran.
    //
    // Bug réel : `ada.kn5` porte `fresnelC = 100` sur son visage, son torse,
    // ses jambes et ses yeux. Ramenés à 1, ils devenaient pleinement
    // métalliques — une statue dorée, signalée par l'utilisateur. Au-dessus du
    // seuil, la propriété est donc traitée comme **absente** (diélectrique),
    // ce qui est le repli sûr : un matériau sans `fresnelC` ne brille pas.
    if f0 > METALLIC_F0_ABSURD {
        return 0.0;
    }
    let f0 = f0.clamp(0.0, 1.0);
    ((f0 - METALLIC_F0_FLOOR) / (METALLIC_F0_FULL - METALLIC_F0_FLOOR)).clamp(0.0, 1.0)
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

    // **Le verre que CSP déclare est du verre physique, pas un fondu.**
    //
    // `[Material_Glass]` remplace le shader par `smGlass`, dont la
    // transparence vient d'un indice de réfraction et d'une épaisseur — jamais
    // d'un canal alpha. Le rendre en fondu revient à atténuer *toute* la
    // réponse du matériau, reflet spéculaire compris : une vitre à 15 %
    // d'opacité ne renvoie que 15 % de son reflet, et c'est ce reflet qui fait
    // qu'une vitre ressemble à une vitre. La transmission, elle, laisse passer
    // le fond en gardant le reflet entier.
    //
    // La texture diffuse est écartée pour la même raison qu'un `ksWindscreen`
    // (écart n°6) : `smGlass` ne s'en sert pas comme d'une couleur, et la
    // garder pose un voile teinté devant l'habitacle.
    if let Some(ior) = textures.csp.and_then(|c| c.glass_ior) {
        return GltfMaterial {
            name: material.name.clone(),
            shader: material.shader.clone(),
            base_color_texture: None,
            normal_texture: normal_map(material),
            roughness_texture: None,
            normal_scale: material.property("normalMult").filter(|v| *v > 0.0).unwrap_or(1.0),
            emissive: [0.0; 3],
            // Une vitre est lisse, et sa réflectance vient de l'IOR, pas d'une
            // métallicité : glTF dérive le F0 de `ior` exactement comme la
            // formule de Schlick que `materials_glass.ini` applique.
            roughness: 0.02,
            metallic: 0.0,
            // La transmission porte la transparence, donc la surface est
            // opaque au sens du tri glTF — et n'a plus besoin d'être triée
            // après coup.
            alpha_mode: AlphaMode::Opaque,
            double_sided: false,
            alpha_cutoff,
            base_color: [1.0, 1.0, 1.0, 1.0],
            transmission: 1.0,
            ior: Some(ior),
            clearcoat: 0.0,
            clearcoat_roughness: 0.0,
        };
    }

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
    //
    // **Troisième endroit, hors verre** : `blend_mode = 1` sur un matériau qui
    // n'est ni du verre ni une découpe. Mesuré sur `senna.kn5` (mod de pilote
    // tiers) — le matériau `senna_head`, shader `ksSkinnedMesh_NMDetaill`
    // ordinaire, porte `blend_mode = 1` sans raison identifiée, et sa diffuse
    // est uniformément opaque (empreinte constante à 255). L'approximer comme
    // une vitre (`glass_opacity`, dérivée de `ksDiffuse = 0.3`) rendait la
    // tête entière translucide. Le verre garde son approximation — c'est elle
    // qui donne une vitre plausible sans reflet à afficher — mais un matériau
    // qui n'est pas du verre et dont l'alpha dit « rien à fondre » n'a pas de
    // raison de la subir.
    // **Ce que le mod déclare l'emporte sur ce qu'on a déduit** — mais
    // seulement là où il déclare quelque chose. Un template de CSP dont la
    // brillance dépend d'une texture de détail qu'on ne charge pas laisse
    // `roughness` à `None`, et l'estimation tirée de `ksSpecularEXP` reste en
    // place : mieux vaut une estimation qu'une valeur juste à moitié.
    let csp = textures.csp.unwrap_or_default();
    let roughness = csp.roughness.unwrap_or(roughness);
    let metallic = csp.metallic.unwrap_or_else(|| metallic_of(material, shader));
    let csp_clearcoat = csp.clearcoat.unwrap_or((0.0, 0.0));

    let base_color_texture = textures.painted_diffuse.clone().or_else(|| base_color_map(material));
    // Troisième cas, découvert après les deux ci-dessus : l'alpha varie, mais
    // pas là où **ce** matériau regarde. Un atlas de carrosserie porte un
    // masque de peinture (écart n°5), et une vitre qui partage cet atlas
    // n'échantillonne que des texels à zéro. Pris pour une découpe, il ne
    // découpe pas la vitre : il l'efface. Un maillage qu'un auteur a pris la
    // peine de modéliser n'est jamais censé être invisible.
    let texture_carries_alpha =
        base_color_texture.is_some() && textures.diffuse_alpha_varies && !textures.diffuse_alpha_blank;
    // Uniquement hors verre : un vrai `ksWindscreen`/`*Glass` garde son
    // approximation même si son alpha se mesure opaque, pour la raison ci-
    // dessus (la transparence d'une vitre vient du reflet, pas de l'alpha).
    let opaque_by_alpha = !is_glass_shader(shader) && textures.diffuse_alpha_opaque;
    // **Et il est opaque pour de bon, pas seulement à opacité 1.** Le laisser
    // en fondu avec un alpha plein donnait la bonne couleur mais gardait la
    // mécanique du transparent : trié après l'opaque, et — sur le plateau du
    // pilote — rendu sans écrire la profondeur, comme une visière. La tête de
    // `senna.kn5` passait ainsi par-dessus tout, un morceau d'oreille restant
    // visible à travers le visage quel que soit l'angle (bug réel, signalé à
    // l'écran). Un matériau dont l'alpha ne découpe rien n'a rien à faire dans
    // la passe transparente.
    let alpha_mode = if alpha_mode == AlphaMode::Blend && opaque_by_alpha {
        AlphaMode::Opaque
    } else {
        alpha_mode
    };
    // **Et le symétrique : un alpha qui ne fait que découper n'est pas un
    // fondu.** Même image — il ne vaut que 0 ou 255 —, mais rendue dans la
    // passe opaque, où la profondeur arbitre. En fondu, elle ne le fait pas :
    // §8.2 retire l'écriture de profondeur à tout le transparent, pour que
    // l'habitacle reste visible derrière le pare-brise. C'est juste pour une
    // vitre et faux pour une décalcomanie, qui se retrouve départagée par
    // l'ordre des matériaux dans le fichier au lieu de sa position.
    //
    // Bug réel, `rss_gtm_lanzo_v10` : le numéro de portière, posé 2,3 mm
    // devant sa plaque, était recouvert par le calque de décalcomanies de
    // toute la voiture, dessiné après lui — et seulement d'un côté, les deux
    // portières n'échantillonnant pas la même région de l'atlas. Mesuré sur le
    // banc : le numéro passait de 4 558 à 119 pixels selon l'angle.
    let cutout = !is_glass_shader(shader) && textures.diffuse_alpha_cutout;
    let (alpha_mode, alpha_cutoff) = if alpha_mode == AlphaMode::Blend && cutout {
        (AlphaMode::Mask, DEFAULT_ALPHA_CUTOFF)
    } else {
        (alpha_mode, alpha_cutoff)
    };
    let base_color = match alpha_mode {
        AlphaMode::Blend if !texture_carries_alpha && !opaque_by_alpha => [1.0, 1.0, 1.0, glass_opacity(material)],
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
        metallic,
        alpha_mode,
        // Glass rendered double-sided shows the inside of the far pane through
        // the near one; §6.1 asks for single-sided there specifically.
        double_sided: false,
        alpha_cutoff,
        base_color,
        transmission: 0.0,
        ior: None,
        clearcoat: csp_clearcoat.0,
        clearcoat_roughness: csp_clearcoat.1,
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

/// Nom de la propriété par laquelle un matériau annonce que sa `txNormal` est
/// en espace **objet** et non en espace tangent (voir [`normal_map`]).
const OBJECT_SPACE_NORMALS: &str = "nmObjectSpace";

/// Normal map d'un matériau, **sauf quand c'en est une de dégâts, et sauf
/// quand elle est en espace objet**.
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
///
/// **L'espace objet est le second cas, et glTF ne sait pas le lire** : son
/// `normalTexture` est définie en espace tangent, point. Lui donner une carte
/// objet fait dépendre l'erreur d'éclairage de l'orientation de la surface —
/// le sommet d'un casque, où la normale objet pointe vers le haut, s'aplatit
/// et s'éteint pendant que les flancs restent à peu près justes. C'est le
/// défaut remonté par l'utilisateur sur le casque du pilote.
///
/// **Mesuré, pas déduit du nom du fichier.** `HELMET_2012_OS.dds` a une
/// moyenne RGB de (127, 115, 147) pour un écart-type de (77, 74, 63) : elle
/// balaie tout le cube, comme une carte objet. Les quatre cartes tangentes du
/// même mannequin sont à (127, 127, 254) avec un écart-type de 5 sur le bleu —
/// une normale qui ne s'écarte guère de la surface, comme le veut l'espace
/// tangent.
///
/// **Et ça ne concerne que les mannequins** : sur l'installation de référence,
/// les cinq matériaux qui déclarent `nmObjectSpace = 1` sont les casques des
/// mannequins Kunos, et pas un seul matériau de voiture (mesuré sur cinq
/// voitures, entre 13 et 38 matériaux chacune à déclarer la propriété, toujours
/// à 0).
///
/// La carte est **abandonnée** plutôt que convertie : reconstruire une carte
/// tangente demanderait un repère par sommet et un recuit complet, pour un
/// casque lisse dont la géométrie porte déjà l'essentiel du relief. Une carte
/// qu'on ne sait pas lire vaut moins que pas de carte du tout.
fn normal_map(material: &Kn5Material) -> Option<String> {
    if material.shader.to_ascii_lowercase().contains("damage") {
        return None;
    }
    if material.property(OBJECT_SPACE_NORMALS).is_some_and(|v| v != 0.0) {
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
    material
        .property("ksDiffuse")
        .unwrap_or(0.3)
        .clamp(GLASS_MIN_OPACITY, 0.6)
}

/// Plancher d'opacité d'une vitre.
///
/// Chez AC une vitre est presque parfaitement transparente (`ksDiffuse = 0.1`
/// sur la Supra) et ce qu'on en voit vient de la **réflexion** : son shader y
/// met un fresnel fort (`fresnelMaxLevel = 0.7`). Notre studio étant sombre, il
/// n'y a presque rien à réfléchir, et une vitre honnête devient une vitre
/// absente — signalée telle quelle par l'utilisateur. Ce plancher lui rend une
/// présence, faute de pouvoir lui rendre son reflet.
const GLASS_MIN_OPACITY: f32 = 0.15;

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

    // Règle : un `ksAlphaRef` nul veut dire « non réglé », pas « ne découpe
    // rien ». En glTF un fragment passe dès que `alpha >= alphaCutoff`, donc
    // un seuil à zéro laisse passer jusqu'aux pixels parfaitement
    // transparents. Bug réel sur `j8_mitsubishi_gto_twin_turbo_91` : les
    // lignes de dégivrage de la lunette arrière, dont la texture est à 87,5 %
    // transparente avec de l'orange dessous, se rendaient en panneau orange
    // plein — tout l'arrière de la voiture avec.
    #[test]
    fn a_zero_alpha_reference_falls_back_to_the_default_cutoff() {
        let unset = material("ksPerPixelAT", 0, true, &[("ksAlphaRef", 0.0)]);
        assert_eq!(
            alpha_mode_of(&unset).1,
            DEFAULT_ALPHA_CUTOFF,
            "un zéro explicite ne doit pas désarmer la découpe"
        );

        let absent = material("ksPerPixelAT", 0, true, &[]);
        assert_eq!(
            alpha_mode_of(&absent).1,
            DEFAULT_ALPHA_CUTOFF,
            "absente, la valeur prend le même défaut"
        );

        let set = material("ksPerPixelAT", 0, true, &[("ksAlphaRef", 0.3)]);
        assert_eq!(alpha_mode_of(&set).1, 0.3, "une valeur utilisable est respectée");
    }

    /// Règle : au-delà de 1, `ksAlphaRef` est un octet, pas une fraction.
    ///
    /// Bug réel sur les cheveux d'`ada.kn5` (`ksAlphaRef = 100`) : ramenés à 1,
    /// seuls les texels parfaitement opaques passaient, et les mèches se
    /// rendaient déchiquetées avec le crâne visible au travers.
    #[test]
    fn an_alpha_reference_above_one_is_read_as_a_byte() {
        let hair = material("ksSkinnedMesh_NMDetaill", 0, true, &[("ksAlphaRef", 100.0)]);
        let cutoff = alpha_mode_of(&hair).1;
        assert!(
            (cutoff - 100.0 / 255.0).abs() < 1e-6,
            "100 se lit sur 255, soit 0,39 (obtenu {cutoff})"
        );

        let absurd = material("ksSkinnedMesh_NMDetaill", 0, true, &[("ksAlphaRef", 1000.0)]);
        assert_eq!(
            alpha_mode_of(&absurd).1,
            DEFAULT_ALPHA_CUTOFF,
            "au-delà de 255, aucune lecture ne tient : le défaut"
        );

        let kunos = material("ksPerPixelAT", 0, true, &[("ksAlphaRef", 1.0)]);
        assert_eq!(alpha_mode_of(&kunos).1, 1.0, "une valeur pile à 1 reste une fraction");
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
                    diffuse_alpha_varies: true,
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

    // Règle : un matériau en fondu qui n'est pas du verre, dont l'alpha se
    // mesure uniformément opaque, rend opaque — pas l'approximation de verre.
    //
    // Bug réel sur `senna.kn5` (mod de pilote tiers) : le matériau de la tête,
    // shader ordinaire mais `blend_mode = 1`, se rendait à 30 % d'opacité
    // (`ksDiffuse`) faute de mieux. Le verre, lui, garde son approximation :
    // c'est elle qui lui donne un aspect plausible.
    // Règle : un `blend_mode = 1` dont l'alpha ne fait que découper se rend en
    // `MASK`, dans la passe opaque. Bug réel, `rss_gtm_lanzo_v10` : en fondu,
    // le numéro de portière n'écrivait pas la profondeur (§8.2) et le calque de
    // décalcomanies de toute la voiture, dessiné après lui, le recouvrait —
    // 4 558 pixels d'un côté de la voiture, 119 de l'autre.
    #[test]
    fn a_cutout_alpha_leaves_the_transparent_pass() {
        let decal = material("ksPerPixelNM_UVMult", 1, false, &[("ksDiffuse", 0.4)]);
        let converted = convert(
            &decal,
            MaterialTextures {
                diffuse_alpha_varies: true,
                diffuse_alpha_cutout: true,
                ..Default::default()
            },
        );
        assert_eq!(
            converted.alpha_mode,
            AlphaMode::Mask,
            "une découpe se rend dans la passe opaque, où la profondeur arbitre"
        );

        // Et le verre garde son fondu : sa transparence vient du reflet, pas
        // de l'alpha, et le cockpit doit rester visible derrière (§8.2).
        let glass = material("ksWindscreen", 1, false, &[("ksDiffuse", 0.3)]);
        assert_eq!(
            convert(
                &glass,
                MaterialTextures {
                    diffuse_alpha_varies: true,
                    diffuse_alpha_cutout: true,
                    ..Default::default()
                }
            )
            .alpha_mode,
            AlphaMode::Blend,
            "une vitre reste en fondu, quoi que dise son alpha"
        );
    }

    #[test]
    fn opaque_footprint_overrides_the_glass_approximation_off_glass() {
        let head = material("ksSkinnedMesh_NMDetaill", 1, false, &[("ksDiffuse", 0.3)]);
        let converted = convert(
            &head,
            MaterialTextures {
                diffuse_alpha_opaque: true,
                ..Default::default()
            },
        );
        assert_eq!(
            converted.base_color[3], 1.0,
            "alpha uniformément opaque : rien à fondre, malgré blend_mode = 1"
        );
        assert_eq!(
            converted.alpha_mode,
            AlphaMode::Opaque,
            "et opaque pour de bon : pas de tri transparent, pas de profondeur ignorée"
        );
        let glass = material("ksWindscreen", 1, false, &[("ksDiffuse", 0.3)]);
        assert!(
            convert(
                &glass,
                MaterialTextures {
                    diffuse_alpha_opaque: true,
                    ..Default::default()
                }
            )
            .base_color[3]
                < 1.0,
            "le verre garde son approximation même si son alpha se mesure opaque"
        );
    }

    /// Règle : une normal map en espace objet n'est pas exportée. `normalTexture`
    /// de glTF est définie en espace tangent, et lui donner une carte objet
    /// éteint le sommet d'un casque tout en laissant ses flancs à peu près
    /// justes — bug réel remonté par l'utilisateur.
    #[test]
    fn object_space_normal_maps_are_dropped() {
        let mut helmet = material("ksPerPixelMultiMap", 0, false, &[("nmObjectSpace", 1.0)]);
        helmet.samplers.push(kn5::Kn5Sampler {
            name: "txNormal".to_string(),
            slot: 1,
            texture: "HELMET_2012_OS.dds".to_string(),
        });
        assert_eq!(
            convert(&helmet, MaterialTextures::default()).normal_texture,
            None,
            "une carte que glTF ne sait pas lire vaut moins que pas de carte"
        );

        let mut tangent = material("ksPerPixelMultiMap", 0, false, &[("nmObjectSpace", 0.0)]);
        tangent.samplers.push(kn5::Kn5Sampler {
            name: "txNormal".to_string(),
            slot: 1,
            texture: "DRIVER_Face_NM.dds".to_string(),
        });
        assert_eq!(
            convert(&tangent, MaterialTextures::default()).normal_texture.as_deref(),
            Some("DRIVER_Face_NM.dds"),
            "la propriété à zéro, elle, ne retire rien"
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

    // Règle : la métallicité vient de `fresnelC`, la réflectance à incidence
    // normale, et le seuil est celui mesuré sur les voitures Kunos — les
    // diélectriques (peinture, plastique, cuir) restent à zéro, le chrome
    // monte (docs/kn5-format.md, écart n°10).
    #[test]
    fn metallic_follows_fresnel_reflectance() {
        let with = |f0: f32| {
            material(
                "ksPerPixelMultiMap",
                0,
                false,
                &[("fresnelC", f0), ("fresnelMaxLevel", 0.7)],
            )
        };
        assert_eq!(
            metallic_of(&with(0.05), "ksPerPixelMultiMap"),
            0.0,
            "peinture et jantes Kunos (0,05) restent diélectriques"
        );
        assert_eq!(
            metallic_of(&with(0.01), "ksPerPixelMultiMap"),
            0.0,
            "le plastique aussi"
        );
        assert!(
            metallic_of(&with(0.20), "ksPerPixelMultiMap") > 0.2,
            "le chrome médian Kunos devient franchement métallique"
        );
        assert_eq!(
            metallic_of(&with(0.40), "ksPerPixelMultiMap"),
            1.0,
            "le haut de la plage Kunos est un métal plein"
        );
        assert_eq!(
            metallic_of(&with(1.2), "ksPerPixelMultiMap"),
            1.0,
            "un moddeur qui pousse le curseur juste au-delà du maximum est entendu"
        );
        assert_eq!(
            metallic_of(&with(100.0), "ksPerPixelMultiMap"),
            0.0,
            "une valeur absurde ne dit rien : diélectrique, pas miroir (ada.kn5 en statue dorée)"
        );

        // Le même veto sur l'autre moitié du bloc fresnel : le gabarit
        // `fresnelC = 0.5` / `fresnelMaxLevel = 100` des maillages de
        // brillance des mannequins, qui chromait le haut du corps de
        // `jill_re3`. Il chromait aussi, à raison, le Terminator `t-800` —
        // perte assumée, faute de pouvoir les distinguer (voir `metallic_of`).
        let shine = material(
            "ksSkinnedMesh",
            0,
            false,
            &[("fresnelC", 0.5), ("fresnelMaxLevel", 100.0)],
        );
        assert_eq!(
            metallic_of(&shine, "ksSkinnedMesh"),
            0.0,
            "un plafond de reflet impossible annule le bloc entier"
        );
    }

    // Règle : deux surfaces ne deviennent jamais métalliques, quoi qu'annonce
    // leur `fresnelC` — une vitre métallique est opaque, un pneu n'est pas un
    // miroir (§6.3).
    #[test]
    fn glass_and_rubber_are_never_metallic() {
        let shiny = material("ksWindscreen", 1, false, &[("fresnelC", 0.9), ("fresnelMaxLevel", 1.0)]);
        assert_eq!(
            metallic_of(&shiny, "ksWindscreen"),
            0.0,
            "un pare-brise reste transparent"
        );
        let tyre = material("ksTyres", 0, false, &[("fresnelC", 0.9), ("fresnelMaxLevel", 1.0)]);
        assert_eq!(metallic_of(&tyre, "ksTyres"), 0.0, "un pneu reste du caoutchouc");
    }

    // Règle : une surface qui ne renvoie rien n'est pas un métal, même quand sa
    // réflectance à incidence normale est franche. Bug réel : le tapis de sol
    // et les coutures de la 250 GTO ressortaient métalliques (écart n°10).
    #[test]
    fn a_surface_that_reflects_nothing_is_never_metallic() {
        let carpet = material(
            "ksPerPixelMultiMap",
            0,
            false,
            &[("fresnelC", 0.20), ("fresnelMaxLevel", 0.02)],
        );
        assert_eq!(
            metallic_of(&carpet, "ksPerPixelMultiMap"),
            0.0,
            "un tapis à fresnelMaxLevel 0,02 ne renvoie rien, quelle que soit sa fresnelC"
        );
    }

    // Règle : sans `fresnelC` — 18 % des matériaux n'en portent pas — on ne
    // devine rien, la surface reste diélectrique.
    #[test]
    fn a_material_without_fresnel_stays_dielectric() {
        let plain = material("ksPerPixelMultiMap", 0, false, &[("ksSpecularEXP", 200.0)]);
        assert_eq!(
            metallic_of(&plain, "ksPerPixelMultiMap"),
            0.0,
            "rien de mesuré, rien de deviné"
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
