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
  import { listLibrary, previewSrc, type ModCard } from "$lib/library";
  import { nav } from "$lib/nav.svelte";

  type Step = "category" | "car" | "track" | "settings";
  let step = $state<Step>("settings");

  let cars = $state<InstalledItem[]>([]);
  let tracks = $state<InstalledItem[]>([]);
  let weathers = $state<WeatherOption[]>([]);
  let skins = $state<SkinItem[]>([]);
  let libCards = $state<ModCard[]>([]);
  let category = $state<string>("all");
  let selectedIntent = $state("");
  let carFilter = $state("");
  let trackFilter = $state("");
  let launching = $state(false);
  let showOptions = $state(false);
  let error = $state("");
  let info = $state("");
  let ready = $state(false);

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

  // --- Catégories (depuis l'overlay) ---
  const carCategory = $derived(
    new Map(libCards.filter((c) => c.kind === "Car" && c.category).map((c) => [c.id_interne, c.category!])),
  );
  const categories = $derived([...new Set([...carCategory.values()])].sort());
  const filteredCars = $derived(
    cars
      .filter((c) => category === "all" || carCategory.get(c.id) === category)
      .filter((c) => !carFilter.trim() || `${c.name} ${c.id}`.toLowerCase().includes(carFilter.toLowerCase())),
  );
  const filteredTracks = $derived(
    tracks.filter((t) => !trackFilter.trim() || `${t.name} ${t.id}`.toLowerCase().includes(trackFilter.toLowerCase())),
  );
  const currentCar = $derived(cars.find((c) => c.id === setup.car_id));
  const currentTrack = $derived(tracks.find((t) => t.id === setup.track_id));
  const currentWeather = $derived(weathers.find((w) => w.id === selectedIntent));
  const carDuoSrc = $derived(previewSrc(nav.sessionCar?.preview ?? null));
  const trackDuoSrc = $derived(previewSrc(nav.sessionTrack?.preview ?? null));

  const sessionTypes: { id: SessionType; label: string }[] = [
    { id: "practice", label: "Practice" },
    { id: "hotlap", label: "Hotlap" },
    { id: "race", label: "Course" },
  ];

  // --- Mémorisation de la sélection (§8.6) ---
  interface Selection {
    category: string;
    car_id: string;
    car_skin: string | null;
    track_id: string;
    track_layout: string | null;
    session_type: SessionType;
  }
  function saveSelection() {
    if (!ready) return;
    const sel: Selection = {
      category,
      car_id: setup.car_id,
      car_skin: setup.car_skin,
      track_id: setup.track_id,
      track_layout: setup.track_layout,
      session_type: setup.session_type,
    };
    localStorage.setItem("pitbox.launchSel", JSON.stringify(sel));
  }
  $effect(() => {
    void [category, setup.car_id, setup.car_skin, setup.track_id, setup.track_layout, setup.session_type];
    saveSelection();
  });

  // --- Météo (intentions + température implicite) ---
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
  let lastHour = $state(-1);
  $effect(() => {
    if (setup.time_hours !== lastHour && selectedIntent) {
      lastHour = setup.time_hours;
      refreshTemp();
    }
  });

  // --- Skins de la voiture ---
  let lastCar = $state("");
  $effect(() => {
    if (setup.car_id && setup.car_id !== lastCar) {
      lastCar = setup.car_id;
      listSkins(setup.car_id).then((s) => {
        skins = s;
        // Skin par défaut : l'étoile « piloté » de la fiche (§12bis.2) si elle
        // pointe un skin existant, sinon le premier. On ne touche pas à un choix
        // déjà valide pour cette voiture.
        if (!skins.find((x) => x.id === setup.car_skin)) {
          const piloted = localStorage.getItem(`pitbox.pilotedSkin.${setup.car_id}`);
          setup.car_skin = (piloted && skins.some((x) => x.id === piloted) ? piloted : skins[0]?.id) ?? null;
        }
      });
    }
  });

  // --- Presets de session par type (§8.4) ---
  interface Persisted {
    ai_count: number; ai_level: number; laps: number; duration_minutes: number;
    time_hours: number; penalties: boolean; jump_start_penalty: number; grip: number;
    qualifying: boolean; qualify_minutes: number; ghost_car: boolean;
    damage: number; fuel_rate: number; tyre_wear: number; intent: string;
  }
  let presets: Record<string, Persisted> = JSON.parse(localStorage.getItem("pitbox.launchPresets") ?? "{}");
  let applying = false;

  function savePreset() {
    presets[setup.session_type] = {
      ai_count: setup.ai_count, ai_level: setup.ai_level, laps: setup.laps,
      duration_minutes: setup.duration_minutes, time_hours: setup.time_hours,
      penalties: setup.penalties, jump_start_penalty: setup.jump_start_penalty, grip: setup.grip,
      qualifying: setup.qualifying, qualify_minutes: setup.qualify_minutes, ghost_car: setup.ghost_car,
      damage: setup.damage, fuel_rate: setup.fuel_rate, tyre_wear: setup.tyre_wear, intent: selectedIntent,
    };
    localStorage.setItem("pitbox.launchPresets", JSON.stringify(presets));
  }
  async function applyPreset(type: SessionType) {
    const p = presets[type];
    if (!p) return;
    applying = true;
    setup.ai_count = p.ai_count; setup.ai_level = p.ai_level; setup.laps = p.laps;
    setup.duration_minutes = p.duration_minutes; setup.time_hours = p.time_hours;
    setup.penalties = p.penalties; setup.jump_start_penalty = p.jump_start_penalty ?? 0;
    setup.grip = p.grip ?? 96; setup.qualifying = p.qualifying ?? false; setup.qualify_minutes = p.qualify_minutes ?? 10;
    setup.ghost_car = p.ghost_car ?? false; setup.damage = p.damage ?? 50;
    setup.fuel_rate = p.fuel_rate ?? 100; setup.tyre_wear = p.tyre_wear ?? 100;
    const opt = weathers.find((w) => w.id === p.intent && w.available);
    if (opt) await selectIntent(opt);
    applying = false;
  }
  async function setSessionType(type: SessionType) {
    if (type === setup.session_type) return;
    savePreset();
    setup.session_type = type;
    await applyPreset(type);
  }
  $effect(() => {
    void [setup.ai_count, setup.ai_level, setup.laps, setup.duration_minutes, setup.time_hours,
      setup.penalties, setup.jump_start_penalty, setup.grip, setup.qualifying, setup.qualify_minutes,
      setup.ghost_car, setup.damage, setup.fuel_rate, setup.tyre_wear, selectedIntent];
    if (ready && !applying && selectedIntent) savePreset();
  });

  // --- Chargement + résolution des défauts (§8.6) ---
  onMount(async () => {
    [cars, tracks, weathers, libCards] = await Promise.all([
      listInstalled("car"), listInstalled("track"), weatherOptions(), listLibrary(),
    ]);

    const saved: Partial<Selection> = JSON.parse(localStorage.getItem("pitbox.launchSel") ?? "{}");
    setup.session_type = saved.session_type ?? "practice";

    // La bibliothèque EST le sélecteur (§8.6) : voiture/circuit viennent du duo
    // de session choisi dans les bibliothèques. On saute directement aux réglages.
    syncFromSession();
    step = "settings";

    const first = weathers.find((w) => w.available);
    if (first) await selectIntent(first);
    await applyPreset(setup.session_type);
    ready = true;
  });

  // Applique le duo de session (§8.6) au setup : voiture, skin piloté, circuit,
  // layout. Repli sur le 1er installé si aucune sélection.
  function syncFromSession() {
    const c = nav.sessionCar;
    const t = nav.sessionTrack;
    setup.car_id = c?.id ?? cars[0]?.id ?? "";
    setup.car_skin = c ? localStorage.getItem(`pitbox.pilotedSkin.${c.id}`) : null;
    setup.track_id = t?.id ?? tracks[0]?.id ?? "";
    const tr = tracks.find((x) => x.id === setup.track_id);
    setup.track_layout =
      t?.layout && tr?.layouts.includes(t.layout) ? t.layout : (tr?.layouts[0] ?? null);
  }

  // Resynchronise si le duo change (l'utilisateur ouvre une autre voiture/circuit
  // dans la bibliothèque puis revient à la session).
  $effect(() => {
    void [nav.sessionCar?.id, nav.sessionTrack?.id, nav.sessionTrack?.layout];
    if (ready) syncFromSession();
  });

  function goCategory(cat: string) {
    category = cat;
    step = "car";
  }

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
      info = "Session envoyée à Content Manager — Assetto Corsa démarre…";
    } catch (e) {
      error = String(e);
    } finally {
      launching = false;
    }
  }
