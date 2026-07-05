<script lang="ts">
  import { onMount } from "svelte";
  import {
    launchSession,
    listModSkins,
    weatherOptions,
    weatherConditions,
    type GridMode,
    type Opponent,
    type RaceSetup,
    type SessionType,
    type SkinItem,
    type WeatherOption,
  } from "$lib/launch";
  import { listLibrary, previewSrc, type ModCard } from "$lib/library";
  import { nav } from "$lib/nav.svelte";
  import { getPreferredSkin } from "$lib/preferred";
  import { t } from "$lib/i18n/index.svelte";
  import OpponentPicker from "./OpponentPicker.svelte";
  import SavedSessionsDialog from "./SavedSessionsDialog.svelte";
  import { saveSession, type SavedSession } from "$lib/savedSessions";

  let libCards = $state<ModCard[]>([]);
  let weathers = $state<WeatherOption[]>([]);
  let selectedIntent = $state("");
  let gridMode = $state<GridMode>("same_category");
  let opponentCount = $state(7);
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
    qualifying: false,
    qualify_minutes: 10,
    ghost_car: false,
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

  const gridModes: { id: GridMode; labelKey: string; subKey: string }[] = [
    { id: "same_car", labelKey: "launch.gridSameCar", subKey: "launch.gridSameCarSub" },
    { id: "same_category", labelKey: "launch.gridSameCategory", subKey: "launch.gridSameCategorySub" },
    { id: "free", labelKey: "launch.gridFree", subKey: "launch.gridFreeSub" },
  ];

  const WEATHER_IDS = ["clear", "few_clouds", "overcast", "fog", "light_rain", "rain", "storm", "snow"] as const;
  const WEATHER_LABEL_KEYS: Record<string, string> = {
    clear: "launch.wxClear",
    few_clouds: "launch.wxFewClouds",
    overcast: "launch.wxOvercast",
    fog: "launch.wxFog",
    light_rain: "launch.wxLightRain",
    rain: "launch.wxRain",
    storm: "launch.wxStorm",
    snow: "launch.wxSnow",
  };

  // --- Saison optionnelle (§8.6bis) : associe une date au race.ini,
  // best-effort côté CSP (voir RaceSetup.season_date côté back). ---
  type Season = "" | "spring" | "summer" | "autumn" | "winter";
  const SEASONS: { id: Season; labelKey: string }[] = [
    { id: "", labelKey: "launch.seasonNone" },
    { id: "spring", labelKey: "launch.seasonSpring" },
    { id: "summer", labelKey: "launch.seasonSummer" },
    { id: "autumn", labelKey: "launch.seasonAutumn" },
    { id: "winter", labelKey: "launch.seasonWinter" },
  ];
  // Mois/jour représentatifs (milieu de saison, hémisphère nord).
  const SEASON_MID: Record<Exclude<Season, "">, [number, number]> = {
    spring: [4, 15],
    summer: [7, 15],
    autumn: [10, 15],
    winter: [1, 15],
  };
  let season = $state<Season>("");
  function applySeason(next: Season) {
    season = next;
    if (!next) {
      setup.season = null;
      setup.season_date = null;
      return;
    }
    const [month, day] = SEASON_MID[next];
    const year = new Date().getFullYear();
    setup.season = next;
    setup.season_date = `${year}-${String(month).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
  }
  const sunRays = Array.from({ length: 8 }, (_, i) => {
    const a = (i * Math.PI) / 4;
    return {
      x1: 19 + Math.cos(a) * 11,
      y1: 19 + Math.sin(a) * 11,
      x2: 19 + Math.cos(a) * 14,
      y2: 19 + Math.sin(a) * 14,
    };
  });

  const carPool = $derived(libCards.filter((c) => c.kind === "Car"));
  const player = $derived(carPool.find((c) => c.id_interne === setup.car_id) ?? null);
  const currentWeather = $derived(weathers.find((w) => w.id === selectedIntent));

  // --- Plateau d'adversaires (§8.6) : 3 modes de vivier, liste ajustable.
  // « Même voiture » = littéralement le même mod que le joueur (juste un
  // skin différent) ; « même catégorie »/« libre » filtrent aussi par
  // fourchette d'année (remplace « même ère »). ---
  function inYearRange(c: ModCard): boolean {
    // Année inconnue : ne pas exclure injustement un mod mal renseigné.
    return c.year == null || (c.year >= setup.year_min && c.year <= setup.year_max);
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
    setup.opponents = await generateOpponents(opponentCount, new Set());
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

  async function addOpponent() {
    const exclude = new Set(setup.opponents.map((o) => o.car_id));
    const extra = await generateOpponents(1, exclude);
    if (extra.length) {
      setup.opponents = [...setup.opponents, ...extra];
      opponentCount = setup.opponents.length;
    }
  }

  function opponentName(carId: string): string {
    return carPool.find((c) => c.id_interne === carId)?.display_name ?? carId;
  }
  /** Vignette de l'adversaire : celle du skin choisi si connue, sinon la
   * preview générique du mod (deux adversaires « même voiture » doivent se
   * distinguer visuellement par leur skin, pas juste par leur nom). */
  function opponentPreview(opp: Opponent): string | null {
    const skin = opp.car_skin ? skinsByCarId[opp.car_id]?.find((s) => s.id === opp.car_skin) : null;
    if (skin?.preview) return previewSrc(skin.preview);
    return previewSrc(carPool.find((c) => c.id_interne === opp.car_id)?.preview ?? null);
  }
  function opponentSkinName(opp: Opponent): string | undefined {
    return opp.car_skin ? skinsByCarId[opp.car_id]?.find((s) => s.id === opp.car_skin)?.name : undefined;
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

  // --- Fourchette de niveau IA (deux curseurs, §8.6) ---
  const RANGE_MIN = 60;
  const RANGE_MAX = 100;
  function clampAiMin() {
    if (setup.ai_level_min > setup.ai_level_max) setup.ai_level_min = setup.ai_level_max;
  }
  function clampAiMax() {
    if (setup.ai_level_max < setup.ai_level_min) setup.ai_level_max = setup.ai_level_min;
  }

  // --- Fourchette d'année (§8.6, remplace « même ère ») ---
  function clampYearMin() {
    if (setup.year_min > setup.year_max) setup.year_min = setup.year_max;
  }
  function clampYearMax() {
    if (setup.year_max < setup.year_min) setup.year_max = setup.year_min;
  }
  const yearMinPct = $derived(((setup.year_min - YEAR_RANGE_MIN) / (YEAR_RANGE_MAX - YEAR_RANGE_MIN)) * 100);
  const yearMaxPct = $derived(((setup.year_max - YEAR_RANGE_MIN) / (YEAR_RANGE_MAX - YEAR_RANGE_MIN)) * 100);
  const aiMinPct = $derived(((setup.ai_level_min - RANGE_MIN) / (RANGE_MAX - RANGE_MIN)) * 100);
  const aiMaxPct = $derived(((setup.ai_level_max - RANGE_MIN) / (RANGE_MAX - RANGE_MIN)) * 100);

  // --- Météo (intentions + température/vent implicites, §8.5/§8.6) ---
  async function selectIntent(opt: WeatherOption) {
    if (!opt.available || !opt.weather) return;
    selectedIntent = opt.id;
    setup.weather = opt.weather;
    await refreshConditions();
  }
  async function refreshConditions() {
    if (!selectedIntent) return;
    const c = await weatherConditions(selectedIntent, setup.time_hours);
    setup.ambient_c = c.ambient;
    setup.road_c = c.road;
    setup.wind_speed_kmh = c.wind_speed_kmh;
    setup.wind_direction_deg = c.wind_direction_deg;
  }
  let lastHour = $state(-1);
  $effect(() => {
    if (setup.time_hours !== lastHour && selectedIntent) {
      lastHour = setup.time_hours;
      refreshConditions();
    }
  });
  function compassKey(deg: number): string {
    const keys = [
      "launch.compassN", "launch.compassNE", "launch.compassE", "launch.compassSE",
      "launch.compassS", "launch.compassSW", "launch.compassW", "launch.compassNW",
    ];
    return keys[Math.round(deg / 45) % 8];
  }

  // --- Mémorisation de la sélection (§8.6) ---
  interface Selection {
    car_id: string;
    car_skin: string | null;
    track_id: string;
    track_layout: string | null;
    session_type: SessionType;
  }
  function saveSelection() {
    if (!ready) return;
    const sel: Selection = {
      car_id: setup.car_id,
      car_skin: setup.car_skin,
      track_id: setup.track_id,
      track_layout: setup.track_layout,
      session_type: setup.session_type,
    };
    localStorage.setItem("pitbox.launchSel", JSON.stringify(sel));
  }
  $effect(() => {
    void [setup.car_id, setup.car_skin, setup.track_id, setup.track_layout, setup.session_type];
    saveSelection();
  });

  // --- Presets de session par type (§8.4) ---
  interface Persisted {
    ai_level_min: number; ai_level_max: number; grid_mode: GridMode; opponent_count: number;
    year_min: number; year_max: number;
    laps: number; duration_minutes: number; time_hours: number;
    penalties: boolean; jump_start_penalty: number; grip: number;
    qualifying: boolean; qualify_minutes: number; ghost_car: boolean;
    damage: number; fuel_rate: number; tyre_wear: number; intent: string; season: Season;
    abs_auto: boolean; traction_control_auto: boolean; ideal_line: boolean;
  }
  let presets: Record<string, Persisted> = JSON.parse(localStorage.getItem("pitbox.launchPresets") ?? "{}");
  let applying = false;

  function savePreset() {
    presets[setup.session_type] = {
      ai_level_min: setup.ai_level_min, ai_level_max: setup.ai_level_max,
      grid_mode: gridMode, opponent_count: opponentCount,
      year_min: setup.year_min, year_max: setup.year_max,
      laps: setup.laps, duration_minutes: setup.duration_minutes, time_hours: setup.time_hours,
      penalties: setup.penalties, jump_start_penalty: setup.jump_start_penalty, grip: setup.grip,
      qualifying: setup.qualifying, qualify_minutes: setup.qualify_minutes, ghost_car: setup.ghost_car,
      damage: setup.damage, fuel_rate: setup.fuel_rate, tyre_wear: setup.tyre_wear, intent: selectedIntent, season,
      abs_auto: setup.abs_auto, traction_control_auto: setup.traction_control_auto, ideal_line: setup.ideal_line,
    };
    localStorage.setItem("pitbox.launchPresets", JSON.stringify(presets));
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
      setup.grip = p.grip ?? 96; setup.qualifying = p.qualifying ?? false; setup.qualify_minutes = p.qualify_minutes ?? 10;
      setup.ghost_car = p.ghost_car ?? false; setup.damage = p.damage ?? 50;
      setup.fuel_rate = p.fuel_rate ?? 100; setup.tyre_wear = p.tyre_wear ?? 100;
      setup.abs_auto = p.abs_auto ?? true; setup.traction_control_auto = p.traction_control_auto ?? true;
      setup.ideal_line = p.ideal_line ?? false;
      applySeason(p.season ?? "");
      const opt = weathers.find((w) => w.id === p.intent && w.available);
      if (opt) await selectIntent(opt);
    }
    if (type === "race") await regenerateGrid();
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
      setup.time_hours, setup.penalties, setup.jump_start_penalty, setup.grip, setup.qualifying, setup.qualify_minutes,
      setup.ghost_car, setup.damage, setup.fuel_rate, setup.tyre_wear, selectedIntent, season,
      setup.abs_auto, setup.traction_control_auto, setup.ideal_line];
    if (ready && !applying && selectedIntent) savePreset();
  });

  // --- Chargement + résolution des défauts (§8.6) ---
  onMount(async () => {
    [weathers, libCards] = await Promise.all([weatherOptions(), listLibrary()]);

    const saved: Partial<Selection> = JSON.parse(localStorage.getItem("pitbox.launchSel") ?? "{}");
    setup.session_type = saved.session_type ?? "practice";

    // La bibliothèque EST le sélecteur (§8.6) : voiture/circuit viennent du duo
    // de session choisi dans les bibliothèques — rien à choisir ici.
    syncFromSession();

    const first = weathers.find((w) => w.available);
    if (first) await selectIntent(first);
    await applyPreset(setup.session_type);
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

  function fmtTime(h: number): string {
    const hh = Math.floor(h), mm = Math.round((h - hh) * 60);
    return `${String(hh).padStart(2, "0")}:${String(mm).padStart(2, "0")}`;
  }

  async function launch() {
    if (launching || !setup.car_id || !setup.track_id) return;
    savePreset();
    launching = true;
    error = ""; info = "";
    try {
      await launchSession($state.snapshot(setup));
      info = t("launch.launchSuccess");
    } catch (e) {
      error = String(e);
    } finally {
      launching = false;
    }
  }

  // --- Sessions sauvegardées nommées (§8.4bis) : instantané complet des
  // réglages (adversaires, météo, options…), rappelable par nom — distinct
  // des presets automatiques par type. Ne touche pas au duo voiture/circuit
  // courant (géré par la bibliothèque, §8.6) : seuls les réglages sont repris. ---
  let sessionDialog = $state<"save" | "load" | null>(null);

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
    sessionDialog = null;
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
    sessionDialog = null;
  }
</script>

{#snippet weatherIcon(id: string)}
  {#if id === "clear"}
    <circle cx="19" cy="19" r="8" fill="none" stroke="var(--yellow)" stroke-width="2" />
    {#each sunRays as r}
      <line x1={r.x1} y1={r.y1} x2={r.x2} y2={r.y2} stroke="var(--yellow)" stroke-width="1.6" stroke-linecap="round" />
    {/each}
  {:else if id === "few_clouds"}
    <circle cx="14" cy="14" r="6" fill="none" stroke="var(--yellow)" stroke-width="1.8" />
    <path d="M12 26 a5 5 0 0 1 0-10 a6 6 0 0 1 11 2 a4 4 0 0 1 1 8 z" fill="none" stroke="var(--txt2)" stroke-width="1.8" />
  {:else if id === "overcast"}
    <path d="M11 27 a6 6 0 0 1 0-12 a7 7 0 0 1 13 2 a5 5 0 0 1 1 10 z" fill="none" stroke="var(--muted)" stroke-width="1.8" />
  {:else if id === "fog"}
    <path d="M8 15 h22 M6 20 h26 M9 25 h20 M11 30 h16" fill="none" stroke="var(--muted)" stroke-width="1.8" stroke-linecap="round" />
  {:else if id === "light_rain"}
    <path d="M11 22 a6 6 0 0 1 0-12 a7 7 0 0 1 13 2 a5 5 0 0 1 1 10 z" fill="none" stroke="var(--muted)" stroke-width="1.8" />
    <path d="M14 27 l-1 4 M20 27 l-1 4" stroke="var(--blue)" stroke-width="1.8" stroke-linecap="round" />
  {:else if id === "rain"}
    <path d="M11 20 a6 6 0 0 1 0-12 a7 7 0 0 1 13 2 a5 5 0 0 1 1 10 z" fill="none" stroke="var(--muted)" stroke-width="1.8" />
    <path d="M12 25 l-1.5 6 M18 25 l-1.5 6 M24 25 l-1.5 6" stroke="var(--blue)" stroke-width="1.8" stroke-linecap="round" />
  {:else if id === "storm"}
    <path d="M11 19 a6 6 0 0 1 0-12 a7 7 0 0 1 13 2 a5 5 0 0 1 1 10 z" fill="none" stroke="var(--muted)" stroke-width="1.8" />
    <path d="M18 24 l-5 7 h4 l-2 6 6-8 h-4 z" fill="var(--yellow)" stroke="none" />
  {:else if id === "snow"}
    <path d="M11 18 a6 6 0 0 1 0-12 a7 7 0 0 1 13 2 a5 5 0 0 1 1 10 z" fill="none" stroke="var(--muted)" stroke-width="1.8" />
    <g stroke="var(--blue)" stroke-width="1.6" stroke-linecap="round">
      <line x1="13" y1="24" x2="13" y2="32" /><line x1="9.5" y1="26" x2="16.5" y2="30" /><line x1="16.5" y1="26" x2="9.5" y2="30" />
      <line x1="25" y1="24" x2="25" y2="32" /><line x1="21.5" y1="26" x2="28.5" y2="30" /><line x1="28.5" y1="26" x2="21.5" y2="30" />
    </g>
  {/if}
{/snippet}

{#snippet seasonIcon(id: string)}
  {#if id === ""}
    <circle cx="19" cy="19" r="9" fill="none" stroke="var(--muted2)" stroke-width="1.8" stroke-dasharray="3 3" />
    <line x1="14" y1="19" x2="24" y2="19" stroke="var(--muted2)" stroke-width="1.8" stroke-linecap="round" />
  {:else if id === "spring"}
    <circle cx="19" cy="11" r="4" fill="none" stroke="var(--green)" stroke-width="1.6" />
    <circle cx="27" cy="19" r="4" fill="none" stroke="var(--green)" stroke-width="1.6" />
    <circle cx="19" cy="27" r="4" fill="none" stroke="var(--green)" stroke-width="1.6" />
    <circle cx="11" cy="19" r="4" fill="none" stroke="var(--green)" stroke-width="1.6" />
    <circle cx="19" cy="19" r="3" fill="var(--yellow)" stroke="none" />
  {:else if id === "summer"}
    <circle cx="19" cy="19" r="10" fill="none" stroke="var(--yellow)" stroke-width="2.6" />
  {:else if id === "autumn"}
    <path d="M19 8 C 26 12, 28 20, 19 30 C 10 20, 12 12, 19 8 Z" fill="none" stroke="var(--yellow)" stroke-width="1.8" />
    <line x1="19" y1="11" x2="19" y2="27" stroke="var(--yellow)" stroke-width="1.2" />
  {:else if id === "winter"}
    <g stroke="var(--blue)" stroke-width="1.6" stroke-linecap="round">
      <line x1="19" y1="7" x2="19" y2="31" />
      <line x1="7" y1="19" x2="31" y2="19" />
      <line x1="10" y1="10" x2="28" y2="28" />
      <line x1="28" y1="10" x2="10" y2="28" />
    </g>
  {/if}
{/snippet}

<div class="flow">
  <!-- Titre seul : pas de rappel du duo voiture/circuit ici (déjà dans la
       colonne latérale, §8.6). Le lancement se fait désormais depuis le
       bouton rouge « Démarrer la session » de la barre latérale, juste sous
       « Paramétrage de la session » — plus de bouton Lancer sur cet écran. -->
  <header class="bar">
    <h1>{t("launch.pageTitle")}</h1>
    <div class="bar-actions">
      <button class="btn" type="button" onclick={() => (sessionDialog = "load")}>{t("launch.loadSession")}</button>
      <button class="btn" type="button" onclick={() => (sessionDialog = "save")}>{t("launch.saveSession")}</button>
    </div>
  </header>

  {#if sessionDialog}
    <SavedSessionsDialog
      mode={sessionDialog}
      onsave={doSaveSession}
      onload={doLoadSession}
      onclose={() => (sessionDialog = null)}
    />
  {/if}

  {#if info}<div class="ok">{info}</div>{/if}
  {#if error}<div class="err">{error}</div>{/if}

  <div class="body">
    <div class="seg types">
      {#each sessionTypes as st}
        <button class:on={setup.session_type === st.id} onclick={() => setSessionType(st.id)}>{t(st.labelKey)}</button>
      {/each}
    </div>

    {#if setup.session_type === "practice"}
      <label class="quickfield"><span>{t("launch.duration")}</span><input class="input" type="number" min="1" max="240" bind:value={setup.duration_minutes} /></label>
    {:else if setup.session_type === "hotlap"}
      <label class="check quickfield"><input type="checkbox" bind:checked={setup.ghost_car} /><span>{t("launch.ghostCar")}</span></label>
    {/if}

    <div class="cols">
      <!-- COLONNE GAUCHE -->
      <div>
        {#if setup.session_type === "race"}
          <!-- Adversaires (Course uniquement, §8.6) -->
          <section class="sect">
            <div class="lbl">{t("launch.opponentsLabel")}</div>
            <div class="modes">
              {#each gridModes as m}
                <button class="mode" class:on={gridMode === m.id} type="button" onclick={() => selectGridMode(m.id)}>
                  <div class="mt">{t(m.labelKey)}</div>
                  <div class="md mono">{t(m.subKey)}</div>
                </button>
              {/each}
            </div>

            {#if gridMode !== "same_car"}
              <!-- Fourchette d'année du vivier (remplace « même ère »), §8.6 -->
              <div class="dual-range year-range">
                <div class="dr-track"></div>
                <div class="dr-fill" style="left:{yearMinPct}%; right:{100 - yearMaxPct}%"></div>
                <input type="range" min={YEAR_RANGE_MIN} max={YEAR_RANGE_MAX} bind:value={setup.year_min} oninput={clampYearMin} />
                <input type="range" min={YEAR_RANGE_MIN} max={YEAR_RANGE_MAX} bind:value={setup.year_max} oninput={clampYearMax} />
              </div>
              <div class="dr-vals mono" style="margin-bottom:12px;">
                <span>{setup.year_min}</span>
                <span>{t("launch.yearRangeHint")}</span>
                <span>{setup.year_max}</span>
              </div>
            {/if}

            <label class="grid-fields">
              <input class="num" type="number" min="0" max="30" value={opponentCount} onchange={(e) => applyOpponentCount(Number(e.currentTarget.value))} />
              <span class="fk">{t("launch.aiCount")}</span>
            </label>
            <div class="oppo">
              <div class="oppo-h">{t("launch.gridGenerated", { count: setup.opponents.length })}</div>
              {#each setup.opponents as opp, i}
                {@const prev = opponentPreview(opp)}
                <div
                  class="oppo-row"
                  role="button"
                  tabindex="0"
                  title={t("launch.opponentEditTooltip")}
                  onclick={() => openPicker(i)}
                  onkeydown={(e) => (e.key === "Enter" || e.key === " ") && openPicker(i)}
                >
                  <div class="oppo-img">{#if prev}<img src={prev} alt="" />{:else}<span class="mono">🏎</span>{/if}</div>
                  <span class="oppo-n">{opponentName(opp.car_id)}{#if opponentSkinName(opp)}<span class="oppo-skin"> · {opponentSkinName(opp)}</span>{/if}</span>
                  <span class="oppo-force mono">{opp.ai_level}</span>
                  <button class="oppo-x" type="button" title={t("common.remove")} onclick={(e) => { e.stopPropagation(); removeOpponent(i); }}>✕</button>
                </div>
              {/each}
              <button class="oppo-add" type="button" onclick={addOpponent}>+ {t("launch.addOpponent")}</button>
            </div>
          </section>

          <!-- Fourchette de niveau IA (§8.6) -->
          <section class="sect">
            <div class="lbl">{t("launch.aiRangeLabel")}</div>
            <div class="dual-range">
              <div class="dr-track"></div>
              <div class="dr-fill" style="left:{aiMinPct}%; right:{100 - aiMaxPct}%"></div>
              <input type="range" min={RANGE_MIN} max={RANGE_MAX} bind:value={setup.ai_level_min} oninput={clampAiMin} />
              <input type="range" min={RANGE_MIN} max={RANGE_MAX} bind:value={setup.ai_level_max} oninput={clampAiMax} />
            </div>
            <div class="dr-vals mono">
              <span>{t("launch.aiMin", { level: setup.ai_level_min })}</span>
              <span>{t("launch.aiRangeHint")}</span>
              <span>{t("launch.aiMax", { level: setup.ai_level_max })}</span>
            </div>
          </section>
        {/if}

        <!-- Simulation : actif quel que soit le type de session (§8.6) -->
        <section class="sect">
          <div class="lbl">{t("launch.simulationLabel")} <span class="lbl-note">{t("launch.simulationNote")}</span></div>
          <div class="opt">
            <div class="opt-head"><span class="opt-name">{t("launch.damageLabel")}</span><span class="opt-val mono">{setup.damage}%</span></div>
            <input type="range" min="0" max="100" bind:value={setup.damage} />
          </div>
          <div class="opt">
            <div class="opt-head"><span class="opt-name">{t("launch.fuelLabel")}</span><span class="opt-val mono">{setup.fuel_rate}%</span></div>
            <input type="range" min="0" max="200" bind:value={setup.fuel_rate} />
          </div>
          <div class="opt">
            <div class="opt-head"><span class="opt-name">{t("launch.tyreLabel")}</span><span class="opt-val mono">{setup.tyre_wear}%</span></div>
            <input type="range" min="0" max="200" bind:value={setup.tyre_wear} />
          </div>
        </section>
      </div>

      <!-- COLONNE DROITE -->
      <div>
        <!-- Météo en icônes SVG (§8.6) -->
        <section class="sect">
          <div class="lbl">{t("launch.weather")}</div>
          <div class="weather">
            {#each WEATHER_IDS as id}
              {@const opt = weathers.find((w) => w.id === id)}
              <button
                class="wcard"
                class:on={selectedIntent === id}
                type="button"
                disabled={!opt?.available}
                title={opt?.reason ?? opt?.backend ?? ""}
                onclick={() => opt && selectIntent(opt)}
              >
                <svg viewBox="0 0 38 38">{@render weatherIcon(id)}</svg>
                <div class="wn">{t(WEATHER_LABEL_KEYS[id])}</div>
              </button>
            {/each}
          </div>
          {#if currentWeather}
            <div class="implicit">
              <div class="imp">
                <div><div class="ik">{t("launch.tempImplicit")}</div><div class="iv mono">{t("launch.tempReading", { air: setup.ambient_c ?? 0, road: setup.road_c ?? 0 })}</div></div>
              </div>
              <div class="imp">
                <div><div class="ik">{t("launch.windImplicit")}</div><div class="iv mono">{t("launch.windReading", { speed: setup.wind_speed_kmh ?? 0, dir: t(compassKey(setup.wind_direction_deg ?? 0)) })}</div></div>
              </div>
            </div>
            <p class="implicit-note">{t("launch.implicitNote")}</p>
          {/if}

          <div class="heure-wrap">
            <div class="opt-head"><span class="opt-name">{t("launch.timeLabelShort")}</span><span class="opt-val mono">{fmtTime(setup.time_hours)}</span></div>
            <input type="range" min="6" max="22" step="0.5" bind:value={setup.time_hours} />
          </div>

          <!-- Saison optionnelle (§8.6bis) : associe une date, best-effort côté
               CSP (couleur des arbres en automne, piste blanche en hiver). -->
          <div class="season-wrap">
            <div class="opt-name" style="margin-bottom:6px;">{t("launch.seasonLabel")}</div>
            <div class="weather season-grid">
              {#each SEASONS as s}
                <button class="wcard" class:on={season === s.id} type="button" onclick={() => applySeason(s.id)}>
                  <svg viewBox="0 0 38 38">{@render seasonIcon(s.id)}</svg>
                  <div class="wn">{t(s.labelKey)}</div>
                </button>
              {/each}
            </div>
          </div>
        </section>

        {#if setup.session_type === "race"}
          <div class="divider"></div>

          <!-- Options de course, toutes visibles (§8.6) -->
          <section class="sect">
            <div class="lbl">{t("launch.raceOptions")}</div>
            <label class="grid-fields" style="margin-bottom:14px;">
              <input class="num" type="number" min="1" max="99" bind:value={setup.laps} />
              <span class="fk">{t("launch.laps")}</span>
            </label>
            <div class="two-col">
              <label><span class="fk">{t("launch.jumpStart")}</span>
                <select class="input sel" bind:value={setup.jump_start_penalty}>
                  <option value={0}>{t("launch.jumpStartNone")}</option>
                  <option value={1}>{t("launch.jumpStartTeleport")}</option>
                  <option value={2}>{t("launch.jumpStartDrivethrough")}</option>
                </select>
              </label>
              <label><span class="fk">{t("launch.gripEvolution")}</span>
                <select class="input sel" bind:value={setup.grip}>
                  <option value={86}>{t("launch.gripGreen")}</option>
                  <option value={92}>{t("launch.gripMedium")}</option>
                  <option value={96}>{t("launch.gripRubbered")}</option>
                  <option value={100}>{t("launch.gripOptimal")}</option>
                </select>
              </label>
            </div>
            <div class="checks">
              <label class="check"><input type="checkbox" bind:checked={setup.qualifying} /><span>{t("launch.qualifying")}</span></label>
              <label class="check"><input type="checkbox" bind:checked={setup.penalties} /><span>{t("launch.penalties")}</span></label>
            </div>
            {#if setup.qualifying}
              <label class="grid-fields" style="margin-top:10px;">
                <input class="num" type="number" min="1" max="60" bind:value={setup.qualify_minutes} />
                <span class="fk">{t("launch.qualifyMinutes")}</span>
              </label>
            {/if}

            <div class="lbl" style="margin-top:18px;">{t("launch.assistsLabel")}</div>
            <div class="checks">
              <label class="check"><input type="checkbox" bind:checked={setup.abs_auto} /><span>{t("launch.absAuto")}</span></label>
              <label class="check"><input type="checkbox" bind:checked={setup.traction_control_auto} /><span>{t("launch.tractionAuto")}</span></label>
              <label class="check"><input type="checkbox" bind:checked={setup.ideal_line} /><span>{t("launch.idealLine")}</span></label>
            </div>
          </section>
        {/if}
      </div>
    </div>
  </div>
</div>

{#if pickerIndex != null}
  <OpponentPicker
    pool={pickerPool}
    currentCarId={setup.opponents[pickerIndex].car_id}
    currentSkinId={setup.opponents[pickerIndex].car_skin}
    onpick={confirmPicker}
    onclose={closePicker}
  />
{/if}

<style>
  /* Écran plein-page (AppShell rend `.content.fixed` pour "race", comme la
     bibliothèque) : .flow gère lui-même son défilement — plus de hack de
     marge négative pour compenser le padding du parent. */
  .flow {
    height: 100%;
    overflow-y: auto;
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
  h1 {
    font-size: 15px;
    font-weight: 600;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    color: var(--muted);
    flex: 1;
  }
  .bar-actions {
    display: flex;
    gap: 8px;
    flex: none;
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
  .quickfield {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--muted);
    margin-bottom: 20px;
  }
  .quickfield input[type="number"] {
    width: 90px;
  }
  .quickfield.check {
    text-transform: none;
    font-size: 12.5px;
    color: var(--txt2);
  }

  .cols {
    display: grid;
    grid-template-columns: 1.35fr 1fr;
    gap: 26px;
  }
  .sect {
    margin-bottom: 22px;
  }
  .lbl {
    color: var(--faint);
    font-size: 9px;
    letter-spacing: 1.5px;
    text-transform: uppercase;
    margin-bottom: 10px;
  }
  .lbl-note {
    text-transform: none;
    letter-spacing: 0;
    color: var(--muted2);
    margin-left: 4px;
  }
  .divider {
    height: 1px;
    background: var(--line);
    margin: 4px 0 20px;
  }

  /* Adversaires */
  .modes {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 1px;
    background: var(--line);
    border: 1px solid var(--line);
    margin-bottom: 12px;
  }
  .year-range {
    margin-bottom: 4px;
  }
  .mode {
    background: var(--panel2);
    padding: 10px 6px;
    text-align: center;
  }
  .mode:hover {
    background: var(--raised);
  }
  .mode.on {
    background: var(--rosso-dim);
    box-shadow: inset 0 -2px 0 var(--rosso);
  }
  .mode .mt {
    font-size: 9.5px;
    color: var(--txt2);
  }
  .mode.on .mt {
    color: var(--rosso-bright);
  }
  .mode .md {
    font-size: 7.5px;
    color: var(--faint);
    margin-top: 2px;
  }
  .grid-fields {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .num {
    width: 70px;
    height: 32px;
    background: var(--bg);
    border: 1px solid var(--line);
    color: var(--txt);
    text-align: center;
    font-size: 13px;
    font-family: var(--mono);
  }
  .fk {
    color: var(--faint);
    font-size: 9px;
    letter-spacing: 1px;
    text-transform: uppercase;
  }
  .oppo {
    border: 1px solid var(--line);
    margin-top: 12px;
  }
  .oppo-h {
    background: var(--raised);
    padding: 6px 10px;
    color: var(--muted);
    font-size: 8px;
    letter-spacing: 1.5px;
    text-transform: uppercase;
  }
  .oppo-row {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 6px 10px;
    border-top: 1px solid var(--line);
    background: var(--panel2);
    cursor: pointer;
  }
  .oppo-row:hover {
    background: var(--raised);
  }
  .oppo-img {
    width: 34px;
    height: 22px;
    border: 1px solid var(--line);
    background: var(--bg);
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
    overflow: hidden;
  }
  .oppo-img img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .oppo-n {
    font-size: 10.5px;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .oppo-skin {
    color: var(--faint);
  }
  .oppo-force {
    font-size: 9px;
    color: var(--green);
  }
  .oppo-x {
    background: transparent;
    color: var(--muted2);
    font-size: 12px;
    padding: 2px 4px;
  }
  .oppo-x:hover {
    background: transparent;
    color: var(--rosso-bright);
  }
  .oppo-add {
    background: var(--panel2);
    padding: 7px 10px;
    border-top: 1px solid var(--line);
    color: var(--rosso-bright);
    font-size: 9.5px;
    text-align: left;
    width: 100%;
  }
  .oppo-add:hover {
    background: var(--rosso-dim);
  }

  /* Fourchette IA (deux curseurs) */
  .dual-range {
    position: relative;
    height: 28px;
  }
  .dr-track {
    position: absolute;
    left: 0;
    right: 0;
    top: 50%;
    height: 3px;
    background: var(--line);
    transform: translateY(-50%);
  }
  .dr-fill {
    position: absolute;
    top: 50%;
    height: 3px;
    background: var(--rosso);
    transform: translateY(-50%);
  }
  .dual-range input[type="range"] {
    position: absolute;
    left: 0;
    top: 0;
    width: 100%;
    height: 28px;
    margin: 0;
    appearance: none;
    background: transparent;
    pointer-events: none;
  }
  .dual-range input[type="range"]::-webkit-slider-runnable-track {
    background: transparent;
  }
  .dual-range input[type="range"]::-webkit-slider-thumb {
    appearance: none;
    pointer-events: auto;
    width: 15px;
    height: 15px;
    border-radius: 50%;
    background: var(--rosso);
    border: 2px solid var(--panel);
    cursor: pointer;
    margin-top: 6.5px;
  }
  .dr-vals {
    display: flex;
    justify-content: space-between;
    font-size: 9.5px;
    color: var(--txt2);
    margin-top: 4px;
  }
  .dr-vals span:nth-child(2) {
    color: var(--faint);
  }

  /* Sliders simples (dégâts/carburant/pneus/heure) */
  .opt {
    margin-bottom: 14px;
  }
  .opt-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
  }
  .opt-name {
    font-size: 10px;
    color: var(--txt2);
    letter-spacing: 0.5px;
    text-transform: uppercase;
  }
  .opt-val {
    margin-left: auto;
    font-size: 11px;
    color: var(--txt);
  }
  .opt input[type="range"],
  .heure-wrap input[type="range"] {
    width: 100%;
  }
  .heure-wrap {
    margin-top: 16px;
  }
  .season-wrap {
    margin-top: 16px;
  }
  /* Mêmes cartes que la météo (.wcard/.wn ci-dessous), juste une colonne de
     plus pour garder les 5 options (Aucune + 4 saisons) sur une seule ligne.
     Sélecteur composé pour primer sur .weather { grid-template-columns } quel
     que soit l'ordre des règles dans la feuille de style. */
  .weather.season-grid {
    grid-template-columns: repeat(5, 1fr);
  }

  /* Météo */
  .weather {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
  }
  .wcard {
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 10px 4px;
    text-align: center;
  }
  .wcard:hover {
    border-color: var(--muted2);
  }
  .wcard.on {
    border-color: var(--rosso);
    background: var(--rosso-dim);
  }
  .wcard:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }
  .wcard svg {
    width: 34px;
    height: 34px;
  }
  .wn {
    font-size: 8.5px;
    margin-top: 5px;
    color: var(--txt2);
  }
  .wcard.on .wn {
    color: var(--rosso-bright);
  }
  .implicit {
    display: flex;
    gap: 16px;
    margin-top: 12px;
    padding: 9px 12px;
    border: 1px solid var(--line);
    background: var(--panel2);
  }
  .imp .ik {
    color: var(--faint);
    font-size: 7.5px;
    letter-spacing: 1px;
    text-transform: uppercase;
  }
  .imp .iv {
    font-size: 11px;
    color: var(--green);
  }
  .implicit-note {
    color: var(--faint);
    font-size: 8px;
    margin-top: 6px;
  }

  /* Options de course */
  .two-col {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    margin-bottom: 12px;
  }
  .two-col label {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .sel {
    background: var(--bg);
    border: 1px solid var(--line);
    color: var(--txt2);
    height: 32px;
    padding: 0 8px;
    font-family: var(--mono);
    font-size: 9.5px;
    width: 100%;
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
