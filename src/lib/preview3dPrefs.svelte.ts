// Réglages de l'aperçu 3D des voitures (docs/SPEC-preview-3d-kn5.md §15).
//
// Partagés par l'écran Réglages (où on les change) et par `CarPreview3D`
// (qui les applique) : d'où un `$state` de module plutôt qu'un état par
// composant, pour qu'un aperçu déjà ouvert suive le réglage sans être remonté.
//
// Persistance via `ui_prefs.json` (`uiPrefs.svelte.ts`), jamais `localStorage`
// — règle d'or n°6 : un réglage qui doit survivre à un redémarrage n'a rien à
// faire dans un stockage que WebView2 n'écrit pas forcément sur disque. Aucun
// fichier Rust dédié : quatre nombres et un booléen ne le justifient pas.
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
} as const;

type NumericKey = keyof typeof PREVIEW3D_RANGES;

/** Type écrit à la main plutôt qu'inféré de l'objet : sans lui, TypeScript
 * réduit la cible d'un `values[key] = …` (clé prise dans une union) à `never`. */
export type Preview3dPrefs = { enabled: boolean } & Record<NumericKey, number>;

function clamp(key: NumericKey, value: number): number {
  const range = PREVIEW3D_RANGES[key];
  if (!Number.isFinite(value)) return range.default;
  return Math.min(range.max, Math.max(range.min, Math.round(value)));
}

// `$state` de module : lu par les composants, écrit par les setters ci-dessous.
const values: Preview3dPrefs = $state({
  enabled: true,
  zoom: PREVIEW3D_RANGES.zoom.default,
  azimuth: PREVIEW3D_RANGES.azimuth.default,
  elevation: PREVIEW3D_RANGES.elevation.default,
  height: PREVIEW3D_RANGES.height.default,
  spin: PREVIEW3D_RANGES.spin.default,
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

function ensureLoaded(): Promise<void> {
  loaded ??= getUiPrefs(Object.values(KEYS)).then((stored) => {
    if (stored[KEYS.enabled] !== null) values.enabled = stored[KEYS.enabled] === "1";
    for (const key of Object.keys(PREVIEW3D_RANGES) as NumericKey[]) {
      const raw = stored[KEYS[key]];
      if (raw !== null) values[key] = clamp(key, Number(raw));
    }
  });
  return loaded;
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

/** Remet le cadrage d'origine, sans toucher à la bascule photo/3D. */
export function resetPreview3dCamera(): void {
  for (const key of Object.keys(PREVIEW3D_RANGES) as NumericKey[]) {
    setPreview3dValue(key, PREVIEW3D_RANGES[key].default);
  }
}
