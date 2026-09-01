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
import type { DriverView } from "./preview";
import { getUiPrefs, removeUiPref, setUiPref } from "./uiPrefs.svelte";

const KEYS = {
  body: "pitbox.driver.body",
  suit: "pitbox.driver.suit",
  gloves: "pitbox.driver.gloves",
  helmet: "pitbox.driver.helmet",
} as const;

/** Ancienne bascule « surcharger le pilote », retirée avec les trois menus de
 * la colonne de session : avoir choisi quelque chose **est** l'activation
 * (SPEC-ecran-pilote §3.2, où le badge se déduit du choix et non d'un
 * interrupteur). Lue une dernière fois pour la migration, puis effacée. */
const LEGACY_ON = "pitbox.driver.override";

export interface DriverOverride {
  /** Corps substitué à celui de la voiture (`driver_60`), `null` = le sien.
   * Ce n'est pas une pièce de plus : il **commande** les trois autres, et le
   * substituer supprime la référence « livrée » (SPEC-ecran-pilote §1.3,
   * §10.1). */
  body: string | null;
  /** Valeurs telles que `skin.ini` les écrit (`plain/red`), `null` = on garde
   * ce que le skin déclare pour cette pièce. */
  suit: string | null;
  gloves: string | null;
  helmet: string | null;
}

const values: DriverOverride = $state({ body: null, suit: null, gloves: null, helmet: null });

let loaded: Promise<void> | null = null;

function ensureLoaded(): Promise<void> {
  loaded ??= getUiPrefs([...Object.values(KEYS), LEGACY_ON]).then((read) => {
    values.body = read[KEYS.body] || null;
    values.suit = read[KEYS.suit] || null;
    values.gloves = read[KEYS.gloves] || null;
    values.helmet = read[KEYS.helmet] || null;
    if (read[LEGACY_ON] != null) {
      // Migration : la bascule décochée voulait dire « ces choix dorment ».
      // Sans elle ils s'appliqueraient d'un coup au premier lancement après
      // la mise à jour, ce que personne n'a demandé — donc on les efface,
      // plutôt que de les réveiller dans le dos de l'utilisateur.
      if (read[LEGACY_ON] !== "1") {
        for (const piece of ["body", "suit", "gloves", "helmet"] as const) {
          values[piece] = null;
          setUiPref(KEYS[piece], "");
        }
      }
      removeUiPref(LEGACY_ON);
    }
  });
  return loaded;
}

void ensureLoaded();

/** L'état courant, réactif. Lire `driverOverride().on`, pas une copie
 * déstructurée : la réactivité se perd à la déstructuration. */
export function driverOverride(): DriverOverride {
  return values;
}

export function setDriverPiece(piece: "suit" | "gloves" | "helmet", id: string | null): void {
  values[piece] = id;
  setUiPref(KEYS[piece], id ?? "");
}

/**
 * Substitue un corps à celui de la voiture, ou rend la main à la voiture avec
 * `null`.
 *
 * **Les trois pièces retombent au défaut** dans les deux sens : la tenue de la
 * livrée est nommée d'après l'ancien corps, donc changer de corps ne casse pas
 * trois choix — il supprime l'option par défaut elle-même (§D6). Le backend
 * l'applique de son côté ; le faire aussi ici, c'est que la piste affichée dise
 * la même chose que ce qu'on voit sur le plateau.
 */
export function setDriverBody(id: string | null): void {
  if (values.body === id) return;
  values.body = id;
  setUiPref(KEYS.body, id ?? "");
  for (const piece of ["suit", "gloves", "helmet"] as const) {
    setDriverPiece(piece, null);
  }
}

/** Tout remettre sur la livrée, sans toucher au corps (§5.6). */
export function resetDriverOutfit(): void {
  for (const piece of ["suit", "gloves", "helmet"] as const) {
    setDriverPiece(piece, null);
  }
}

/**
 * Ce qui part au backend avec une demande d'aperçu, ou `null` quand aucune
 * pièce n'est imposée.
 *
 * Une pièce laissée à `null` n'est pas envoyée : le backend garde alors celle
 * que le `skin.ini` de la livrée déclare, plutôt que de déshabiller le pilote.
 */
export function driverOverridePayload(): Omit<DriverView, "steer"> | null {
  const { body, suit, gloves, helmet } = values;
  // `body` ici, `model` là-bas : le backend nomme le mannequin comme
  // `driver3d.ini` le nomme, l'écran l'appelle « le corps ».
  return body || suit || gloves || helmet ? { model: body, suit, gloves, helmet } : null;
}
