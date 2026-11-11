// Presets de vignettes de la grille (docs/SPEC-grille.md GRILLE§5.6, GRILLE§6).
//
// **Un preset, pas un gabarit unique.** La première version n'avait qu'un jeu
// de valeurs, et elle butait sur un constat de l'utilisateur : la preview
// d'origine d'Assetto Corsa est plus *jolie* que notre rendu, plus vitrine,
// alors que le nôtre est plus *lisible*. Ce ne sont pas deux qualités
// d'exécution du même objectif, ce sont deux objectifs — identifier vite, ou
// avoir envie de regarder. Un compromis unique les aurait mal servis tous les
// deux.
//
// Ça ne touche pas à la propriété non négociable du GRILLE§5.6 : « toute valeur doit
// rester identique pour les 312 ». Elle porte sur un jeu d'images, pas sur le
// nombre de jeux possibles — chaque preset reste uniforme chez lui.
//
// **Trois choix de structure, chacun contre une solution plus évidente :**
//
//  1. *Les embarqués sont en lecture seule, et on les duplique.* Un preset vide
//     est une douzaine de curseurs de rien ; une copie de Vitrine est à un
//     réglage d'être la sienne. Bénéfice inattendu : ça supprime tout le
//     versionnage du GRILLE§5.7 — plus besoin de deviner « l'utilisateur a-t-il
//     personnalisé ? » pour savoir s'il hérite du nouveau défaut. Les embarqués
//     évoluent avec l'app, les copies ne bougent jamais.
//  2. *Le mat fait partie du preset.* La moitié de ce qui rend la preview d'AC
//     belle est son **fond**, cuit dans l'image. Le nôtre est du CSS : c'est
//     donc là qu'il se règle. Un Vitrine à l'éclairage dramatique sur un fond
//     gris moyen ne donnerait que la moitié de l'effet, et la moins
//     spectaculaire.
//  3. *Le mat n'entre pas dans l'empreinte.* Il ne change aucun pixel du PNG —
//     changer une couleur de carte ne doit pas régénérer 312 images. Deux
//     presets aux gabarits identiques et aux mats différents partagent donc
//     leurs images, ce qui est exactement ce qu'on veut.
//
// Persistance dans `ui_prefs.json` (règle d'or n°6, jamais `localStorage`), une
// clé pour la liste des presets de l'utilisateur, sur le patron de
// `driverOutfits.svelte.ts` — une liste de quelques objets ne justifie pas un
// fichier Rust dédié.
import type { GridTemplate, LivePreset } from "./gridThumbs";
import { sweepGridTemplates } from "./gridThumbs";
import { t } from "$lib/i18n/index.svelte";
import { getUiPrefs, setUiPrefs } from "$lib/uiPrefs.svelte";
import { FEATURE_GRID_THUMBS } from "$lib/features";

const KEYS = {
  enabled: "pitbox.gridThumbs",
  /** Les presets de l'utilisateur, en JSON. Les embarqués vivent dans le code. */
  presets: "pitbox.gridThumbs.presets",
  /** Quel preset pour quelle densité de grille. */
  dense: "pitbox.gridThumbs.preset.dense",
  comfortable: "pitbox.gridThumbs.preset.comfortable",
} as const;

/**
 * Les huit valeurs de rendu, et leurs bornes.
 *
 * Un écart assumé vis-à-vis du GRILLE§5.6, qui demande de relever l'azimut sur les
 * previews Kunos plutôt que de l'inventer : c'est déjà fait, et c'est le défaut
 * de l'aperçu 3D — 318°, trois-quarts avant **gauche**, la convention de toutes
 * les photos du jeu. La grille restant mixte pour toujours (GRILLE§7), aligner
 * l'angle sur celui des previews d'origine est ce qui réduit le plus
 * durablement l'écart entre les deux sources.
 */
