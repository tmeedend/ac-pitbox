<script lang="ts">
  import { onMount } from "svelte";
  import {
    launchSession,
    listInstalled,
    listSkins,
    weatherOptions,
    weatherTemp,
    type InstalledItem,
    type RaceSetup,
    type SessionType,
    type SkinItem,
    type WeatherOption,
  } from "$lib/launch";
  import { previewSrc } from "$lib/library";
  import { nav } from "$lib/nav.svelte";

  let cars = $state<InstalledItem[]>([]);
  let tracks = $state<InstalledItem[]>([]);
  let weathers = $state<WeatherOption[]>([]);
  let skins = $state<SkinItem[]>([]);
  let selectedIntent = $state("");
  let carFilter = $state("");
  let trackFilter = $state("");
  let launching = $state(false);
  let showOptions = $state(false);
  let error = $state("");
  let info = $state("");

  let setup = $state<RaceSetup>({
    car_id: "",
    car_skin: null,
    track_id: "",
    track_layout: null,
    session_type: "practice",
    ai_count: 7,
    ai_level: 96,
    laps: 5,
    duration_minutes: 15,
    weather: "",
    time_hours: 13,
    ambient_c: null,
    road_c: null,
    penalties: false,
    jump_start_penalty: 0,
    grip: 96,
    qualifying: false,
    qualify_minutes: 10,
    ghost_car: false,
    damage: 50,
    fuel_rate: 100,
    tyre_wear: 100,
  });

  // Charge les skins quand la voiture change.
  let lastCar = $state("");
  $effect(() => {
    if (setup.car_id && setup.car_id !== lastCar) {
      lastCar = setup.car_id;
      listSkins(setup.car_id).then((s) => {
        skins = s;
        if (!skins.find((x) => x.id === setup.car_skin)) {
          setup.car_skin = skins[0]?.id ?? null;
        }
      });
    }
  });

  const currentWeather = $derived(weathers.find((w) => w.id === selectedIntent));

  // --- Presets de session par type (§8.4) : réglages persistants par type ---
  interface Persisted {
    ai_count: number;
    ai_level: number;
    laps: number;
    duration_minutes: number;
    time_hours: number;
    penalties: boolean;
    jump_start_penalty: number;
    grip: number;
    qualifying: boolean;
    qualify_minutes: number;
    ghost_car: boolean;
    damage: number;
    fuel_rate: number;
    tyre_wear: number;
    intent: string;
  }
  let presets: Record<string, Persisted> = JSON.parse(
    localStorage.getItem("pitbox.launchPresets") ?? "{}",
  );
  let applying = false;

  function savePreset() {
    presets[setup.session_type] = {
      ai_count: setup.ai_count,
      ai_level: setup.ai_level,
      laps: setup.laps,
      duration_minutes: setup.duration_minutes,
      time_hours: setup.time_hours,
      penalties: setup.penalties,
      jump_start_penalty: setup.jump_start_penalty,
      grip: setup.grip,
      qualifying: setup.qualifying,
      qualify_minutes: setup.qualify_minutes,
      ghost_car: setup.ghost_car,
      damage: setup.damage,
      fuel_rate: setup.fuel_rate,
      tyre_wear: setup.tyre_wear,
      intent: selectedIntent,
    };
    localStorage.setItem("pitbox.launchPresets", JSON.stringify(presets));
  }

  async function applyPreset(type: SessionType) {
    const p = presets[type];
    if (!p) return;
    applying = true;
    setup.ai_count = p.ai_count;
    setup.ai_level = p.ai_level;
    setup.laps = p.laps;
    setup.duration_minutes = p.duration_minutes;
    setup.time_hours = p.time_hours;
    setup.penalties = p.penalties;
    setup.jump_start_penalty = p.jump_start_penalty ?? 0;
    setup.grip = p.grip ?? 96;
    setup.qualifying = p.qualifying ?? false;
    setup.qualify_minutes = p.qualify_minutes ?? 10;
    setup.ghost_car = p.ghost_car ?? false;
    setup.damage = p.damage ?? 50;
    setup.fuel_rate = p.fuel_rate ?? 100;
    setup.tyre_wear = p.tyre_wear ?? 100;
    const opt = weathers.find((w) => w.id === p.intent && w.available);
    if (opt) await selectIntent(opt);
    applying = false;
  }

  async function setSessionType(type: SessionType) {
    if (type === setup.session_type) return;
    savePreset(); // mémorise le type courant
    setup.session_type = type;
    await applyPreset(type); // recharge les réglages du nouveau type
  }

  async function selectIntent(opt: WeatherOption) {
    if (!opt.available || !opt.weather) return;
    selectedIntent = opt.id;
    setup.weather = opt.weather;
    await refreshTemp();
  }

  async function refreshTemp() {
    if (!selectedIntent) return;
    const t = await weatherTemp(selectedIntent, setup.time_hours);
    setup.ambient_c = t.ambient;
    setup.road_c = t.road;
  }

  const sessionTypes: { id: SessionType; label: string }[] = [
    { id: "practice", label: "Practice" },
    { id: "hotlap", label: "Hotlap" },
    { id: "race", label: "Course" },
  ];

  const filteredCars = $derived(
    carFilter.trim()
      ? cars.filter((c) => `${c.name} ${c.id}`.toLowerCase().includes(carFilter.toLowerCase()))
      : cars,
  );
  const filteredTracks = $derived(
    trackFilter.trim()
      ? tracks.filter((t) => `${t.name} ${t.id}`.toLowerCase().includes(trackFilter.toLowerCase()))
      : tracks,
  );
  const currentTrack = $derived(tracks.find((t) => t.id === setup.track_id));

  onMount(async () => {
    [cars, tracks, weathers] = await Promise.all([
      listInstalled("car"),
      listInstalled("track"),
      weatherOptions(),
    ]);
    if (cars.length) setup.car_id = cars[0].id;
    if (tracks.length) {
      setup.track_id = tracks[0].id;
      setup.track_layout = tracks[0].layouts[0] ?? null;
    }

    // Pré-remplissage depuis un bouton « Conduire » (§8.6).
    const pf = nav.prefill;
    if (pf) {
      if (pf.kind === "Car") {
        if (!cars.find((c) => c.id === pf.id)) {
          cars = [{ id: pf.id, name: pf.name, layouts: [] }, ...cars];
        }
        setup.car_id = pf.id;
      } else {
        let t = tracks.find((c) => c.id === pf.id);
        if (!t) {
          t = { id: pf.id, name: pf.name, layouts: [] };
          tracks = [t, ...tracks];
        }
        setup.track_id = pf.id;
        setup.track_layout = t.layouts[0] ?? null;
      }
      nav.prefill = null;
    }

    const first = weathers.find((w) => w.available);
    if (first) await selectIntent(first);
    await applyPreset(setup.session_type); // restaure les réglages du type courant
  });

  // Recalcule la température implicite quand l'heure change.
  let lastHour = $state(-1);
  $effect(() => {
    if (setup.time_hours !== lastHour && selectedIntent) {
      lastHour = setup.time_hours;
      refreshTemp();
    }
  });

  // Persiste les réglages du type courant à chaque modification (§8.4).
  $effect(() => {
    // dépendances réactives
    void [
      setup.ai_count,
      setup.ai_level,
      setup.laps,
      setup.duration_minutes,
      setup.time_hours,
      setup.penalties,
      setup.jump_start_penalty,
      setup.grip,
      setup.qualifying,
      setup.qualify_minutes,
      setup.ghost_car,
      setup.damage,
      setup.fuel_rate,
      setup.tyre_wear,
      selectedIntent,
    ];
    if (!applying && selectedIntent) savePreset();
  });

  // Réinitialise le layout quand le circuit change.
  function onTrackChange() {
    setup.track_layout = currentTrack?.layouts[0] ?? null;
  }

  function fmtTime(h: number): string {
    const hh = Math.floor(h);
    const mm = Math.round((h - hh) * 60);
    return `${String(hh).padStart(2, "0")}:${String(mm).padStart(2, "0")}`;
  }

  async function launch() {
    if (launching || !setup.car_id || !setup.track_id) return;
    savePreset();
    launching = true;
    error = "";
    info = "";
    try {
      await launchSession($state.snapshot(setup));
      info = "Session envoyée à Content Manager — Assetto Corsa démarre…";
    } catch (e) {
      error = String(e);
    } finally {
      launching = false;
    }
  }
