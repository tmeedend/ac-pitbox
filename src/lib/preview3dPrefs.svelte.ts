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
import { getUiPrefs, setUiPref } from "./uiPrefs.svelte";

/** Clés de `ui_prefs.json`. `preview3d` seule existait avant les autres : elle
 * porte la bascule photo/3D de la zone héros, et garde donc son nom. */
const KEYS = {
  enabled: "pitbox.preview3d",
  zoom: "pitbox.preview3d.zoom",
  azimuth: "pitbox.preview3d.azimuth",
  elevation: "pitbox.preview3d.elevation",
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
  /** Rotation de la caméra autour de l'axe vertical, en degrés. */
  azimuth: { min: 0, max: 359, step: 1, default: 40 },
  /** Hauteur de la caméra, en degrés au-dessus de l'horizon. Plafonnée sous
   * l'angle polaire maximal des contrôles. */
  elevation: { min: 0, max: 80, step: 1, default: 16 },
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
  spin: PREVIEW3D_RANGES.spin.default,
});

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

export function setPreview3dEnabled(enabled: boolean): void {
  values.enabled = enabled;
  setUiPref(KEYS.enabled, enabled ? "1" : "0");
}

export function setPreview3dValue(key: NumericKey, value: number): void {
  values[key] = clamp(key, value);
  setUiPref(KEYS[key], String(values[key]));
}

/** Remet le cadrage d'origine, sans toucher à la bascule photo/3D. */
export function resetPreview3dCamera(): void {
  for (const key of Object.keys(PREVIEW3D_RANGES) as NumericKey[]) {
    setPreview3dValue(key, PREVIEW3D_RANGES[key].default);
  }
}
