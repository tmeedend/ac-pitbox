// The opponents grid of the session screen: the pool it draws from, the
// liveries it has looked up, and every gesture that changes its rows.
//
// It lives here rather than in `Launch.svelte` because it is the part of that
// screen that moves most, and the part whose bugs were subtle (two identical
// drivers on one grid, a regeneration landing after the user had imposed
// opponents). The screen keeps what is the screen's: the dialogs, the
// messages, and `setup` itself — whose `opponents` this class reads and writes
// through the host, since a loaded session replaces the whole object.
import { withCountryLabels } from "$lib/flags.svelte";
import { hasOwnDriver } from "$lib/driver/driverOverride.svelte";
import {
  buildCardIndex,
  buildPredicate,
  filterDefs,
  parseFilters,
  serializeFilters,
  type FilterMap,
} from "$lib/library/filters";
import { matchesQuery } from "$lib/library/cardSearch";
import type { ModCard } from "$lib/library/library";
import { getPreferredSkin } from "$lib/preferred";
import type { OpponentsAction } from "$lib/shell/nav.svelte";
import { centerSpreadOf } from "$lib/launch/aiBand";
import { listModSkins, type RaceSetup, type SkinItem } from "$lib/launch/launch";
import { AI_LEVEL_MAX, AI_LEVEL_MIN, clampAiLevel, newOpponent, type Opponent } from "$lib/launch/opponent";
import { defaultGridFilters } from "$lib/launch/opponentPool";
import type { SavedGrid } from "$lib/launch/savedGrids";
import {
  hasDuplicateDrivers,
  markTaken,
  migrateTabPool,
  newTaken,
  pickCars,
  pickSkin,
  restoreOpponent,
  type SavedPool,
  type TakenIdentities,
} from "$lib/launch/gridRules";

/** What the grid needs from the screen that owns it. Getters, not values:
 * `setup` is replaced wholesale when a saved session is loaded. */
export interface GridHost {
  readonly setup: RaceSetup;
  /** The whole car library, the car being driven included. */
  readonly carPool: ModCard[];
}

export class OpponentGrid {
  // Assigned by the constructor, after the field initialisers: the derived
  // fields below that read it go through `$derived.by`, whose function runs
  // only when the value is first read.
  #host: GridHost;
  // Jeton de génération du plateau (§6.3ter) : `regenerate` est asynchrone
  // (résolution des skins par IPC) et peut encore être « en vol » quand
  // `impose` prend la main — sans garde, son résultat arrive après coup et
  // écrase les adversaires qu'on vient d'imposer. Toute régénération capture
  // le jeton courant et n'applique son résultat que s'il n'a pas été invalidé
  // entre-temps par un appel plus récent.
  #gen = 0;

  /** How many opponents the grid is meant to have. */
  count = $state(7);

