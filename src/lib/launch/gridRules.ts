// The pure rules of the opponents grid: which cars a random fill draws, which
// livery each opponent gets, when two drivers read as the same person, and how
// a grid or a pool saved by an older version is read back.
//
// Kept apart from `opponentGrid.svelte.ts`, which holds the grid's state, so
// that the rules can be tested on their own — they are where the grid's real
// bugs were (two identical drivers on one grid, a pool silently reset).
import { clampAiLevel, type Opponent } from "$lib/launch/opponent";
import type { SkinItem } from "$lib/launch/launch";
import { defaultGridFilters } from "$lib/launch/opponentPool";
import type { FilterMap } from "$lib/library/filters";
import type { ModCard } from "$lib/library/library";

/**
 * What a grid already carries, so that the next draw avoids it.
 *
 * **Deux choses à éviter, pas une.** Le skin lui-même, pour que deux lignes
 * de la même voiture ne soient pas la même image ; et surtout le **pilote**
 * qu'il déclare — le jeu nomme l'IA d'après le `ui_skin.json` de sa livrée,
 * donc deux livrées différentes portant « 59 Juan » produisent deux lignes
 * qu'on ne distingue pas, alors même que les skins diffèrent. C'était le
 * défaut visible : un plateau avec deux fois le même pilote.
 *
 * `taken` est partagé par TOUT le plateau et non par voiture : c'est
 * l'identité du pilote qui doit être unique dans la grille, pas dans une
 * marque. Quand le vivier de livrées est épuisé, on reprend — un plateau
 * tronqué serait pire, et l'avertissement de vivier maigre l'a déjà annoncé.
 */
export interface TakenIdentities {
  skins: Set<string>;
  drivers: Set<string>;
}

export const newTaken = (): TakenIdentities => ({ skins: new Set(), drivers: new Set() });

/** Ce qui doit rester unique : le couple numéro + nom, insensible à la
 * casse. Une livrée muette ne participe pas — elle n'impose rien. */
export function driverKey(skin: SkinItem): string | null {
  const key = `${skin.number ?? ""}|${skin.driver ?? ""}`.trim().toLowerCase();
  return key === "|" ? null : key;
}

/** Records a livery, and the driver it declares, as taken. */
export function markTaken(taken: TakenIdentities, skin: SkinItem): void {
  taken.skins.add(skin.id);
  const key = driverKey(skin);
  if (key) taken.drivers.add(key);
}

/**
 * Picks a livery among `skins`, avoiding what `taken` holds, and records it.
 * `null` when the car has no livery at all.
 */
export function pickSkin(skins: SkinItem[], taken: TakenIdentities, random: () => number = Math.random): string | null {
  if (!skins.length) return null;
  const free = skins.filter((sk) => {
    if (taken.skins.has(sk.id)) return false;
    const key = driverKey(sk);
    return !key || !taken.drivers.has(key);
  });
  // Repli en deux temps : d'abord une livrée simplement pas encore prise,
  // ensuite n'importe laquelle. Mieux vaut répéter un pilote que rendre une
  // ligne sans livrée.
  const from = free.length ? free : skins.filter((sk) => !taken.skins.has(sk.id));
  const pool = from.length ? from : skins;
  const pick = pool[Math.floor(random() * pool.length)];
  markTaken(taken, pick);
  return pick.id;
}

/**
 * The `n` cars of a random fill, drawn from `pool`. `excludeCarIds` = mods déjà
 * présents dans le plateau, évités en priorité.
 * Si le vivier distinct est épuisé (un vivier d'une seule voiture, par
 * exemple), on complète en dupliquant un mod déjà choisi avec un skin
 * différent plutôt que de tronquer le plateau — c'est ce que l'avertissement
 * de vivier maigre annonce (CIBLE§3.5). The livery is the caller's draw.
 *
 * **Aucun repli sur la bibliothèque entière quand le vivier est vide** : un
 * filtre qui ne garde rien doit rendre un plateau vide, pas un plateau tiré
 * ailleurs. Le repli d'avant venait des onglets, dont le vivier pouvait être
 * vide sans que rien ne le dise ; le compteur `Pool · 0 cars` le dit
 * maintenant, et les deux boutons sont éteints.
 */
