// Gabarit des vignettes régénérées de la grille (docs/SPEC-grille.md §5.6, §6).
//
// **Séparé des réglages de l'aperçu 3D, et ce n'est pas un rangement.** Les
// deux règlent une caméra et des lumières, mais tourner l'aperçu d'une fiche ne
// coûte rien et ne dure que le temps qu'on la regarde, là où toucher à ce
// gabarit-ci périme les 312 images de la grille. C'est cette asymétrie qui
// justifie qu'un écran prévienne et que l'autre n'avertisse jamais (§6.1).
//
// D'où le modèle « rien ne s'applique avant Appliquer » (§6.3), repris de
// `preview3dPrefs` : `values` est ce qu'on voit, `stored` ce qui est sur
// disque, et la différence est la facture affichée en pied d'écran.
//
// Persistance via `ui_prefs.json` — règle d'or n°6, jamais `localStorage` : un
// réglage qui doit survivre à un redémarrage n'a rien à faire dans un stockage
// que WebView2 n'écrit pas forcément sur disque.
import type { GridTemplate } from "./gridThumbs";
import { sweepGridTemplates } from "./gridThumbs";
import { getUiPrefs, setUiPrefs } from "./uiPrefs.svelte";

/** Clés de `ui_prefs.json`. */
const KEYS = {
  enabled: "pitbox.gridThumbs",
  version: "pitbox.gridThumbs.version",
  azimuth: "pitbox.gridThumbs.azimuth",
  elevation: "pitbox.gridThumbs.elevation",
  fov: "pitbox.gridThumbs.fov",
  margin: "pitbox.gridThumbs.margin",
  key: "pitbox.gridThumbs.key",
  fill: "pitbox.gridThumbs.fill",
  rim: "pitbox.gridThumbs.rim",
  shadow: "pitbox.gridThumbs.shadow",
} as const;

/**
 * Version du **gabarit d'origine** (§5.7).
 *
 * À incrémenter quand les défauts ci-dessous changent. Qui n'y a jamais touché
 * hérite du nouveau sans rien faire — ses valeurs ne sont pas sur disque ; qui
 * l'a personnalisé garde le sien, et cette version est ce qui permet de le lui
 * dire. Dans les deux cas la régénération est **proposée, jamais forcée** : une
 * grille reste parfaitement utilisable avec des vignettes d'un gabarit
 * antérieur.
 */
export const TEMPLATE_VERSION = 1;

/**
 * Bornes et valeurs de départ, en unités lisibles par l'utilisateur.
 *
 * Ce sont les valeurs du §5.6, à un écart près, assumé : **l'azimut n'a pas été
 * réinventé**. Le §5.6 demande de le relever sur les previews Kunos plutôt que
 * de l'inventer ; c'est déjà fait, et c'est le défaut de l'aperçu 3D — 318°,
 * trois-quarts avant **gauche**, la convention de toutes les photos du jeu. La
 * grille restant mixte pour toujours (§7), aligner l'angle sur celui des
 * previews d'origine est ce qui réduit le plus durablement l'écart entre les
 * deux sources.
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
} as const satisfies Record<keyof GridTemplate, { min: number; max: number; step: number; default: number }>;

type TemplateKey = keyof typeof GRID_THUMB_RANGES;

const TEMPLATE_KEYS = Object.keys(GRID_THUMB_RANGES) as TemplateKey[];

/** Reprend les huit valeurs d'une source quelconque. Écrit champ par champ et
 * non par `Object.fromEntries` : c'est ce qui fait vérifier par TypeScript
 * qu'aucune n'est oubliée le jour où le gabarit en gagne une. */
function template(read: (key: TemplateKey) => number): GridTemplate {
  return {
    azimuth: read("azimuth"),
    elevation: read("elevation"),
    fov: read("fov"),
    margin: read("margin"),
    key: read("key"),
    fill: read("fill"),
    rim: read("rim"),
    shadow: read("shadow"),
  };
}

/** Le gabarit d'origine, celui que rétablit le bouton du §6.3. */
export function originalTemplate(): GridTemplate {
  return template((key) => GRID_THUMB_RANGES[key].default);
}

function clamp(key: TemplateKey, value: number): number {
  const range = GRID_THUMB_RANGES[key];
  if (!Number.isFinite(value)) return range.default;
  return Math.min(range.max, Math.max(range.min, Math.round(value)));
}

type Values = GridTemplate & { enabled: boolean };

// `$state` de module : lu par la grille et par l'écran de réglages, écrit par
// les setters ci-dessous.
const values: Values = $state({
  // Éteint par défaut, comme le profil « Normal » du §5.5 : régénérer trois
  // cents voitures est un travail qu'on choisit, pas un défaut qu'on subit.
  enabled: false,
  ...originalTemplate(),
});

