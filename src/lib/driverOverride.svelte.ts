// La tenue de pilote que l'utilisateur impose, voiture par voiture
// (docs/SPEC-ecran-pilote.md §1.4, révisé).
//
// **Le choix est par voiture, et c'est un revirement assumé.** La spec le
// voulait global — « mon pilote », pas « le pilote de cette livrée » — et
// l'intention était bonne : se reconnaître d'une voiture à l'autre. Mais la
// conséquence ne l'était pas : un casque choisi une fois s'imposait à 312
// voitures que l'utilisateur n'avait jamais ouvertes, sans qu'il l'ait
// demandé ni qu'on le lui dise. Une application qui change le réglage de tout
// le parc dans le dos de son utilisateur a tort, même si le réglage est joli.
//
// D'où une cascade à trois niveaux, résolue par `driverFor` :
//
//  1. la tenue **choisie pour cette voiture**, s'il y en a une → elle gagne ;
//  2. sinon, si l'option est active, la **tenue par défaut** — l'une des
//     tenues enregistrées, désignée par l'utilisateur ;
//  3. sinon, **la livrée** : le backend garde alors ce que `skin.ini` déclare.
//
// Le niveau 1 gagne toujours : activer l'option n'écrase jamais un choix fait
// à la main, et la désactiver ne perd rien.
//
// Persistance dans `ui_prefs.json` (règle d'or n°6), une clé par voiture, sur
// le patron de `preferred.ts` — ce n'est pas qu'une affaire de cohérence : le
// filtre « pilote modifié » de la bibliothèque lit ce drapeau **par carte**,
// donc de façon synchrone, et `peekUiPref` existe pour ce cas précis.
import { outfitByName } from "./driverOutfits.svelte";
import { StorageKey } from "./storage";
import type { DriverView } from "./preview";
import { getUiPrefs, peekUiPref, removeUiPref, setUiPref } from "./uiPrefs.svelte";

/** Nom de la tenue enregistrée qui sert de tenue par défaut, ou vide.
 *
 * **Une seule clé, pas deux.** Il y avait aussi une case « appliquer », qui
 * doublait l'information : désigner une tenue par défaut *est* l'activation,
 * et « aucune » *est* la désactivation. La case ne servait qu'à créer un état
 * grisé incompréhensible tant que rien n'était désigné. */
const FALLBACK_KEYS: Record<"race" | "street", string> = {
  race: "pitbox.driver.fallback.race",
  street: "pitbox.driver.fallback.street",
};
/** L'ancien réglage unique, relu une dernière fois pour être réparti sur les
 * deux classes puis effacé (voir `ensureLoaded`). */
const LEGACY_FALLBACK_KEY = "pitbox.driver.fallback";

/** Les quatre pièces. `null` = ce que la voiture ou sa livrée décide. */
export interface DriverOutfit {
  /** Corps substitué à celui de la voiture. Aperçu seulement, par nature :
   * le corps est nommé dans `data.acd`, que le serveur de course vérifie. */
  body: string | null;
  suit: string | null;
  gloves: string | null;
  helmet: string | null;
}

export const EMPTY_OUTFIT: DriverOutfit = { body: null, suit: null, gloves: null, helmet: null };

export type Piece = "suit" | "gloves" | "helmet";

/** Vrai quand rien n'est choisi — la voiture et sa livrée décident de tout. */
export function isEmpty(outfit: DriverOutfit): boolean {
  return !outfit.body && !outfit.suit && !outfit.gloves && !outfit.helmet;
}

// --- Ce qui est choisi pour une voiture donnée ------------------------------

/**
 * La tenue que l'utilisateur a choisie **pour cette voiture**, ou `null`.
 *
 * Lecture synchrone (`peekUiPref`) : la bibliothèque l'appelle une fois par
 * carte, potentiellement des centaines de fois par rendu.
 */