export function pickCars(
  pool: ModCard[],
  n: number,
  excludeCarIds: Set<string>,
  random: () => number = Math.random,
): ModCard[] {
  if (n <= 0 || !pool.length) return [];
  const fresh = pool.filter((c) => !excludeCarIds.has(c.id_interne)).sort(() => random() - 0.5);
  const picks: ModCard[] = fresh.slice(0, n);
  const dupSource = picks.length ? picks : pool;
  let idx = 0;
  while (picks.length < n) {
    picks.push(dupSource[idx % dupSource.length]);
    idx++;
  }
  return picks;
}

/** Deux pilotes sous la même identité (SETUP§1.9). La génération l'évite ; ceci
 * n'attrape que ce que l'utilisateur a forcé à la main, et le dit plutôt que
 * de le corriger dans son dos. A name typed on a row overrides both the number
 * and the name its livery declares. */
export function hasDuplicateDrivers(
  opponents: Opponent[],
  skinOf: (opp: Opponent) => SkinItem | undefined,
): boolean {
  const seen = new Set<string>();
  for (const opp of opponents) {
    const sk = skinOf(opp);
    const key = `${opp.driver_name ?? sk?.number ?? ""}|${opp.driver_name ?? sk?.driver ?? ""}`.trim().toLowerCase();
    if (key === "|") continue;
    if (seen.has(key)) return true;
    seen.add(key);
  }
  return false;
}

/** Remet une ligne relue sur disque dans la forme courante : les quatre
 * champs de §4.2 n'existaient pas, et `??` ne suffirait pas — un `undefined`
 * qui traverserait jusqu'au backend s'y lirait comme un champ absent, pas
 * comme `Auto`.
 *
 * Forces recalées à la relecture, pas seulement à l'édition : un plateau
 * enregistré quand le plancher était 60 porte des valeurs que Content
 * Manager n'accepte pas. */
export function restoreOpponent(o: Opponent): Opponent {
  return {
    car_id: o.car_id,
    ai_level: o.ai_level == null ? null : clampAiLevel(o.ai_level),
    car_skin: o.car_skin ?? null,
    driver_name: o.driver_name ?? null,
    nationality: o.nationality ?? null,
    ballast: o.ballast ?? 0,
    restrictor: o.restrictor ?? 0,
  };
}

/** The pool fields of a per-type preset or a saved session.
 *
 * Vivier d'adversaires (CIBLE§3.3), sérialisé par `serializeFilters` — la même
 * forme que les filtres de bibliothèque, relue par le même `parseFilters`.
 * Absent sur un preset antérieur aux jetons : `migrateTabPool` reprend
 * alors les trois anciens champs (`grid_mode`, `category_selection`,
 * `year_min`/`year_max`), qui restent déclarés pour cette seule relecture
 * et ne sont plus jamais écrits. */
export interface SavedPool {
  grid_filters?: string;
  grid_pinned?: string[];
  grid_mode?: "same_car" | "same_category" | "free";
  category_selection?: string;
  year_min?: number;
  year_max?: number;
}

/**
 * Reconstruit le vivier depuis les trois champs de l'époque des onglets
 * (CIBLE§3.1), for a preset that has no `grid_filters`.
 *
 * Une migration plutôt qu'un repli sur les défauts : un utilisateur qui
 * courait en « Même catégorie / 2010-2016 » retrouve exactement ce vivier,
 * dit cette fois par deux jetons qu'il peut combiner. Le mode « même
 * voiture » se traduit par le jeton `Model` de la voiture du preset — la
 * seule perte assumée est qu'il ne suit plus la voiture pilotée, ce qui est
 * précisément ce que « une puce pose un jeton et rien d'autre » signifie.
 */
export function migrateTabPool(p: SavedPool, player: ModCard | null): FilterMap {
  const migrated: FilterMap = defaultGridFilters();
  if (p.grid_mode === "same_car") {
    const name = player?.display_name ?? player?.id_interne;
    if (name) migrated.model = { type: "val", values: [{ value: name, sign: 1 }], op: "and" };
  } else if (p.grid_mode === "same_category") {
    const cat = p.category_selection && p.category_selection !== "__same_category__" ? p.category_selection : player?.category;
    if (cat) migrated.category = { type: "val", values: [{ value: cat, sign: 1 }], op: "and" };
  }
  // 0 des deux côtés voulait déjà dire « pas de borne » (`inYearRange`), et
  // `parseFilters` traite un 0 de la même façon : rien à convertir.
  const min = p.year_min && p.year_min > 0 ? p.year_min : null;
  const max = p.year_max && p.year_max > 0 ? p.year_max : null;
  if (min != null || max != null) migrated.year = { type: "range", min, max };
  return migrated;
}