</script>

<div class="flow">
  <!-- Rappel du duo sélectionné (§8.6) + Lancer. La sélection se fait dans les
       bibliothèques, pas ici. -->
  <header class="bar">
    <div class="duo">
      <div class="duo-item">
        <div class="duo-img">{#if carDuoSrc}<img src={carDuoSrc} alt="" />{:else}<span>🚗</span>{/if}</div>
        <div class="duo-txt">
          <div class="duo-k">VOITURE</div>
          <div class="duo-n">{nav.sessionCar?.name ?? "— aucune"}</div>
        </div>
      </div>
      <span class="duo-plus">+</span>
      <div class="duo-item">
        <div class="duo-img">{#if trackDuoSrc}<img src={trackDuoSrc} alt="" />{:else}<span>🏁</span>{/if}</div>
        <div class="duo-txt">
          <div class="duo-k">CIRCUIT</div>
          <div class="duo-n">{nav.sessionTrack?.name ?? "— aucun"}{setup.track_layout ? ` · ${setup.track_layout}` : ""}</div>
        </div>
      </div>
    </div>
    <button class="btn btn-primary launch" type="button" onclick={launch} disabled={launching || !setup.car_id || !setup.track_id}>
      {launching ? "Lancement…" : "▶ Lancer"}
    </button>
  </header>

  {#if info}<div class="ok">{info}</div>{/if}
  {#if error}<div class="err">{error}</div>{/if}

  <!-- ÉTAPE CATÉGORIE -->
  {#if step === "category"}
    <section class="screen">
      <h2>Catégorie</h2>
      <p class="hint">Choisis un vivier cohérent, ou « Toutes » pour piocher partout.</p>
      <div class="cats">
        <button class="cat" class:on={category === "all"} onclick={() => goCategory("all")}>
          <span class="cat-name">Toutes</span>
          <span class="cat-n">{cars.length}</span>
        </button>
        {#each categories as cat}
          {@const n = [...carCategory.values()].filter((c) => c === cat).length}
          <button class="cat" class:on={category === cat} onclick={() => goCategory(cat)}>
            <span class="cat-name">{cat}</span>
            <span class="cat-n">{n}</span>
          </button>
        {/each}
      </div>
    </section>

  <!-- ÉTAPE VOITURE -->
  {:else if step === "car"}
    <section class="screen">
      <div class="screen-head">
        <h2>Voiture <span class="count">{filteredCars.length}</span></h2>
        <input class="input filter" placeholder="filtrer…" bind:value={carFilter} />
      </div>
      <div class="gallery">
        {#each filteredCars as c (c.id)}
          {@const src = previewSrc(c.preview)}
          <button class="tile" class:on={setup.car_id === c.id} onclick={() => (setup.car_id = c.id)}>
            <div class="thumb">{#if src}<img src={src} alt={c.name} loading="lazy" />{:else}<span class="noimg">Voiture</span>{/if}</div>
            <span class="tile-name">{c.name}</span>
          </button>
        {/each}
      </div>
      {#if skins.length > 1}
        <div class="skins-row">
          <span class="skins-label">Skin</span>
          {#each skins as sk (sk.id)}
            {@const s = previewSrc(sk.preview)}
            <button class="skin" class:on={setup.car_skin === sk.id} title={sk.name} onclick={() => (setup.car_skin = sk.id)}>
              {#if s}<img src={s} alt={sk.name} loading="lazy" />{:else}<span class="noimg">{sk.id}</span>{/if}
            </button>
          {/each}
        </div>
      {/if}
      <div class="nav-btns">
        <button class="btn" type="button" onclick={() => (step = "category")}>← Catégorie</button>
        <button class="btn btn-primary" type="button" onclick={() => (step = "track")}>Circuit →</button>
      </div>
    </section>

  <!-- ÉTAPE CIRCUIT -->
  {:else if step === "track"}
    <section class="screen">
      <div class="screen-head">
        <h2>Circuit <span class="count">{filteredTracks.length}</span></h2>
        <input class="input filter" placeholder="filtrer…" bind:value={trackFilter} />
      </div>
      <div class="track-layout">
        <div class="gallery tracks">
          {#each filteredTracks as t (t.id)}
            {@const src = previewSrc(t.preview)}
            <button class="tile" class:on={setup.track_id === t.id} onclick={() => { setup.track_id = t.id; setup.track_layout = t.layouts[0] ?? null; }}>
              <div class="thumb outline">{#if src}<img src={src} alt={t.name} loading="lazy" />{:else}<span class="noimg">Circuit</span>{/if}</div>
              <span class="tile-name">{t.name}</span>
            </button>
          {/each}
        </div>
        {#if currentTrack}
          {@const src = previewSrc(currentTrack.preview)}
          <aside class="track-info">
            <div class="ti-preview">{#if src}<img src={src} alt={currentTrack.name} />{/if}</div>
            <h3>{currentTrack.name}</h3>
            {#if currentTrack.layouts.length}
              <label class="layout">
                <span>Layout</span>
                <select class="input" bind:value={setup.track_layout}>
                  {#each currentTrack.layouts as l}<option value={l}>{l}</option>{/each}
                </select>
              </label>
            {:else}
              <div class="mono single-layout">layout unique</div>
            {/if}
          </aside>
        {/if}
      </div>
      <div class="nav-btns">
        <button class="btn" type="button" onclick={() => (step = "car")}>← Voiture</button>
        <button class="btn btn-primary" type="button" onclick={() => (step = "settings")}>Réglages →</button>
      </div>
    </section>

  <!-- ÉTAPE RÉGLAGES -->
  {:else if step === "settings"}
    <section class="screen">
      <h2>Réglages</h2>

      <div class="seg types">
        {#each sessionTypes as st}
          <button class:on={setup.session_type === st.id} onclick={() => setSessionType(st.id)}>{st.label}</button>
        {/each}
      </div>

      <!-- Réglages dépendants du type (§8.4) -->
      <div class="opts">
        {#if setup.session_type === "race"}
          <label><span>Adversaires IA</span><input class="input" type="number" min="0" max="30" bind:value={setup.ai_count} /></label>
          <label><span>Niveau IA ({setup.ai_level})</span><input type="range" min="70" max="100" bind:value={setup.ai_level} /></label>
          <label><span>Tours</span><input class="input" type="number" min="1" max="99" bind:value={setup.laps} /></label>
          <label><span>Dégâts ({setup.damage}%)</span><input type="range" min="0" max="100" bind:value={setup.damage} /></label>
          <label><span>Carburant ({setup.fuel_rate}%)</span><input type="range" min="0" max="100" bind:value={setup.fuel_rate} /></label>
          <label><span>Usure pneus ({setup.tyre_wear}%)</span><input type="range" min="0" max="100" bind:value={setup.tyre_wear} /></label>
        {:else if setup.session_type === "practice"}
          <label><span>Durée (min)</span><input class="input" type="number" min="1" max="240" bind:value={setup.duration_minutes} /></label>
          <label><span>Niveau IA ({setup.ai_level})</span><input type="range" min="70" max="100" bind:value={setup.ai_level} /></label>
        {:else if setup.session_type === "hotlap"}
          <label class="check"><input type="checkbox" bind:checked={setup.ghost_car} /><span>Ghost car</span></label>
        {/if}
      </div>

      <!-- Météo & heure (universels) -->
      <div class="wx">
        <div>
          <h3>Météo</h3>
          <div class="intents">
            {#each weathers as w}
              <button class="intent" class:on={selectedIntent === w.id} disabled={!w.available} title={w.reason ?? w.backend ?? ""} onclick={() => selectIntent(w)}>{w.label}</button>
            {/each}
          </div>
          {#if currentWeather}
            <div class="weather-meta">
              {#if currentWeather.backend}<span class="backend">{currentWeather.backend}</span>{/if}
              {#if setup.ambient_c !== null}<span class="temp mono">~{setup.ambient_c}°C air · {setup.road_c}°C piste</span>{/if}
            </div>
          {/if}
        </div>
        <div>
          <h3>Heure — {fmtTime(setup.time_hours)}</h3>
          <input type="range" min="6" max="22" step="0.5" bind:value={setup.time_hours} class="time" />
        </div>
      </div>

      <!-- Options de course (Course uniquement) -->
      {#if setup.session_type === "race"}
        <section class="advanced">
          <button class="adv-toggle" type="button" onclick={() => (showOptions = !showOptions)}>
            {showOptions ? "▾" : "▸"} Options de course
          </button>
          {#if showOptions}
            <div class="adv-body">
              <label class="check"><input type="checkbox" bind:checked={setup.penalties} /><span>Pénalités</span></label>
              <label><span>Faux départ</span>
                <select class="input" bind:value={setup.jump_start_penalty}>
                  <option value={0}>Aucune</option><option value={1}>Téléport au stand</option><option value={2}>Drive-through</option>
                </select>
              </label>
              <label><span>Évolution du grip</span>
                <select class="input" bind:value={setup.grip}>
                  <option value={86}>Vert (86%)</option><option value={92}>Moyen (92%)</option><option value={96}>Roulé (96%)</option><option value={100}>Optimal (100%)</option>
                </select>
              </label>
              <label class="check"><input type="checkbox" bind:checked={setup.qualifying} /><span>Qualification</span></label>
              {#if setup.qualifying}
                <label><span>Qualif (min)</span><input class="input" type="number" min="1" max="60" bind:value={setup.qualify_minutes} /></label>
              {/if}
            </div>
          {/if}
        </section>
      {/if}

      <div class="nav-btns">
        <button class="btn" type="button" onclick={() => (step = "track")}>← Circuit</button>
        <button class="btn btn-primary big" type="button" onclick={launch} disabled={launching || !setup.car_id || !setup.track_id}>
          {launching ? "Lancement…" : "▶ Lancer"}
        </button>
      </div>
    </section>
  {/if}
</div>

<style>
  .flow {
    margin: -28px -32px;
    padding: 0 0 40px;
    min-height: calc(100vh - 3px);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 14px 32px;
    border-bottom: 1px solid var(--line);
    background: var(--panel2);
    position: sticky;
    top: 0;
    z-index: 10;
  }
  .duo {
    display: flex;
    align-items: center;
    gap: 14px;
    flex: 1;
    min-width: 0;
  }
  .duo-item {
    display: flex;
    align-items: center;
    gap: 9px;
    min-width: 0;
  }
  .duo-img {
    width: 64px;
    height: 40px;
    flex: none;
    background: var(--bg);
    border: 1px solid var(--line);
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
  }
  .duo-img img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .duo-txt {
    min-width: 0;
  }
  .duo-k {
    font-size: 8.5px;
    letter-spacing: 1px;
    color: var(--faint);
    font-family: var(--mono);
  }
  .duo-n {
    font-size: 12.5px;
    color: var(--txt);
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .duo-plus {
    color: var(--faint);
    flex: none;
  }
  .launch {
    flex: none;
    font-size: 13px;
    padding: 9px 20px;
  }
  .screen {
    padding: 22px 32px;
  }
  .screen-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 14px;
  }
  h2 {
    font-size: 16px;
    font-weight: 600;
  }
  .count {
    color: var(--faint);
    font-family: var(--mono);
    font-size: 12px;
    margin-left: 6px;
  }
  h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--txt2);
    margin-bottom: 8px;
  }
  .hint {
    color: var(--muted);
    margin-bottom: 16px;
  }
  .filter {
    width: 220px;
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

  /* Catégories */
  .cats {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 10px;
  }
  .cat {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: var(--card);
    border: 1px solid var(--line);
    padding: 16px;
    font-size: 14px;
    color: var(--txt);
  }
  .cat:hover {
    border-color: var(--faint);
  }
  .cat.on {
    border-color: var(--rosso);
    color: var(--rosso-bright);
  }
  .cat-n {
    font-family: var(--mono);
    color: var(--muted);
    font-size: 12px;
  }

  /* Galeries voiture/circuit */
  .gallery {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
    gap: 12px;
  }
  .tile {
    background: var(--card);
    border: 1px solid var(--line);
    padding: 0;
    overflow: hidden;
    text-align: left;
  }
  .tile:hover {
    border-color: var(--faint);
  }
  .tile.on {
    border-color: var(--rosso);
  }
  .thumb {
    aspect-ratio: 16 / 9;
    background: var(--bg);
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
  }
  .thumb.outline {
    background: var(--panel);
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .thumb.outline img {
    object-fit: contain;
    padding: 8px;
  }
  .noimg {
    color: var(--faint);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 1px;
  }
  .tile-name {
    display: block;
    padding: 7px 9px;
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .skins-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 14px;
    flex-wrap: wrap;
  }
  .skins-label {
    font-size: 10px;
    text-transform: uppercase;
    color: var(--muted);
    margin-right: 4px;
  }
  .skin {
    width: 60px;
    height: 34px;
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

  .track-layout {
    display: grid;
    grid-template-columns: 1fr 240px;
    gap: 18px;
  }
  .track-info {
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 14px;
    height: fit-content;
    position: sticky;
    top: 80px;
  }
  .ti-preview {
    aspect-ratio: 16 / 9;
    background: var(--panel);
    display: flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 10px;
  }
  .ti-preview img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    padding: 6px;
  }
  .track-info h3 {
    font-size: 13px;
    color: var(--txt);
    text-transform: none;
    letter-spacing: 0;
  }
  .layout {
    display: flex;
    flex-direction: column;
    gap: 5px;
    margin-top: 12px;
    font-size: 10px;
    text-transform: uppercase;
    color: var(--muted);
  }
  .single-layout {
    color: var(--faint);
    font-size: 11px;
    margin-top: 10px;
  }

  /* Réglages */
  .seg,
  .types {
    display: flex;
    border: 1px solid var(--line);
    width: fit-content;
    margin-bottom: 18px;
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
    margin-bottom: 22px;
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
  .opts input[type="range"] {
    width: 180px;
  }
  .check {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    text-transform: none;
    font-size: 12.5px;
    color: var(--txt2);
    cursor: pointer;
  }
  .wx {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 24px;
    margin-bottom: 22px;
    max-width: 640px;
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
    gap: 12px;
    margin-top: 8px;
    font-size: 11px;
  }
  .backend {
    color: var(--green);
    text-transform: uppercase;
    font-size: 10px;
  }
  .temp {
    color: var(--muted);
  }
  .time {
    width: 220px;
  }
  .advanced {
    border: 1px solid var(--line);
    margin-bottom: 22px;
    max-width: 640px;
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

  .nav-btns {
    display: flex;
    justify-content: space-between;
    margin-top: 24px;
    padding-top: 18px;
    border-top: 1px solid var(--line);
  }
  .big {
    font-size: 14px;
    padding: 11px 28px;
  }
</style>