  // --- The pool (CIBLE§3.3) ---------------------------------------------------
  //
  // **The filter defines the pool, never the grid.** Three tabs used to do it
  // (`Same car` / `By category` / `Free`), and they were a poorer copy of the
  // filter bar: they could not combine `#gt3` AND 2010-2016 AND "except
  // Kunos", which chips make trivial. What the tabs really carried was the
  // GESTURE that turns a pool into a grid, and there are now two of them,
  // explicit and both working on this same set: `Fill` and `Choose`.
  //
  // The whole car library comes in — including the car being driven. No hidden
  // "except mine" rule: a rule the chips do not show is exactly the kind of
  // reconciliation this refactor exists to delete, and the `Same car` chip
  // needs the car to be in there anyway.
  readonly defs = withCountryLabels(filterDefs("Car"));
  filters = $state<FilterMap>(defaultGridFilters());
  // Aucun filtre épinglé : la barre s'ouvre sur son champ de recherche et son
  // menu, et les trois puces sont ce qui la remplit en un clic.
  pinned = $state<string[]>([]);
  query = $state("");
  readonly index = $derived.by(() =>
    buildCardIndex(this.#host.carPool, this.defs, true, hasOwnDriver, this.#host.setup.car_id),
  );
  readonly #matches = $derived(buildPredicate(this.defs, this.filters, this.index.ctx));
  readonly pool = $derived.by(() => this.#host.carPool.filter((c) => this.#matches(c) && matchesQuery(c, this.query)));

  // --- Skins par voiture (cache, SESSION§3.3) : chargés à la demande pour
  // assigner un skin à chaque adversaire, et réutilisés par la popup. ---
  skinsByCarId = $state<Record<string, SkinItem[]>>({});

  /** Calculé ici et non dans le plateau depuis que l'alerte doit **remonter sur
   * l'entrée de navigation** (L5§1.3) : une alerte sur une page qu'on ne
   * regarde pas ne vaut pas mieux que pas d'alerte. */
  readonly duplicateDrivers = $derived.by(() => hasDuplicateDrivers(this.#host.setup.opponents, (o) => this.skinOf(o)));

  constructor(host: GridHost) {
    this.#host = host;
  }

  get #opponents(): Opponent[] {
    return this.#host.setup.opponents;
  }
  set #opponents(value: Opponent[]) {
    this.#host.setup.opponents = value;
  }

  async ensureSkins(carId: string): Promise<SkinItem[]> {
    const cached = this.skinsByCarId[carId];
    if (cached) return cached;
    let skins: SkinItem[];
    try {
      skins = await listModSkins(carId);
    } catch {
      skins = [];
    }
    this.skinsByCarId = { ...this.skinsByCarId, [carId]: skins };
    return skins;
  }

  /** La livrée d'une ligne, quand elle est connue — et par elle, le pilote que
   * le jeu nommera. */
  skinOf(opp: Opponent): SkinItem | undefined {
    return opp.car_skin ? this.skinsByCarId[opp.car_id]?.find((sk) => sk.id === opp.car_skin) : undefined;
  }

  /** Le registre de ce que le plateau courant porte déjà : une ligne ajoutée
   * après coup doit éviter les mêmes pilotes que le tirage initial. `skip`
   * exclut la ligne qu'on est en train de remplacer, qui ne se fait pas
   * concurrence à elle-même. */
  async #takenFromGrid(skip = -1): Promise<TakenIdentities> {
    const taken = newTaken();
    for (const [i, o] of this.#opponents.entries()) {
      if (i === skip || !o.car_skin) continue;
      const sk = (await this.ensureSkins(o.car_id)).find((x) => x.id === o.car_skin);
      if (sk) markTaken(taken, sk);
      else taken.skins.add(o.car_skin);
    }
    return taken;
  }

  async #skinFor(carId: string, taken: TakenIdentities): Promise<string | null> {
    return pickSkin(await this.ensureSkins(carId), taken);
  }