export function driverOwn(carId: string): DriverOutfit | null {
  const raw = peekUiPref(StorageKey.driverOutfit(carId));
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as Partial<DriverOutfit>;
    const outfit: DriverOutfit = {
      body: parsed.body ?? null,
      suit: parsed.suit ?? null,
      gloves: parsed.gloves ?? null,
      helmet: parsed.helmet ?? null,
    };
    return isEmpty(outfit) ? null : outfit;
  } catch {
    return null;
  }
}

/** Cette voiture a-t-elle une tenue à elle ? Le drapeau du filtre de la
 * bibliothèque, et la seule chose qu'elle ait besoin de savoir. */
export function hasOwnDriver(carId: string): boolean {
  return driverOwn(carId) != null;
}

/** La tenue effectivement portée par le pilote de cette voiture, cascade
 * résolue. C'est ce que l'aperçu affiche, et ce que la pose au lancement
 * écrira le jour où elle existera. */
export function driverFor(carId: string | null, kind: CarClass): DriverOutfit {
  if (!carId) return EMPTY_OUTFIT;
  return driverOwn(carId) ?? (applyFallback(kind) ? fallbackOutfit(kind) : EMPTY_OUTFIT);
}

/** Vrai quand ce que porte cette voiture vient de la tenue par défaut et non
 * d'un choix fait sur elle — ce que la ligne de session doit distinguer. */
export function wearsFallback(carId: string | null, kind: CarClass): boolean {
  return !!carId && driverOwn(carId) == null && applyFallback(kind) && !isEmpty(fallbackOutfit(kind));
}

function write(carId: string, outfit: DriverOutfit): void {
  const key = StorageKey.driverOutfit(carId);
  // Une entrée vide est **retirée**, pas écrite : c'est ce qui fait la
  // différence entre « cette voiture est réglée sur la tenue de sa livrée » et
  // « cette voiture n'est pas réglée », donc entre paraître dans le filtre
  // « pilote modifié » ou non.
  if (isEmpty(outfit)) removeUiPref(key);
  else setUiPref(key, JSON.stringify(outfit));
}

export function setDriverPiece(carId: string, piece: Piece, id: string | null): void {
  // Le choix **propre** à la voiture, jamais la cascade : partir de la tenue
  // par défaut inscrirait celle-ci en dur sur cette voiture au premier clic,
  // et la détacherait du réglage qu'elle est censée suivre.
  const current = driverOwn(carId) ?? EMPTY_OUTFIT;
  write(carId, { ...current, [piece]: id });
}

/**
 * Substitue un corps à celui de la voiture, ou rend la main avec `null`.
 *
 * **Les trois pièces retombent au défaut** dans les deux sens : la tenue de la
 * livrée est nommée d'après l'ancien corps, donc changer de corps ne casse pas
 * trois choix — il supprime l'option par défaut elle-même (§D6).
 */
export function setDriverBody(carId: string, id: string | null): void {
  const current = driverOwn(carId) ?? EMPTY_OUTFIT;
  if (current.body === id) return;
  write(carId, { ...EMPTY_OUTFIT, body: id });
}

/** Pose une tenue complète sur une voiture, en un seul enregistrement. */
export function setDriverOutfit(carId: string, outfit: DriverOutfit): void {
  write(carId, outfit);
}

/** Rend cette voiture à sa livrée : l'entrée disparaît (§5.6). */
export function resetDriverOutfit(carId: string): void {
  removeUiPref(StorageKey.driverOutfit(carId));
}

// --- Les tenues par défaut, une par classe de voiture ------------------------

