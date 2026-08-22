// Réglages de l'aperçu 3D des voitures (docs/SPEC-preview-3d-kn5.md §15).
//
// Partagés par l'écran Réglages (où on les change) et par `CarPreview3D`
// (qui les applique) : d'où un `$state` de module plutôt qu'un état par
// composant, pour qu'un aperçu déjà ouvert suive le réglage sans être remonté.
//
// Persistance via `ui_prefs.json` (`uiPrefs.svelte.ts`), jamais `localStorage`
// — règle d'or n°6 : un réglage qui doit survivre à un redémarrage n'a rien à
// faire dans un stockage que WebView2 n'écrit pas forcément sur disque. Aucun
// fichier Rust dédié : une poignée de nombres et trois choix ne le justifient
// pas.
//
// Un seul de ces réglages intéresse le backend — le plafond du cache — et
// c'est **ce module qui le lui pousse** (`setPreviewCacheCap`), au chargement
// et à chaque changement. Le backend ne lit jamais `ui_prefs.json` : le schéma
// de ce fichier appartient au frontend (voir l'en-tête de `ui_prefs.rs`).
import { setPreviewCacheCap } from "./preview";
import { getUiPrefs, setUiPref, setUiPrefs } from "./uiPrefs.svelte";

/** Clés de `ui_prefs.json`. `preview3d` seule existait avant les autres : elle
 * porte la bascule photo/3D de la zone héros, et garde donc son nom. */
const KEYS = {
  enabled: "pitbox.preview3d",
  zoom: "pitbox.preview3d.zoom",
  azimuth: "pitbox.preview3d.azimuth",
  elevation: "pitbox.preview3d.elevation",
  height: "pitbox.preview3d.height",
  spin: "pitbox.preview3d.spin",
  cacheMb: "pitbox.preview3d.cacheMb",
  intro: "pitbox.preview3d.intro",
  quality: "pitbox.preview3d.quality",
} as const;

/**
 * Bornes et valeurs par défaut, en unités lisibles par l'utilisateur : des
 * degrés pour les angles, des pourcentages pour ce qui n'a pas d'unité
 * naturelle. Les défauts reproduisent **exactement** le cadrage d'origine, donc
 * quelqu'un qui n'ouvre jamais ces réglages ne voit rien changer.
 */
export const PREVIEW3D_RANGES = {
  /** 100 % = la distance calculée sur la taille du modèle. Bornes choisies pour
   * rester dans les limites de zoom des contrôles souris (`minDistance` /
   * `maxDistance`), sinon le réglage serait annulé au premier rendu. */
  zoom: { min: 50, max: 200, step: 5, default: 100 },
  /** Rotation de la caméra autour de l'axe vertical, en degrés. Le défaut est
   * l'angle des `preview.jpg` Kunos : trois-quarts avant **gauche**, comme
   * toutes les photos du jeu (§15 point 7). */
  azimuth: { min: 0, max: 359, step: 1, default: 318 },
  /** Plongée de la caméra, en degrés au-dessus de l'horizon. Plafonnée sous
   * l'angle polaire maximal des contrôles. */
  elevation: { min: 0, max: 80, step: 1, default: 13 },
  /** Hauteur de la caméra, en pourcentage du rayon du modèle. Monte ou
   * descend le point visé **sans toucher à la plongée** : c'est ce qui décide
   * de la place de la voiture dans le cadre, là où l'angle décide de ce qu'on
   * voit de son toit. 0 vise le centre du modèle. */
  height: { min: -60, max: 60, step: 1, default: 0 },
  /** Vitesse du plateau tournant. 0 % = plateau à l'arrêt. */
  spin: { min: 0, max: 200, step: 5, default: 100 },
  /** Plafond du cache d'aperçus, **en mégaoctets**. En Mo et non en Go parce
   * que toute la mécanique de ce module travaille sur des entiers (`clamp`
   * arrondit) : un pas d'un demi-gigaoctet s'exprime en 512 Mo sans flottant.
   * Bornes reprises telles quelles côté Rust, qui borne à son tour — la
   * validation ne dépend pas de l'écran qui envoie la valeur. */
  cacheMb: { min: 512, max: 20480, step: 512, default: 2048 },
} as const;