</script>

<div class="launch">
  <header>
    <h2>Lancer une session</h2>
    <p class="sub">Pit Box construit le race.ini et le passe à Content Manager (§8.3). Le contenu sélectionné est activé au besoin.</p>
  </header>

  <div class="grid">
    <!-- Voiture -->
    <section>
      <h3>Voiture</h3>
      <input class="input filter" placeholder="filtrer…" bind:value={carFilter} />
      <select class="input" bind:value={setup.car_id} size="6">
        {#each filteredCars as c (c.id)}
          <option value={c.id}>{c.name}</option>
        {/each}
      </select>
      {#if skins.length > 1}
        <div class="skins">
          {#each skins as sk (sk.id)}
            {@const src = previewSrc(sk.preview)}
            <button
              class="skin"
              class:on={setup.car_skin === sk.id}
              title={sk.name}
              onclick={() => (setup.car_skin = sk.id)}
            >
              {#if src}<img src={src} alt={sk.name} loading="lazy" />{:else}<span class="noimg">{sk.id}</span>{/if}
            </button>
          {/each}
        </div>
      {/if}
    </section>

    <!-- Circuit -->
    <section>
      <h3>Circuit</h3>
      <input class="input filter" placeholder="filtrer…" bind:value={trackFilter} />
      <select class="input" bind:value={setup.track_id} size="6" onchange={onTrackChange}>
        {#each filteredTracks as t (t.id)}
          <option value={t.id}>{t.name}</option>
        {/each}
      </select>
      {#if currentTrack && currentTrack.layouts.length}
        <label class="layout">
          <span>Layout</span>
          <select class="input" bind:value={setup.track_layout}>
            {#each currentTrack.layouts as l}<option value={l}>{l}</option>{/each}
          </select>
        </label>
      {/if}
    </section>
  </div>

  <!-- Type de session -->
  <section>
    <h3>Type de session</h3>
    <div class="seg">
      {#each sessionTypes as st}
        <button class:on={setup.session_type === st.id} onclick={() => setSessionType(st.id)}>{st.label}</button>
      {/each}
    </div>
  </section>

  <!-- Réglages dépendants du type (§8.4) -->
  <div class="opts">
    {#if setup.session_type === "race"}
      <label>
        <span>Adversaires IA</span>
        <input class="input" type="number" min="0" max="30" bind:value={setup.ai_count} />
      </label>
      <label>
        <span>Niveau IA ({setup.ai_level})</span>
        <input type="range" min="70" max="100" bind:value={setup.ai_level} />
      </label>
      <label>
        <span>Tours</span>
        <input class="input" type="number" min="1" max="99" bind:value={setup.laps} />
      </label>
      <label>
        <span>Dégâts ({setup.damage}%)</span>
        <input type="range" min="0" max="100" bind:value={setup.damage} />
      </label>
      <label>
        <span>Carburant ({setup.fuel_rate}%)</span>
        <input type="range" min="0" max="100" bind:value={setup.fuel_rate} />
      </label>
      <label>
        <span>Usure pneus ({setup.tyre_wear}%)</span>
        <input type="range" min="0" max="100" bind:value={setup.tyre_wear} />
      </label>
    {:else if setup.session_type === "practice"}
      <label>
        <span>Durée (min)</span>
        <input class="input" type="number" min="1" max="240" bind:value={setup.duration_minutes} />
      </label>
      <label>
        <span>Niveau IA ({setup.ai_level})</span>
        <input type="range" min="70" max="100" bind:value={setup.ai_level} />
      </label>
    {:else if setup.session_type === "hotlap"}
      <label class="check">
        <input type="checkbox" bind:checked={setup.ghost_car} />
        <span>Ghost car</span>
      </label>
    {/if}
  </div>

  <!-- Météo & heure -->
  <div class="grid">
    <section>
      <h3>Météo</h3>
      <div class="intents">
        {#each weathers as w}
          <button
            class="intent"
            class:on={selectedIntent === w.id}
            disabled={!w.available}
            title={w.reason ?? w.backend ?? ""}
            onclick={() => selectIntent(w)}
          >
            {w.label}
          </button>
        {/each}
      </div>
      {#if currentWeather}
        <div class="weather-meta">
          {#if currentWeather.backend}<span class="backend">{currentWeather.backend}</span>{/if}
          {#if setup.ambient_c !== null}
            <span class="temp mono">~{setup.ambient_c}°C air · {setup.road_c}°C piste</span>
          {/if}
        </div>
      {/if}
    </section>
    <section>
      <h3>Heure — {fmtTime(setup.time_hours)}</h3>
      <input type="range" min="6" max="22" step="0.5" bind:value={setup.time_hours} class="time" />
    </section>
  </div>

  <!-- Options de course (repliable, §8.6) — mode Course uniquement -->
  {#if setup.session_type === "race"}
    <section class="advanced">
      <button class="adv-toggle" type="button" onclick={() => (showOptions = !showOptions)}>
        {showOptions ? "▾" : "▸"} Options de course
      </button>
      {#if showOptions}
        <div class="adv-body">
          <label class="check">
            <input type="checkbox" bind:checked={setup.penalties} />
            <span>Pénalités</span>
          </label>
          <label>
            <span>Faux départ</span>
            <select class="input" bind:value={setup.jump_start_penalty}>
              <option value={0}>Aucune</option>
              <option value={1}>Téléport au stand</option>
              <option value={2}>Drive-through</option>
            </select>
          </label>
          <label>
            <span>Évolution du grip</span>
            <select class="input" bind:value={setup.grip}>
              <option value={86}>Vert (86%)</option>
              <option value={92}>Moyen (92%)</option>
              <option value={96}>Roulé (96%)</option>
              <option value={100}>Optimal (100%)</option>
            </select>
          </label>
          <label class="check">
            <input type="checkbox" bind:checked={setup.qualifying} />
            <span>Qualification</span>
          </label>
          {#if setup.qualifying}
            <label>
              <span>Qualif (min)</span>
              <input class="input" type="number" min="1" max="60" bind:value={setup.qualify_minutes} />
            </label>
          {/if}
        </div>
      {/if}
    </section>
  {/if}

  {#if error}<div class="err">{error}</div>{/if}
  {#if info}<div class="ok">{info}</div>{/if}

  <div class="footer">
    <button class="btn btn-primary big" type="button" onclick={launch} disabled={launching || !setup.car_id || !setup.track_id}>
      {launching ? "Lancement…" : "▶ Lancer"}
    </button>
  </div>
</div>

<style>
  .launch {
    max-width: 720px;
    padding-bottom: 40px;
  }
  header {
    margin-bottom: 20px;
  }
  h2 {
    font-size: 15px;
    font-weight: 600;
  }
  .sub {
    color: var(--muted);
    margin-top: 6px;
    line-height: 1.5;
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 18px;
  }
  section {
    margin-bottom: 18px;
  }
  h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--txt2);
    margin-bottom: 8px;
  }
  .filter {
    margin-bottom: 6px;
  }
  select[size] {
    height: auto;
  }
  .layout {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
    font-size: 11px;
    text-transform: uppercase;
    color: var(--muted);
  }
  .layout .input {
    flex: 1;
  }
  .seg {
    display: flex;
    border: 1px solid var(--line);
    width: fit-content;
  }
  .seg button {
    background: var(--panel2);
    color: var(--muted);
    padding: 9px 22px;
    font-size: 12px;
    border-right: 1px solid var(--line);
  }
  .seg button:last-child {
    border-right: none;
  }
  .seg button.on {
    background: var(--raised);
    color: var(--txt);
  }
  .opts {
    display: flex;
    flex-wrap: wrap;
    gap: 18px;
    align-items: flex-end;
    margin-bottom: 18px;
  }
  .opts label,
  .check {
    display: flex;
    flex-direction: column;
    gap: 5px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--muted);
  }
  .opts input[type="number"] {
    width: 90px;
  }
  .opts input[type="range"],
  .time {
    width: 200px;
  }
  .skins {
    display: flex;
    gap: 6px;
    margin-top: 8px;
    overflow-x: auto;
    padding-bottom: 4px;
  }
  .skin {
    flex: none;
    width: 56px;
    height: 32px;
    border: 1px solid var(--line);
    background: var(--bg);
    padding: 0;
    overflow: hidden;
  }
  .skin.on {
    border-color: var(--rosso);
  }
  .skin img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .skin .noimg {
    font-size: 7px;
    color: var(--faint);
  }
  .advanced {
    border: 1px solid var(--line);
    margin-bottom: 16px;
  }
  .adv-toggle {
    width: 100%;
    text-align: left;
    background: var(--panel2);
    color: var(--txt2);
    padding: 10px 14px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
  }
  .adv-body {
    display: flex;
    flex-wrap: wrap;
    gap: 16px;
    align-items: flex-end;
    padding: 14px;
    border-top: 1px solid var(--line);
  }
  .adv-body .input {
    width: 150px;
  }
  .adv-body input[type="number"] {
    width: 80px;
  }
  .intents {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .intent {
    background: var(--panel2);
    border: 1px solid var(--line);
    color: var(--txt2);
    padding: 7px 12px;
    font-size: 12px;
  }
  .intent:hover:not(:disabled) {
    border-color: var(--faint);
  }
  .intent.on {
    background: var(--rosso-dim);
    border-color: var(--rosso);
    color: var(--rosso-bright);
  }
  .intent:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }
  .weather-meta {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 8px;
    font-size: 11px;
  }
  .backend {
    color: var(--green);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    font-size: 10px;
  }
  .temp {
    color: var(--muted);
  }
  .check {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    text-transform: none;
    font-size: 12.5px;
    color: var(--txt2);
    margin-bottom: 18px;
    cursor: pointer;
  }
  .err {
    padding: 10px 12px;
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    font-size: 12px;
    margin-bottom: 14px;
  }
  .ok {
    padding: 10px 12px;
    background: var(--green-dim);
    border: 1px solid var(--green-border);
    color: var(--green);
    font-size: 12px;
    margin-bottom: 14px;
  }
  .footer {
    border-top: 1px solid var(--line);
    padding-top: 18px;
  }
  .big {
    font-size: 14px;
    padding: 11px 28px;
  }
</style>