/**
 * Les deux familles de voitures, du point de vue du pilote.
 *
 * **Une tenue par défaut ne veut pas dire la même chose des deux côtés.** Sur
 * une voiture de course, la tenue fait partie de la livrée — c'est l'écurie
 * qui habille son pilote, et beaucoup voudront la garder telle quelle. Sur une
 * voiture de rue, personne n'a rien prévu, et c'est justement là qu'une tenue
 * à soi a le plus de sens. Un réglage unique obligeait à choisir entre les
 * deux ; il y en a donc deux, et chacun peut valoir « aucune ».
 *
 * Tout ce qui n'est pas annoncé `race` compte comme `street` : c'est le cas
 * de la majorité des mods, dont beaucoup ne renseignent aucune classe, et le
 * défaut sûr est celui où la livrée n'a rien prévu.
 */
export type CarClass = "race" | "street";

export function carClassOf(raw: string | null | undefined): CarClass {
  return (raw ?? "").trim().toLowerCase() === "race" ? "race" : "street";
}

const fallback = $state<Record<CarClass, string>>({ race: "", street: "" });

let loaded: Promise<void> | null = null;

function ensureLoaded(): Promise<void> {
  loaded ??= getUiPrefs([FALLBACK_KEYS.race, FALLBACK_KEYS.street, LEGACY_FALLBACK_KEY]).then((read) => {
    const legacy = read[LEGACY_FALLBACK_KEY] ?? "";
    // Migration : le réglage unique d'avant vaut pour les deux classes. Le
    // recopier plutôt que de le ranger d'un seul côté — un utilisateur qui
    // avait désigné une tenue la voit se comporter comme avant, et c'est à
    // lui de décider laquelle des deux il rend à la livrée.
    fallback.race = read[FALLBACK_KEYS.race] ?? legacy;
    fallback.street = read[FALLBACK_KEYS.street] ?? legacy;
    if (legacy) {
      setUiPref(FALLBACK_KEYS.race, fallback.race);
      setUiPref(FALLBACK_KEYS.street, fallback.street);
      removeUiPref(LEGACY_FALLBACK_KEY);
    }
  });
  return loaded;
}

void ensureLoaded();

/** Nom de la tenue enregistrée désignée comme tenue par défaut de cette
 * classe, ou "". */
export function fallbackName(kind: CarClass): string {
  return fallback[kind];
}

/** Y a-t-il une tenue par défaut pour cette classe ? Désigner, c'est activer. */
export function applyFallback(kind: CarClass): boolean {
  return fallback[kind] !== "";
}

/** Les quatre pièces de la tenue par défaut, résolues depuis les tenues
 * enregistrées. Le nom est stocké, pas les pièces : renommer ou modifier une
 * tenue enregistrée doit suivre partout, et deux copies d'une même tenue
 * finiraient par diverger. */
export function fallbackOutfit(kind: CarClass): DriverOutfit {
  return outfitByName(fallback[kind]) ?? EMPTY_OUTFIT;
}

export function setFallbackName(kind: CarClass, name: string): void {
  fallback[kind] = name;
  // Attendre la lecture avant d'écrire : `ensureLoaded` termine par une
  // affectation en bloc, qui écraserait ce choix s'il arrivait après. Même
  // défaut, même remède que `driverOutfits.saveOutfit`.
  void ensureLoaded().then(() => {
    fallback[kind] = name;
    setUiPref(FALLBACK_KEYS[kind], name);
  });
}

// --- Ce qui part au backend -------------------------------------------------

/**
 * Ce qui accompagne une demande d'aperçu pour cette voiture, ou `null` quand
 * elle porte ce que sa livrée prévoit.
 *
 * Une pièce laissée à `null` n'est pas envoyée : le backend garde alors celle
 * que le `skin.ini` de la livrée déclare, plutôt que de déshabiller le pilote.
 */
export function driverOverridePayload(carId: string | null, kind: CarClass): Omit<DriverView, "steer"> | null {
  const outfit = driverFor(carId, kind);
  if (isEmpty(outfit)) return null;
  // `body` ici, `model` là-bas : le backend nomme le mannequin comme
  // `driver3d.ini` le nomme, l'écran l'appelle « le corps ».
  const { body, suit, gloves, helmet } = outfit;
  return { model: body, suit, gloves, helmet };
}