/** Les seuls réglages que « Rétablir le cadrage d'origine » remet à zéro.
 * Le plafond de cache partage la mécanique des curseurs mais n'a rien à voir
 * avec la caméra : le remettre à 2 Go au passage évincerait des entrées pour
 * un bouton qui ne parle que de cadrage. */
const FRAMING_KEYS = ["zoom", "azimuth", "elevation", "height", "spin"] as const;

/** Effet appliqué au plateau tournant quand un modèle s'affiche.
 * `ramp` monte en douceur jusqu'à la vitesse réglée ; `launch` part vite et
 * ralentit jusqu'à elle. */
export const INTRO_EFFECTS = ["none", "ramp", "launch"] as const;
export type IntroEffect = (typeof INTRO_EFFECTS)[number];

/** Qualité de **rendu** — et d'elle seule : rien ici ne touche à la conversion
 * du modèle, donc changer de niveau n'invalide aucune entrée de cache et
 * s'applique à l'image suivante. */
export const PREVIEW_QUALITIES = ["standard", "high", "ultra"] as const;
export type PreviewQuality = (typeof PREVIEW_QUALITIES)[number];

type NumericKey = keyof typeof PREVIEW3D_RANGES;

/** Type écrit à la main plutôt qu'inféré de l'objet : sans lui, TypeScript
 * réduit la cible d'un `values[key] = …` (clé prise dans une union) à `never`. */
export type Preview3dPrefs = {
  enabled: boolean;
  intro: IntroEffect;
  quality: PreviewQuality;
} & Record<NumericKey, number>;

function clamp(key: NumericKey, value: number): number {
  const range = PREVIEW3D_RANGES[key];
  if (!Number.isFinite(value)) return range.default;
  return Math.min(range.max, Math.max(range.min, Math.round(value)));
}

// `$state` de module : lu par les composants, écrit par les setters ci-dessous.
const values: Preview3dPrefs = $state({
  enabled: true,
  // Le plus discret des deux effets par défaut : une montée en douceur se
  // remarque à peine, là où un départ lancé est un parti pris.
  intro: "ramp",
  quality: "high",
  zoom: PREVIEW3D_RANGES.zoom.default,
  azimuth: PREVIEW3D_RANGES.azimuth.default,
  elevation: PREVIEW3D_RANGES.elevation.default,
  height: PREVIEW3D_RANGES.height.default,
  spin: PREVIEW3D_RANGES.spin.default,
  cacheMb: PREVIEW3D_RANGES.cacheMb.default,
});

/** Compteur de remises à zéro de la vue. Ce n'est pas un réglage : il n'est ni
 * lu ni écrit sur disque, il ne sert qu'à ce qu'un aperçu monté ailleurs voie
 * passer la demande. Un compteur et non un booléen — deux remises à zéro
 * successives doivent produire deux événements distincts. */
let resets = $state(0);

export function preview3dResets(): number {
  return resets;
}

/** Replace la voiture selon les réglages et la remet à tourner. */
export function resetPreview3dView(): void {
  resets += 1;
}

// Chargé une fois pour toute la session, dès l'import du module : un aperçu
// monté tout de suite ne doit pas s'ouvrir sur les valeurs par défaut puis
// sauter sur celles de l'utilisateur.
let loaded: Promise<void> | null = null;

/** Relit un choix dans sa liste d'options. Une valeur inconnue — fichier
 * édité à la main, réglage retiré d'une version à l'autre — retombe sur le
 * défaut plutôt que de se propager dans une comparaison qui ne matchera
 * jamais. */
function oneOf<T extends string>(raw: string | null, allowed: readonly T[], fallback: T): T {
  return allowed.includes(raw as T) ? (raw as T) : fallback;
}

function ensureLoaded(): Promise<void> {
  loaded ??= getUiPrefs(Object.values(KEYS)).then((stored) => {
    if (stored[KEYS.enabled] !== null) values.enabled = stored[KEYS.enabled] === "1";
    values.intro = oneOf(stored[KEYS.intro], INTRO_EFFECTS, values.intro);
    values.quality = oneOf(stored[KEYS.quality], PREVIEW_QUALITIES, values.quality);
    for (const key of Object.keys(PREVIEW3D_RANGES) as NumericKey[]) {
      const raw = stored[KEYS[key]];
      if (raw !== null) values[key] = clamp(key, Number(raw));
    }
    // Le backend part sur son propre défaut tant que personne ne lui a rien
    // dit : c'est ici, et seulement ici, qu'il apprend le réglage enregistré.
    pushCacheCap();
  });
  return loaded;
}

