<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import {
    launchSession,
    isSteamRunning,
    listModSkins,
    getModCspFeatures,
    weatherOptions,
    weatherConditions,
    type GridMode,
    type Opponent,
    type RaceSetup,
    type Season,
    type SessionType,
    type SkinItem,
    type WeatherOption,
  } from "$lib/launch";
  import { listLibrary, previewSrc, type ModCard } from "$lib/library";
  import { getSessionBackground } from "$lib/media";
  import { nav, type OpponentsAction } from "$lib/nav.svelte";
  import { getPreferredSkin } from "$lib/preferred";
  import { t } from "$lib/i18n/index.svelte";
  import WeatherBlock from "./launch/WeatherBlock.svelte";
  import OpponentsBlock from "./launch/OpponentsBlock.svelte";
  import SessionOptionsBlock from "./launch/SessionOptionsBlock.svelte";
  import SimulationBlock from "./launch/SimulationBlock.svelte";
  import SessionTypeBlock from "./launch/SessionTypeBlock.svelte";
  import SavedSessionsBlock from "./launch/SavedSessionsBlock.svelte";
  import LoadingState from "./LoadingState.svelte";
  import { saveSession, listSavedSessions, type SavedSession } from "$lib/savedSessions";

  import { errorText } from "$lib/errors";
  import { StorageKey } from "$lib/storage";
  let libCards = $state<ModCard[]>([]);
  let weathers = $state<WeatherOption[]>([]);
  let selectedIntent = $state("");
  let gridMode = $state<GridMode>("same_category");
  let opponentCount = $state(7);
  // Jeton de génération du plateau (§6.3ter) : `regenerateGrid` est asynchrone
  // (résolution des skins par IPC) et peut encore être « en vol » quand
  // `applyOpponentsAction` prend la main — sans garde, son résultat arrive
  // après coup et écrase les adversaires qu'on vient d'imposer. Toute
  // régénération capture le jeton courant et n'applique son résultat que s'il
  // n'a pas été invalidé entre-temps par un appel plus récent.
  let opponentsGen = 0;
  // Voiture pour laquelle le plateau courant a été construit (§8.6ter).
  // Persistée avec lui : cet écran est démonté dès qu'on passe à la
  // bibliothèque, donc c'est le seul moyen, au remontage, de savoir si le
  // plateau restauré correspond encore à la voiture pilotée.
  let gridCarId = $state<string | null>(null);
  let launching = $state(false);
  let error = $state("");
  let info = $state("");
  let ready = $state(false);

  // --- Fourchette d'année du vivier d'adversaires (§8.6, remplace « même ère ») ---
  const YEAR_RANGE_MIN = 1950;
  const YEAR_RANGE_MAX = new Date().getFullYear();

  let setup = $state<RaceSetup>({
    car_id: "",
    car_skin: null,
    track_id: "",
    track_layout: null,
    session_type: "practice",
    opponents: [],
    ai_level_min: 92,
    ai_level_max: 98,
    laps: 5,
    weather: "",
    time_hours: 13,
    ambient_c: null,
    road_c: null,
    wind_speed_kmh: null,
    wind_direction_deg: null,
    year_min: YEAR_RANGE_MIN,
    year_max: YEAR_RANGE_MAX,
    season: null,
    season_date: null,
    penalties: false,
    jump_start_penalty: 0,
    grip: 96,
    practice_enabled: false,
    practice_minutes: 20,
    qualify_enabled: true,
    qualify_minutes: 10,
    ghost_car: false,
    start_from_pit: true,
    damage: 50,
    fuel_rate: 100,
    tyre_wear: 100,
    tyre_blankets: false,
    abs_auto: true,
    traction_control_auto: true,
    ideal_line: false,
  });

  // --- Saison optionnelle (§8.6bis) : associe une date au preset Quick
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
   * température recommandée (§8.6bis) — la changer remet des valeurs
   * cohérentes, comme un changement de météo. */
  function selectSeason(next: Season) {
    applySeason(next);
    void refreshConditions(true);
  }

  // --- Support CSP effectif du circuit courant (§6.4bis) : détecté à la
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
  const trackSupportsSeason = $derived(trackCspFeatures.includes("season"));
  const trackSupportsRain = $derived(trackCspFeatures.includes("rainfx"));

  const carPool = $derived(libCards.filter((c) => c.kind === "Car"));
  const player = $derived(carPool.find((c) => c.id_interne === setup.car_id) ?? null);
  const currentWeather = $derived(weathers.find((w) => w.id === selectedIntent));

  // --- Plateau d'adversaires (§8.6) : 3 modes de vivier, liste ajustable.
  // « Même voiture » = littéralement le même mod que le joueur (juste un
  // skin différent) ; « même catégorie »/« libre » filtrent aussi par
  // fourchette d'année (remplace « même ère »). ---
  function inYearRange(c: ModCard): boolean {
    // Année inconnue : ne pas exclure injustement un mod mal renseigné.
    if (c.year == null) return true;
    // 0 (ou champ vidé, qui retombe à 0 côté NumberStepper) = pas de borne
    // de ce côté — l'utilisateur tape juste le champ qui l'intéresse.
    if (setup.year_min > 0 && c.year < setup.year_min) return false;
    if (setup.year_max > 0 && c.year > setup.year_max) return false;
    return true;
  }
  function poolForMode(mode: GridMode): ModCard[] {
    if (!player) return carPool;
    if (mode === "same_car") return [player];
    const others = carPool.filter((c) => c.id_interne !== setup.car_id);
    const byCategory = mode === "same_category" && player.category ? others.filter((c) => c.category === player.category) : others;
    return byCategory.filter(inYearRange);
  }

  function randomLevel(): number {
    const { ai_level_min: min, ai_level_max: max } = setup;
    if (max <= min) return min;
    return Math.round(min + Math.random() * (max - min));
  }

  // --- Skins par voiture (cache, §8.6/§8.6bis) : chargés à la demande pour
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
  /** Pioche un skin pour `carId`, en évitant `used` (skins déjà pris pour ce
   * même mod dans le plateau courant) tant qu'il en reste de disponibles. */
  async function skinFor(carId: string, used: Set<string>): Promise<string | null> {
    const skins = await ensureSkins(carId);
    if (!skins.length) return null;
    const fresh = skins.filter((s) => !used.has(s.id));
    const from = fresh.length ? fresh : skins;
    const pick = from[Math.floor(Math.random() * from.length)];
    used.add(pick.id);
    return pick.id;
  }

  /** Génère `n` adversaires pour le mode courant. `excludeCarIds` = mods déjà
   * présents dans le plateau, évités en priorité (sauf en « même voiture »,
   * où il n'y a qu'un seul mod possible). Si le vivier distinct est épuisé
   * (ex. tous les modèles d'une catégorie déjà utilisés), on complète en
   * dupliquant un mod déjà choisi avec un skin différent plutôt que de
   * tronquer le plateau. */
  async function generateOpponents(n: number, excludeCarIds: Set<string>): Promise<Opponent[]> {
    if (n <= 0) return [];
    const pool = poolForMode(gridMode);
    const source = pool.length ? pool : carPool.filter((c) => c.id_interne !== setup.car_id);
    if (!source.length) return [];

    const fresh = source.filter((c) => !excludeCarIds.has(c.id_interne)).sort(() => Math.random() - 0.5);
    const picks: ModCard[] = fresh.slice(0, n);
    const dupSource = picks.length ? picks : source;
    let idx = 0;
    while (picks.length < n) {
      picks.push(dupSource[idx % dupSource.length]);
      idx++;
    }

    const usedByCar = new Map<string, Set<string>>();
    const out: Opponent[] = [];
    for (const c of picks) {
      const used = usedByCar.get(c.id_interne) ?? new Set<string>();
      usedByCar.set(c.id_interne, used);
      out.push({ car_id: c.id_interne, ai_level: randomLevel(), car_skin: await skinFor(c.id_interne, used) });
    }
    return out;
  }

  async function regenerateGrid() {
    const gen = ++opponentsGen;
    // Posé avant l'attente, pas après : c'est un marqueur d'intention, sinon
    // l'effet de resynchronisation redéclencherait une génération pendant
    // celle-ci.
    gridCarId = setup.car_id;
    const opponents = await generateOpponents(opponentCount, new Set());
    // Une action plus récente (nouvelle régénération, ou adversaires imposés
    // depuis la bibliothèque) a pris le dessus entre-temps : ne pas écraser.
    if (gen === opponentsGen) setup.opponents = opponents;
  }

  async function selectGridMode(mode: GridMode) {
    gridMode = mode;
    await regenerateGrid();
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
  function setOpponentLevel(index: number, raw: number) {
    const level = Math.max(RANGE_MIN, Math.min(RANGE_MAX, Math.round(raw) || RANGE_MIN));
    const opponents = [...setup.opponents];
    opponents[index] = { ...opponents[index], ai_level: level };
    setup.opponents = opponents;
  }

  async function addOpponent() {
    const exclude = new Set(setup.opponents.map((o) => o.car_id));
    const extra = await generateOpponents(1, exclude);
    if (extra.length) {
      setup.opponents = [...setup.opponents, ...extra];
      opponentCount = setup.opponents.length;
    }
  }

  /** Ajoute la même voiture qu'un adversaire existant, avec un skin différent
   * (pas encore pris par un autre adversaire de ce mod dans le plateau) —
   * rebouclé sur les skins déjà pris si tous sont épuisés (`skinFor`, même
   * logique que la génération initiale). Insérée juste après la ligne source. */
  async function duplicateOpponentWithVariant(index: number) {
    const source = setup.opponents[index];
    const used = new Set(
      setup.opponents.filter((o) => o.car_id === source.car_id).map((o) => o.car_skin ?? "").filter(Boolean),
    );
    const skin = await skinFor(source.car_id, used);
    const clone: Opponent = { car_id: source.car_id, car_skin: skin, ai_level: randomLevel() };
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
   * le redéclenche, et sans cet alignement il voit `car_id !== lastCarForGrid`
   * et lance une régénération inutile ; (2) `opponentsGen` incrémenté pour
   * invalider toute régénération DÉJÀ en vol (ex. si le type de session était
   * déjà "course" à l'arrivée sur cet écran, `onMount` en a lancé une). */
  function applyOpponentsAction(action: OpponentsAction) {
    opponentsGen++;
    gridCarId = setup.car_id;
    setup.session_type = "race";
    gridMode = "free";
    const additions: Opponent[] = action.carIds.map((carId) => ({
      car_id: carId,
      ai_level: randomLevel(),
      car_skin: getPreferredSkin(carId)?.id ?? null,
    }));
    setup.opponents = action.mode === "set" ? additions : [...setup.opponents, ...additions];
    opponentCount = setup.opponents.length;
  }

  // --- Popup de sélection d'adversaire (§8.6ter) : changer voiture (parmi le
  // vivier du mode courant) et skin, pour un réglage fin du plateau. ---
  let pickerIndex = $state<number | null>(null);
  const pickerPool = $derived(pickerIndex != null ? poolForMode(gridMode) : []);
  function openPicker(index: number) {
    pickerIndex = index;
  }
  function closePicker() {
    pickerIndex = null;
  }
  function confirmPicker(carId: string, skinId: string | null) {
    if (pickerIndex == null) return;
    const opponents = [...setup.opponents];
    opponents[pickerIndex] = { ...opponents[pickerIndex], car_id: carId, car_skin: skinId };
    setup.opponents = opponents;
    pickerIndex = null;
  }

  // --- Fourchette de niveau IA (§8.6) : bornes réutilisées par le réglage
  // individuel d'un adversaire (setOpponentLevel) — le curseur double lui-même
  // est rendu par OpponentsBlock. ---
  const RANGE_MIN = 60;
  const RANGE_MAX = 100;

  // --- Météo (intentions + température/vent, §8.5/§8.6) ---
  // Air, piste et vent sont des valeurs **recommandées** par météo+saison, mais
  // restent modifiables à la main (§8.6bis) : `tempsOverridden`/`windOverridden`
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
  // --- Mémorisation de la sélection + presets (§8.4/§8.6) ---
  // `opponents` en fait partie (§8.6ter, bug réel) : sans elle, revenir sur cet
  // écran après être allé choisir un circuit/une voiture démonte puis remonte
  // Launch.svelte — `setup.opponents` (état local) repart de zéro, et
  // `applyPreset` régénère alors un plateau aléatoire à la place de celui,
  // potentiellement construit à la main (mode « libre »), qu'avait l'utilisateur.
  //
  // Persisté côté Rust (`launch_state.json`, écriture synchrone), pas en
  // `localStorage` : même bug que le duo voiture/circuit (§8.6, voir
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
    /** Voiture pour laquelle le plateau restauré a été construit (§8.6ter).
     * Sans elle, revenir sur cet écran après avoir changé de voiture dans la
     * bibliothèque remonte le composant, qui ne peut plus distinguer « plateau
     * fait pour cette voiture » de « plateau hérité de la précédente ». */
    grid_car_id: string | null;
  }

  // --- Presets de session par type (§8.4) ---
  interface Persisted {
    ai_level_min: number; ai_level_max: number; grid_mode: GridMode; opponent_count: number;
    year_min: number; year_max: number;
    laps: number; time_hours: number;
    penalties: boolean; jump_start_penalty: number; grip: number;
    practice_enabled: boolean; practice_minutes: number;
    qualify_enabled: boolean; qualify_minutes: number; ghost_car: boolean; start_from_pit: boolean;
    damage: number; fuel_rate: number; tyre_wear: number; tyre_blankets: boolean; intent: string; season: Season;
    abs_auto: boolean; traction_control_auto: boolean; ideal_line: boolean;
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
      grid_car_id: gridCarId,
    };
    invoke("save_launch_state", { state: { selection, presets } }).catch((e) => console.error("save_launch_state", e));
  }
  $effect(() => {
    void [setup.car_id, setup.car_skin, setup.track_id, setup.track_layout, setup.session_type, setup.opponents];
    persistLaunchState();
  });

  function savePreset() {
    presets[setup.session_type] = {
      ai_level_min: setup.ai_level_min, ai_level_max: setup.ai_level_max,
      grid_mode: gridMode, opponent_count: opponentCount,
      year_min: setup.year_min, year_max: setup.year_max,
      laps: setup.laps, time_hours: setup.time_hours,
      penalties: setup.penalties, jump_start_penalty: setup.jump_start_penalty, grip: setup.grip,
      practice_enabled: setup.practice_enabled, practice_minutes: setup.practice_minutes,
      qualify_enabled: setup.qualify_enabled, qualify_minutes: setup.qualify_minutes, ghost_car: setup.ghost_car,
      start_from_pit: setup.start_from_pit,
      damage: setup.damage, fuel_rate: setup.fuel_rate, tyre_wear: setup.tyre_wear, tyre_blankets: setup.tyre_blankets,
      intent: selectedIntent, season,
      abs_auto: setup.abs_auto, traction_control_auto: setup.traction_control_auto, ideal_line: setup.ideal_line,
    };
    persistLaunchState();
  }
  async function applyPreset(type: SessionType) {
    const p = presets[type];
    applying = true;
    if (p) {
      setup.ai_level_min = p.ai_level_min ?? 92; setup.ai_level_max = p.ai_level_max ?? 98;
      gridMode = p.grid_mode ?? "same_category"; opponentCount = p.opponent_count ?? 7;
      setup.year_min = p.year_min ?? YEAR_RANGE_MIN; setup.year_max = p.year_max ?? YEAR_RANGE_MAX;
      setup.laps = p.laps; setup.time_hours = p.time_hours;
      setup.penalties = p.penalties; setup.jump_start_penalty = p.jump_start_penalty ?? 0;
      setup.grip = p.grip ?? 96;
      setup.practice_enabled = p.practice_enabled ?? false; setup.practice_minutes = p.practice_minutes ?? 20;
      setup.qualify_enabled = p.qualify_enabled ?? true; setup.qualify_minutes = p.qualify_minutes ?? 10;
      setup.ghost_car = p.ghost_car ?? false; setup.start_from_pit = p.start_from_pit ?? true;
      setup.damage = p.damage ?? 50;
      setup.fuel_rate = p.fuel_rate ?? 100; setup.tyre_wear = p.tyre_wear ?? 100;
      setup.tyre_blankets = p.tyre_blankets ?? false;
      setup.abs_auto = p.abs_auto ?? true; setup.traction_control_auto = p.traction_control_auto ?? true;
      setup.ideal_line = p.ideal_line ?? false;
      applySeason(p.season ?? "");
      const opt = weathers.find((w) => w.id === p.intent && w.available);
      if (opt) await selectIntent(opt);
    }
    // Ne régénère que s'il n'y a vraiment rien à préserver (première visite
    // de l'écran course, ou aucun adversaire restauré) — jamais en écrasant
    // silencieusement un plateau déjà construit (§8.6ter, bug réel).
    if (type === "race" && setup.opponents.length === 0) await regenerateGrid();
    applying = false;
  }
  async function setSessionType(type: SessionType) {
    if (type === setup.session_type) return;
    savePreset();
    setup.session_type = type;
    await applyPreset(type);
  }
  $effect(() => {
    void [setup.ai_level_min, setup.ai_level_max, gridMode, opponentCount, setup.year_min, setup.year_max,
      setup.laps,
      setup.time_hours, setup.penalties, setup.jump_start_penalty, setup.grip,
      setup.practice_enabled, setup.practice_minutes, setup.qualify_minutes,
      setup.ghost_car, setup.start_from_pit, setup.damage, setup.fuel_rate, setup.tyre_wear, setup.tyre_blankets,
      selectedIntent, season,
      setup.abs_auto, setup.traction_control_auto, setup.ideal_line];
    if (ready && !applying && selectedIntent) savePreset();
  });

  // --- Chargement + résolution des défauts (§8.6) ---
  onMount(async () => {
    [weathers, libCards] = await Promise.all([weatherOptions(), listLibrary()]);

    const state = await loadLaunchState();
    // Repli sur l'ancien `localStorage` seulement si le fichier Rust n'a rien
    // (première ouverture après la mise à jour) — voir `nav.svelte.ts` pour le
    // même schéma sur le duo voiture/circuit.
    const hasPersisted = state.selection !== null || state.presets !== null;
    presets = hasPersisted ? (state.presets ?? {}) : JSON.parse(localStorage.getItem(StorageKey.launchPresets) ?? "{}");
    const saved: Partial<Selection> = hasPersisted
      ? (state.selection ?? {})
      : JSON.parse(localStorage.getItem(StorageKey.launchSelection) ?? "{}");
    setup.session_type = saved.session_type ?? "practice";
    if (saved.opponents?.length) setup.opponents = saved.opponents;

    // La bibliothèque EST le sélecteur (§8.6) : voiture/circuit viennent du duo
    // de session choisi dans les bibliothèques — rien à choisir ici.
    syncFromSession();
    // Voiture du plateau restauré — **pas** celle qui vient d'être
    // synchronisée : c'est toute la différence entre « ce plateau est fait
    // pour cette voiture » et « ce plateau vient d'une autre voiture ».
    // S'aligner sur la voiture courante rendait le second cas indétectable,
    // et laissait le plateau de l'ancienne voiture après un changement fait
    // depuis la bibliothèque (bug réel : Shelby restées face à une autre
    // voiture). Fichier d'avant ce champ : on suppose le plateau à jour,
    // c'était le comportement précédent.
    gridCarId = saved.grid_car_id ?? setup.car_id;

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

  // Applique le duo de session (§8.6) au setup : voiture, skin piloté, circuit,
  // layout. Repli sur le 1er installé si aucune sélection.
  function syncFromSession() {
    const c = nav.sessionCar;
    const tr = nav.sessionTrack;
    setup.car_id = c?.id ?? carPool[0]?.id_interne ?? "";
    // Skin de session choisi sur la fiche (§8.6), repli sur mémorisé.
    setup.car_skin = c?.skin ?? (c ? getPreferredSkin(c.id)?.id ?? null : null);
    setup.track_id = tr?.id ?? "";
    setup.track_layout = tr?.layout ?? null;
  }

  // Resynchronise si le duo change (l'utilisateur ouvre une autre voiture/circuit
  // dans la bibliothèque puis revient à la session).
  //
  // Le plateau se régénère quand la **voiture pilotée** change, parce que le
  // vivier en dépend : « même voiture » n'a plus rien à voir, « même
  // catégorie » change de catégorie. Deux cas où on n'y touche pas :
  // - mode « libre », dont le vivier est indépendant de la voiture pilotée —
  //   régénérer jetterait un plateau souvent réglé à la main ;
  // - changement de **skin** seul : `setup.car_id` ne bouge pas, donc rien ne
  //   se déclenche (`nav.sessionCar?.skin` n'est lu que pour resynchroniser).
  $effect(() => {
    void [nav.sessionCar?.id, nav.sessionCar?.skin, nav.sessionTrack?.id, nav.sessionTrack?.layout];
    if (!ready) return;
    syncFromSession();
    if (setup.session_type === "race" && gridMode !== "free" && setup.car_id !== gridCarId) {
      void regenerateGrid();
    }
  });

  // Fond photo derrière l'interface (§6.2/§9.3) : combo exact → même circuit →
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
  // de la barre latérale (§8.6bis) : réactif plutôt que dans onMount, pour
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

  // --- Contrôle Steam (§9.2bis) ---
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
    error = ""; info = "";
    try {
      await launchSession($state.snapshot(setup));
      info = t("launch.launchSuccess");
    } catch (e) {
      error = errorText(e);
    } finally {
      launching = false;
    }
  }

  // --- Sessions sauvegardées nommées (§8.4bis) : instantané complet des
  // réglages (adversaires, météo, options…), rappelable par nom — distinct
  // des presets automatiques par type. Ne touche pas au duo voiture/circuit
  // courant (géré par la bibliothèque, §8.6) : seuls les réglages sont repris.
  // La liste (carte « Sessions enregistrées ») est filtrée par type — un
  // effet la recharge à chaque changement d'onglet, et le save/delete la
  // rafraîchissent en plus puisqu'ils ne changent pas le type. ---
  let saveDialogOpen = $state(false);
  let savedList = $state<SavedSession[]>([]);
  $effect(() => {
    const type = setup.session_type;
    // Le type peut changer avant que la réponse (invoke Rust) n'arrive :
    // n'applique le résultat que s'il correspond encore au type courant,
    // sinon une réponse tardive écraserait la liste avec le mauvais type.
    listSavedSessions(type).then((list) => {
      if (setup.session_type === type) savedList = list;
    });
  });

  async function doSaveSession(name: string) {
    await saveSession({
      name,
      savedAt: new Date().toISOString(),
      setup: $state.snapshot(setup),
      gridMode,
      opponentCount,
      season,
      intent: selectedIntent,
    });
    savedList = await listSavedSessions(setup.session_type);
    saveDialogOpen = false;
  }

  function doLoadSession(s: SavedSession) {
    // Conserve la voiture/le circuit courants (nav.sessionCar/Track fait déjà
    // foi) — seuls les réglages de la session chargée sont appliqués.
    const { car_id: _carId, car_skin: _carSkin, track_id: _trackId, track_layout: _trackLayout, ...settings } = s.setup;
    setup = { ...setup, ...settings };
    gridMode = s.gridMode;
    opponentCount = s.opponentCount;
    season = s.season;
    selectedIntent = s.intent;
  }
</script>

<div class="flow" class:has-bg={!!backgroundSrc} style:--session-bg={backgroundSrc ? `url('${backgroundSrc}')` : undefined}>
  <!-- Titre seul : pas de rappel du duo voiture/circuit ici (déjà dans la
       colonne latérale, §8.6). Le lancement se fait désormais depuis le
       bouton rouge « Démarrer la session » de la barre latérale, juste sous
       « Paramétrage de la session » — plus de bouton Lancer sur cet écran. -->
  <header class="bar">
    <h1 class="lbl-screen">{t("launch.pageTitle")}</h1>
  </header>

  {#if info}<div class="ok">{info}</div>{/if}
  {#if error}<div class="err">{error}</div>{/if}

  {#if !ready}
    <LoadingState />
  {:else}
  <div class="body">
    <div class="cols">
      <!-- COLONNE GAUCHE -->
      <div>
        <SessionTypeBlock sessionType={setup.session_type} onselect={setSessionType} />

        <SessionOptionsBlock {setup} />

        <SimulationBlock {setup} />

        {#if setup.session_type === "race"}
          <OpponentsBlock
            {setup}
            {gridMode}
            {opponentCount}
            {carPool}
            {skinsByCarId}
            yearRangeMax={YEAR_RANGE_MAX}
            {pickerPool}
            {pickerIndex}
            onselectmode={selectGridMode}
            oncountchange={applyOpponentCount}
            onremove={removeOpponent}
            onadd={addOpponent}
            onduplicate={duplicateOpponentWithVariant}
            onsetlevel={setOpponentLevel}
            onopenpicker={openPicker}
            onclosepicker={closePicker}
            onconfirmpicker={confirmPicker}
          />
        {/if}
      </div>

      <!-- COLONNE DROITE -->
      <div>
        <SavedSessionsBlock
          sessionType={setup.session_type}
          {savedList}
          dialogOpen={saveDialogOpen}
          onopendialog={() => (saveDialogOpen = true)}
          onclosedialog={() => (saveDialogOpen = false)}
          onsave={doSaveSession}
          onload={doLoadSession}
        />

        <WeatherBlock
          {setup}
          {weathers}
          {selectedIntent}
          {currentWeather}
          {trackSupportsSeason}
          {trackSupportsRain}
          {season}
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

<!-- Steam manquant (§9.2bis) : dialogue bloquant plutôt qu'un message dans le
     bandeau, parce qu'il y a un geste à faire hors de l'app et qu'il faut
     revérifier après — un texte passif laisserait l'utilisateur relancer dans
     le vide. -->
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
  /* Dialogue Steam — même langage visuel que `SavedSessionsDialog` ; le CSS
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
  /* Fond photo assombri et flouté (§6.2/§9.3) : appliqué seulement si un
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
  .ok,
  .err {
    margin: 14px 32px 0;
    padding: 10px 12px;
    font-size: 12px;
  }
  .ok {
    background: var(--green-dim);
    border: 1px solid var(--green-border);
    color: var(--green);
  }
  .err {
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
  }
  .body {
    padding: 22px 32px 40px;
  }

  .cols {
    display: grid;
    grid-template-columns: 1.35fr 1fr;
    gap: 26px;
  }
</style>
