// Presets de vignettes de la grille (docs/SPEC-grille.md §5.6, §6).
//
// **Un preset, pas un gabarit unique.** La première version n'avait qu'un jeu
// de valeurs, et elle butait sur un constat de l'utilisateur : la preview
// d'origine d'Assetto Corsa est plus *jolie* que notre rendu, plus vitrine,
// alors que le nôtre est plus *lisible*. Ce ne sont pas deux qualités
// d'exécution du même objectif, ce sont deux objectifs — identifier vite, ou
// avoir envie de regarder. Un compromis unique les aurait mal servis tous les
// deux.
//
// Ça ne touche pas à la propriété non négociable du §5.6 : « toute valeur doit
// rester identique pour les 312 ». Elle porte sur un jeu d'images, pas sur le
// nombre de jeux possibles — chaque preset reste uniforme chez lui.
//
// **Trois choix de structure, chacun contre une solution plus évidente :**
//
//  1. *Les embarqués sont en lecture seule, et on les duplique.* Un preset vide
//     est une douzaine de curseurs de rien ; une copie de Vitrine est à un
//     réglage d'être la sienne. Bénéfice inattendu : ça supprime tout le
//     versionnage du §5.7 — plus besoin de deviner « l'utilisateur a-t-il
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
import type { GridTemplate } from "./gridThumbs";
import { sweepGridTemplates } from "./gridThumbs";
import { getUiPrefs, setUiPrefs } from "./uiPrefs.svelte";

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
 * Un écart assumé vis-à-vis du §5.6, qui demande de relever l'azimut sur les
 * previews Kunos plutôt que de l'inventer : c'est déjà fait, et c'est le défaut
 * de l'aperçu 3D — 318°, trois-quarts avant **gauche**, la convention de toutes
 * les photos du jeu. La grille restant mixte pour toujours (§7), aligner
 * l'angle sur celui des previews d'origine est ce qui réduit le plus
 * durablement l'écart entre les deux sources.
 */
export const GRID_THUMB_RANGES = {
  /** Rotation de la caméra autour de l'axe vertical, en degrés. */
  azimuth: { min: 0, max: 359, step: 1, default: 318 },
  /** Plongée au-dessus de l'horizon. Basse : une vignette de catalogue montre
   * le profil d'une voiture, pas son toit. */
  elevation: { min: 0, max: 40, step: 1, default: 8 },
  /** Champ de vision vertical, en degrés. 22° est un équivalent long focal :
   * peu de distorsion, donc des proportions qui se comparent d'une voiture à
   * l'autre. */
  fov: { min: 10, max: 50, step: 1, default: 22 },
  /** Marge autour de la boîte englobante, en pourcentage. Le cadrage est
   * **ajusté et non à l'échelle** : chaque voiture remplit le cadre quelle que
   * soit sa taille réelle. On perd le gabarit relatif, on gagne que chaque
   * vignette est lisible à 190 px — pour un catalogue dont le métier est
   * l'identification, c'est le bon échange (§5.6). */
  margin: { min: 0, max: 25, step: 1, default: 6 },
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
   * **Négatif remonte la voiture dans le cadre** — ce dont un preset à reflet a
   * besoin, le reflet prenant la place en dessous. Zéro vise le centre, ce que
   * fait un catalogue : la voiture au milieu, rien autour. */
  height: { min: -40, max: 40, step: 1, default: 0 },
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
 * Historique, parce que c'est ce qui donne la règle :
 *  - **1** — première version.
 *  - **2** — le fond cuit était un plan transparent accroché à la caméra, donc
 *    dessiné *après* la voiture (three.js rend toute la liste opaque avant la
 *    liste transparente, et `renderOrder` ne trie qu'à l'intérieur d'une
 *    liste). Toutes les vignettes du preset Officiel sortaient noires. Il passe
 *    par `scene.background`, qui échappe à ce classement.
 */
export const RENDERER_VERSION = 2;

/** Le fond de carte d'un preset : les deux bouts du dégradé radial du mat
 * (§2.2). Clair au centre pour décoller la voiture, sombre aux bords pour
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
    background: read("background"),
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

/** Le mat d'origine : celui de la grille depuis le §2.2. */
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
    template: defaultTemplate(),
    mat: CATALOGUE_MAT,
    skipStock: false,
  },
  {
    // Le problème B du §1 pris par l'autre bout : ici on ne cherche pas à
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
      azimuth: 318,
      elevation: 5,
      fov: 20,
      margin: 1,
      key: 90,
      fill: 15,
      rim: 140,
      shadow: 45,
      background: 0,
      // La voiture remonte dans le cadre pour laisser la place au reflet, et
      // le sol arrive : flaque de lumière, puis reflet court dessus. C'est ce
      // qui sépare une photo de studio d'un détourage — et c'est aussi ce qui
      // fait que cette vignette-là n'est plus entièrement transparente, donc
      // qu'elle veut le mat sombre ci-dessous sous peine de soucoupe.
      height: -12,
      floor: 85,
      reflection: 55,
    },
    // Le fond quasi noir des previews d'Assetto Corsa, reproduit là où il
    // s'écrit chez nous : dans la carte.
    mat: { hi: "#191a1e", lo: "#0a0a0c" },
    skipStock: false,
  },
  {
    // **Le preset qui retourne le problème de la grille mixte** au lieu de le
    // contenir. Les voitures chiffrées et le contenu de base gardent leur
    // `preview.png` pour toujours (§7) ; plutôt que d'encadrer cette disparité,
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
    // Les valeurs sont à arrêter dans l'atelier, bascule « comparer à l'image
    // d'origine » allumée : le critère de ce preset est mesurable — il ne doit
    // pas se voir — donc il se règle contre la vraie image, jamais de mémoire.
    id: "officiel",
    name: "",
    builtin: true,
    template: {
      azimuth: 318,
      elevation: 6,
      fov: 20,
      margin: 4,
      key: 100,
      fill: 25,
      rim: 60,
      shadow: 30,
      height: -5,
      // Une flaque discrète et **aucun reflet** : les previews du jeu montrent
      // un halo au sol sous la voiture, jamais un miroir.
      floor: 50,
      reflection: 0,
      background: 100,
    },
    // Le fond des previews du jeu, mesuré sur un `preview.jpg` : rgb(12,13,15)
    // sous la voiture, rgb(2,3,5) dans les coins.
    mat: { hi: "#0c0d0f", lo: "#020305" },
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
 * `presetForDensity`, qui lit ce qui est enregistré). */
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
  // Éteint par défaut, comme le profil « Normal » du §5.5 : régénérer trois
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
 * l'écran chiffre, et ce que « Annuler » jette (§6.3). */
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

/** La génération est-elle allumée **sur disque** ? `stored` et non `values` :
 * cocher la case ne doit pas lancer trois cents conversions avant Enregistrer. */
export function gridThumbsOn(): boolean {
  return stored.enabled;
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
 * l'utilisateur n'ait dit « Appliquer » (§6.3).
 */
export function presetForDensity(density: GridDensity): GridPreset {
  const list = storedPresets();
  return find(list, stored.bound[density]) ?? list[0];
}

/** Les presets réellement en usage — un par densité, dédoublonnés. Ce sont eux
 * dont les images doivent survivre au balayage. */
export function livePresets(): GridPreset[] {
  const ids = new Set(Object.values(stored.bound));
  return storedPresets().filter((p) => ids.has(p.id));
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

/** Revient sur ce qui est enregistré : le « Annuler » du §6.3, qui ne coûte
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