/** Les valeurs telles qu'elles sont **sur disque**. */
const stored: Values = $state({ ...values });

/** Version du gabarit d'origine que l'utilisateur a vue la dernière fois qu'il
 * a enregistré le sien. `null` = il n'a jamais personnalisé. */
let storedVersion = $state<number | null>(null);

let loaded: Promise<void> | null = null;

function ensureLoaded(): Promise<void> {
  loaded ??= getUiPrefs(Object.values(KEYS)).then((read) => {
    if (read[KEYS.enabled] !== null) values.enabled = read[KEYS.enabled] === "1";
    for (const key of TEMPLATE_KEYS) {
      const raw = read[KEYS[key]];
      if (raw !== null) values[key] = clamp(key, Number(raw));
    }
    const version = read[KEYS.version];
    storedVersion = version === null ? null : Number(version);
    Object.assign(stored, $state.snapshot(values));
  });
  return loaded;
}

void ensureLoaded();

/** Attend que les réglages enregistrés soient là. La file de génération y passe
 * avant sa première vignette : produire trois cents images sur les valeurs par
 * défaut pour découvrir ensuite que l'utilisateur en avait d'autres serait cinq
 * minutes de travail à refaire. */
export function gridThumbsReady(): Promise<void> {
  return ensureLoaded();
}

/** Les réglages courants, réactifs. */
export function gridThumbPrefs(): Values {
  return values;
}

/** Le gabarit **enregistré** — celui sous lequel les images existent, et donc
 * le seul avec lequel la grille a le droit de chercher ou de produire. Bouger
 * un curseur ne doit pas mettre trois cents vignettes au rebut avant que
 * l'utilisateur n'ait dit « Appliquer » (§6.3). */
export function appliedTemplate(): GridTemplate {
  return template((key) => stored[key]);
}

/** Le gabarit **en cours d'édition**, celui que l'aperçu de réglage montre. */
export function editedTemplate(): GridTemplate {
  return template((key) => values[key]);
}

const pending = $derived(JSON.stringify(values) !== JSON.stringify(stored));

/** Vrai tant qu'un réglage bougé n'a pas été appliqué. */
export function gridThumbsDirty(): boolean {
  return pending;
}

/** Un gabarit d'origine plus récent existe-t-il, alors que l'utilisateur a le
 * sien ? (§5.7) */
export function newerDefaultTemplate(): boolean {
  return storedVersion !== null && storedVersion < TEMPLATE_VERSION;
}

/**
 * Allume ou éteint les vignettes régénérées, **et l'écrit tout de suite**.
 *
 * C'est un interrupteur, pas un gabarit : il ne périme aucune image et ne
 * déclenche aucun travail par lui-même — les cartes visibles demanderont les
 * leurs, ou reprendront la `preview.png` du mod. Il n'a donc rien à faire dans
 * la facture du §6.3, qui n'a de sens que pour ce qui régénère.
 */
export async function setGridThumbsEnabled(enabled: boolean): Promise<void> {
  values.enabled = enabled;
  stored.enabled = enabled;
  await setUiPrefs({ [KEYS.enabled]: enabled ? "1" : "0" });
}

export function setGridThumbValue(key: TemplateKey, value: number): void {
  values[key] = clamp(key, value);
}

/** Rétablit le gabarit d'origine — à l'écran seulement, tant que rien n'est
 * appliqué. */
export function resetGridTemplate(): void {
  Object.assign(values, originalTemplate());
}

/** Revient sur ce qui est enregistré : le « Annuler » du §6.3, qui ne coûte
 * rien et rend l'expérimentation gratuite. */
export function revertGridThumbPrefs(): void {
  Object.assign(values, $state.snapshot(stored));
}

/**
 * Applique : écrit le gabarit, puis efface ce que le précédent avait produit.
 *
 * L'ordre compte peu ici, mais le balayage, lui, n'est pas optionnel : le
 * magasin de vignettes n'a **aucune passe d'éviction** — c'est ce qui le garde
 * hors du plafond du cache — donc rien d'autre ne ramasserait les 312 images du
 * gabarit d'avant.
 */
export async function applyGridThumbPrefs(): Promise<void> {
  const entries: Record<string, string> = {
    [KEYS.enabled]: values.enabled ? "1" : "0",
    [KEYS.version]: String(TEMPLATE_VERSION),
  };
  for (const key of TEMPLATE_KEYS) entries[KEYS[key]] = String(values[key]);
  await setUiPrefs(entries);
  Object.assign(stored, $state.snapshot(values));
  storedVersion = TEMPLATE_VERSION;
  await sweepGridTemplates(appliedTemplate()).catch((e) => {
    // Best-effort : des images d'un gabarit antérieur qui traînent coûtent du
    // disque, pas une erreur d'affichage — leur nom ne peut plus être demandé.
    console.error("sweep_grid_templates", e);
    return 0;
  });
}
