<script lang="ts">
  import { onMount, untrack } from "svelte";
  import {
    launchSession,
    isSteamRunning,
    getModCspFeatures,
    weatherOptions,
    weatherConditions,
    trackSun,
    trackStates,
    nationalities,
    type RaceSetup,
    type Season,
    type SessionType,
    type Nationality,
    type TrackStateOption,
    type TrackSun,
    type WeatherOption,
  } from "$lib/launch/launch";
  import { carClassOf, driverFor, isEmpty } from "$lib/driver/driverOverride.svelte";
  import { setGridCars } from "$lib/launch/gridMods.svelte";
  import { playerHandicap, setPlayerHandicap } from "$lib/launch/playerHandicap.svelte";
  import { getModDetail, listLibrary, previewSrc, type ModCard } from "$lib/library/library";
  import { getSessionBackground } from "$lib/detail/media";
  import { nav, pickSession, type OpponentsAction } from "$lib/shell/nav.svelte";
  import { hasOpponents, openSetupPage, sessionNav, sessionNavReady } from "$lib/shell/sessionNav.svelte";
  import { carAssists, setCarAssists } from "$lib/launch/carAssists.svelte";
  import { getPreferredSkin } from "$lib/preferred";
  import { listActiveTrackSkins } from "$lib/inventory/submods";
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
    type SessionPreset,
  } from "$lib/launch/savedSessions";
  import { deleteSavedGrid, listSavedGrids, saveGrid, type SavedGrid } from "$lib/launch/savedGrids";
  import { OpponentGrid } from "$lib/launch/opponentGrid.svelte";
  import { restoreOpponent } from "$lib/launch/gridRules";
  import { presetFromSetup, readPreset } from "$lib/launch/typePresets";
  import { loadLaunchState, saveLaunchState, selectionOf, type TypePresets } from "$lib/launch/launchState.svelte";
  import { restoreCar, restoreTrack } from "$lib/launch/sessionRestore";

  import { errorText } from "$lib/errors";
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

  // --- The opponents grid (CIBLE§3.3) -----------------------------------------
  //
  // Its pool, its liveries and every gesture on its rows live in `OpponentGrid`;
  // what stays here is what belongs to the screen: the dialogs and the
  // messages they leave in the banner.
  const grid = new OpponentGrid({
    get setup() {
      return setup;
    },
    get carPool() {
      return carPool;
    },
  });

  // --- Grilles enregistrées (§5) : two buttons and a dialog, as for sessions.
  let gridDialog = $state<"save" | "load" | null>(null);
  let savedGrids = $state<SavedGrid[]>([]);

  async function openGridDialog(mode: "save" | "load") {
    savedGrids = await listSavedGrids();
    gridDialog = mode;
  }

  async function doSaveGrid(name: string) {
    gridDialog = null;
    try {
      await saveGrid(grid.toSaved(name));
    } catch (e) {
      error = errorText(e);
    }
  }

  /** Une voiture disparue de la bibliothèque depuis l'enregistrement est
   * retirée en le disant, jamais en échouant. */
  async function doLoadGrid(name: string) {
    const saved = savedGrids.find((g) => g.name === name);
    gridDialog = null;
    if (!saved) return;
    const { kept, missing } = grid.loadSaved(saved);
    warning = missing ? t("launch.gridMissingCars", { count: missing }) : "";
    info = t("launch.gridLoaded", { name: saved.name, count: kept });
  }

  async function removeSavedGrid(name: string) {
    await deleteSavedGrid(name);
    savedGrids = await listSavedGrids();
  }

  /** Adversaires envoyés depuis la sélection groupée de la bibliothèque
   * voitures (§6.3ter). Bascule sur le type Course directement : les
   * adversaires imposés ne doivent pas être écrasés par un plateau tiré pour
   * l'occasion — `impose` invalide toute génération déjà en vol. */
  function applyOpponentsAction(action: OpponentsAction) {
    setup.session_type = "race";
    grid.impose(action);
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

  async function replaceOpponent(carId: string) {
    const i = pickerIndex;
    closePicker();
    if (i == null) return;
    await grid.replace(i, carId);
  }

  async function addOpponentsFromPicker(carIds: string[]) {
    closePicker();
    await grid.add(carIds);
  }

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
  // --- Mémorisation de la sélection + presets par type (SESSION§3) ---
  // The file and its format live in `launchState.ts` and `typePresets.ts`;
  // the screen only says when to write and applies what it reads back.
  let presets: TypePresets = {};
  let applying = false;

  function persistLaunchState() {
    if (!ready) return;
    saveLaunchState(selectionOf(setup), presets);
  }
  $effect(() => {
    void [setup.car_id, setup.car_skin, setup.track_id, setup.track_layout, setup.session_type, setup.opponents,
      setup.player_ballast, setup.player_restrictor];
    persistLaunchState();
  });

  function savePreset() {
    presets[setup.session_type] = presetFromSetup(
      setup,
      { count: grid.count, pool: grid.serializedPool(), pinned: grid.pinned },
      selectedIntent,
      season,
    );
    persistLaunchState();
  }
  async function applyPreset(type: SessionType) {
    const p = presets[type];
    applying = true;
    if (p) {
      const v = readPreset(p);
      Object.assign(setup, v.setup);
      grid.count = v.opponentCount;
      grid.restorePool(p, player);
      // Par le store, qui est la valeur vivante : les écrire dans `setup` seul
      // laisserait la carte voiture du panneau gauche afficher les anciennes.
      setCarAssists(v.abs, v.tractionControl);
      applySeason(v.season);
      const opt = weathers.find((w) => w.id === v.intent && w.available);
      if (opt) await selectIntent(opt);
    }
    // Ne remplit que s'il n'y a vraiment rien à préserver (première visite
    // de l'écran course/trackday, ou aucun adversaire restauré) — jamais en
    // écrasant silencieusement un plateau déjà construit (SESSION§3.3, bug réel).
    //
    // `fill` et non `regenerate` : celle-ci **garde les voitures** et
    // ne retire au sort que ce qui est posé dessus, donc sur un plateau vide
    // elle ne faisait rien du tout. Une course ouverte pour la première fois
    // restait sans adversaire, alors qu'on doit pouvoir la lancer sans être
    // allé sur la page adversaires (L5§1.5).
    if (hasOpponents(type) && setup.opponents.length === 0) await grid.fill();
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
      hasOpponents(setup.session_type) && (grid.pool.length < grid.count || grid.duplicateDrivers);
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
      grid.count, grid.filters, grid.query, grid.pinned,
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
    presets = state.presets;
    const saved = state.selection;
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
    if (setup.opponents.length) grid.count = setup.opponents.length;
    ready = true;
    // Migration depuis `localStorage`, ou simplement première écriture :
    // s'assure que `launch_state.json` reflète l'état actuel sans attendre un
    // changement de réglage par l'utilisateur.
    if (!state.fromFile) persistLaunchState();
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
  let savedList = $state<SessionPreset[]>([]);
  $effect(() => {
    const type = setup.session_type;
    // Ouvrir la modale relit le dossier de presets : le scénario réel est
    // d'aller composer un preset dans Content Manager puis de revenir, et il
    // doit apparaître sans redémarrer l'app. Lu en tête, avant toute sortie —
    // une dépendance lue plus bas ne serait jamais enregistrée.
    void sessionDialog;
    // Le type ne filtre plus, il TRIE (SETUP§2.11) : la liste les porte toutes, et
    // celles du type courant viennent en tête. Le type peut changer avant que
    // la réponse (invoke Rust) n'arrive — n'applique le résultat que s'il
    // correspond encore, sinon une réponse tardive rendrait un tri périmé.
    listSavedSessions(type).then((list) => {
      if (setup.session_type === type) savedList = list;
    });
  });

  /** Le décompte du bouton ne compte que ce qui se charge : un preset dont le
   * mode n'a pas d'équivalent Pit Box est listé pour qu'on sache qu'il existe,
   * mais l'annoncer dans « Charger (12) » promettrait douze sessions. */
  const loadableCount = $derived(savedList.filter((e) => e.session).length);

  /** Ce qui distingue deux entrées d'un coup d'œil : son type — devenu une
   * propriété affichée depuis qu'il ne filtre plus —, son circuit, sa date.
   * Un preset qu'on n'a pas su convertir affiche sa raison à la place : la
   * ligne reste, et elle dit pourquoi elle ne se charge pas (SESSION§3.6). */
  function savedMeta(e: SessionPreset): string {
    if (!e.session) return errorText(e.reason ?? "");
    const s = e.session;
    const track = libCards.find((c) => c.id_interne === s.setup.track_id)?.display_name ?? s.setup.track_id;
    return [t(`launch.type.${s.setup.session_type}`), track, formatSavedAt(s.savedAt)].filter(Boolean).join(" · ");
  }

  async function removeSavedSession(path: string) {
    // Par chemin, et seulement pour les nôtres : un preset composé dans
    // Content Manager est listé ici, jamais supprimé d'ici (SESSION§3.6). Le
    // backend refuse de toute façon, la croix ne s'affiche simplement pas.
    try {
      await deleteSavedSession(path);
    } catch (e) {
      error = errorText(e);
    }
    savedList = await listSavedSessions(setup.session_type);
  }

  async function doSaveSession(name: string) {
    // Skins de circuit actifs : état de déploiement, pas un champ de `setup` —
    // capturé ici pour pouvoir remettre le circuit dans l'apparence qu'il
    // avait quand la session a été enregistrée. Un échec de lecture ne doit
    // pas empêcher la sauvegarde du reste : liste vide plutôt que rien.
    const trackSkins = setup.track_id ? await listActiveTrackSkins(setup.track_id).catch(() => []) : [];
    // L'échec d'une **écriture** ne s'avale pas (règle d'or n°6) : un dossier
    // de presets en lecture seule doit se voir, pas laisser croire que la
    // session est enregistrée. La modale reste ouverte pour qu'on puisse
    // réessayer sous un autre nom.
    error = "";
    try {
      await saveSession({
        name,
        savedAt: new Date().toISOString(),
        setup: $state.snapshot(setup),
        opponentCount: grid.count,
        gridFilters: grid.serializedPool(),
        gridPinned: [...grid.pinned],
        season,
        intent: selectedIntent,
        trackSkins,
      });
      sessionDialog = null;
    } catch (e) {
      error = errorText(e);
    }
    savedList = await listSavedSessions(setup.session_type);
  }

  /** Charge une session enregistrée (SESSION§3.5) : réglages **et** duo de session
   * (voiture + skin piloté, circuit + tracé + skins de circuit).
   *
   * Rien n'est bloquant ici : un mod supprimé depuis la sauvegarde laisse la
   * sélection courante en place et se signale dans le bandeau d'avertissement,
   * plutôt que d'interrompre le chargement du reste. Une session enregistrée
   * survit à des années de bibliothèque remaniée — l'échec partiel est le cas
   * normal, pas l'exception. */
  async function doLoadSession(s: SavedSession, notes: string[] = []) {
    error = ""; info = ""; warning = "";
    // Ce qu'un preset Content Manager n'a pas pu porter (le skin du joueur
    // avant tout) arrive avec l'entrée et rejoint le bandeau : même endroit
    // que les mods disparus, pour la même raison — il n'y a rien à décider.
    const warnings: string[] = notes.map((k) => errorText(k));

    setup = { ...setup, ...s.setup, opponents: (s.setup.opponents ?? []).map(restoreOpponent) };
    // Par le store, qui est la valeur vivante : l'écrire dans `setup` seul
    // laisserait la carte du panneau gauche afficher l'ancienne.
    setPlayerHandicap(s.setup.player_ballast ?? 0, s.setup.player_restrictor ?? 0);
    setCarAssists(s.setup.abs ?? "factory", s.setup.traction_control ?? "factory");
    // Le type fait partie de ce qui est rechargé, et il vit désormais dans la
    // colonne de session : sans ça, la liste resterait sur l'ancien.
    sessionNav.type = s.setup.session_type;
    grid.count = s.opponentCount;
    // Même migration que pour un preset par type : une sauvegarde d'avant les
    // jetons retrouve son vivier, elle ne retombe pas sur les défauts.
    grid.restorePool(
      { grid_filters: s.gridFilters, grid_pinned: s.gridPinned, grid_mode: s.gridMode, category_selection: s.categorySelection },
      player,
    );
    season = s.season;
    selectedIntent = s.intent;

    await restoreCar(s.setup.car_id, s.setup.car_skin, libCards, (id) => grid.ensureSkins(id), warnings);
    await restoreTrack(s.setup.track_id, s.setup.track_layout, s.trackSkins, libCards, warnings);

    // Réaligne `setup` sur le duo de session : `pickSession` l'a mis à jour
    // pour ce qui a été retrouvé, et pour ce qui manquait c'est la sélection
    // courante qui fait foi. Sans cet appel, l'id d'un mod disparu resterait
    // dans `setup` et partirait tel quel au lancement quand NI la voiture NI
    // le circuit n'ont pu être rétablis — aucun `pickSession` n'ayant eu lieu,
    // l'effet de resynchronisation ne passe pas de lui-même.
    syncFromSession();

    warning = warnings.join(" ");
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
        >{t("launch.loadSession")}{#if loadableCount}&nbsp;({loadableCount}){/if}</button
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
          opponentCount={grid.count}
          defs={grid.defs}
          bind:filters={grid.filters}
          bind:pinned={grid.pinned}
          bind:query={grid.query}
          index={grid.index}
          poolCount={grid.pool.length}
          playerCard={player}
          oncountchange={(n) => grid.setCount(n)}
          onfill={() => void grid.fill()}
        />
        <GridBlock
          {setup}
          {carPool}
          skinsByCarId={grid.skinsByCarId}
          index={grid.index}
          poolCount={grid.pool.length}
          {nationalityList}
          duplicateDrivers={grid.duplicateDrivers}
          onchoose={openAddPicker}
          onregenerate={() => void grid.regenerate()}
          onremove={(i) => grid.remove(i)}
          onduplicate={(i) => grid.duplicate(i)}
          onsetlevel={(i, raw) => grid.setLevel(i, raw)}
          onsetcell={(i, patch) => grid.setCell(i, patch)}
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
    entries={savedList.map((e) => ({
      id: e.path,
      name: e.name,
      meta: savedMeta(e),
      badge: e.origin === "cm" ? t("launch.fromCm") : undefined,
      deletable: e.origin === "pitbox",
      disabled: !e.session,
    }))}
    onsave={(name) => void doSaveSession(name)}
    onpick={(path) => {
      const e = savedList.find((x) => x.path === path);
      sessionDialog = null;
      if (e?.session) void doLoadSession(e.session, e.notes);
    }}
    ondelete={(path) => void removeSavedSession(path)}
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
    bind:filters={grid.filters}
    bind:pinned={grid.pinned}
    bind:query={grid.query}
    perfRefId={setup.car_id}
    gridCount={setup.opponents.length}
    gridTarget={grid.count}
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
