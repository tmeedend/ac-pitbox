// An opponent of the grid, and the strength bounds it is clamped to.
//
// Its own module for the same mechanical reason as `aiBand.ts`: `launch.ts`
// imports `gridThumbs.svelte.ts`, hence runes, hence cannot be loaded by
// Vitest. `gridRules.ts`, which is tested, needs these — `launch.ts`
// re-exports them, so its callers do not change.

/**
 * Bornes de la force d'une IA, **celles du curseur de Content Manager**.
 *
 * 70 et non 60 : CM ne descend pas plus bas, et une valeur hors de sa plage
 * part dans un preset qu'il recalera lui-même — le réglage ne serait donc pas
 * celui qu'on affiche. Relevé sur son écran, et confirmé sur un preset de
 * grille réel dont la ligne la plus faible vaut exactement 70.
 *
 * Ici et pas dans un composant : l'écran dessine la fourchette, la ligne du
 * plateau borne sa propre valeur, et `savedSessions`/les presets recalent ce
 * qu'ils relisent. Trois copies d'un même nombre finissent par diverger.
 */
export const AI_LEVEL_MIN = 70;
export const AI_LEVEL_MAX = 100;

/** Recale une force dans la plage de CM. Une valeur enregistrée avant que le
 * plancher ne soit corrigé (60 était offert) remonte donc à 70 au lieu de
 * partir telle quelle. */
export function clampAiLevel(level: number): number {
  if (!Number.isFinite(level)) return AI_LEVEL_MAX;
  return Math.max(AI_LEVEL_MIN, Math.min(AI_LEVEL_MAX, Math.round(level)));
}

/** Ce que Content Manager écrit dans un tableau numérique par ligne pour dire
 * « laissé au jeu » (§4.1). Relevé sur un preset de grille réel, où il voisine
 * un `"0"` explicite : les deux ne veulent pas dire la même chose. */
export const AUTO_CELL = null;

export interface Opponent {
  car_id: string;
  /** Force de l'IA, ou `null` pour **`Auto`** : la ligne n'a pas de surcharge
   * et le jeu tire dans la fourchette globale. Ce n'est donc pas une valeur
   * qu'on tire nous-mêmes — c'est son absence, et le format la connaît déjà. */
  ai_level: number | null;
  /** Skin de l'adversaire, choisi (auto ou via la popup de sélection). */
  car_skin: string | null;
  /** Nom de pilote et nationalité repris à la main, ou `null` pour `Auto` — le
   * jeu prend alors ce que le `ui_skin.json` de la livrée déclare. La
   * nationalité est un nom de pays anglais entier, jamais un code ISO. */
  driver_name: string | null;
  nationality: string | null;
  /** Lest et bride d'équilibrage, 0 à 100. Pas d'`Auto` : « aucun lest » se
   * dit par 0, et c'est ce que CM écrit. */
  ballast: number;
  restrictor: number;
}

/** Un adversaire neuf : une voiture, une livrée, et rien d'autre. Tout le reste
 * vaut `Auto`, ce qui est le bon défaut — poser une valeur est un geste. */
export function newOpponent(carId: string, skinId: string | null): Opponent {
  return {
    car_id: carId,
    ai_level: AUTO_CELL,
    car_skin: skinId,
    driver_name: AUTO_CELL,
    nationality: AUTO_CELL,
    ballast: 0,
    restrictor: 0,
  };
}