export const GRID_THUMB_RANGES = {
  /** Rotation de la caméra autour de l'axe vertical, en degrés. */
  azimuth: { min: 0, max: 359, step: 1, default: 320 },
  /** Plongée au-dessus de l'horizon. Basse : une vignette de catalogue montre
   * le profil d'une voiture, pas son toit. */
  elevation: { min: 0, max: 40, step: 1, default: 8 },
  /** Champ de vision vertical, en degrés. 22° est un équivalent long focal :
   * peu de distorsion, donc des proportions qui se comparent d'une voiture à
   * l'autre. */
  fov: { min: 10, max: 50, step: 1, default: 26 },
  /** Marge autour de la boîte englobante, en pourcentage. Le cadrage est
   * **ajusté et non à l'échelle** : chaque voiture remplit le cadre quelle que
   * soit sa taille réelle. On perd le gabarit relatif, on gagne que chaque
   * vignette est lisible à 190 px — pour un catalogue dont le métier est
   * l'identification, c'est le bon échange (GRILLE§5.6).
   *
   * **Ce n'est pas « le pourcentage de cadre laissé vide ».** À marge nulle, la
   * voiture n'occupe déjà que ~75 % de la largeur : le cadrage ajuste la
   * **boîte englobante en 3D**, projetée, qui est plus grande que la silhouette
   * visible — coins de la boîte, rétroviseurs, pare-chocs. C'est la constante
   * qui manquait pour traduire une mesure en réglage, et elle a coûté un
   * aller-retour : les 66 % relevés sur les images du jeu avaient été lus comme
   * « marge ≈ 50 % », ce qui donnait 51 % d'occupation, très en dessous. La
   * relation est `occupation ≈ 75 / (1 + marge/100)`, vérifiée à deux marges
   * (4 % → 72,7 %, 48 % → 50,8 %), donc viser 66 % demande **15 %**.
   *
   * La borne haute reste large : elle n'interdit plus rien, et un preset peut
   * vouloir laisser respirer. */
  margin: { min: 0, max: 70, step: 1, default: 6 },
  /** Intensité de la lumière principale, en pourcentage. */
  key: { min: 0, max: 300, step: 5, default: 100 },
  /** Complément, en pourcentage de la principale. */
  fill: { min: 0, max: 100, step: 5, default: 25 },
  /** Contre-jour, en pourcentage de la principale. **Le paramètre le plus
   * directement utile au problème de départ** : c'est lui qui détache la
   * silhouette d'une carrosserie noire, plus que n'importe quel réglage de
   * fond. */
  rim: { min: 0, max: 200, step: 5, default: 60 },
  /** Opacité de l'ombre de contact. Sans elle la voiture flotte. */
  shadow: { min: 0, max: 100, step: 5, default: 35 },
  /** Où la caméra vise, en pourcentage du rayon au-dessus du centre du modèle.
   *
   * **Négatif remonte la voiture dans le cadre**, positif la descend. Les
   * images du jeu la posent bas — mesuré sur les 178 officielles : marge haute
   * de 33 % contre 17 % en bas, centre à 58 % de la hauteur. Zéro viserait le
   * centre du modèle.
   *
   * The low placement does not need a positive value once the camera looks
   * down a little: at 10° of elevation and 0 here, Officiel already lands on
   * the game's 32 % / 16 % margins (measured, see that preset). */
  height: { min: -40, max: 40, step: 1, default: 8 },
  /** Flaque de lumière peinte au sol. **Zéro retire le sol entièrement**, et
   * l'image ne porte alors que la voiture — c'est ce qui garde une vignette de
   * catalogue parfaitement détourée, donc posable sur n'importe quel fond. */
  floor: { min: 0, max: 200, step: 5, default: 0 },
  /** Reflet de la voiture sur ce sol. **Il exige la flaque** : un reflet est
   * une modulation de la luminosité d'un sol, et il n'y a rien à moduler sur du
   * transparent. C'est la raison pour laquelle un preset qui reflète cuit une
   * part de son fond dans l'image, là où un preset de catalogue n'en cuit
   * aucune. */
  reflection: { min: 0, max: 100, step: 5, default: 0 },
  /** Flou du reflet, en **dixièmes** — la mécanique de ce module travaille sur
   * des entiers, et un dixième se voit à l'œil. C'est ce qui sépare le sol de
   * salon du sol mouillé de jeu vidéo, et c'est le réglage le plus visible d'un
   * preset de vitrine une fois le reflet allumé. Plafonné à 4 : au-delà, le
   * reflet n'est plus qu'une tache, et la plage haute ne servait qu'à rendre le
   * curseur imprécis là où il compte (même constat que sur l'aperçu de la
   * fiche). */
  reflectionBlur: { min: 0, max: 40, step: 1, default: 5 },
  /** Fond cuit dans l'image. **Zéro laisse la vignette détourée** et c'est le
   * cas normal : la carte fournit le fond, donc il suit le thème sans jamais
   * demander de régénérer. Au-dessus, le rendu porte son propre fond — la seule
   * façon qu'une vignette soit indiscernable d'une `preview.png` d'origine dans
   * la même grille, ce qui **retourne** le problème de la grille mixte au lieu
   * de le contenir. Il reprend les couleurs du mat du preset, pour que l'image
   * et sa carte ne puissent pas diverger.
   *
   * **Tout ou rien**, d'où le pas de 100 : un fond à moitié cuit empile deux
   * fonds l'un sur l'autre, ce qui n'est ni l'un ni l'autre des deux objectifs.
   * L'écran le présente donc en case à cocher. */
  background: { min: 0, max: 100, step: 100, default: 0 },
  /** **Same camera distance for every car**, as the game does, instead of
   * fitting each one to the frame: a small car then looks small next to a
   * large one. Measured on the game's previews, where an Abarth fills 45 % of
   * the width and a GT 70 %. Off by default — fitting is what keeps a catalogue
   * readable at 190 px — and all or nothing, so a checkbox. */
  trueScale: { min: 0, max: 100, step: 100, default: 0 },
  /** **Ambient light**: the showroom environment, which is what paint
   * reflects, in percent of the renderer's own level. Unlike the three lamps
   * it lights no side in particular — it is what keeps a black car from
   * rendering as a black shape, the gap the game's previews showed most on
   * dark paint. 100 is the level every preset had before it was a setting. */
  ambient: { min: 0, max: 300, step: 5, default: 100 },
  /** **Each image exposed to its own brightness** (`meteredExposure`): a dark
   * car comes out a little brighter, a light one a little darker.
   *
   * **Not what the game does**, and that is worth knowing: Kunos shot its
   * previews at a fixed exposure (`ppeffects.ini` of the SDK's preview tool,
   * `AUTO_EXPOSURE` off). It is there because our studio does not light like
   * theirs, and it closes part of that gap — measured, it keeps white paint
   * from burning out.
   *
   * **Off by default, and a deliberate exception to GRILLE§5.6** (fixed
   * exposure, so a black car and a white one stay comparable): it is there
   * for a preset whose goal is to be indistinguishable from the game, and
   * pulls every car towards the same brightness. All or nothing. */
  autoExposure: { min: 0, max: 100, step: 100, default: 0 },
  /** **The camera the game shot its own previews with** (`placeGameCamera`):
   * one position for the whole library, set relative to each car's own origin,
   * so the angle a car is seen from varies with the car exactly as in the
   * game's images. When on, the framing values above do not apply. Off by
   * default: an orbit at one angle keeps a catalogue comparable. */
  gameFraming: { min: 0, max: 100, step: 100, default: 0 },
  /** **Colour saturation** (`gradeColours`). The same for every car, so it
   * keeps a catalogue comparable — it only moves the whole set. 100 leaves
   * the render as it is; the game's previews measure at 75. */
  saturation: { min: 50, max: 150, step: 5, default: 100 },
  /** **Cool tint**, in percent of the cast the game's previews carry (their
   * whites are bluish: an Abarth 500 is (151, 157, 163) there). 0 is neutral. */
  coolTint: { min: 0, max: 200, step: 5, default: 0 },
  /** **Window reflections** (`reflectGlass`): what the glass mirrors, in
   * percent of what the paint does. Our studio is dark, so windows read as
   * dark holes where the game's previews show a pale sheen. */
  glass: { min: 0, max: 600, step: 10, default: 100 },
  /** **The disc of light** under the car (`placePool`), the game's turntable:
   * its radius, in percent of the car's (of the reference radius under the
   * game's camera, so the same for every car as in the game). 0 keeps the
   * wide glow, which reads as a blurred horizon from a low camera. */
  poolSize: { min: 0, max: 300, step: 5, default: 0 },
  /** How soft the disc's edge is, in percent of its radius. */
  poolEdge: { min: 0, max: 100, step: 5, default: 30 },
} as const satisfies Record<keyof GridTemplateValues, { min: number; max: number; step: number; default: number }>;

