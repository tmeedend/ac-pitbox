<script lang="ts">
  import { onMount } from "svelte";
  import {
    launchSession,
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
  import SavedSessionsDialog from "./SavedSessionsDialog.svelte";
  import NumberStepper from "./NumberStepper.svelte";
  import WeatherBlock from "./launch/WeatherBlock.svelte";
  import OpponentsBlock from "./launch/OpponentsBlock.svelte";
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
    duration_minutes: 15,
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
    qualifying: false,
    qualify_minutes: 10,
    ghost_car: false,
    start_from_pit: true,
    damage: 50,
    fuel_rate: 100,
    tyre_wear: 100,
    abs_auto: true,
    traction_control_auto: true,
    ideal_line: false,
  });

  const sessionTypes: { id: SessionType; labelKey: string }[] = [
    { id: "practice", labelKey: "launch.typePractice" },
    { id: "hotlap", labelKey: "launch.typeHotlap" },
    { id: "race", labelKey: "launch.typeRace" },
  ];

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
    lastCarForGrid = setup.car_id;
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
  // --- Mémorisation de la sélection (§8.6) ---
  // `opponents` en fait partie (§8.6ter, bug réel) : sans elle, revenir sur cet
  // écran après être allé choisir un circuit/une voiture démonte puis remonte
  // Launch.svelte — `setup.opponents` (état local) repart de zéro, et
  // `applyPreset` régénère alors un plateau aléatoire à la place de celui,
  // potentiellement construit à la main (mode « libre »), qu'avait l'utilisateur.
  interface Selection {
    car_id: string;
    car_skin: string | null;
    track_id: string;
    track_layout: string | null;
    session_type: SessionType;
    opponents: Opponent[];
  }
  function saveSelection() {
    if (!ready) return;
    const sel: Selection = {
      car_id: setup.car_id,
      car_skin: setup.car_skin,
      track_id: setup.track_id,
      track_layout: setup.track_layout,
      session_type: setup.session_type,
      opponents: setup.opponents,
    };
    localStorage.setItem(StorageKey.launchSelection, JSON.stringify(sel));
  }
  $effect(() => {
    void [setup.car_id, setup.car_skin, setup.track_id, setup.track_layout, setup.session_type, setup.opponents];
    saveSelection();
  });

  // --- Presets de session par type (§8.4) ---
  interface Persisted {
    ai_level_min: number; ai_level_max: number; grid_mode: GridMode; opponent_count: number;
    year_min: number; year_max: number;
    laps: number; duration_minutes: number; time_hours: number;
    penalties: boolean; jump_start_penalty: number; grip: number;
    practice_enabled: boolean; practice_minutes: number;
    qualifying: boolean; qualify_minutes: number; ghost_car: boolean; start_from_pit: boolean;
    damage: number; fuel_rate: number; tyre_wear: number; intent: string; season: Season;
    abs_auto: boolean; traction_control_auto: boolean; ideal_line: boolean;
  }
  let presets: Record<string, Persisted> = JSON.parse(localStorage.getItem(StorageKey.launchPresets) ?? "{}");
  let applying = false;

  function savePreset() {
    presets[setup.session_type] = {
      ai_level_min: setup.ai_level_min, ai_level_max: setup.ai_level_max,
      grid_mode: gridMode, opponent_count: opponentCount,
      year_min: setup.year_min, year_max: setup.year_max,
      laps: setup.laps, duration_minutes: setup.duration_minutes, time_hours: setup.time_hours,
      penalties: setup.penalties, jump_start_penalty: setup.jump_start_penalty, grip: setup.grip,
      practice_enabled: setup.practice_enabled, practice_minutes: setup.practice_minutes,
      qualifying: setup.qualifying, qualify_minutes: setup.qualify_minutes, ghost_car: setup.ghost_car,
      start_from_pit: setup.start_from_pit,
      damage: setup.damage, fuel_rate: setup.fuel_rate, tyre_wear: setup.tyre_wear, intent: selectedIntent, season,
      abs_auto: setup.abs_auto, traction_control_auto: setup.traction_control_auto, ideal_line: setup.ideal_line,
    };
    localStorage.setItem(StorageKey.launchPresets, JSON.stringify(presets));
  }
  async function applyPreset(type: SessionType) {
    const p = presets[type];
    applying = true;
    if (p) {
      setup.ai_level_min = p.ai_level_min ?? 92; setup.ai_level_max = p.ai_level_max ?? 98;
      gridMode = p.grid_mode ?? "same_category"; opponentCount = p.opponent_count ?? 7;
      setup.year_min = p.year_min ?? YEAR_RANGE_MIN; setup.year_max = p.year_max ?? YEAR_RANGE_MAX;
      setup.laps = p.laps; setup.duration_minutes = p.duration_minutes; setup.time_hours = p.time_hours;
      setup.penalties = p.penalties; setup.jump_start_penalty = p.jump_start_penalty ?? 0;
      setup.grip = p.grip ?? 96;
      setup.practice_enabled = p.practice_enabled ?? false; setup.practice_minutes = p.practice_minutes ?? 20;
      setup.qualifying = p.qualifying ?? false; setup.qualify_minutes = p.qualify_minutes ?? 10;
      setup.ghost_car = p.ghost_car ?? false; setup.start_from_pit = p.start_from_pit ?? true;
      setup.damage = p.damage ?? 50;
      setup.fuel_rate = p.fuel_rate ?? 100; setup.tyre_wear = p.tyre_wear ?? 100;
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
      setup.laps, setup.duration_minutes,
      setup.time_hours, setup.penalties, setup.jump_start_penalty, setup.grip,
      setup.practice_enabled, setup.practice_minutes, setup.qualifying, setup.qualify_minutes,
      setup.ghost_car, setup.start_from_pit, setup.damage, setup.fuel_rate, setup.tyre_wear, selectedIntent, season,
      setup.abs_auto, setup.traction_control_auto, setup.ideal_line];
    if (ready && !applying && selectedIntent) savePreset();
  });

  // --- Chargement + résolution des défauts (§8.6) ---
  onMount(async () => {
    [weathers, libCards] = await Promise.all([weatherOptions(), listLibrary()]);

    const saved: Partial<Selection> = JSON.parse(localStorage.getItem(StorageKey.launchSelection) ?? "{}");
    setup.session_type = saved.session_type ?? "practice";
    if (saved.opponents?.length) setup.opponents = saved.opponents;

    // La bibliothèque EST le sélecteur (§8.6) : voiture/circuit viennent du duo
    // de session choisi dans les bibliothèques — rien à choisir ici.
    syncFromSession();
    // Aligné tout de suite sur la voiture qui vient d'être synchronisée :
    // sans ça, l'effet de resynchronisation plus bas (déclenché par `ready`
    // qui passe à `true` en fin de montage) le voit encore vide, croit à un
    // changement de voiture, et régénère un plateau à la place de celui
    // qu'on vient de restaurer juste au-dessus.
    lastCarForGrid = setup.car_id;

    const first = weathers.find((w) => w.available);
    if (first) await selectIntent(first);
    await applyPreset(setup.session_type);
    if (setup.opponents.length) opponentCount = setup.opponents.length;
    ready = true;
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
  // dans la bibliothèque puis revient à la session) — régénère aussi le plateau
  // si la voiture change (le vivier dépend d'elle).
  let lastCarForGrid = $state("");
  $effect(() => {
    void [nav.sessionCar?.id, nav.sessionCar?.skin, nav.sessionTrack?.id, nav.sessionTrack?.layout];
    if (ready) {
      syncFromSession();
      if (setup.session_type === "race" && setup.car_id !== lastCarForGrid) {
        lastCarForGrid = setup.car_id;
        void regenerateGrid();
      }
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

  async function launch() {
    if (launching || !setup.car_id || !setup.track_id) return;
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
    savedList = listSavedSessions(setup.session_type);
  });

  function doSaveSession(name: string) {
    saveSession({
      name,
      savedAt: new Date().toISOString(),
      setup: $state.snapshot(setup),
      gridMode,
      opponentCount,
      season,
      intent: selectedIntent,
    });
    savedList = listSavedSessions(setup.session_type);
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

  function fmtSavedAt(iso: string): string {
    return iso.slice(0, 16).replace("T", " ");
  }
</script>

<div class="flow" class:has-bg={!!backgroundSrc} style:--session-bg={backgroundSrc ? `url('${backgroundSrc}')` : undefined}>
  <!-- Titre seul : pas de rappel du duo voiture/circuit ici (déjà dans la
       colonne latérale, §8.6). Le lancement se fait désormais depuis le
       bouton rouge « Démarrer la session » de la barre latérale, juste sous
       « Paramétrage de la session » — plus de bouton Lancer sur cet écran. -->
  <header class="bar">
    <h1>{t("launch.pageTitle")}</h1>
  </header>

  {#if saveDialogOpen}
    <SavedSessionsDialog
      sessionType={setup.session_type}
      onsave={doSaveSession}
      onclose={() => (saveDialogOpen = false)}
    />
  {/if}

  {#if info}<div class="ok">{info}</div>{/if}
  {#if error}<div class="err">{error}</div>{/if}

  <div class="body">
    <div class="cols">
      <!-- COLONNE GAUCHE -->
      <div>
        <section class="blk">
          <header class="blk-h"><span class="blk-t">{t("launch.sessionTypeLabel")}</span></header>
          <div class="blk-b">
            <div class="seg types">
              {#each sessionTypes as st}
                <button class:on={setup.session_type === st.id} onclick={() => setSessionType(st.id)}>{t(st.labelKey)}</button>
              {/each}
            </div>
          </div>
        </section>

        <!-- Options de session (§8.4/§8.6) : première carte de la colonne, la
             plus consultée — contenu dépendant du type choisi ci-dessus. -->
        <section class="blk">
          <header class="blk-h"><span class="blk-t">{t("launch.sessionOptionsLabel")}</span></header>
          <div class="blk-b">
            <!-- Tout sur une ligne : évolution du grip et pénalités sont
                 envoyées par le backend quel que soit le type de session
                 (Penalties dans les 3 ModeData, TrackPropertiesData au niveau
                 racine du preset, pas dans ModeData) — rien ne justifie de les
                 cantonner à Course. Faux départ / tours / essais / qualif
                 restent Course uniquement : absents des schémas Practice/Hotlap
                 (pas de grille, pas de phase weekend). -->
            <div class="opts-row">
              {#if setup.session_type === "practice"}
                <label class="grid-fields">
                  <NumberStepper width={90} min={1} max={240} bind:value={setup.duration_minutes} />
                  <span class="fk lbl-key">{t("launch.duration")}</span>
                </label>
              {:else if setup.session_type === "hotlap"}
                <label class="check"><input type="checkbox" bind:checked={setup.ghost_car} /><span>{t("launch.ghostCar")}</span></label>
              {:else}
                <label class="grid-fields">
                  <NumberStepper min={1} max={99} bind:value={setup.laps} />
                  <span class="fk lbl-key">{t("launch.laps")}</span>
                </label>
                <div><span class="fk lbl-key">{t("launch.jumpStart")}</span>
                  <div class="seg-v">
                    <button type="button" class:on={setup.jump_start_penalty === 0} onclick={() => (setup.jump_start_penalty = 0)}>{t("launch.jumpStartNone")}</button>
                    <button type="button" class:on={setup.jump_start_penalty === 1} onclick={() => (setup.jump_start_penalty = 1)}>{t("launch.jumpStartTeleport")}</button>
                    <button type="button" class:on={setup.jump_start_penalty === 2} onclick={() => (setup.jump_start_penalty = 2)}>{t("launch.jumpStartDrivethrough")}</button>
                  </div>
                </div>
              {/if}

              <div><span class="fk lbl-key">{t("launch.gripEvolution")}</span>
                <div class="seg-v">
                  <button type="button" class:on={setup.grip === 86} onclick={() => (setup.grip = 86)}>{t("launch.gripGreen")}</button>
                  <button type="button" class:on={setup.grip === 92} onclick={() => (setup.grip = 92)}>{t("launch.gripMedium")}</button>
                  <button type="button" class:on={setup.grip === 96} onclick={() => (setup.grip = 96)}>{t("launch.gripRubbered")}</button>
                  <button type="button" class:on={setup.grip === 100} onclick={() => (setup.grip = 100)}>{t("launch.gripOptimal")}</button>
                </div>
              </div>

              {#if setup.session_type === "race"}
                <label class="check"><input type="checkbox" bind:checked={setup.practice_enabled} /><span>{t("launch.freePractice")}</span></label>
                {#if setup.practice_enabled}
                  <label class="grid-fields">
                    <NumberStepper min={1} max={120} bind:value={setup.practice_minutes} />
                    <span class="fk lbl-key">{t("launch.practiceMinutes")}</span>
                  </label>
                {/if}
                <label class="check"><input type="checkbox" bind:checked={setup.qualifying} /><span>{t("launch.qualifying")}</span></label>
                {#if setup.qualifying}
                  <label class="grid-fields">
                    <NumberStepper min={1} max={60} bind:value={setup.qualify_minutes} />
                    <span class="fk lbl-key">{t("launch.qualifyMinutes")}</span>
                  </label>
                {/if}
              {/if}
              <label class="check"><input type="checkbox" bind:checked={setup.penalties} /><span>{t("launch.penalties")}</span></label>

              {#if setup.session_type === "practice"}
                <div><span class="fk lbl-key">{t("launch.startFrom")}</span>
                  <div class="seg-v">
                    <button type="button" class:on={setup.start_from_pit} onclick={() => (setup.start_from_pit = true)}>{t("launch.startFromPit")}</button>
                    <button type="button" class:on={!setup.start_from_pit} onclick={() => (setup.start_from_pit = false)}>{t("launch.startFromTrack")}</button>
                  </div>
                </div>
              {/if}
            </div>
          </div>
        </section>

        <!-- Simulation, aides à la conduite comprises : actif quel que soit
             le type de session (§8.6). -->
        <section class="blk">
          <header class="blk-h"><span class="blk-t">{t("launch.simulationLabel")}</span></header>
          <div class="blk-b">
          <div class="opt-row">
            <div class="opt">
              <div class="opt-head"><span class="opt-name lbl-key">{t("launch.damageLabel")}</span><span class="opt-val mono">{setup.damage}%</span></div>
              <input type="range" min="0" max="100" bind:value={setup.damage} style="--f:{setup.damage}%" />
            </div>
            <div class="opt">
              <div class="opt-head"><span class="opt-name lbl-key">{t("launch.fuelLabel")}</span><span class="opt-val mono">{setup.fuel_rate}%</span></div>
              <input type="range" min="0" max="200" bind:value={setup.fuel_rate} style="--f:{setup.fuel_rate / 2}%" />
            </div>
            <div class="opt">
              <div class="opt-head"><span class="opt-name lbl-key">{t("launch.tyreLabel")}</span><span class="opt-val mono">{setup.tyre_wear}%</span></div>
              <input type="range" min="0" max="200" bind:value={setup.tyre_wear} style="--f:{setup.tyre_wear / 2}%" />
            </div>
          </div>

          <div class="lbl section">{t("launch.assistsLabel")}</div>
          <div class="checks">
            <label class="check"><input type="checkbox" bind:checked={setup.abs_auto} /><span>{t("launch.absAuto")}</span></label>
            <label class="check"><input type="checkbox" bind:checked={setup.traction_control_auto} /><span>{t("launch.tractionAuto")}</span></label>
            <label class="check"><input type="checkbox" bind:checked={setup.ideal_line} /><span>{t("launch.idealLine")}</span></label>
          </div>
          </div>
        </section>

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
        <!-- Sessions enregistrées (§8.4bis) : liste du type courant, mise à
             jour par l'effet qui alimente `savedList` ; charge au clic,
             Sauvegarder ouvre la popup de nommage (avec écrasement d'une
             sauvegarde existante en option). -->
        <section class="blk">
          <header class="blk-h">
            <span class="blk-t">{t("launch.savedSessionsLabel")}</span>
            <span class="blk-n">{savedList.length}</span>
          </header>
          <div class="blk-b">
            <button class="btn saved-save-btn" type="button" onclick={() => (saveDialogOpen = true)}>{t("launch.saveSession")}</button>
            <div class="saved-list">
              {#if !savedList.length}
                <div class="saved-empty">{t("launch.noSavedSessions")}</div>
              {:else}
                {#each savedList as s (s.name)}
                  <button class="saved-item" type="button" onclick={() => doLoadSession(s)}>
                    <div class="saved-item-b">
                      <div class="saved-item-name">{s.name}</div>
                      <div class="saved-item-meta mono">{fmtSavedAt(s.savedAt)}</div>
                    </div>
                  </button>
                {/each}
              {/if}
            </div>
          </div>
        </section>

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
</div>

<style>
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
  /* Même traitement que le h2 de Transversal (« Add-ons voiture ») : un titre
     d'écran se lit, il ne se murmure pas — l'ancien gris 15px capitales était
     plus discret que les rubriques qu'il coiffe. */
  h1 {
    font-size: 18px;
    font-weight: 600;
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

  .seg,
  .types {
    display: flex;
    border: 1px solid var(--line);
    width: fit-content;
    margin-bottom: 16px;
  }
  .seg button {
    background: var(--panel2);
    color: var(--muted);
    padding: 9px 26px;
    font-size: 12px;
    letter-spacing: 1px;
    border-right: 1px solid var(--line);
  }
  .seg button:last-child {
    border-right: none;
  }
  .seg button.on {
    background: var(--rosso);
    color: #fff;
  }
  .cols {
    display: grid;
    grid-template-columns: 1.35fr 1fr;
    gap: 26px;
  }
  .grid-fields {
    display: inline-flex;
    align-items: center;
    gap: 12px;
  }
  /* Couleur/taille/interlettrage viennent de `.lbl-key` (global, harmonisation
     §chantier libellés) : ne reste ici que ce que `.lbl-key` ne couvre pas. */
  .fk {
    text-transform: uppercase;
  }

  /* Sessions enregistrées (§8.4bis) */
  .saved-save-btn {
    width: 100%;
    margin-bottom: 12px;
  }
  /* Hauteur plafonnée + défilement propre : la carte ne doit pas grandir
     sans limite si l'utilisateur accumule des sauvegardes. */
  .saved-list {
    max-height: 260px;
    overflow-y: auto;
    border: 1px solid var(--line);
  }
  .saved-empty {
    padding: 14px 10px;
    color: var(--muted);
    font-size: 11px;
    text-align: center;
  }
  .saved-item {
    display: flex;
    align-items: center;
    width: 100%;
    padding: 8px 10px;
    background: var(--panel2);
    border-bottom: 1px solid var(--line);
    text-align: left;
  }
  .saved-item:last-child {
    border-bottom: none;
  }
  .saved-item:hover {
    background: var(--raised);
  }
  .saved-item-b {
    flex: 1;
    min-width: 0;
  }
  .saved-item-name {
    font-size: 12px;
    color: var(--txt);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .saved-item-meta {
    font-size: 10px;
    color: var(--muted);
    margin-top: 2px;
  }

  /* Sliders simples (dégâts/carburant/pneus/heure) */
  /* Dégâts/carburant/pneus groupés sur une même ligne (2 si l'espace manque) :
     inutile de laisser chaque curseur s'étirer sur toute la largeur pour un
     réglage 0-100/200 qui se lit très bien en plus étroit. */
  .opt-row {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
    gap: 10px 16px;
  }
  .opt {
    margin-bottom: 14px;
  }
  .opt-row .opt {
    margin-bottom: 0;
  }
  .opt-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
  }
  /* Couleur/taille/interlettrage viennent de `.lbl-key` (global, harmonisation
     §chantier libellés) : ne reste ici que ce que `.lbl-key` ne couvre pas. */
  .opt-name {
    text-transform: uppercase;
  }
  .opt-val {
    margin-left: auto;
    font-size: 11px;
    color: var(--txt);
  }
  /* Curseurs simples (dégâts/carburant/pneus) : le thumb natif du
     navigateur est rond, incohérent avec le thème rectangulaire de l'app —
     resimulé en carré comme les curseurs doubles ci-dessus. */
  .opt input[type="range"] {
    width: 100%;
    height: 20px;
    margin: 0;
    appearance: none;
    background: transparent;
  }
  .opt input[type="range"]::-webkit-slider-runnable-track {
    height: 3px;
    background: linear-gradient(to right, var(--rosso) 0%, var(--rosso) var(--f, 0%), var(--line) var(--f, 0%), var(--line) 100%);
  }
  .opt input[type="range"]::-webkit-slider-thumb {
    appearance: none;
    width: 10px;
    height: 20px;
    border-radius: 2px;
    background: var(--rosso);
    border: 2px solid var(--panel);
    cursor: pointer;
    margin-top: -8.5px;
  }

  /* Options de session : tout sur une ligne (retombe à la ligne seulement si
     la largeur manque vraiment) — un groupe par réglage, chacun garde sa
     largeur naturelle plutôt que de s'étirer dans une grille. */
  .opts-row {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: 14px 16px;
  }
  .opts-row > div {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  /* Groupe de boutons rectangulaire (remplace les <select> natifs, dont la
     popup n'est pas pilotable à la manette) : chaque option est un bouton
     focusable, sélectionnable au clic comme au clic manette (bouton A). */
  .seg-v {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--line);
  }
  .seg-v button {
    background: var(--panel2);
    color: var(--txt2);
    text-align: left;
    padding: 7px 9px;
    font-size: 11px;
    border-bottom: 1px solid var(--line);
  }
  .seg-v button:last-child {
    border-bottom: none;
  }
  .seg-v button:hover {
    background: var(--raised);
  }
  .seg-v button.on {
    background: var(--rosso);
    color: #fff;
  }
  .checks {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 8px 10px;
    cursor: pointer;
    font-size: 10px;
    color: var(--txt2);
  }
</style>
