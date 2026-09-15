<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import {
    launchSession,
    assistLevelFrom,
    AI_LEVEL_MAX,
    AI_LEVEL_MIN,
    clampAiLevel,
    START_MODES,
    newOpponent,
    type StartMode,
    isSteamRunning,
    nearestGrip,
    listModSkins,
    getModCspFeatures,
    weatherOptions,
    weatherConditions,
    trackSun,
    trackStates,
    nationalities,
    type AssistLevel,
    type Opponent,
    type PracticeStart,
    type RaceSetup,
    type Season,
    type SessionType,
    type SkinItem,
    type Nationality,
    type TrackStateOption,
    type TrackStateRef,
    type TrackSun,
    type WeatherOption,
  } from "$lib/launch/launch";
  import { carClassOf, driverFor, isEmpty } from "$lib/driver/driverOverride.svelte";
  import { centerSpreadOf } from "$lib/launch/aiBand";
  import { buildCardIndex, buildPredicate, filterDefs, parseFilters, serializeFilters, type FilterMap } from "$lib/library/filters";
  import { matchesQuery } from "$lib/library/cardSearch";
  import { hasOwnDriver } from "$lib/driver/driverOverride.svelte";
  import { defaultGridFilters } from "$lib/launch/opponentPool";
  import { setGridCars } from "$lib/launch/gridMods.svelte";
  import { playerHandicap, setPlayerHandicap } from "$lib/launch/playerHandicap.svelte";
  import { getModDetail, listLibrary, previewSrc, type ModCard } from "$lib/library/library";
  import { getSessionBackground } from "$lib/detail/media";
  import { nav, pickSession, type OpponentsAction } from "$lib/shell/nav.svelte";
  import { hasOpponents, openSetupPage, sessionNav, sessionNavReady } from "$lib/shell/sessionNav.svelte";
  import { carAssists, setCarAssists } from "$lib/launch/carAssists.svelte";
  import { getPreferredSkin, setPreferredLayout, setPreferredSkin } from "$lib/preferred";
  import { listActiveTrackSkins, listTrackSkinOptions, setTrackSkinActive, syncTrackSkins } from "$lib/inventory/submods";
  import { t } from "$lib/i18n/index.svelte";
  import ConditionsBlock from "./ConditionsBlock.svelte";
  import OpponentsBlock from "./OpponentsBlock.svelte";
  import GridBlock from "./GridBlock.svelte";
  import SessionOptionsBlock from "./SessionOptionsBlock.svelte";
  import SimulationBlock from "./SimulationBlock.svelte";
  import NamedListDialog from "$lib/components/ui/NamedListDialog.svelte";
  import OpponentPicker from "./OpponentPicker.svelte";
  import LoadingState from "$lib/components/ui/LoadingState.svelte";
  import {
    saveSession,
    listSavedSessions,
    deleteSavedSession,
    formatSavedAt,
    type SavedSession,
  } from "$lib/launch/savedSessions";
  import { deleteSavedGrid, listSavedGrids, saveGrid, type SavedGrid } from "$lib/launch/savedGrids";

  import { errorText } from "$lib/errors";
  import { StorageKey } from "$lib/storage";
  let libCards = $state<ModCard[]>([]);
  let weathers = $state<WeatherOption[]>([]);
  // Lus côté Rust dans la table du jeu (SETUP§2.2) : l'écran ne connaît plus la
  // liste, il la reçoit — c'est ce qui permettra d'y ajouter des états d'une
  // autre provenance sans le toucher.
  let trackStateList = $state<TrackStateOption[]>([]);
  // La liste des nationalités du jeu (§4.2), avec leurs drapeaux. Lue à
  // l'ouverture de l'écran comme les états de piste, et vide quand
  // l'installation n'est pas lisible — la cellule retombe alors sur la saisie
  // libre plutôt que d'offrir un menu vide.
  let nationalityList = $state<Nationality[]>([]);
  let selectedIntent = $state("");
  let opponentCount = $state(7);
  // Jeton de génération du plateau (§6.3ter) : `regenerateGrid` est asynchrone
  // (résolution des skins par IPC) et peut encore être « en vol » quand
  // `applyOpponentsAction` prend la main — sans garde, son résultat arrive
  // après coup et écrase les adversaires qu'on vient d'imposer. Toute
  // régénération capture le jeton courant et n'applique son résultat que s'il
  // n'a pas été invalidé entre-temps par un appel plus récent.
  let opponentsGen = 0;
  let launching = $state(false);
  let error = $state("");
  let info = $state("");
  // Ce qui n'a pas pu être rétabli au chargement d'une session enregistrée
  // (SESSION§3.5) : bandeau dans la page, au même endroit que le retour de
  // lancement — pas une popup. Il n'y a rien à décider, juste à savoir que la
  // session ne sera pas exactement celle qui avait été enregistrée.
  let warning = $state("");
  let ready = $state(false);

  let setup = $state<RaceSetup>({
    car_id: "",
    car_skin: null,
    driver: null,
    track_id: "",
    track_layout: null,
    session_type: "practice",
    opponents: [],
    ai_level: 95,
    ai_spread: 3,
    aggression: 0,
    aggression_spread: 0,
    player_ballast: 0,
    player_restrictor: 0,
    start_mode: "random",
    laps: 5,
    weather: "",
    time_hours: 13,
    ambient_c: null,
    road_c: null,
    wind_speed_kmh: null,
    wind_direction_deg: null,
    season: null,
    season_date: null,
    penalties: false,
    jump_start_penalty: 0,
    track_state: null,
    grip: 96,
    practice_enabled: false,
    practice_minutes: 20,
    qualify_enabled: true,
    qualify_minutes: 10,
    ghost_car: false,
    ghost_advantage: 0,
    practice_start: "pit",
    damage: 50,
    fuel_rate: 100,
    tyre_wear: 100,
    tyre_blankets: false,
    abs: "factory",
    traction_control: "factory",
    ideal_line: false,
  });

  // --- Saison optionnelle (SESSION§3.3) : associe une date au preset Quick
  // Drive (udt/dtv), best-effort côté CSP (voir RaceSetup.season_date côté back). ---
  // Mois/jour représentatifs (milieu de saison, hémisphère nord).
  const SEASON_MID: Record<Exclude<Season, "">, [number, number]> = {
    spring: [4, 15],
    summer: [7, 15],
    autumn: [10, 15],
    winter: [1, 15],
  };
  let season = $state<Season>("");
  /** Pose la saison sans effet de bord (utilisé aussi en interne : chargement
   * de preset, correction auto si le circuit ne gère pas la saison). Le reset
   * des températures recommandées est déclenché séparément, uniquement quand
   * l'utilisateur choisit lui-même une saison (voir `selectSeason`). */
  function applySeason(next: Season) {
    season = next;
    if (!next) {
      setup.season = null;
      setup.season_date = null;
    } else {
      const [month, day] = SEASON_MID[next];
      const year = new Date().getFullYear();
      setup.season = next;
      setup.season_date = `${year}-${String(month).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
    }
  }
  /** Choix de saison par l'utilisateur (bouton) : la saison influence la
   * température recommandée (SESSION§3.3) — la changer remet des valeurs
   * cohérentes, comme un changement de météo. */
  function selectSeason(next: Season) {
    applySeason(next);
    void refreshConditions(true);
  }

  // --- Support CSP effectif du circuit courant (§6) : détecté à la
  // volée (config propre au mod + config CSP "chargée" séparément — voir
  // get_mod_csp_features), pas figé à l'import. Sert à griser la saison si le
  // circuit ne sait pas la gérer, et à avertir si la pluie n'a pas de
  // paramétrage identifié pour ce circuit. ---
  let trackCspFeatures = $state<string[]>([]);
  $effect(() => {
    const id = setup.track_id;
    if (!id) {
      trackCspFeatures = [];
      return;
    }
    getModCspFeatures(id)
      .then((f) => {
        trackCspFeatures = f;
        // Le circuit ne gère pas la saison : pas la peine de garder une
        // sélection qui n'aura de toute façon aucun effet ici.
        if (!f.includes("season") && season !== "") applySeason("");
      })
      .catch(() => (trackCspFeatures = []));
  });
  // --- Course du soleil du circuit (SESSION§3.3) : alimente la bande jour/nuit
  // sous le curseur d'heure. Recalculée au changement de circuit, de layout ou
  // de date de saison — les trois entrées dont dépendent lever et coucher. ---
  let sun = $state<TrackSun | null>(null);
  $effect(() => {
    const id = setup.track_id;
    const layout = setup.track_layout;
    const date = setup.season_date;
    if (!id) {
      sun = null;
      return;
    }
    trackSun(id, layout, date)
      .then((s) => {
        // Le circuit a pu changer pendant l'appel : ne pas écraser la course
        // du soleil d'un autre circuit avec une réponse en retard.
        if (setup.track_id === id && setup.track_layout === layout && setup.season_date === date) sun = s;
      })
      .catch(() => (sun = null));
  });

  const trackSupportsSeason = $derived(trackCspFeatures.includes("season"));
  const trackSupportsRain = $derived(trackCspFeatures.includes("rainfx"));

  const carPool = $derived(libCards.filter((c) => c.kind === "Car"));
  /** Carte du circuit choisi — sert à connaître ses catégories (§5). */
  const trackCard = $derived(
    libCards.find((c) => c.kind === "Track" && c.id_interne === setup.track_id) ?? null,
  );
  const player = $derived(carPool.find((c) => c.id_interne === setup.car_id) ?? null);
  const currentWeather = $derived(weathers.find((w) => w.id === selectedIntent));

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
  const gridDefs = filterDefs("Car");
  let gridFilters = $state<FilterMap>(defaultGridFilters());
  // Aucun filtre épinglé : la barre s'ouvre sur son champ de recherche et son
  // menu, et les trois puces sont ce qui la remplit en un clic.
  let gridPinned = $state<string[]>([]);
  let gridQuery = $state("");
  const gridIndex = $derived(buildCardIndex(carPool, gridDefs, true, hasOwnDriver, setup.car_id));
  const gridMatches = $derived(buildPredicate(gridDefs, gridFilters, gridIndex.ctx));
  const gridPool = $derived(carPool.filter((c) => gridMatches(c) && matchesQuery(c, gridQuery)));

  // --- Skins par voiture (cache, SESSION§3.3) : chargés à la demande pour
  // assigner un skin à chaque adversaire, et réutilisés par la popup. ---
  let skinsByCarId = $state<Record<string, SkinItem[]>>({});
  async function ensureSkins(carId: string): Promise<SkinItem[]> {
    const cached = skinsByCarId[carId];
    if (cached) return cached;
    let skins: SkinItem[];
    try {
      skins = await listModSkins(carId);
    } catch {
      skins = [];
    }
    skinsByCarId = { ...skinsByCarId, [carId]: skins };
    return skins;
  }
  /**
   * Pioche un skin pour `carId`, en évitant ce qui est déjà pris.
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
  interface TakenIdentities {
    skins: Set<string>;
    drivers: Set<string>;
  }
  const newTaken = (): TakenIdentities => ({ skins: new Set(), drivers: new Set() });

  /** Ce qui doit rester unique : le couple numéro + nom, insensible à la
   * casse. Une livrée muette ne participe pas — elle n'impose rien. */
  function driverKey(skin: SkinItem): string | null {
    const key = `${skin.number ?? ""}|${skin.driver ?? ""}`.trim().toLowerCase();
    return key === "|" ? null : key;
  }

  /** Le registre de ce que le plateau courant porte déjà : une ligne ajoutée
   * après coup doit éviter les mêmes pilotes que le tirage initial. `skip`
   * exclut la ligne qu'on est en train de remplacer, qui ne se fait pas
   * concurrence à elle-même. */
  async function takenFromGrid(skip = -1): Promise<TakenIdentities> {
    const taken = newTaken();
    for (const [i, o] of setup.opponents.entries()) {
      if (i === skip || !o.car_skin) continue;
      taken.skins.add(o.car_skin);
      const sk = (await ensureSkins(o.car_id)).find((x) => x.id === o.car_skin);
      const key = sk && driverKey(sk);
      if (key) taken.drivers.add(key);
    }
    return taken;
  }

  async function skinFor(carId: string, taken: TakenIdentities): Promise<string | null> {
    const skins = await ensureSkins(carId);
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
    const pick = pool[Math.floor(Math.random() * pool.length)];
    taken.skins.add(pick.id);
    const key = driverKey(pick);
    if (key) taken.drivers.add(key);
    return pick.id;
  }

  /** Génère `n` adversaires pour le mode courant. `excludeCarIds` = mods déjà
   * présents dans le plateau, évités en priorité (sauf en « même voiture »,
   * `excludeCarIds` = mods déjà présents dans le plateau, évités en priorité.
   * Si le vivier distinct est épuisé (un vivier d'une seule voiture, par
   * exemple), on complète en dupliquant un mod déjà choisi avec un skin
   * différent plutôt que de tronquer le plateau — c'est ce que
   * l'avertissement de vivier maigre annonce (CIBLE§3.5).
   *
   * **Aucun repli sur la bibliothèque entière quand le vivier est vide** : un
   * filtre qui ne garde rien doit rendre un plateau vide, pas un plateau tiré
   * ailleurs. Le repli d'avant venait des onglets, dont le vivier pouvait être
   * vide sans que rien ne le dise ; le compteur `Pool · 0 cars` le dit
   * maintenant, et les deux boutons sont éteints. */
  async function generateOpponents(n: number, excludeCarIds: Set<string>): Promise<Opponent[]> {
    if (n <= 0) return [];
    const source = gridPool;
    if (!source.length) return [];

    const fresh = source.filter((c) => !excludeCarIds.has(c.id_interne)).sort(() => Math.random() - 0.5);
    const picks: ModCard[] = fresh.slice(0, n);
    const dupSource = picks.length ? picks : source;
    let idx = 0;
    while (picks.length < n) {
      picks.push(dupSource[idx % dupSource.length]);
      idx++;
    }

    const taken = newTaken();
    const out: Opponent[] = [];
    for (const c of picks) out.push(newOpponent(c.id_interne, await skinFor(c.id_interne, taken)));
    return out;
  }

  /** `Fill N at random` (CIBLE§3.3) : tire N voitures dans le vivier et **remplace**
   * le plateau. Le chemin de celui qui veut courir tout de suite. */
  async function fillGrid() {
    const gen = ++opponentsGen;
    const opponents = await generateOpponents(opponentCount, new Set());
    // Une action plus récente (nouvelle régénération, ou adversaires imposés
    // depuis la bibliothèque) a pris le dessus entre-temps : ne pas écraser.
    if (gen === opponentsGen) setup.opponents = opponents;
  }

  /** `Regenerate` (§4.1) : garde les voitures, **retire au sort ce qui avait
   * été tiré sur elles** — skin et force. Ce n'est pas `Fill` sous un autre
   * nom : « le plateau est bon mais les livrées se répètent » et « le plateau
   * n'est pas le bon » sont deux gestes qu'on veut séparément. */
  async function regenerateGrid() {
    const gen = ++opponentsGen;
    const taken = newTaken();
    const out: Opponent[] = [];
    for (const opp of setup.opponents) {
      // La livrée est retirée au sort, **pas** les cellules `Auto` : `Auto`
      // n'est pas une valeur qu'on tire, c'est l'absence de surcharge, et
      // c'est le jeu qui tire dedans (§4.1). Ce qui change vraiment ici est
      // donc la livrée — et avec elle le nom de pilote `Auto`, qui en vient.
      out.push({ ...opp, car_skin: await skinFor(opp.car_id, taken) });
    }
    if (gen === opponentsGen) setup.opponents = out;
  }

  async function applyOpponentCount(raw: number) {
    const n = Math.max(0, Math.min(30, Math.round(raw) || 0));
    opponentCount = n;
    const current = setup.opponents;
    if (n < current.length) {
      setup.opponents = current.slice(0, n);
    } else if (n > current.length) {
      const exclude = new Set(current.map((o) => o.car_id));
      const extra = await generateOpponents(n - current.length, exclude);
      setup.opponents = [...current, ...extra];
    }
  }

  function removeOpponent(index: number) {
    setup.opponents = setup.opponents.filter((_, i) => i !== index);
    opponentCount = setup.opponents.length;
  }

  /** Réglage individuel du niveau IA d'un adversaire (clic sur le chiffre),
   * indépendant de la fourchette globale qui ne sert qu'à la génération. */
  /** Remet une ligne relue sur disque dans la forme courante : les quatre
   * champs de §4.2 n'existaient pas, et `??` ne suffirait pas — un `undefined`
   * qui traverserait jusqu'au backend s'y lirait comme un champ absent, pas
   * comme `Auto`. */
  function restoreOpponent(o: Opponent): Opponent {
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

  // --- Grilles enregistrées (§5) -------------------------------------------
  //
  // Une grille n'est PAS une session : elle ne porte que les adversaires et ce
  // qui fait le caractère du plateau (fourchette de force, agressivité), donc
  // la charger dans une session déjà configurée ne touche ni à la météo, ni à
  // l'heure, ni au type de session. C'est le cas réel : le même plateau GT3 sur
  // dix circuits.
  let gridDialog = $state<"save" | "load" | null>(null);
  let savedGrids = $state<SavedGrid[]>([]);

  async function openGridDialog(mode: "save" | "load") {
    savedGrids = await listSavedGrids();
    gridDialog = mode;
  }

  async function doSaveGrid(name: string) {
    gridDialog = null;
    try {
      await saveGrid({
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
      });
    } catch (e) {
      error = errorText(e);
    }
  }

  /** Charge une grille : **seuls les adversaires changent**, plus ce qui fait
   * le caractère du plateau. Une voiture disparue de la bibliothèque depuis
   * l'enregistrement est retirée en le disant, jamais en échouant — une grille
   * survit à des années de bibliothèque remaniée. */
  async function doLoadGrid(name: string) {
    const grid = savedGrids.find((g) => g.name === name);
    gridDialog = null;
    if (!grid) return;
    const known = new Set(carPool.map((c) => c.id_interne));
    const kept = grid.opponents.filter((o) => known.has(o.car_id)).map(restoreOpponent);
    const missing = grid.opponents.length - kept.length;
    opponentsGen++;
    setup.opponents = kept;
    opponentCount = kept.length;
    const band = centerSpreadOf(clampAiLevel(grid.aiLevelMin), clampAiLevel(grid.aiLevelMax));
    setup.ai_level = band.center;
    setup.ai_spread = band.spread;
    setup.aggression = Math.max(0, Math.min(100, grid.aggression));
    warning = missing ? t("launch.gridMissingCars", { count: missing }) : "";
    info = t("launch.gridLoaded", { name: grid.name, count: kept.length });
  }

  async function removeSavedGrid(name: string) {
    await deleteSavedGrid(name);
    savedGrids = await listSavedGrids();
  }

  /** Une cellule d'une ligne du plateau (§4.1/§4.2). `null` sur un texte, et
   * `null` sur la force, valent **`Auto`** : la ligne n'a pas de surcharge et
   * le jeu décide. C'est ce que fait un champ vidé — le geste naturel pour dire
   * « je ne décide pas », et la raison pour laquelle aucun menu de ligne n'est
   * nécessaire pour y revenir. Le lest et la bride n'ont pas d'`Auto` : « rien »
   * s'y dit par 0, comme dans le preset. */
  function setOpponentCell(index: number, patch: Partial<Opponent>) {
    const opponents = [...setup.opponents];
    opponents[index] = { ...opponents[index], ...patch };
    setup.opponents = opponents;
  }

  /** Force d'une ligne. `null` = la cellule repasse en `Auto` — c'est ce que
   * fait un champ vidé, le geste naturel pour dire « je ne décide pas ». */
  function setOpponentLevel(index: number, raw: number | null) {
    const opponents = [...setup.opponents];
    opponents[index] = { ...opponents[index], ai_level: raw == null ? null : clampAiLevel(raw) };
    setup.opponents = opponents;
  }

  /** Ajoute la même voiture qu'un adversaire existant, avec un skin différent
   * (pas encore pris par un autre adversaire de ce mod dans le plateau) —
   * rebouclé sur les skins déjà pris si tous sont épuisés (`skinFor`, même
   * logique que la génération initiale). Insérée juste après la ligne source. */
  async function duplicateOpponentWithVariant(index: number) {
    const source = setup.opponents[index];
    const skin = await skinFor(source.car_id, await takenFromGrid());
    const clone: Opponent = { ...source, car_skin: skin };
    setup.opponents = [...setup.opponents.slice(0, index + 1), clone, ...setup.opponents.slice(index + 1)];
    opponentCount = setup.opponents.length;
  }

  /** Adversaires envoyés depuis la sélection groupée de la bibliothèque
   * voitures (§6.3ter). Bascule sur le type Course et le mode « libre »
   * directement (sans passer par `selectGridMode`, qui régénérerait le
   * plateau et écraserait les adversaires en cours). « set » remplace
   * entièrement la liste ; « add » la complète — dans les deux cas, les
   * adversaires déjà présents (même issus d'un mode même-voiture/même-catégorie
   * avant bascule) sont préservés pour « add », perdus pour « set ».
   *
   * Deux gardes contre une régénération asynchrone qui écraserait le résultat
   * après coup : (1) `lastCarForGrid` aligné AVANT de toucher `session_type` —
   * l'effet de resynchronisation de session (plus haut) lit aussi
   * `setup.session_type`/`setup.car_id`, donc passer `session_type` à "race"
   * `opponentsGen` est incrémenté pour invalider toute génération DÉJÀ en vol
   * (ex. si le type de session était déjà "course" à l'arrivée sur cet écran,
   * `onMount` en a lancé une) : sans ça, son résultat arrive après coup et
   * écrase les adversaires qu'on vient d'imposer. */
  function applyOpponentsAction(action: OpponentsAction) {
    opponentsGen++;
    setup.session_type = "race";
    const additions: Opponent[] = action.carIds.map((carId) =>
      newOpponent(carId, getPreferredSkin(carId)?.id ?? null),
    );
    setup.opponents = action.mode === "set" ? additions : [...setup.opponents, ...additions];
    opponentCount = setup.opponents.length;
  }

  // --- Modale de sélection d'adversaire (SESSION§3) ---
  //
  // Elle reçoit **toute** la bibliothèque voitures, et surtout **l'état de
  // filtre du bloc lui-même** (CIBLE§3.3) — elle ne dérive plus rien. C'est plus
  // simple que ce qui était en place : il n'y a qu'un vivier, celui que la
  // barre de filtres montre, et la modale en est la vue détaillée. Retirer un
  // jeton dans la modale élargit donc aussi le vivier du `Fill` — ce sont les
  // deux gestes d'un seul et même ensemble, pas deux ensembles à réconcilier.
  let pickerIndex = $state<number | null>(null);
  let pickerAdding = $state(false);
  const pickerOpen = $derived(pickerAdding || pickerIndex != null);

  function openPicker(index: number) {
    pickerIndex = index;
  }
  function openAddPicker() {
    pickerAdding = true;
  }
  function closePicker() {
    pickerIndex = null;
    pickerAdding = false;
  }

  /** Remplacement d'une ligne : la force est celle de la ligne, le skin est
   * tiré dans ceux de la nouvelle voiture (SESSION§3). */
  async function replaceOpponent(carId: string) {
    const i = pickerIndex;
    closePicker();
    if (i == null) return;
    const skin = await skinFor(carId, await takenFromGrid(i));
    const opponents = [...setup.opponents];
    opponents[i] = { ...opponents[i], car_id: carId, car_skin: skin };
    setup.opponents = opponents;
  }

  /** Ajout en fin de plateau, dans l'ordre de la liste. Skin et force suivent
   * les règles déjà en place — rien de neuf ici. */
  async function addOpponentsFromPicker(carIds: string[]) {
    closePicker();
    const additions: Opponent[] = [];
    const taken = await takenFromGrid();
    for (const carId of carIds) additions.push(newOpponent(carId, await skinFor(carId, taken)));
    if (!additions.length) return;
    setup.opponents = [...setup.opponents, ...additions];
    opponentCount = setup.opponents.length;
  }

  /** La livrée d'une ligne, quand elle est connue — et par elle, le pilote que
   * le jeu nommera. */
  function skinOfOpponent(opp: Opponent): SkinItem | undefined {
    return opp.car_skin ? skinsByCarId[opp.car_id]?.find((sk) => sk.id === opp.car_skin) : undefined;
  }

  /** Deux pilotes sous la même identité (SETUP§1.9). La génération l'évite ; ceci
   * n'attrape que ce que l'utilisateur a forcé à la main, et le dit plutôt que
   * de le corriger dans son dos.
   *
   * Calculé ici et non dans le plateau depuis que l'alerte doit **remonter sur
   * l'entrée de navigation** (L5§1.3) : une alerte sur une page qu'on ne
   * regarde pas ne vaut pas mieux que pas d'alerte. */
  const duplicateDrivers = $derived.by(() => {
    const seen = new Set<string>();
    for (const opp of setup.opponents) {
      const sk = skinOfOpponent(opp);
      const key = `${opp.driver_name ?? sk?.number ?? ""}|${opp.driver_name ?? sk?.driver ?? ""}`.trim().toLowerCase();
      if (key === "|") continue;
      if (seen.has(key)) return true;
      seen.add(key);
    }
    return false;
  });

  /** Sur quelle page de l'écran on est (L5§1) — la sous-entrée n'existe
   * que sous un type qui aligne un plateau. */
  const onOpponentsPage = $derived(sessionNav.page === "opponents" && hasOpponents(setup.session_type));

  /** Le circuit choisi n'est pas catégorisé comme circuit fermé, et le type
   * demandé se chronomètre au tour. Comparé sans le `#` : la catégorie est
   * stockée avec, mais un tag saisi à la main ou une règle personnalisée peut
   * l'écrire sans. */
  const trackNotCircuit = $derived.by(() => {
    const cats = trackCard?.categories ?? [];
    if (!cats.length) return false;
    if (setup.session_type !== "hotlap" && setup.session_type !== "race") return false;
    return !cats.some((c) => c.replace(/^#/, "").toLowerCase() === "circuit");
  });

  // --- Fourchette de niveau IA (SESSION§3) : bornes réutilisées par le réglage
  // individuel d'un adversaire (setOpponentLevel) — le curseur double lui-même
  // est rendu par OpponentsBlock. ---
  const RANGE_MIN = AI_LEVEL_MIN;
  const RANGE_MAX = AI_LEVEL_MAX;

  // --- Météo (intentions + température/vent, SESSION§3.3/SESSION§3) ---
  // Air, piste et vent sont des valeurs **recommandées** par météo+saison, mais
  // restent modifiables à la main (SESSION§3.3) : `tempsOverridden`/`windOverridden`
  // mémorisent que l'utilisateur a corrigé les valeurs proposées, pour ne plus
  // les écraser tant que la météo ou la saison ne change pas. Un changement de
  // météo ou de saison remet toujours des valeurs recommandées fraîches (reset
  // explicite) — même logique pour les deux, gardée en deux drapeaux séparés
  // parce qu'on peut vouloir corriger la température sans toucher au vent.
  let tempsOverridden = $state(false);
  let windOverridden = $state(false);
  async function selectIntent(opt: WeatherOption) {
    if (!opt.available || !opt.weather) return;
    selectedIntent = opt.id;
    setup.weather = opt.weather;
    await refreshConditions(true);
  }
  async function refreshConditions(resetOverride: boolean) {
    if (!selectedIntent) return;
    const c = await weatherConditions(selectedIntent, setup.time_hours, setup.season);
    if (resetOverride || !tempsOverridden) {
      setup.ambient_c = c.ambient;
      setup.road_c = c.road;
    }
    if (resetOverride || !windOverridden) {
      setup.wind_speed_kmh = c.wind_speed_kmh;
      setup.wind_direction_deg = c.wind_direction_deg;
    }
    if (resetOverride) {
      tempsOverridden = false;
      windOverridden = false;
    }
  }
  function overrideTemps() {
    tempsOverridden = true;
  }
  function overrideWind() {
    windOverridden = true;
  }
  let lastHour = $state(-1);
  $effect(() => {
    if (setup.time_hours !== lastHour && selectedIntent) {
      lastHour = setup.time_hours;
      refreshConditions(false);
    }
  });
  // --- Mémorisation de la sélection + presets (SESSION§3) ---
  // `opponents` en fait partie (SESSION§3.3, bug réel) : sans elle, revenir sur cet
  // écran après être allé choisir un circuit/une voiture démonte puis remonte
  // Launch.svelte — `setup.opponents` (état local) repart de zéro, et
  // `applyPreset` régénère alors un plateau aléatoire à la place de celui,
  // potentiellement construit à la main (mode « libre »), qu'avait l'utilisateur.
  //
  // Persisté côté Rust (`launch_state.json`, écriture synchrone), pas en
  // `localStorage` : même bug que le duo voiture/circuit (SESSION§3, voir
  // `nav.svelte.ts`/`session_state.rs`) — `localStorage` n'est pas garanti
  // synchrone sur disque côté WebView2, ce qui perdait les réglages de
  // session à la fermeture de l'app plutôt qu'au prochain changement d'onglet.
  interface Selection {
    car_id: string;
    car_skin: string | null;
    track_id: string;
    track_layout: string | null;
    session_type: SessionType;
    opponents: Opponent[];
    player_ballast: number;
    player_restrictor: number;
  }

  // --- Presets de session par type (SESSION§3) ---
  interface Persisted {
    /** Centre et écart (SETUP§2.9). Un preset d'avant porte encore `ai_level_min`
     * et `ai_level_max` : `applyPreset` les convertit, il ne les jette pas. */
    ai_level?: number; ai_spread?: number; aggression_spread?: number;
    ai_level_min?: number; ai_level_max?: number;
    opponent_count: number;
    /** Absents sur un preset antérieur au §4.4 : les défauts de Content
     * Manager, dernier sur la grille et agressivité nulle. */
    aggression?: number; start_mode?: StartMode; ghost_advantage?: number;
    /** Vivier d'adversaires (CIBLE§3.3), sérialisé par `serializeFilters` — la même
     * forme que les filtres de bibliothèque, relue par le même `parseFilters`.
     * Absent sur un preset antérieur aux jetons : `migrateGridPreset` reprend
     * alors les trois anciens champs (`grid_mode`, `category_selection`,
     * `year_min`/`year_max`), qui restent déclarés pour cette seule relecture
     * et ne sont plus jamais écrits. */
    grid_filters?: string;
    grid_pinned?: string[];
    grid_mode?: "same_car" | "same_category" | "free";
    category_selection?: string;
    year_min?: number; year_max?: number;
    laps: number; time_hours: number;
    penalties: boolean; jump_start_penalty: number;
    /** L'état de piste entier (L4§4.7). `grip` reste écrit pour qu'un retour en
     * arrière de version retrouve quelque chose, et relu quand `track_state`
     * manque. */
    track_state?: TrackStateRef | null; grip: number;
    practice_enabled: boolean; practice_minutes: number;
    qualify_enabled: boolean; qualify_minutes: number; ghost_car: boolean; practice_start: PracticeStart;
    damage: number; fuel_rate: number; tyre_wear: number; tyre_blankets: boolean; intent: string; season: Season;
    /** Trois états depuis SESSION§3 ; `abs_auto`/`traction_control_auto` sont les
     * booléens d'avant, relus une dernière fois par `assistLevelFrom`. */
    abs?: AssistLevel; traction_control?: AssistLevel;
    abs_auto?: boolean; traction_control_auto?: boolean;
    ideal_line: boolean;
  }
  let presets: Record<string, Persisted> = {};
  let applying = false;

  interface LaunchStateFile {
    selection: Selection | null;
    presets: Record<string, Persisted> | null;
  }
  function loadLaunchState(): Promise<LaunchStateFile> {
    return invoke<LaunchStateFile>("get_launch_state").catch(() => ({ selection: null, presets: null }));
  }
  // Envoie systématiquement l'état complet (sélection + presets) : la commande
  // réécrit tout le fichier à chaque appel, comme `save_session_picks` — un
  // envoi partiel effacerait l'autre moitié.
  function persistLaunchState() {
    if (!ready) return;
    const selection: Selection = {
      car_id: setup.car_id,
      car_skin: setup.car_skin,
      track_id: setup.track_id,
      track_layout: setup.track_layout,
      session_type: setup.session_type,
      opponents: setup.opponents,
      // Dans la sélection et non dans les presets par type : ces deux-là ne
      // dépendent pas du type de session (SETUP§2.7).
      player_ballast: setup.player_ballast,
      player_restrictor: setup.player_restrictor,
    };
    invoke("save_launch_state", { state: { selection, presets } }).catch((e) => console.error("save_launch_state", e));
  }
  $effect(() => {
    void [setup.car_id, setup.car_skin, setup.track_id, setup.track_layout, setup.session_type, setup.opponents,
      setup.player_ballast, setup.player_restrictor];
    persistLaunchState();
  });

  /**
   * Rétablit le vivier d'un preset, **ou le reconstruit** depuis les trois
   * champs de l'époque des onglets (CIBLE§3.1).
   *
   * Une migration plutôt qu'un repli sur les défauts : un utilisateur qui
   * courait en « Même catégorie / 2010-2016 » retrouve exactement ce vivier,
   * dit cette fois par deux jetons qu'il peut combiner. Le mode « même
   * voiture » se traduit par le jeton `Model` de la voiture du preset — la
   * seule perte assumée est qu'il ne suit plus la voiture pilotée, ce qui est
   * précisément ce que « une puce pose un jeton et rien d'autre » signifie.
   */
  function applyGridPreset(p: Persisted) {
    if (p.grid_filters) {
      const snap = parseFilters(p.grid_filters, gridDefs);
      gridQuery = snap.query;
      gridFilters = snap.filters;
      gridPinned = p.grid_pinned ?? [];
      return;
    }
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
    gridQuery = "";
    gridFilters = migrated;
    gridPinned = [];
  }

  function savePreset() {
    presets[setup.session_type] = {
      ai_level: setup.ai_level, ai_spread: setup.ai_spread, aggression_spread: setup.aggression_spread,
      aggression: setup.aggression, start_mode: setup.start_mode, ghost_advantage: setup.ghost_advantage,
      opponent_count: opponentCount,
      grid_filters: serializeFilters(gridQuery, gridFilters), grid_pinned: [...gridPinned],
      laps: setup.laps, time_hours: setup.time_hours,
      penalties: setup.penalties, jump_start_penalty: setup.jump_start_penalty,
      track_state: setup.track_state ? { ...setup.track_state } : null, grip: setup.grip,
      practice_enabled: setup.practice_enabled, practice_minutes: setup.practice_minutes,
      qualify_enabled: setup.qualify_enabled, qualify_minutes: setup.qualify_minutes, ghost_car: setup.ghost_car,
      practice_start: setup.practice_start,
      damage: setup.damage, fuel_rate: setup.fuel_rate, tyre_wear: setup.tyre_wear, tyre_blankets: setup.tyre_blankets,
      intent: selectedIntent, season,
      abs: setup.abs, traction_control: setup.traction_control, ideal_line: setup.ideal_line,
    };
    persistLaunchState();
  }
  async function applyPreset(type: SessionType) {
    const p = presets[type];
    applying = true;
    if (p) {
      // Recalés : un preset enregistré quand le plancher était 60 porte des
      // valeurs que Content Manager n'accepte pas, et les envoyer telles quelles
      // ferait courir une session que l'écran n'annonce pas.
      //
      // Et converti : un preset d'avant le modèle centre ± écart porte deux
      // bornes. Les convertir plutôt que retomber sur le défaut, sinon une
      // difficulté réglée depuis des mois se réinitialise sans un mot.
      if (p.ai_level != null) {
        setup.ai_level = clampAiLevel(p.ai_level);
        setup.ai_spread = Math.max(0, p.ai_spread ?? 0);
      } else {
        const band = centerSpreadOf(clampAiLevel(p.ai_level_min ?? 92), clampAiLevel(p.ai_level_max ?? 98));
        setup.ai_level = band.center;
        setup.ai_spread = band.spread;
      }
      setup.aggression_spread = Math.max(0, Math.min(100, p.aggression_spread ?? 0));
      setup.aggression = Math.max(0, Math.min(100, p.aggression ?? 0));
      // Les quatre valeurs courantes se relisent telles quelles ; seul le
      // `"custom"` d'avant les segments retombe sur le défaut. La liste était
      // écrite à l'envers — elle n'acceptait que `first` et `last`, donc un
      // preset portant `second` ou `random` revenait sur `random` en silence.
      setup.start_mode = START_MODES.includes(p.start_mode as StartMode) ? (p.start_mode as StartMode) : "random";
      setup.ghost_advantage = Math.max(0, Math.min(5, p.ghost_advantage ?? 0));
      opponentCount = p.opponent_count ?? 7;
      applyGridPreset(p);
      setup.laps = p.laps; setup.time_hours = p.time_hours;
      setup.penalties = p.penalties; setup.jump_start_penalty = p.jump_start_penalty ?? 0;
      // L'état entier s'il est là, le pourcentage seul sinon : `TrackConditionBlock`
      // retrouve alors l'état natif le plus proche, ce que faisait l'ancien select.
      setup.track_state = p.track_state ?? null;
      setup.grip = nearestGrip(p.grip ?? 100);
      setup.practice_enabled = p.practice_enabled ?? false; setup.practice_minutes = p.practice_minutes ?? 20;
      setup.qualify_enabled = p.qualify_enabled ?? true; setup.qualify_minutes = p.qualify_minutes ?? 10;
      setup.ghost_car = p.ghost_car ?? false; setup.practice_start = p.practice_start ?? "pit";
      setup.damage = p.damage ?? 50;
      setup.fuel_rate = p.fuel_rate ?? 100; setup.tyre_wear = p.tyre_wear ?? 100;
      setup.tyre_blankets = p.tyre_blankets ?? false;
      // Par le store, qui est la valeur vivante : les écrire dans `setup` seul
      // laisserait la carte voiture du panneau gauche afficher les anciennes.
      setCarAssists(assistLevelFrom(p.abs, p.abs_auto), assistLevelFrom(p.traction_control, p.traction_control_auto));
      setup.ideal_line = p.ideal_line ?? false;
      applySeason(p.season ?? "");
      const opt = weathers.find((w) => w.id === p.intent && w.available);
      if (opt) await selectIntent(opt);
    }
    // Ne remplit que s'il n'y a vraiment rien à préserver (première visite
    // de l'écran course/trackday, ou aucun adversaire restauré) — jamais en
    // écrasant silencieusement un plateau déjà construit (SESSION§3.3, bug réel).
    //
    // `fillGrid` et non `regenerateGrid` : celle-ci **garde les voitures** et
    // ne retire au sort que ce qui est posé dessus, donc sur un plateau vide
    // elle ne faisait rien du tout. Une course ouverte pour la première fois
    // restait sans adversaire, alors qu'on doit pouvoir la lancer sans être
    // allé sur la page adversaires (L5§1.5).
    if (hasOpponents(type) && setup.opponents.length === 0) await fillGrid();
    applying = false;
  }
  async function setSessionType(type: SessionType) {
    if (type === setup.session_type) return;
    savePreset();
    setup.session_type = type;
    await applyPreset(type);
  }

  // --- Le type vient de la colonne de session (L5§1) --------------------
  //
  // `sessionNav` est la valeur vivante, cet écran la recopie : la liste des
  // types est dans le panneau gauche, qui est à l'écran en permanence, alors
  // que celui-ci n'est monté que pendant qu'on le regarde. Un seul sens de
  // circulation, comme pour le lest et la bride — l'écran ne réécrit dans le
  // store qu'au **chargement** (son propre montage, une session enregistrée),
  // jamais en réaction.
  $effect(() => {
    const wanted = sessionNav.type;
    if (!ready) return;
    if (untrack(() => setup.session_type) !== wanted) void setSessionType(wanted);
  });
  // Le type décide de ce qui est AFFICHÉ et de ce qui est envoyé au jeu, jamais
  // de ce qui est mémorisé (L5§1.4) : la page adversaires d'un type qui n'en a
  // pas se referme, le plateau reste intact derrière.
  $effect(() => {
    if (!hasOpponents(sessionNav.type) && sessionNav.page === "opponents") openSetupPage();
  });
  // Ce que la sous-entrée annonce (L5§1.2/L5§1.3). Écrit d'ici parce que c'est ici
  // qu'on le sait ; lu là-bas parce que c'est là qu'il faut le voir sans
  // changer de page.
  $effect(() => {
    sessionNav.count = setup.opponents.length;
    sessionNav.center = setup.ai_level;
    sessionNav.spread = setup.ai_spread;
    // Les deux alertes de la page (L5§1.3) : un vivier trop maigre pour le nombre
    // demandé — vide compris —, et deux pilotes sous la même identité.
    sessionNav.alert =
      hasOpponents(setup.session_type) && (gridPool.length < opponentCount || duplicateDrivers);
  });

  // ABS et contrôle de traction : même circulation à sens unique que le lest
  // et la bride depuis qu'ils vivent dans la carte voiture (L5§2.2).
  $effect(() => {
    setup.abs = carAssists.abs;
    setup.traction_control = carAssists.tractionControl;
  });
  // Lest et bride : le store est la valeur vivante (il s'édite dans le panneau
  // gauche, toujours à l'écran), `setup` la recopie. **Un seul sens** — un
  // effet en retour ferait s'entre-réveiller les deux. Ce qui écrit dans
  // l'autre sens, c'est un chargement : preset, session enregistrée.
  $effect(() => {
    setup.player_ballast = playerHandicap.ballast;
    setup.player_restrictor = playerHandicap.restrictor;
  });

  // La garde d'activation de la colonne de session lit le plateau courant
  // (SESSION§3) : elle est rendue ailleurs, et n'a pas d'autre moyen de le voir.
  $effect(() => {
    setGridCars(setup.opponents.map((o) => o.car_id));
  });

  $effect(() => {
    void [setup.ai_level, setup.ai_spread, setup.aggression, setup.aggression_spread, setup.start_mode, setup.ghost_advantage,
      opponentCount, gridFilters, gridQuery, gridPinned,
      setup.laps,
      setup.time_hours, setup.penalties, setup.jump_start_penalty, setup.grip, setup.track_state,
      setup.practice_enabled, setup.practice_minutes, setup.qualify_minutes,
      setup.ghost_car, setup.practice_start, setup.damage, setup.fuel_rate, setup.tyre_wear, setup.tyre_blankets,
      selectedIntent, season,
      setup.abs, setup.traction_control, setup.ideal_line];
    if (ready && !applying && selectedIntent) savePreset();
  });

  // --- Chargement + résolution des défauts (SESSION§3) ---
  onMount(async () => {
    [weathers, libCards, trackStateList, nationalityList] = await Promise.all([
      weatherOptions(),
      listLibrary(),
      trackStates(),
      nationalities().catch(() => []),
    ]);

    const state = await loadLaunchState();
    // Repli sur l'ancien `localStorage` seulement si le fichier Rust n'a rien
    // (première ouverture après la mise à jour) — voir `nav.svelte.ts` pour le
    // même schéma sur le duo voiture/circuit.
    const hasPersisted = state.selection !== null || state.presets !== null;
    presets = hasPersisted ? (state.presets ?? {}) : JSON.parse(localStorage.getItem(StorageKey.launchPresets) ?? "{}");
    const saved: Partial<Selection> = hasPersisted
      ? (state.selection ?? {})
      : JSON.parse(localStorage.getItem(StorageKey.launchSelection) ?? "{}");
    // Le type vient de la colonne de session, qui a hydraté le sien depuis ce
    // même fichier — attendre sa lecture plutôt que de relire la nôtre :
    // les deux se courent sinon après, et un type choisi dans la liste avant
    // que cet écran ne soit monté se ferait écraser par celui du disque.
    await sessionNavReady;
    setup.session_type = sessionNav.type;
    // Forces recalées à la relecture, pas seulement à l'édition : un plateau
    // enregistré quand le plancher était 60 porte des valeurs que Content
    // Manager n'accepte pas.
    //
    // Une force enregistrée avant les cellules `Auto` reste une valeur
    // **explicite**, elle ne devient pas `Auto` : rien ne distingue un nombre
    // tiré au hasard par l'ancienne génération d'un nombre posé à la main, et
    // effacer le second serait pire que garder le premier. Le champ se vide
    // d'un geste pour repasser en `Auto`.
    if (saved.opponents?.length) setup.opponents = saved.opponents.map(restoreOpponent);

    // La bibliothèque EST le sélecteur (SESSION§3) : voiture/circuit viennent du duo
    // de session choisi dans les bibliothèques — rien à choisir ici.
    syncFromSession();
    const first = weathers.find((w) => w.available);
    if (first) await selectIntent(first);
    await applyPreset(setup.session_type);
    if (setup.opponents.length) opponentCount = setup.opponents.length;
    ready = true;
    // Migration depuis `localStorage`, ou simplement première écriture :
    // s'assure que `launch_state.json` reflète l'état actuel sans attendre un
    // changement de réglage par l'utilisateur.
    if (!hasPersisted) persistLaunchState();
  });

  // Applique le duo de session (SESSION§3) au setup : voiture, skin piloté, circuit,
  // layout. Repli sur le 1er installé si aucune sélection.
  function syncFromSession() {
    const c = nav.sessionCar;
    const tr = nav.sessionTrack;
    setup.car_id = c?.id ?? carPool[0]?.id_interne ?? "";
    // Skin de session choisi sur la fiche (SESSION§3), repli sur mémorisé.
    setup.car_skin = c?.skin ?? (c ? getPreferredSkin(c.id)?.id ?? null : null);
    setup.track_id = tr?.id ?? "";
    setup.track_layout = tr?.layout ?? null;
  }

  // Resynchronise si le duo change (l'utilisateur ouvre une autre voiture/
  // circuit dans la bibliothèque puis revient à la session).
  //
  // **Le plateau ne se régénère plus tout seul au changement de voiture**, et
  // c'est la disparition des onglets qui l'emporte (§4.1). La règle était
  // « régénérer sauf en mode libre, sauf si le plateau a été touché à la
  // main » — trois conditions pour deviner si le plateau appartenait encore à
  // l'utilisateur ou au vivier. Le vivier est maintenant un filtre, qu'un
  // changement de voiture ne déplace pas (les jetons `Model` et `Category`
  // portent une valeur), donc régénérer jetterait un plateau au profit d'un
  // tirage dans le **même** vivier. Le plateau ne change plus que sur un
  // geste : `Fill`, `Choose`, `Regenerate`.
  $effect(() => {
    void [nav.sessionCar?.id, nav.sessionCar?.skin, nav.sessionTrack?.id, nav.sessionTrack?.layout];
    if (!ready) return;
    syncFromSession();
  });

  // Fond photo derrière l'interface (§6.2/SESSION§3) : combo exact → même circuit →
  // background officiel CSP → null (fond neutre actuel, aucun changement visuel).
  // Non bloquant pour l'écran : une erreur reste silencieuse, ce fond est un
  // agrément, jamais une donnée dont dépend le lancement de la session.
  let backgroundSrc = $state<string | null>(null);
  $effect(() => {
    const carId = setup.car_id;
    const trackId = setup.track_id;
    const layoutId = setup.track_layout;
    if (!carId || !trackId) {
      backgroundSrc = null;
      return;
    }
    getSessionBackground(carId, trackId, layoutId)
      .then((path) => {
        if (setup.car_id === carId && setup.track_id === trackId && setup.track_layout === layoutId) {
          backgroundSrc = previewSrc(path);
        }
      })
      .catch(() => {
        backgroundSrc = null;
      });
  });

  // Lancement immédiat demandé depuis le bouton rouge « Démarrer la session »
  // de la barre latérale (SESSION§3.3) : réactif plutôt que dans onMount, pour
  // couvrir aussi bien l'arrivée fraîche sur cet écran que le cas où il est
  // déjà ouvert (auquel cas onMount ne se redéclenche pas).
  $effect(() => {
    if (nav.autoLaunch && ready) {
      nav.autoLaunch = false;
      launch();
    }
  });

  // Action « adversaires » posée depuis la bibliothèque voitures (§6.3ter) :
  // même schéma que autoLaunch ci-dessus, consommée une fois l'écran prêt
  // (couvre l'arrivée fraîche sur cet écran comme le cas déjà ouvert).
  $effect(() => {
    if (nav.opponentsAction && ready) {
      const action = nav.opponentsAction;
      nav.opponentsAction = null;
      applyOpponentsAction(action);
    }
  });

  // --- Contrôle Steam (SESSION§2.3) ---
  // Assetto Corsa est un jeu Steam : sans Steam, le lancement échoue côté
  // Content Manager, après que Pit Box a rendu la main — aucune erreur ne
  // remonte jusqu'ici, l'utilisateur voit juste une session qui ne démarre
  // pas. Le seul moment où on peut encore expliquer, c'est avant de lancer.
  let steamPromptOpen = $state(false);
  let steamStillMissing = $state(false);
  let steamChecking = $state(false);

  // Un échec de la vérification elle-même ne doit pas empêcher de jouer :
  // dans le doute on laisse passer, l'échec côté CM reste le pire cas.
  async function steamReady(): Promise<boolean> {
    try {
      return await isSteamRunning();
    } catch {
      return true;
    }
  }

  async function launch() {
    if (launching || !setup.car_id || !setup.track_id) return;
    if (!(await steamReady())) {
      steamStillMissing = false;
      steamPromptOpen = true;
      return;
    }
    await doLaunch();
  }

  async function confirmSteamStarted() {
    if (steamChecking) return;
    steamChecking = true;
    const ok = await steamReady();
    steamChecking = false;
    if (!ok) {
      steamStillMissing = true;
      return;
    }
    steamPromptOpen = false;
    await doLaunch();
  }

  async function doLaunch() {
    savePreset();
    launching = true;
    // L'avertissement de chargement porte sur ce qui n'a pas pu être rétabli :
    // une fois la session lancée telle qu'elle est, il n'a plus d'objet.
    error = ""; info = ""; warning = "";
    try {
      // Le pilote est résolu **au moment du lancement**, pas tenu à jour dans
      // `setup` : sa source est la cascade par voiture (`driverFor`), qui
      // dépend de la voiture choisie et de la tenue par défaut, deux choses qui
      // bougent ailleurs dans l'app.
      // La classe de la voiture décide de laquelle des deux tenues par défaut
      // s'applique : demandée ici, une fois, au moment où elle sert.
      const carDetail = setup.car_id ? await getModDetail(setup.car_id).catch(() => null) : null;
      const outfit = driverFor(setup.car_id || null, carClassOf(carDetail?.car_class));
      await launchSession({
        ...$state.snapshot(setup),
        driver: isEmpty(outfit)
          ? null
          : { model: outfit.body, suit: outfit.suit, gloves: outfit.gloves, helmet: outfit.helmet },
      });
      info = t("launch.launchSuccess");
    } catch (e) {
      error = errorText(e);
    } finally {
      launching = false;
    }
  }

  // --- Sessions sauvegardées nommées (SESSION§3.5) : instantané complet des
  // réglages (adversaires, météo, options…), rappelable par nom — distinct
  // des presets automatiques par type. Ne touche pas au duo voiture/circuit
  // courant (géré par la bibliothèque, SESSION§3) : seuls les réglages sont repris.
  // La liste (carte « Sessions enregistrées ») est filtrée par type — un
  // effet la recharge à chaque changement d'onglet, et le save/delete la
  // rafraîchissent en plus puisqu'ils ne changent pas le type. ---
  // Deux boutons et une modale, comme les grilles (SETUP§2.11) : enregistrer et
  // recharger une configuration nommée est le même geste des deux côtés, il ne
  // peut pas avoir deux grammaires d'interface.
  //
  // **Dans la barre de titre**, et pas à côté du type de session : ça
  // suggérerait que la sauvegarde est rattachée au type, alors qu'elle porte
  // sur toute la configuration de la page. L'action se place au niveau de ce
  // qu'elle enregistre — d'où `Save grid…` en bas de la grille et celle-ci en
  // en-tête d'écran.
  let sessionDialog = $state<"save" | "load" | null>(null);
  let savedList = $state<SavedSession[]>([]);
  $effect(() => {
    const type = setup.session_type;
    // Le type ne filtre plus, il TRIE (SETUP§2.11) : la liste les porte toutes, et
    // celles du type courant viennent en tête. Le type peut changer avant que
    // la réponse (invoke Rust) n'arrive — n'applique le résultat que s'il
    // correspond encore, sinon une réponse tardive rendrait un tri périmé.
    listSavedSessions(type).then((list) => {
      if (setup.session_type === type) savedList = list;
    });
  });

  /** Ce qui distingue deux sauvegardes d'un coup d'œil : son type — devenu une
   * propriété affichée depuis qu'il ne filtre plus —, son circuit, sa date. */
  function savedMeta(s: SavedSession): string {
    const track = libCards.find((c) => c.id_interne === s.setup.track_id)?.display_name ?? s.setup.track_id;
    return [t(`launch.type.${s.setup.session_type}`), track, formatSavedAt(s.savedAt)].filter(Boolean).join(" · ");
  }

  async function removeSavedSession(name: string) {
    // Le type vient de l'entrée elle-même, pas de l'écran : la liste n'est plus
    // filtrée, donc on peut très bien supprimer une session d'un autre type que
    // celui qu'on est en train de régler.
    const entry = savedList.find((s) => s.name === name);
    if (entry) await deleteSavedSession(entry.setup.session_type, name);
    savedList = await listSavedSessions(setup.session_type);
  }

  async function doSaveSession(name: string) {
    // Skins de circuit actifs : état de déploiement, pas un champ de `setup` —
    // capturé ici pour pouvoir remettre le circuit dans l'apparence qu'il
    // avait quand la session a été enregistrée. Un échec de lecture ne doit
    // pas empêcher la sauvegarde du reste : liste vide plutôt que rien.
    const trackSkins = setup.track_id ? await listActiveTrackSkins(setup.track_id).catch(() => []) : [];
    await saveSession({
      name,
      savedAt: new Date().toISOString(),
      setup: $state.snapshot(setup),
      opponentCount,
      gridFilters: serializeFilters(gridQuery, gridFilters),
      gridPinned: [...gridPinned],
      season,
      intent: selectedIntent,
      trackSkins,
    });
    savedList = await listSavedSessions(setup.session_type);
    sessionDialog = null;
  }

  /** Charge une session enregistrée (SESSION§3.5) : réglages **et** duo de session
   * (voiture + skin piloté, circuit + tracé + skins de circuit).
   *
   * Rien n'est bloquant ici : un mod supprimé depuis la sauvegarde laisse la
   * sélection courante en place et se signale dans le bandeau d'avertissement,
   * plutôt que d'interrompre le chargement du reste. Une session enregistrée
   * survit à des années de bibliothèque remaniée — l'échec partiel est le cas
   * normal, pas l'exception. */
  async function doLoadSession(s: SavedSession) {
    error = ""; info = ""; warning = "";
    const warnings: string[] = [];

    setup = { ...setup, ...s.setup, opponents: (s.setup.opponents ?? []).map(restoreOpponent) };
    // Par le store, qui est la valeur vivante : l'écrire dans `setup` seul
    // laisserait la carte du panneau gauche afficher l'ancienne.
    setPlayerHandicap(s.setup.player_ballast ?? 0, s.setup.player_restrictor ?? 0);
    setCarAssists(s.setup.abs ?? "factory", s.setup.traction_control ?? "factory");
    // Le type fait partie de ce qui est rechargé, et il vit désormais dans la
    // colonne de session : sans ça, la liste resterait sur l'ancien.
    sessionNav.type = s.setup.session_type;
    opponentCount = s.opponentCount;
    // Même migration que pour un preset par type : une sauvegarde d'avant les
    // jetons retrouve son vivier, elle ne retombe pas sur les défauts.
    applyGridPreset({ grid_filters: s.gridFilters, grid_pinned: s.gridPinned, grid_mode: s.gridMode,
      category_selection: s.categorySelection } as Persisted);
    season = s.season;
    selectedIntent = s.intent;

    await restoreCar(s.setup.car_id, s.setup.car_skin, warnings);
    await restoreTrack(s.setup.track_id, s.setup.track_layout, s.trackSkins, warnings);

    // Réaligne `setup` sur le duo de session : `pickSession` l'a mis à jour
    // pour ce qui a été retrouvé, et pour ce qui manquait c'est la sélection
    // courante qui fait foi. Sans cet appel, l'id d'un mod disparu resterait
    // dans `setup` et partirait tel quel au lancement quand NI la voiture NI
    // le circuit n'ont pu être rétablis — aucun `pickSession` n'ayant eu lieu,
    // l'effet de resynchronisation ne passe pas de lui-même.
    syncFromSession();

    warning = warnings.join(" ");
  }

  /** Rétablit la voiture pilotée et son skin. Passe par `pickSession` et non
   * par `setup` : le duo de session est la source de vérité (SESSION§3), l'effet de
   * resynchronisation réécrirait sinon `setup.car_id` avec la voiture restée
   * dans la barre latérale. */
  async function restoreCar(carId: string, skinId: string | null, warnings: string[]) {
    if (!carId) return;
    const card = carPool.find((c) => c.id_interne === carId);
    if (!card) {
      warnings.push(t("launch.loadWarnCarMissing", { id: carId }));
      return;
    }
    const skins = await ensureSkins(carId);
    const skin = skinId ? skins.find((sk) => sk.id === skinId) ?? null : null;
    if (skinId && !skin) warnings.push(t("launch.loadWarnCarSkinMissing", { id: skinId }));
    if (skin) setPreferredSkin(carId, skin);
    const meta = [card.brand, card.year].filter(Boolean).join(" · ");
    pickSession("Car", {
      id: carId,
      name: card.display_name ?? carId,
      meta,
      preview: skin?.preview ?? card.preview,
      layout: null,
      skin: skin?.id ?? null,
      outline: null,
    });
  }

  /** Rétablit le circuit, son tracé et ses skins. Même principe que
   * `restoreCar` : tout passe par le duo de session. */
  async function restoreTrack(trackId: string, layoutId: string | null, trackSkins: string[] | undefined, warnings: string[]) {
    if (!trackId) return;
    const card = libCards.find((c) => c.id_interne === trackId && c.kind === "Track");
    if (!card) {
      warnings.push(t("launch.loadWarnTrackMissing", { id: trackId }));
      return;
    }
    const detail = await getModDetail(trackId).catch(() => null);
    const layouts = detail?.track?.layouts ?? [];
    const layout = layoutId ? layouts.find((l) => l.id === layoutId) ?? null : null;
    if (layoutId && !layout) warnings.push(t("launch.loadWarnLayoutMissing", { id: layoutId }));
    if (layout) setPreferredLayout(trackId, layout);
    // Avant `pickSession`, pas après : la barre latérale recharge sa liste de
    // skins de circuit quand `nav.sessionTrack` change, donc basculer les
    // skins d'abord lui fait lire l'état déjà à jour. Dans l'autre ordre, elle
    // afficherait les cases de l'état précédent jusqu'au prochain changement
    // de circuit.
    await restoreTrackSkins(trackId, trackSkins, warnings);
    const meta = card.author ?? "";
    pickSession("Track", {
      id: trackId,
      name: card.display_name ?? trackId,
      meta,
      preview: layout?.preview ?? card.preview,
      layout: layout?.id ?? null,
      skin: null,
      outline: layout?.outline ?? card.outline,
    });
  }

  /** Remet exactement le jeu de skins de circuit de la sauvegarde (§8) :
   * ceux qui manquent sont activés, ceux en trop désactivés — un skin resté
   * actif d'une session précédente changerait sinon l'apparence du circuit
   * sans que rien ne le signale. */
  async function restoreTrackSkins(trackId: string, wanted: string[] | undefined, warnings: string[]) {
    if (!wanted) return;
    try {
      await syncTrackSkins(trackId);
      const options = await listTrackSkinOptions(trackId);
      const missing = wanted.filter((name) => !options.some((o) => o.name === name));
      if (missing.length) warnings.push(t("launch.loadWarnTrackSkinsMissing", { names: missing.join(", ") }));
      for (const o of options) {
        const active = wanted.includes(o.name);
        if (o.active !== active) await setTrackSkinActive(trackId, o.name, active);
      }
    } catch (e) {
      warnings.push(t("launch.loadWarnTrackSkinsFailed", { error: errorText(e) }));
    }
  }
</script>

<div class="flow" class:has-bg={!!backgroundSrc} style:--session-bg={backgroundSrc ? `url('${backgroundSrc}')` : undefined}>
  <!-- Le titre suit la navigation (L5§1.6) : le type, puis le type et sa
       sous-entrée. « Paramétrage de la session » ne disait plus rien depuis que
       la liste des types est dans la colonne — c'est elle qui nomme l'écran.
       Enregistrer/charger restent où ils étaient : ils portent sur toute la
       configuration, pas sur le type. -->
  <header class="bar">
    <h1 class="lbl-screen">
      {t(`launch.type.${setup.session_type}`)}{#if onOpponentsPage}<span class="crumb">
          · {t("launch.opponentsLabel")}</span
        >{/if}
    </h1>
    <!-- Le décompte passe sur le bouton : il disait « 12 » à côté d'un titre
         qui ne parlait pas de sauvegardes. -->
    <div class="hbtns">
      <button class="btn" type="button" onclick={() => (sessionDialog = "save")}>{t("launch.saveSession")}</button>
      <button class="btn" type="button" onclick={() => (sessionDialog = "load")}
        >{t("launch.loadSession")}{#if savedList.length}&nbsp;({savedList.length}){/if}</button
      >
    </div>
  </header>

  {#if info}<div class="ok">{info}</div>{/if}
  {#if error}<div class="errbox">{error}</div>{/if}
  {#if warning}<div class="warnbox banner">⚠ {warning}</div>{/if}

  {#if !ready}
    <LoadingState />
  {:else if onOpponentsPage}
    <!-- LA PAGE ADVERSAIRES (L5§5) : un seul enchaînement, pleine largeur,
         sans césure de carte entre le générateur et sa sortie. Le bloc du haut
         configure un générateur, le plateau en EST la sortie — séparés par une
         gouttière, trois liens réels devenaient invisibles : la bannière de
         vivier explique le contenu du plateau, `Tirer au hasard` et
         `Régénérer` font des choses voisines, et la colonne « Force » réagit à
         un curseur hors de vue. -->
    <div class="body">
      <div class="oppopage">
        <OpponentsBlock
          {setup}
          {opponentCount}
          defs={gridDefs}
          bind:filters={gridFilters}
          bind:pinned={gridPinned}
          bind:query={gridQuery}
          index={gridIndex}
          poolCount={gridPool.length}
          playerCard={player}
          oncountchange={applyOpponentCount}
          onfill={() => void fillGrid()}
        />
        <GridBlock
          {setup}
          {carPool}
          {skinsByCarId}
          index={gridIndex}
          poolCount={gridPool.length}
          {nationalityList}
          {duplicateDrivers}
          onchoose={openAddPicker}
          onregenerate={() => void regenerateGrid()}
          onremove={removeOpponent}
          onduplicate={duplicateOpponentWithVariant}
          onsetlevel={setOpponentLevel}
          onsetcell={setOpponentCell}
          onsavegrid={() => void openGridDialog("save")}
          onloadgrid={() => void openGridDialog("load")}
          onopenpicker={openPicker}
        />
      </div>
    </div>
  {:else}
    <div class="body">
      <div class="cols">
        <!-- COLONNE CENTRALE — la session et ses règles.
             Un réglage se range selon sa PORTÉE, jamais selon sa fréquence
             d'usage : ce qui porte sur la voiture est dans le panneau gauche,
             ce qui porte sur les conditions dans le rail de droite, ce qui
             porte sur les adversaires a sa page. -->
        <div>
          <!-- Un hotlap et une course supposent un tracé qui boucle et qu'on
               chronomètre ; sur une montée ou un point-à-point, elles partent
               mais ne veulent rien dire. On avertit sans bloquer — l'app
               n'arbitre pas ce que l'utilisateur a le droit de lancer, et la
               catégorie vient de règles que lui-même peut modifier.
               Au niveau de la page depuis que le type n'a plus de bloc : c'est
               le couple type + circuit qui est en cause, pas un réglage. -->
          {#if trackNotCircuit}
            <p class="warnbox banner-warn">⚠ {t("launch.trackNotCircuitWarning")}</p>
          {/if}

          <SessionOptionsBlock {setup} />

          <SimulationBlock {setup} />
        </div>

        <!-- RAIL DROIT — les conditions, en un seul bloc (L5§4).
             `Track condition` et `Weather` étaient deux cartes, et la première
             entrée de l'état de piste est « Auto (posé par la météo) » : une
             entrée qui nomme sa voisine ne se lit que si cette voisine est sous
             les yeux. Elles n'étaient pas voisines par commodité de mise en
             page, elles ne faisaient qu'un. -->
        <div>
          <ConditionsBlock
            {setup}
            {weathers}
            {selectedIntent}
            {currentWeather}
            {trackSupportsSeason}
            {sun}
            {trackSupportsRain}
            {season}
            states={trackStateList}
            onselectintent={selectIntent}
            onselectseason={selectSeason}
            onoverridetemps={overrideTemps}
            onoverridewind={overrideWind}
          />
        </div>
      </div>
    </div>
  {/if}
</div>

<!-- Steam manquant (SESSION§2.3) : dialogue bloquant plutôt qu'un message dans le
     bandeau, parce qu'il y a un geste à faire hors de l'app et qu'il faut
     revérifier après — un texte passif laisserait l'utilisateur relancer dans
     le vide. -->
{#if sessionDialog}
  <NamedListDialog
    mode={sessionDialog}
    searchable
    title={t(sessionDialog === "save" ? "launch.saveSessionTitle" : "launch.loadSessionTitle")}
    placeholder={t(sessionDialog === "save" ? "launch.sessionNamePlaceholder" : "launch.sessionSearchPlaceholder")}
    emptyText={t("launch.noSavedSessions")}
    entries={savedList.map((s) => ({ name: s.name, meta: savedMeta(s) }))}
    onsave={(name) => void doSaveSession(name)}
    onpick={(name) => {
      const s = savedList.find((x) => x.name === name);
      sessionDialog = null;
      if (s) void doLoadSession(s);
    }}
    ondelete={(name) => void removeSavedSession(name)}
    onclose={() => (sessionDialog = null)}
  />
{/if}

{#if gridDialog}
  <NamedListDialog
    mode={gridDialog}
    searchable
    title={t(gridDialog === "save" ? "launch.saveGridTitle" : "launch.loadGridTitle")}
    placeholder={t(gridDialog === "save" ? "launch.gridNamePlaceholder" : "launch.gridSearchPlaceholder")}
    emptyText={t("launch.noSavedGrids")}
    entries={savedGrids.map((g) => ({
      name: g.name,
      meta: t("launch.gridAiCount", { count: g.opponents.length }),
      badge: g.fromCm ? t("launch.fromCm") : undefined,
    }))}
    onsave={(name) => void doSaveGrid(name)}
    onpick={(name) => void doLoadGrid(name)}
    ondelete={(name) => void removeSavedGrid(name)}
    onclose={() => (gridDialog = null)}
  />
{/if}

{#if pickerOpen}
  <OpponentPicker
    pool={carPool}
    mode={pickerIndex != null ? "replace" : "add"}
    bind:filters={gridFilters}
    bind:pinned={gridPinned}
    bind:query={gridQuery}
    perfRefId={setup.car_id}
    gridCount={setup.opponents.length}
    gridTarget={opponentCount}
    slotNumber={(pickerIndex ?? 0) + 1}
    currentCarId={pickerIndex != null ? setup.opponents[pickerIndex].car_id : null}
    onadd={addOpponentsFromPicker}
    onreplace={replaceOpponent}
    onclose={closePicker}
  />
{/if}

{#if steamPromptOpen}
  <div class="backdrop">
    <div class="modal">
      <h2>{t("launch.steamRequiredTitle")}</h2>
      <p>{t("launch.steamRequiredBody")}</p>
      {#if steamStillMissing}
        <p class="steam-missing">{t("launch.steamStillMissing")}</p>
      {/if}
      <div class="steam-actions">
        <button class="btn btn-ghost" type="button" onclick={() => (steamPromptOpen = false)}>
          {t("common.cancel")}
        </button>
        <button class="btn btn-primary" type="button" disabled={steamChecking} onclick={confirmSteamStarted}>
          {steamChecking ? t("common.working") : t("launch.steamStarted")}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* Dialogue Steam — même langage visuel que `NamedListDialog` ; le CSS
     des composants étant scopé, il se recopie plutôt qu'il ne s'hérite. */
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .modal {
    width: 420px;
    max-width: 92vw;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    background: var(--panel);
    border: 1px solid var(--rosso);
  }
  .modal h2 {
    font-size: 13px;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    color: var(--txt2);
  }
  .modal p {
    font-size: 12px;
    line-height: 1.5;
    color: var(--txt2);
  }
  .steam-missing {
    color: var(--yellow);
  }
  .steam-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  /* Écran plein-page (AppShell rend `.content.fixed` pour "race", comme la
     bibliothèque) : .flow gère lui-même son défilement — plus de hack de
     marge négative pour compenser le padding du parent. */
  .flow {
    height: 100%;
    overflow-y: auto;
    /* Contient le flou du fond (voir .flow.has-bg::before) : `filter` déborde
       naturellement de la boîte source, sans ça il déborderait aussi sur la
       colonne de gauche du shell (AppShell n'a pas de scroll horizontal). */
    overflow-x: hidden;
    position: relative;
  }
  /* Fond photo assombri et flouté (§6.2/SESSION§3) : appliqué seulement si un
     média a été résolu, sinon le fond neutre existant reste inchangé. Flou
     posé sur un calque séparé (::before, derrière tout le contenu) plutôt
     que sur .flow directement — un `filter` sur .flow flouterait aussi les
     champs/texte qu'il contient. Le calque est statique (pas d'anim, pas de
     scroll dépendant de la position) : le navigateur le peint une fois et le
     recompose tel quel, un flou marqué ne coûte donc rien en continu malgré
     le rayon élevé — c'est un flou léger, recalculé sans arrêt (ou un flou
     posé sur un élément qui bouge), qui serait cher, pas celui-ci. */
  .flow.has-bg::before {
    content: "";
    position: absolute;
    inset: 0;
    z-index: -1;
    background-image: linear-gradient(rgba(5, 5, 7, 0.86), rgba(5, 5, 7, 0.86)), var(--session-bg);
    background-size: cover;
    background-position: center;
    filter: blur(32px);
  }
  /* Les deux boutons de sauvegarde, à droite du titre d'écran. */
  .hbtns {
    display: flex;
    gap: 8px;
  }
  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 14px 32px;
    border-bottom: 1px solid var(--line);
    background: var(--panel2);
    position: sticky;
    top: 0;
    z-index: 10;
  }
  /* Taille/graisse viennent de `.lbl-screen` (global, harmonisation §chantier
     libellés) — même traitement que le h2 de Transversal (« Add-ons
     voiture »). */
  h1 {
    flex: 1;
  }
  /* Le fil d'Ariane du titre : la sous-entrée, dans le gris du sous-titre —
     ce qui est en gras, c'est le type, pas la page. */
  .crumb {
    color: var(--muted);
    font-weight: 400;
  }
  .ok,
  .errbox,
  .banner {
    margin: 14px 32px 0;
  }
  .banner {
    padding: 10px 12px;
    font-size: 12px;
  }
  .ok {
    background: var(--green-dim);
    border: 1px solid var(--green-border);
    color: var(--green);
  }
  /* Le seuil de mise en page est une requête de **conteneur** et non de média :
     ce qui décide, c'est la largeur réellement reçue par le corps de l'écran —
     le rail de navigation et la colonne de session ont déjà pris la leur — et
     le zoom d'interface déplace la largeur de la fenêtre sans rien changer à
     celle-là. */
  .body {
    container: session / inline-size;
    padding: 22px 32px 40px;
  }

  /* Centré et plafonné, jamais collé à un bord.
     Le plafond est là pour qu'un très grand écran ne délaye pas l'écran sur
     deux mètres, pas pour le rétrécir : il est assez haut pour que la fenêtre
     habituelle le remplisse et n'en voie jamais la marge. Ce qui protège les
     curseurs d'une course souris absurde n'est pas lui mais la mise en colonnes
     des blocs eux-mêmes — trois curseurs côte à côte dans Simulation, une
     largeur fixe pour les deux fourchettes. */
  /* La page adversaires : un seul enchaînement vertical, pleine largeur, sans
     cadre entre le générateur et sa sortie (CIBLE§5.1). Le plafond est celui des
     deux colonnes, pour que le passage d'une page à l'autre ne déplace pas les
     bords de l'écran. */
  .oppopage {
    display: flex;
    flex-direction: column;
    gap: 14px;
    max-width: 1720px;
    margin-inline: auto;
  }
  /* Le bandeau de type + circuit, au-dessus du premier bloc de la colonne. */
  .banner-warn {
    margin-bottom: 14px;
  }
  .cols {
    display: grid;
    grid-template-columns: minmax(0, 1.35fr) minmax(0, 1fr);
    gap: 26px;
    max-width: 1720px;
    margin-inline: auto;
  }
  /* Sous ce seuil, la colonne de droite **passe dessous** plutôt que de se
     comprimer : en dessous d'environ 380 px elle ne sait plus afficher la bande
     jour/nuit ni les quatre valeurs de l'état de piste sur une ligne. La colonne
     unique se plafonne à son tour et reste centrée. */
  @container session (max-width: 980px) {
    .cols {
      grid-template-columns: minmax(0, 1fr);
      max-width: 880px;
    }
  }
</style>