/**
 * Les valeurs **réglées** d'un preset : le gabarit moins le mat.
 *
 * Le mat est une couleur de carte, et la version du moteur n'appartient à
 * personne : `renderTemplate` les ajoute au moment de rendre. Les garder hors
 * de ce type est ce qui empêche de les enregistrer dans le preset — donc de
 * figer une version de moteur dans un preset dupliqué il y a six mois.
 */
export type GridTemplateValues = Omit<GridTemplate, "matHi" | "matLo" | "renderer">;

type TemplateKey = keyof typeof GRID_THUMB_RANGES;

export const TEMPLATE_KEYS = Object.keys(GRID_THUMB_RANGES) as TemplateKey[];

/**
 * Version du **moteur de rendu**, dans l'empreinte de chaque vignette.
 *
 * **À incrémenter dès qu'une correction change les pixels produits** — même
 * règle, et même raison, que `preview::CONVERTER_VERSION` côté conversion.
 * Sans elle, une vignette porte le `.kn5`, la livrée, les configs CSP, la
 * version du convertisseur et le gabarit… mais rien du code qui dessine : une
 * image fausse reste donc servie pour toujours, parfaitement valide au regard
 * de tout ce que son nom sait vérifier.
 *
 * Elle ne bouge que pour du **code**. Une valeur de preset qui change — le
 * cadrage d'Officiel repris sur une mesure, par exemple — est déjà couverte par
 * l'empreinte du gabarit, et l'incrémenter ferait alors régénérer les autres
 * presets pour rien.
 *
 * Historique, parce que c'est ce qui donne la règle :
 *  - **1** — première version.
 *  - **3** — rendu **suréchantillonné** : 2048×1152 réduit en 1024×576. Le MSAA
 *    seul laissait du crénelage sur les arêtes claires et les jantes, parce
 *    qu'il échantillonne la couverture des triangles et non l'ombrage.
 *  - **2** — le fond cuit était un plan transparent accroché à la caméra, donc
 *    dessiné *après* la voiture (three.js rend toute la liste opaque avant la
 *    liste transparente, et `renderOrder` ne trie qu'à l'intérieur d'une
 *    liste). Toutes les vignettes du preset Officiel sortaient noires. Il passe
 *    par `scene.background`, qui échappe à ce classement.
 *  - **4** — the car is centred on its **projected** box (a lens shift), not
 *    aimed at the centre of its 3D box: perspective was pulling every
 *    silhouette ~6 % towards its nose. Every preset moves.
 *  - **5** — a floor above 100 % gets brighter instead of doing nothing: it
 *    was an opacity, capped at 1. Only floors above 100 % change pixels;
 *    every other image is redrawn identically, the price of never leaving a
 *    stale one behind.
 */
export const RENDERER_VERSION = 5;

/** Le fond de carte d'un preset : les deux bouts du dégradé radial du mat
 * (GRILLE§2.2). Clair au centre pour décoller la voiture, sombre aux bords pour
 * contenir l'image. */