/** Dernier plafond réellement transmis au backend. Appliquer un plafond
 * déclenche une éviction — la répéter à l'identique reparcourrait le dossier
 * du cache pour rien. */
let pushedCacheMb = 0;

function pushCacheCap(): void {
  if (values.cacheMb === pushedCacheMb) return;
  pushedCacheMb = values.cacheMb;
  setPreviewCacheCap(values.cacheMb * 1024 * 1024).catch((e) => {
    // Best-effort : le plafond précédent reste en vigueur, rien ne casse.
    // Tracé quand même — un cache qui ignore son réglage sans un mot serait
    // indiagnosticable.
    console.error("set_preview_cache_cap", e);
    pushedCacheMb = 0;
  });
}

void ensureLoaded();

/** Les réglages courants, réactifs. Lire `preview3dPrefs().zoom` et non une
 * copie déstructurée : la réactivité se perd à la déstructuration. */
export function preview3dPrefs() {
  return values;
}

/** Attend que les réglages enregistrés soient là — pour un appelant qui ne peut
 * pas se permettre de construire une scène sur les valeurs par défaut. */
export function preview3dReady(): Promise<void> {
  return ensureLoaded();
}

// --- Persistance : appliquée tout de suite, écrite un peu après -----------
//
// Le réglage s'applique à l'image suivante (`values` est un `$state` lu par
// l'aperçu), mais l'écriture disque attend que le curseur s'arrête. Sans ce
// délai, un glissé de curseur réécrivait `ui_prefs.json` en entier à chaque
// pas — une cinquantaine de fois pour un seul geste, fichier complet et
// écriture Rust synchrone à chaque fois.
const PERSIST_DEBOUNCE_MS = 400;

let pending: Record<string, string> = {};
let timer: ReturnType<typeof setTimeout> | null = null;
let dirty = $state(false);

/** Vrai tant qu'un réglage bougé n'est pas encore sur disque. */
export function preview3dDirty(): boolean {
  return dirty;
}

function queue(key: string, value: string): void {
  pending[key] = value;
  dirty = true;
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => void flushPreview3dPrefs(), PERSIST_DEBOUNCE_MS);
}

/** Écrit tout de suite ce qui attend, et rend la main quand c'est **sur
 * disque** — ce que le bouton Enregistrer a besoin de savoir pour annoncer
 * « Enregistré » sans mentir. Appelé aussi au démontage des curseurs : le
 * délai ci-dessus ne doit pas survivre à la fermeture du panneau. */
export async function flushPreview3dPrefs(): Promise<void> {
  if (timer) {
    clearTimeout(timer);
    timer = null;
  }
  const entries = pending;
  pending = {};
  if (Object.keys(entries).length) await setUiPrefs(entries);
  // Le plafond suit le même délai que l'écriture disque : sans ça, un glissé
  // de curseur déclencherait une éviction par pas — et une éviction est
  // irréversible, contrairement à une écriture de préférence.
  if (KEYS.cacheMb in entries) pushCacheCap();
  // Relu après l'attente : un réglage bougé pendant l'écriture est encore en
  // attente, et effacer le drapeau ici le rendrait invisible.
  dirty = Object.keys(pending).length > 0;
}

export function setPreview3dEnabled(enabled: boolean): void {
  values.enabled = enabled;
  // Une case à cocher n'est pas un geste continu : elle s'écrit tout de suite.
  setUiPref(KEYS.enabled, enabled ? "1" : "0");
}

export function setPreview3dValue(key: NumericKey, value: number): void {
  values[key] = clamp(key, value);
  queue(KEYS[key], String(values[key]));
}

export function setPreview3dIntro(intro: IntroEffect): void {
  values.intro = intro;
  setUiPref(KEYS.intro, intro);
}

export function setPreview3dQuality(quality: PreviewQuality): void {
  values.quality = quality;
  setUiPref(KEYS.quality, quality);
}

/** Remet le cadrage d'origine, sans toucher ni à la bascule photo/3D, ni à
 * quoi que ce soit qui ne concerne pas la caméra (voir `FRAMING_KEYS`). */
export function resetPreview3dCamera(): void {
  for (const key of FRAMING_KEYS) {
    setPreview3dValue(key, PREVIEW3D_RANGES[key].default);
  }
}