  /** Génère `n` adversaires tirés dans le vivier (`pickCars`), chacun avec sa
   * livrée. */
  async #generate(n: number, excludeCarIds: Set<string>): Promise<Opponent[]> {
    const picks = pickCars(this.pool, n, excludeCarIds);
    const taken = newTaken();
    const out: Opponent[] = [];
    for (const c of picks) out.push(newOpponent(c.id_interne, await this.#skinFor(c.id_interne, taken)));
    return out;
  }

  /** `Fill N at random` (CIBLE§3.3) : tire N voitures dans le vivier et **remplace**
   * le plateau. Le chemin de celui qui veut courir tout de suite. */
  async fill() {
    const gen = ++this.#gen;
    const opponents = await this.#generate(this.count, new Set());
    // Une action plus récente (nouvelle régénération, ou adversaires imposés
    // depuis la bibliothèque) a pris le dessus entre-temps : ne pas écraser.
    if (gen === this.#gen) this.#opponents = opponents;
  }

  /** `Regenerate` (§4.1) : garde les voitures, **retire au sort ce qui avait
   * été tiré sur elles** — skin et force. Ce n'est pas `Fill` sous un autre
   * nom : « le plateau est bon mais les livrées se répètent » et « le plateau
   * n'est pas le bon » sont deux gestes qu'on veut séparément. */
  async regenerate() {
    const gen = ++this.#gen;
    const taken = newTaken();
    const out: Opponent[] = [];
    for (const opp of this.#opponents) {
      // La livrée est retirée au sort, **pas** les cellules `Auto` : `Auto`
      // n'est pas une valeur qu'on tire, c'est l'absence de surcharge, et
      // c'est le jeu qui tire dedans (§4.1). Ce qui change vraiment ici est
      // donc la livrée — et avec elle le nom de pilote `Auto`, qui en vient.
      out.push({ ...opp, car_skin: await this.#skinFor(opp.car_id, taken) });
    }
    if (gen === this.#gen) this.#opponents = out;
  }

  async setCount(raw: number) {
    const n = Math.max(0, Math.min(30, Math.round(raw) || 0));
    this.count = n;
    const current = this.#opponents;
    if (n < current.length) {
      this.#opponents = current.slice(0, n);
    } else if (n > current.length) {
      const exclude = new Set(current.map((o) => o.car_id));
      const extra = await this.#generate(n - current.length, exclude);
      this.#opponents = [...current, ...extra];
    }
  }

  remove(index: number) {
    this.#opponents = this.#opponents.filter((_, i) => i !== index);
    this.count = this.#opponents.length;
  }

  /** Une cellule d'une ligne du plateau (§4.1/§4.2). `null` sur un texte, et
   * `null` sur la force, valent **`Auto`** : la ligne n'a pas de surcharge et
   * le jeu décide. C'est ce que fait un champ vidé — le geste naturel pour dire
   * « je ne décide pas », et la raison pour laquelle aucun menu de ligne n'est
   * nécessaire pour y revenir. Le lest et la bride n'ont pas d'`Auto` : « rien »
   * s'y dit par 0, comme dans le preset. */
  setCell(index: number, patch: Partial<Opponent>) {
    const opponents = [...this.#opponents];
    opponents[index] = { ...opponents[index], ...patch };
    this.#opponents = opponents;
  }

  /** Force d'une ligne, indépendante de la fourchette globale qui ne sert qu'à
   * la génération. `null` = la cellule repasse en `Auto` — c'est ce que
   * fait un champ vidé, le geste naturel pour dire « je ne décide pas ». */
  setLevel(index: number, raw: number | null) {
    const opponents = [...this.#opponents];
    opponents[index] = { ...opponents[index], ai_level: raw == null ? null : clampAiLevel(raw) };
    this.#opponents = opponents;
  }

  /** Ajoute la même voiture qu'un adversaire existant, avec un skin différent
   * (pas encore pris par un autre adversaire de ce mod dans le plateau) —
   * rebouclé sur les skins déjà pris si tous sont épuisés (`pickSkin`, même
   * logique que la génération initiale). Insérée juste après la ligne source. */
  async duplicate(index: number) {
    const source = this.#opponents[index];
    const skin = await this.#skinFor(source.car_id, await this.#takenFromGrid());
    const clone: Opponent = { ...source, car_skin: skin };
    this.#opponents = [...this.#opponents.slice(0, index + 1), clone, ...this.#opponents.slice(index + 1)];
    this.count = this.#opponents.length;
  }

  /** Adversaires envoyés depuis la sélection groupée de la bibliothèque
   * voitures (§6.3ter). « set » remplace entièrement la liste ; « add » la
   * complète. Le jeton est incrémenté pour invalider toute génération DÉJÀ en
   * vol (ex. si le type de session était déjà "course" à l'arrivée sur cet
   * écran, le montage en a lancé une) : sans ça, son résultat arrive après
   * coup et écrase les adversaires qu'on vient d'imposer. */
  impose(action: OpponentsAction) {
    this.#gen++;
    const additions: Opponent[] = action.carIds.map((carId) =>
      newOpponent(carId, getPreferredSkin(carId)?.id ?? null),
    );
    this.#opponents = action.mode === "set" ? additions : [...this.#opponents, ...additions];
    this.count = this.#opponents.length;
  }

  /** Remplacement d'une ligne : la force est celle de la ligne, le skin est
   * tiré dans ceux de la nouvelle voiture (SESSION§3). */
  async replace(index: number, carId: string) {
    const skin = await this.#skinFor(carId, await this.#takenFromGrid(index));
    const opponents = [...this.#opponents];
    opponents[index] = { ...opponents[index], car_id: carId, car_skin: skin };
    this.#opponents = opponents;
  }

  /** Ajout en fin de plateau, dans l'ordre de la liste. Skin et force suivent
   * les règles déjà en place — rien de neuf ici. */
  async add(carIds: string[]) {
    const additions: Opponent[] = [];
    const taken = await this.#takenFromGrid();
    for (const carId of carIds) additions.push(newOpponent(carId, await this.#skinFor(carId, taken)));
    if (!additions.length) return;
    this.#opponents = [...this.#opponents, ...additions];
    this.count = this.#opponents.length;
  }

  /** Rétablit le vivier d'un preset ou d'une session enregistrée, **ou le
   * reconstruit** depuis les champs de l'époque des onglets (`migrateTabPool`).
   * `player` is the car being driven, which the old `same car` tab meant. */
  restorePool(p: SavedPool, player: ModCard | null) {
    if (p.grid_filters) {
      const snap = parseFilters(p.grid_filters, this.defs);
      this.query = snap.query;
      this.filters = snap.filters;
      this.pinned = p.grid_pinned ?? [];
      return;
    }
    this.query = "";
    this.filters = migrateTabPool(p, player);
    this.pinned = [];
  }

  /** The pool as presets and saved sessions store it. */
  serializedPool(): string {
    return serializeFilters(this.query, this.filters);
  }

  // --- Grilles enregistrées (§5) -------------------------------------------
  //
  // Une grille n'est PAS une session : elle ne porte que les adversaires et ce
  // qui fait le caractère du plateau (fourchette de force, agressivité), donc
  // la charger dans une session déjà configurée ne touche ni à la météo, ni à
  // l'heure, ni au type de session. C'est le cas réel : le même plateau GT3 sur
  // dix circuits.

  /** The grid as it is saved under `name`. */
  toSaved(name: string): SavedGrid {
    const setup = this.#host.setup;
    return {
      name,
      savedAt: new Date().toISOString(),
      // Une **copie**, jamais un lien (CIBLE§5.1) : sans `$state.snapshot`, c'est
      // le proxy réactif du plateau courant qui partirait au backend, et
      // modifier le plateau changerait la grille enregistrée.
      opponents: $state.snapshot(setup.opponents),
      // Les grilles gardent les bornes : c'est le vocabulaire de Content
      // Manager, d'où viennent les grilles importées. La conversion se fait
      // ici, à la frontière, plutôt que deux vocabulaires dans le modèle.
      aiLevelMin: Math.max(AI_LEVEL_MIN, setup.ai_level - setup.ai_spread),
      aiLevelMax: Math.min(AI_LEVEL_MAX, setup.ai_level + setup.ai_spread),
      aggression: setup.aggression,
    };
  }

  /** Charge une grille : **seuls les adversaires changent**, plus ce qui fait
   * le caractère du plateau. Une voiture disparue de la bibliothèque depuis
   * l'enregistrement est retirée — the caller says so —, jamais en échouant :
   * une grille survit à des années de bibliothèque remaniée. */
  loadSaved(grid: SavedGrid): { kept: number; missing: number } {
    const setup = this.#host.setup;
    const known = new Set(this.#host.carPool.map((c) => c.id_interne));
    const kept = grid.opponents.filter((o) => known.has(o.car_id)).map(restoreOpponent);
    this.#gen++;
    setup.opponents = kept;
    this.count = kept.length;
    const band = centerSpreadOf(clampAiLevel(grid.aiLevelMin), clampAiLevel(grid.aiLevelMax));
    setup.ai_level = band.center;
    setup.ai_spread = band.spread;
    setup.aggression = Math.max(0, Math.min(100, grid.aggression));
    return { kept: kept.length, missing: grid.opponents.length - kept.length };
  }
}