export interface GridMat {
  hi: string;
  lo: string;
}

export interface GridPreset {
  id: string;
  /** Nom tapé par l'utilisateur. **Vide pour un embarqué** : le sien est une
   * clé i18n, pour suivre la langue de l'app. Une copie, elle, garde le nom
   * qu'on lui a donné quelle que soit la langue — c'est le nom de quelqu'un,
   * pas une étiquette de produit. */
  name: string;
  builtin: boolean;
  template: GridTemplateValues;
  mat: GridMat;
  /** Laisser le contenu de base tel quel.
   *
   * N'a de sens que pour un preset qui **imite** les previews d'origine : les
   * 178 voitures Kunos ont déjà exactement ce rendu, les régénérer coûterait
   * trois minutes pour un résultat identique. Avec un preset qui cherche
   * l'homogénéité, au contraire, les sauter ruine précisément ce qu'on
   * cherche. */
  skipStock: boolean;
}

/** Reprend les huit valeurs d'une source quelconque. Écrit champ par champ et
 * non par `Object.fromEntries` : c'est ce qui fait vérifier par TypeScript
 * qu'aucune n'est oubliée le jour où le gabarit en gagne une. */
function template(read: (key: TemplateKey) => number): GridTemplateValues {
  return {
    azimuth: read("azimuth"),
    elevation: read("elevation"),
    fov: read("fov"),
    margin: read("margin"),
    key: read("key"),
    fill: read("fill"),
    rim: read("rim"),
    shadow: read("shadow"),
    height: read("height"),
    floor: read("floor"),
    reflection: read("reflection"),
    reflectionBlur: read("reflectionBlur"),
    background: read("background"),
    trueScale: read("trueScale"),
    ambient: read("ambient"),
    autoExposure: read("autoExposure"),
    gameFraming: read("gameFraming"),
    saturation: read("saturation"),
    coolTint: read("coolTint"),
    glass: read("glass"),
    poolSize: read("poolSize"),
    poolEdge: read("poolEdge"),
  };
}

/**
 * Le gabarit tel qu'il part au rendu : les valeurs du preset **plus son mat**.
 *
 * Le mat vit à part dans l'écran parce que c'est une couleur de carte, pas un
 * réglage de rendu — et le backend ne le compte dans l'empreinte que quand le
 * preset le cuit. Mais il doit voyager avec le reste, sans quoi un fond cuit ne
 * saurait pas de quelle couleur être.
 *
 * **Tout ce qui rend ou cherche une image passe par ici**, jamais par
 * `preset.template` directement : c'est cet objet-là qui fait la clé, des deux
 * côtés de l'IPC.
 */
export function renderTemplate(preset: GridPreset): GridTemplate {
  return { ...preset.template, matHi: preset.mat.hi, matLo: preset.mat.lo, renderer: RENDERER_VERSION };
}

function defaultTemplate(): GridTemplateValues {
  return template((key) => GRID_THUMB_RANGES[key].default);
}

/** Le mat d'origine : celui de la grille depuis le GRILLE§2.2. */
const CATALOGUE_MAT: GridMat = { hi: "#2b2d33", lo: "#17181c" };

/**
 * Les presets livrés avec l'app. **En lecture seule** : on les duplique pour
 * s'en faire un.
 *
 * `id` est stable et ne se traduit jamais — c'est lui qui est enregistré, pas
 * le nom affiché.
 */
