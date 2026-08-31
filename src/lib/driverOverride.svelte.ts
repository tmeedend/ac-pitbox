// La tenue de pilote que l'utilisateur impose, s'il en impose une
// (docs/SPEC-preview-3d-kn5.md §4.6ter).
//
// **Globale, pas par voiture ni par skin** : c'est « mon pilote », pas « le
// pilote de cette livrée ». Décidé ainsi parce que le contraire obligerait à
// refaire le choix pour chaque combinaison voiture+skin, alors que l'intérêt
// est justement de se reconnaître d'une voiture à l'autre.
//
// Persistance dans `ui_prefs.json` (règle d'or n°6 : jamais `localStorage`
// pour un réglage qui doit survivre à un redémarrage). Quatre entrées, ça ne
// justifie pas un fichier Rust dédié.
import { getUiPrefs, setUiPref } from "./uiPrefs.svelte";

const KEYS = {
  on: "pitbox.driver.override",
  suit: "pitbox.driver.suit",
  gloves: "pitbox.driver.gloves",
  helmet: "pitbox.driver.helmet",
} as const;

export interface DriverOverride {
  /** Décoché par défaut : la très grande majorité des voitures habillent déjà
   * leur pilote correctement, et une voiture de course lui met les couleurs de
   * son écurie. */
  on: boolean;
  /** Valeurs telles que `skin.ini` les écrit (`plain/red`), `null` = on garde
   * ce que le skin déclare pour cette pièce. */
  suit: string | null;
  gloves: string | null;
  helmet: string | null;
}

const values: DriverOverride = $state({ on: false, suit: null, gloves: null, helmet: null });

let loaded: Promise<void> | null = null;

function ensureLoaded(): Promise<void> {
  loaded ??= getUiPrefs(Object.values(KEYS)).then((read) => {
    values.on = read[KEYS.on] === "1";
    values.suit = read[KEYS.suit] || null;
    values.gloves = read[KEYS.gloves] || null;
    values.helmet = read[KEYS.helmet] || null;
  });
  return loaded;
}

void ensureLoaded();

/** L'état courant, réactif. Lire `driverOverride().on`, pas une copie
 * déstructurée : la réactivité se perd à la déstructuration. */
export function driverOverride(): DriverOverride {
  return values;
}

export function setDriverOverrideOn(on: boolean): void {
  values.on = on;
  setUiPref(KEYS.on, on ? "1" : "0");
}

export function setDriverPiece(piece: "suit" | "gloves" | "helmet", id: string | null): void {
  values[piece] = id;
  setUiPref(KEYS[piece], id ?? "");
}

/**
 * Ce qui part au backend avec une demande d'aperçu, ou `null` quand aucune
 * pièce n'est imposée.
 *
 * Une pièce laissée à `null` n'est pas envoyée : le backend garde alors celle
 * que le `skin.ini` de la livrée déclare, plutôt que de déshabiller le pilote.
 */
export function driverOverridePayload(): Pick<DriverOverride, "suit" | "gloves" | "helmet"> | null {
  if (!values.on) return null;
  const { suit, gloves, helmet } = values;
  return suit || gloves || helmet ? { suit, gloves, helmet } : null;
}