export const BUILTIN_PRESETS: readonly GridPreset[] = [
  {
    id: "catalogue",
    name: "",
    builtin: true,
    // The game's colour grade, measured on Officiel: the same for every car,
    // so it moves the whole set without costing comparability.
    // Height -7: the car centred in the frame (margins 22.1 % above, 21.6 %
    // below over six cars). The +8 default came from the game's previews,
    // which sit low; a catalogue has no reason to copy that.
    template: { ...defaultTemplate(), height: -7, saturation: 75, coolTint: 100 },
    mat: CATALOGUE_MAT,
    skipStock: false,
  },
  {
    // Le problème B du GRILLE§1 pris par l'autre bout : ici on ne cherche pas à
    // identifier vite, on cherche à avoir envie de regarder. Contre-jour poussé
    // — c'est lui qui découpe une silhouette sombre sur un fond sombre —,
    // principale retenue, complément réduit pour garder les ombres fermées,
    // cadrage plus serré et plus bas : la « pub » tient autant au cadrage qu'à
    // la lumière.
    //
    // **Ces valeurs sont un point de départ, pas une mesure.** Le critère de ce
    // preset est le plaisir des yeux, qui n'a pas de valeur numérique : elles
    // sont faites pour être arrêtées dans l'atelier, comme les défauts de
    // l'aperçu 3D l'ont été.
    id: "vitrine",
    name: "",
    builtin: true,
    template: {
      azimuth: 320,
      elevation: 5,
      fov: 26,
      margin: 1,
      // **Une principale très basse, et c'est le réglage qui fait le preset.**
      // Le complément et le contre-jour s'expriment en pourcentage d'elle : la
      // descendre à 30 % éteint donc tout l'éclairage direct, et ce qui reste
      // vient presque entièrement du showroom. La voiture n'est plus posée sous
      // des projecteurs, elle baigne dans une pièce sombre — d'où le contraste
      // profond et les reflets étirés qu'on cherchait.
      key: 30,
      fill: 15,
      rim: 140,
      shadow: 45,
      background: 0,
      // The car sits **high** to leave room for its reflection: at -12 the
      // reflection shows under it (at +8, the value this preset first had,
      // the car sat low and the reflection was cut by the frame — the
      // comment here said "the car rises" while the value lowered it). The
      // floor comes with it: pool, then a short reflection on it. That is
      // what separates a studio photo from a cut-out — and what makes this
      // thumbnail no longer fully transparent, hence the dark mat below.
      height: -12,
      floor: 85,
      // Un reflet **franc et très flou** : à 90 % il porte l'image, et à 3,5 il
      // est laqué et non mouillé. C'est le couple qui sépare une photo de
      // showroom d'un rendu de jeu vidéo.
      reflection: 90,
      reflectionBlur: 35,
      trueScale: 0,
      ambient: 100,
      autoExposure: 0,
      gameFraming: 0,
      // The game's colour grade, as on Catalogue.
      saturation: 75,
      coolTint: 100,
      glass: 100,
      poolSize: 0,
      poolEdge: 30,
    },
    // Le fond quasi noir des previews d'Assetto Corsa, reproduit là où il
    // s'écrit chez nous : dans la carte.
    mat: { hi: "#191a1e", lo: "#0a0a0c" },
    skipStock: false,
  },
  {
    // **Le preset qui retourne le problème de la grille mixte** au lieu de le
    // contenir. Les voitures chiffrées et le contenu de base gardent leur
    // `preview.png` pour toujours (GRILLE§7) ; plutôt que d'encadrer cette disparité,
    // celui-ci l'efface — il imite ce rendu, donc une voiture régénérée ne se
    // distingue plus de celle d'à côté.
    //
    // Deux conséquences, et elles sont le preset :
    //  - **Son fond est cuit dans l'image.** Indiscernable exige que le fond
    //    soit dans le fichier, comme il l'est chez Kunos. La transparence était
    //    un moyen, pas une fin.
    //  - **Il ne régénère pas le contenu de base.** Les 178 voitures du jeu ont
    //    déjà exactement ce rendu : les refaire coûterait trois minutes pour un
    //    résultat identique. C'est ce qui rend ce preset le moins cher des
    //    trois, alors qu'il est le plus ambitieux.
    //
    // **Cadrage et fond viennent d'une mesure sur les 178 voitures
    // officielles**, pas d'un coup d'œil (`preview.jpg` de la première livrée,
    // réduite en 128×72, boîte des pixels qui s'écartent du fond) :
    //
    //  - la voiture occupe **66 % de la largeur** du cadre (voir `margin` : à
    //    marge nulle, la voiture n'occupe déjà que ~75 %) ;
    //  - elle est **basse** : 33 % de marge en haut contre 18 % en bas, centre
    //    à 57 % de la hauteur ;
    //  - le fond est **plat et quasi noir** (luminance 4 dans les coins comme
    //    au-dessus de la voiture) et ce qui éclaire, c'est le **sol** :
    //    luminance 15 en bas du cadre. Ce n'est donc pas un halo derrière la
    //    voiture, contrairement à ce que l'œil croit y voir — c'est une flaque
    //    au sol sur un fond uni.
    //  - format : 1022×574 en médiane, soit le 16:9 que la sortie utilise déjà.
    //
    // **Les deux côtés ont été mesurés avec le même instrument**, et c'est la
    // seule façon de comparer deux cadrages : mesurer le leur et estimer le
    // nôtre a produit une marge trois fois trop grande. Piège rencontré au
    // passage — le seuil de détection doit exclure la flaque de lumière au sol,
    // sinon elle entre dans la boîte et augmenter la marge semble augmenter la
    // largeur mesurée.
    //
    // **Second pass (2026-11): every value below was tuned against the real
    // thing, not by eye.** Stock cars were rendered in the running app with
    // the studio rig and set next to their own `preview.jpg` (same car, same
    // skin), and both were measured with one instrument: silhouette box, body
    // luminance percentiles, corner and floor luminance. Thirteen cars in all;
    // the final check ran on eight, four of which had not been used for the
    // tuning. Mean over those eight, game → this preset (→ previous values):
    //  - width 67.6 % → 69.9 % (61.3), top 32.2 % → 32.9 % (38.7),
    //    bottom 16.2 % → 16.3 % (15.2);
    //  - body p10 28 → 32 (13: dark paint was crushed to black), p50 67 → 69;
    //  - corners 4 → 6 (11), bottom of frame 6 → 10 (20).
    //
    // What the first pass got wrong, in order of how much it showed:
    //  - **the angle**: the game shows more flank (306°, not 320°) and looks
    //    from higher (10°, not 6°);
    //  - **the perspective**: the game's camera is close with a wide lens —
    //    the near front wheel is clearly bigger than the rear one. 40°, not
    //    26°, is what reproduces it, and the lens then frames the car on its
    //    own: margin 5, height 0;
    //  - **the light**: the game's paint never goes black — its shadows are
    //    filled by the environment. Full fill, and a key low enough that
    //    white cars stop clipping;
    //  - **the floor**: the pool is a faint patch under the car, not a lit
    //    floor across the frame — 35, not 100.
    //
    // Two gaps no value could close were then fixed in the rendering code
    // (renderer 4, `trueScale`), measured on twelve cars:
    //  - the car sat ~6 % right of the game's: now centred, 50 % vs 48.4 %;
    //  - the game does **not** fit each car to the frame — it shoots them all
    //    from the same distance, so an Abarth fills 45 % of the width and a GT
    //    70 %. `trueScale` does the same: Abarth 44 %, Lotus 49 61 % (59),
    //    MX-5 57 % (56), mean width 64.5 % (65.4).
    //
    // **The game's own camera and auto exposure** finish the job. The camera
    // (`gameFraming`) is the one Kunos shot its previews with, read from the
    // SDK. Auto exposure is not the game's — its preview tool exposes every
    // car alike — but measured with that camera it still closes part of the
    // gap our studio leaves (mean error 10.7 against 11.5 levels, white paint
    // no longer burnt): the one preset that breaks GRILLE§5.6's fixed
    // exposure, because its goal is to be indistinguishable from the game.
    //
    // **What still differs**: black paint stays black where the game's is dark
    // grey, and red is more vivid. Neither light nor exposure reaches it —
    // at 300 % ambient, or exposure ×1.8, a black Cayenne keeps black sides —
    // so the gap is in the paint itself as we render it. The colour space was
    // ruled out by measurement (docs/CHANTIERS.md).
    id: "officiel",
    name: "",
    builtin: true,
    template: {
      azimuth: 306,
      elevation: 10,
      fov: 40,
      margin: 5,
      key: 45,
      fill: 100,
      rim: 70,
      shadow: 30,
      height: 0,
      // A pool and **no reflection**: the game's images light the floor under
      // the car, they do not mirror the car in it. 60 measured with the game
      // camera and auto exposure: floor brightness under the car and at the
      // bottom of the frame both within a level of the game's (35 left them
      // 4 levels dark; 100 overshot, and auto exposure then darkened the car).
      floor: 60,
      reflection: 0,
      reflectionBlur: 5,
      background: 100,
      trueScale: 100,
      // Ambient 160 with the key down to 45: the lowest gap to the game over
      // twelve cars (body luminance percentiles, 12.7 → 11.2 levels on
      // average). Higher only burns white paint — the AE86 goes from 143 to
      // 190 median at 300, against 144 in the game.
      ambient: 160,
      // See the head of this preset, and `meteredExposure` for the measure.
      autoExposure: 100,
      // The game's own preview camera — see `placeGameCamera`. The framing
      // values above stay for a copy that turns it off.
      gameFraming: 100,
      // The game's colour grade, measured — see `gradeColours`.
      saturation: 75,
      coolTint: 100,
      // Window reflections: a modest, measured gain — the top of the car
      // (windows) 1.5 levels closer to the game, colours unchanged. Higher
      // lightens the windows further but tints tinted glass green. The rest of
      // the gap is the game's bright environment in a clear pane, which our
      // dark studio does not have (docs/CHANTIERS.md).
      glass: 200,
      // The game's turntable: a disc of light on a darker floor, not a glow
      // (`placePool`). Size set on column profiles at both edges of the
      // frame, which line up with the game's: wall, dark floor band, disc.
      poolSize: 140,
      poolEdge: 50,
    },
    // Measured identical on every stock preview, which means a fixed backdrop
    // in the game: near-black with a slight blue-green cast (top ≈ #020406,
    // floor in front of the car ≈ #0b0f10). The radial gradient of the
    // catalogue mat has no place here — what looks like a halo behind the car
    // is the pool on the floor in front of it.
    mat: { hi: "#05080a", lo: "#020305" },
    skipStock: true,
  },
];

function clamp(key: TemplateKey, value: number): number {
  const range = GRID_THUMB_RANGES[key];
  if (!Number.isFinite(value)) return range.default;
  return Math.min(range.max, Math.max(range.min, Math.round(value)));
}

/** Les deux densités de grille, qui choisissent chacune leur preset. */
export type GridDensity = "dense" | "comfortable";

const DENSITIES: GridDensity[] = ["dense", "comfortable"];

/** Sur quoi retombe une densité dont le preset a disparu. */
const FALLBACK: Record<GridDensity, string> = { dense: "catalogue", comfortable: "vitrine" };

/** Le preset lié à une densité **en cours d'édition** (par opposition à
 * `presetFor`, qui lit ce qui est enregistré). */
export function editedPresetFor(density: GridDensity): GridPreset | undefined {
  return [...BUILTIN_PRESETS, ...values.user].find((p) => p.id === values.bound[density]);
}

interface Values {
  enabled: boolean;
  /** Les presets de l'utilisateur, dans l'ordre où il les a créés. */
  user: GridPreset[];
  /** L'id du preset de chaque densité. */
  bound: Record<GridDensity, string>;
}

// `$state` de module : lu par la grille et par l'écran de réglages, écrit par
// les fonctions ci-dessous.
const values: Values = $state({
  // Éteint par défaut, comme le profil « Normal » du GRILLE§5.5 : régénérer trois
  // cents voitures est un travail qu'on choisit, pas un défaut qu'on subit.
  enabled: false,
  user: [],
  // **La densité est déjà une déclaration d'intention** : passer en dense,
  // c'est dire « je cherche » ; passer en confortable, c'est dire « je
  // regarde ». Accrocher le style à ce geste n'ajoute pas un réglage, ça donne
  // un second sens à un contrôle qui le portait déjà.
  bound: { ...FALLBACK },
});

/** Ce qui est **sur disque**. La différence avec `values` est ce que le pied de
 * l'écran chiffre, et ce que « Annuler » jette (GRILLE§6.3). */
const stored: Values = $state({ enabled: false, user: [], bound: { ...FALLBACK } });

let loaded: Promise<void> | null = null;

function ensureLoaded(): Promise<void> {
  loaded ??= getUiPrefs(Object.values(KEYS)).then((read) => {
    if (read[KEYS.enabled] !== null) values.enabled = read[KEYS.enabled] === "1";
    values.user = parsePresets(read[KEYS.presets]);
    // Un preset supprimé laisse une densité orpheline : elle retombe sur son
    // embarqué plutôt que de ne rien afficher.
    values.bound = {
      dense: known(read[KEYS.dense]) ?? FALLBACK.dense,
      comfortable: known(read[KEYS.comfortable]) ?? FALLBACK.comfortable,
    };
    Object.assign(stored, $state.snapshot(values));
  });
  return loaded;
}

function known(id: string | null): string | null {
  if (!id) return null;
  return BUILTIN_PRESETS.some((p) => p.id === id) || values.user.some((p) => p.id === id) ? id : null;
}

/** Relit la liste enregistrée. Une entrée abîmée — fichier édité à la main,
 * champ ajouté depuis — est **complétée** plutôt que jetée : perdre le preset
 * de quelqu'un pour un champ manquant serait un mauvais échange. */
function parsePresets(raw: string | null): GridPreset[] {
  if (!raw) return [];
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter((entry): entry is Partial<GridPreset> => !!entry && typeof entry === "object")
      .filter((entry) => typeof entry.id === "string" && entry.id.length > 0)
      .map((entry) => ({
        id: entry.id as string,
        name: typeof entry.name === "string" ? entry.name : "",
        builtin: false,
        template: template((key) => clamp(key, Number(entry.template?.[key]))),
        mat: {
          hi: typeof entry.mat?.hi === "string" ? entry.mat.hi : CATALOGUE_MAT.hi,
          lo: typeof entry.mat?.lo === "string" ? entry.mat.lo : CATALOGUE_MAT.lo,
        },
        skipStock: entry.skipStock === true,
      }));
  } catch {
    return [];
  }
}

void ensureLoaded();

/** Attend que les réglages enregistrés soient là. La file de génération y passe
 * avant sa première vignette : produire trois cents images sur les valeurs par
 * défaut pour découvrir ensuite que l'utilisateur en avait d'autres serait cinq
 * minutes de travail à refaire. */
export function gridThumbsReady(): Promise<void> {
  return ensureLoaded();
}

/**
 * La génération est-elle allumée **sur disque** ?
 *
 * `stored` et non `values` : cocher la case ne doit pas lancer trois cents
 * conversions avant Enregistrer.
 *
 * C'est aussi **le point d'étranglement de l'interrupteur de fonctionnalité**
 * (`FEATURE_GRID_THUMBS`). Tout ce qui demande, produit ou affiche une vignette
 * passe par ici : le gater à la source évite d'avoir à se souvenir de chacun des
 * appelants, et le réglage de l'utilisateur reste intact sur disque pour le jour
 * où l'interrupteur repasse à `true`.
 */
export function gridThumbsOn(): boolean {
  return FEATURE_GRID_THUMBS && stored.enabled;
}

/** Les réglages en cours d'édition, réactifs. */
export function gridThumbPrefs(): Values {
  return values;
}

/** Tous les presets, embarqués d'abord. Réactif. */
export function allPresets(): GridPreset[] {
  return [...BUILTIN_PRESETS, ...values.user];
}

/** Les presets **enregistrés**, seuls à avoir des images sur disque. */
function storedPresets(): GridPreset[] {
  return [...BUILTIN_PRESETS, ...stored.user];
}

function find(list: readonly GridPreset[], id: string): GridPreset | undefined {
  return list.find((p) => p.id === id);
}

/**
 * Le preset **enregistré** d'une densité de grille — celui sous lequel les
 * images existent, et donc le seul avec lequel la grille a le droit de chercher
 * ou de produire.
 *
 * Bouger un curseur ne doit pas mettre trois cents vignettes au rebut avant que
 * l'utilisateur n'ait dit « Appliquer » (GRILLE§6.3).
 */
export function presetFor(density: GridDensity): GridPreset {
  const list = storedPresets();
  return find(list, stored.bound[density]) ?? list[0];
}

/** Les presets réellement en usage — un par densité, dédoublonnés. Ce sont eux
 * dont les images doivent survivre au balayage. */
export function livePresets(): GridPreset[] {
  const ids = new Set(Object.values(stored.bound));
  return storedPresets().filter((p) => ids.has(p.id));
}

/** Le nom affiché d'un preset : sa clé i18n s'il est livré avec l'app — pour
 * qu'il suive la langue — le nom tapé sinon, qui appartient à quelqu'un et ne
 * se traduit donc pas. */
export function presetName(preset: GridPreset): string {
  return preset.builtin ? t("settings.gridPreset_" + preset.id) : preset.name;
}

/** The presets in use, as a skeleton freezes their thumbnails (ESPACE§3.3):
 * none when the thumbnails are off, so nothing is frozen. */
export function liveTemplates(): LivePreset[] {
  if (!gridThumbsOn()) return [];
  return livePresets().map((p) => ({ id: p.id, template: renderTemplate(p) }));
}

const pending = $derived(JSON.stringify(values) !== JSON.stringify(stored));

/** Vrai tant qu'un réglage bougé n'a pas été appliqué. */
export function gridThumbsDirty(): boolean {
  return pending;
}

export function setGridThumbsEnabled(enabled: boolean): void {
  values.enabled = enabled;
}

export function bindPreset(density: GridDensity, id: string): void {
  values.bound = { ...values.bound, [density]: id };
}

/** Modifie une valeur d'un preset de l'utilisateur. Un embarqué est en lecture
 * seule : il faut le dupliquer, et c'est ce que dit l'écran. */
export function setPresetValue(id: string, key: TemplateKey, value: number): void {
  const preset = find(values.user, id);
  if (!preset) return;
  preset.template = { ...preset.template, [key]: clamp(key, value) };
}

/**
 * Change le fond de carte d'un preset de l'utilisateur.
 *
 * Rappel de ce que ça coûte, parce que ce n'est pas symétrique : sur un preset
 * détouré, changer ces deux couleurs est **gratuit** — le mat est du CSS, aucune
 * image ne le porte. Sur un preset à fond cuit, elles sont peintes dans le PNG,
 * donc l'empreinte les compte et les images se refont (voir `renderTemplate`).
 */
export function setPresetMat(id: string, mat: Partial<GridMat>): void {
  const preset = find(values.user, id);
  if (preset) preset.mat = { ...preset.mat, ...mat };
}

export function setPresetSkipStock(id: string, skip: boolean): void {
  const preset = find(values.user, id);
  if (preset) preset.skipStock = skip;
}

export function renamePreset(id: string, name: string): void {
  const preset = find(values.user, id);
  if (preset) preset.name = name.trim();
}

/**
 * Duplique un preset — c'est **le seul moyen** d'en avoir un à soi.
 *
 * Un preset vide serait une douzaine de curseurs de rien ; une copie de Vitrine
 * est à un réglage d'être la sienne. Renvoie l'id de la copie, pour que l'écran
 * la sélectionne aussitôt.
 */
export function duplicatePreset(id: string, name: string): string | null {
  const source = find(allPresets(), id);
  if (!source) return null;
  const copy: GridPreset = {
    id: "u" + Date.now().toString(36),
    name: name.trim(),
    builtin: false,
    template: { ...source.template },
    mat: { ...source.mat },
    skipStock: source.skipStock,
  };
  values.user = [...values.user, copy];
  return copy.id;
}

/** Supprime un preset de l'utilisateur. Les densités qui le montraient
 * retombent sur leur embarqué — jamais sur rien. */
export function deletePreset(id: string): void {
  values.user = values.user.filter((p) => p.id !== id);
  for (const density of DENSITIES) {
    if (values.bound[density] === id) values.bound = { ...values.bound, [density]: FALLBACK[density] };
  }
}

/** Revient sur ce qui est enregistré : le « Annuler » du GRILLE§6.3, qui ne coûte
 * rien et rend l'expérimentation gratuite. */
export function revertGridThumbPrefs(): void {
  Object.assign(values, structuredClone($state.snapshot(stored)));
}

/**
 * Enregistre, puis efface les images qu'aucun preset vivant ne réclame plus.
 *
 * Le balayage n'est pas optionnel : le magasin de vignettes n'a **aucune passe
 * d'éviction** — c'est ce qui le garde hors du plafond du cache — donc rien
 * d'autre ne ramasserait les images d'un preset supprimé ou modifié. Il garde
 * en revanche celles de **tous** les presets en usage, et c'est ce qui rend le
 * changement de densité instantané une fois les deux jeux produits.
 */
export async function saveGridThumbPrefs(): Promise<void> {
  await setUiPrefs({
    [KEYS.enabled]: values.enabled ? "1" : "0",
    [KEYS.presets]: JSON.stringify($state.snapshot(values.user)),
    [KEYS.dense]: values.bound.dense,
    [KEYS.comfortable]: values.bound.comfortable,
  });
  Object.assign(stored, structuredClone($state.snapshot(values)));
  // `renderTemplate` et non `p.template` : c'est l'empreinte du gabarit **tel
  // qu'il a rendu** que le magasin porte dans ses noms de fichier, mat compris
  // quand le preset le cuit. La comparer sans le mat garderait les mauvaises
  // images, et effacerait les bonnes.
  await sweepGridTemplates(livePresets().map(renderTemplate)).catch((e) => {
    // Best-effort : des images d'un preset disparu qui traînent coûtent du
    // disque, pas une erreur d'affichage — leur nom ne peut plus être demandé.
    console.error("sweep_grid_templates", e);
    return 0;
  });
}
