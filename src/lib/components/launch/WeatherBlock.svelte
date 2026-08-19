<script lang="ts">
  // Bloc « Météo & saison » de l'écran Lancement (§8.5/§8.6/§8.6bis) : cartes
  // météo en icônes SVG, température/vent implicites (jamais saisis
  // manuellement, seulement corrigeables), heure de la session, saison
  // optionnelle. Vue de présentation : le déclenchement des calculs
  // (weatherConditions, mémorisation de l'override) reste dans Launch.svelte,
  // qui pilote aussi ce même état depuis d'autres sources (presets, sessions
  // sauvegardées) — ce bloc ne fait qu'afficher et notifier.
  import type { RaceSetup, Season, WeatherOption } from "$lib/launch";
  import { t } from "$lib/i18n/index.svelte";
  import NumberStepper from "../NumberStepper.svelte";
  import Slider from "../Slider.svelte";
  import Tooltip from "../Tooltip.svelte";

  let {
    setup,
    weathers,
    selectedIntent,
    currentWeather,
    trackSupportsSeason,
    trackSupportsRain,
    season,
    onselectintent,
    onselectseason,
    onoverridetemps,
    onoverridewind,
  }: {
    setup: RaceSetup;
    weathers: WeatherOption[];
    selectedIntent: string;
    currentWeather: WeatherOption | undefined;
    trackSupportsSeason: boolean;
    trackSupportsRain: boolean;
    season: Season;
    onselectintent: (opt: WeatherOption) => void;
    onselectseason: (id: Season) => void;
    onoverridetemps: () => void;
    onoverridewind: () => void;
  } = $props();

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

  const SEASONS: { id: Season; labelKey: string }[] = [
    { id: "", labelKey: "launch.seasonNone" },
    { id: "spring", labelKey: "launch.seasonSpring" },
    { id: "summer", labelKey: "launch.seasonSummer" },
    { id: "autumn", labelKey: "launch.seasonAutumn" },
    { id: "winter", labelKey: "launch.seasonWinter" },
  ];

  const sunRays = Array.from({ length: 8 }, (_, i) => {
    const a = (i * Math.PI) / 4;
    return {
      x1: 19 + Math.cos(a) * 11,
      y1: 19 + Math.sin(a) * 11,
      x2: 19 + Math.cos(a) * 14,
      y2: 19 + Math.sin(a) * 14,
    };
  });

  function fmtTime(h: number): string {
    const hh = Math.floor(h), mm = Math.round((h - hh) * 60);
    return `${String(hh).padStart(2, "0")}:${String(mm).padStart(2, "0")}`;
  }

  // 8 directions cardinales, dans l'ordre de `compassKey` — le champ n'accepte
  // que ces valeurs canoniques (0/45/90…) : le degré exact renvoyé par la météo
  // n'a pas de sens à retaper à la main, seul le secteur compte pour le jeu.
  const COMPASS_DEGREES = [0, 45, 90, 135, 180, 225, 270, 315];
  function compassKey(deg: number): string {
    const keys = [
      "launch.compassN", "launch.compassNE", "launch.compassE", "launch.compassSE",
      "launch.compassS", "launch.compassSW", "launch.compassW", "launch.compassNW",
    ];
    return keys[Math.round(deg / 45) % 8];
  }
  // Secteur affiché dans le sélecteur : la valeur recommandée par la météo
  // tombe rarement pile sur un multiple de 45°, on l'arrondit au plus proche
  // pour que le menu ait toujours une option correspondante sélectionnée.
  const windDirBucket = $derived(Math.round((setup.wind_direction_deg ?? 0) / 45) * 45 % 360);
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

<!-- Météo en icônes SVG (§8.6) -->
<section class="blk">
  <header class="blk-h"><span class="blk-t">{t("launch.weather")}</span></header>
  <div class="blk-b">
  <div class="weather">
    {#each WEATHER_IDS as id}
      {@const opt = weathers.find((w) => w.id === id)}
      <button
        class="wcard"
        class:on={selectedIntent === id}
        type="button"
        disabled={!opt?.available}
        title={opt?.reason ?? opt?.backend ?? ""}
        onclick={() => opt && onselectintent(opt)}
      >
        <svg viewBox="0 0 38 38">{@render weatherIcon(id)}</svg>
        <div class="wn">{t(WEATHER_LABEL_KEYS[id])}</div>
      </button>
    {/each}
  </div>
  {#if currentWeather}
    <div class="implicit">
      <div class="imp temp-imp">
        <div class="ik lbl-key">{t("launch.tempAirLabel")}</div>
        <NumberStepper
          width={68}
          min={-20}
          max={45}
          value={setup.ambient_c ?? 0}
          onchange={(v) => { setup.ambient_c = v; onoverridetemps(); }}
        />
      </div>
      <div class="imp temp-imp">
        <div class="ik lbl-key">{t("launch.tempRoadLabel")}</div>
        <NumberStepper
          width={68}
          min={-20}
          max={65}
          value={setup.road_c ?? 0}
          onchange={(v) => { setup.road_c = v; onoverridetemps(); }}
        />
      </div>
      <div class="imp wind-imp">
        <div class="ik lbl-key">{t("launch.windImplicit")}</div>
        <div class="wind-fields">
          <NumberStepper
            width={58}
            min={0}
            max={120}
            value={setup.wind_speed_kmh ?? 0}
            onchange={(v) => { setup.wind_speed_kmh = v; onoverridewind(); }}
          />
          <select
            class="input mono wind-dir"
            value={windDirBucket}
            onchange={(e) => { setup.wind_direction_deg = Number(e.currentTarget.value); onoverridewind(); }}
          >
            {#each COMPASS_DEGREES as deg}
              <option value={deg}>{t(compassKey(deg))}</option>
            {/each}
          </select>
        </div>
      </div>
      <div class="imp time-imp">
        <!-- Full 24 h range: night sessions are legitimate (CSP/Sol handle lighting),
             nothing in the launch path depends on daylight. Max is 23.5 rather than 24
             so the last step reads 23:30 instead of a nonsensical 24:00. -->
        <Slider
          label={t("launch.timeLabelShort")}
          value={setup.time_hours}
          min={0}
          max={23.5}
          step={0.5}
          display={fmtTime(setup.time_hours)}
          oninput={(v) => (setup.time_hours = v)}
        />
      </div>
    </div>
    <p class="implicit-note">{t("launch.implicitNote")}</p>
  {/if}
  {#if currentWeather?.wet && !trackSupportsRain}
    <p class="warn-note">
      ⚠ {t("launch.rainUnsupportedWarning")}
      <Tooltip text={t("launch.cspFirstLaunchHint")}><button type="button" class="info-i">ⓘ</button></Tooltip>
    </p>
  {/if}

  <!-- Saison optionnelle (§8.6bis) : associe une date, best-effort côté
       CSP (couleur des arbres en automne, piste blanche en hiver).
       Reste cliquable même sans config CSP identifiée pour les
       ajustements saisonniers (§6.4bis) — juste signalée (pas de
       garantie de rendu), jamais bloquée : une config CSP absente ici
       ne veut pas dire absente pour de bon, seulement pas encore
       téléchargée par Content Manager (mod tout juste importé,
       premier lancement…). -->
  <div class="season-wrap">
    <div class="opt-name lbl-key" style="margin-bottom:6px;">{t("launch.seasonLabel")}</div>
    <div class="weather season-grid">
      <!-- Date manuelle (§8.6bis) : sélectionner une saison ci-contre pose
           déjà cette date (SEASON_MID, calculée côté Launch.svelte) — ce
           champ permet de la voir et, si besoin, de la corriger précisément
           sans passer par une saison. Ne remet pas `season` à "" : une date
           tapée à la main reste compatible avec la saison affichée tant que
           l'utilisateur ne touche pas aux cartes. -->
      <label class="wcard date-card">
        <input
          type="date"
          class="date-input mono"
          value={setup.season_date ?? ""}
          onchange={(e) => (setup.season_date = e.currentTarget.value || null)}
        />
        <div class="wn">{t("launch.seasonDateLabel")}</div>
      </label>
      {#each SEASONS as s}
        {@const unsupported = s.id !== "" && !trackSupportsSeason}
        <button
          class="wcard"
          class:on={season === s.id}
          class:unsupported
          type="button"
          onclick={() => onselectseason(s.id)}
        >
          <svg viewBox="0 0 38 38">{@render seasonIcon(s.id)}</svg>
          <div class="wn">{t(s.labelKey)}</div>
        </button>
      {/each}
    </div>
    {#if !trackSupportsSeason}
      <p class="implicit-note">
        {t("launch.seasonUnsupportedNote")}
        <Tooltip text={t("launch.cspFirstLaunchHint")}><button type="button" class="info-i">ⓘ</button></Tooltip>
      </p>
    {/if}
  </div>
  </div>
</section>

<style>

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
  /* Même rôle que .mt/.seg button/.seg-v button dans Launch.svelte : le
     libellé d'une option cliquable, même taille partout. Un libellé long
     (« Quelques nuages ») peut passer sur deux lignes dans une carte
     étroite — la grille l'absorbe (pas de hauteur fixe). */
  .wn {
    font-size: 11px;
    margin-top: 5px;
    color: var(--txt2);
  }
  .wcard.on .wn {
    color: var(--rosso-bright);
  }
  .implicit {
    display: flex;
    flex-wrap: wrap;
    gap: 16px;
    margin-top: 12px;
    padding: 9px 12px;
    border: 1px solid var(--line);
    background: var(--panel2);
  }
  /* Rôle différent de .mt/.wn (clé de champ, pas une option cliquable) : couleur
     et interlettrage viennent de `.lbl-key` (global, harmonisation §chantier
     libellés) — sans majuscules, contrairement à `.lbl`, ce qui correspond
     déjà à ce qu'on voulait ici (les majuscules ajoutaient une lourdeur
     inutile sur un texte aussi petit). Ne reste que l'espacement propre à ce
     bloc dense. */
  .imp .ik {
    margin-bottom: 5px;
  }
  .wind-fields {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  /* `.input` (global) part de width:100% pour un champ de formulaire pleine
     largeur ; ici il doit tenir à côté du stepper de vitesse, dans une
     cellule de la même famille que les steppers de température. */
  .wind-dir {
    width: auto;
    padding: 5px 6px;
    font-size: 10.5px;
  }
  .implicit-note {
    color: var(--muted);
    font-size: 8px;
    margin-top: 6px;
  }
  .warn-note {
    color: var(--yellow);
    font-size: 10px;
    margin-top: 10px;
    padding: 7px 9px;
    background: #1a1708;
    border: 1px solid #4a4426;
  }
  /* Signale sans bloquer (§8.6bis) : une config CSP absente ici ne veut pas
     dire absente pour de bon (voir le commentaire sur .season-wrap), donc
     jamais un simple `:disabled` — juste un repère visuel discret. */
  .wcard.unsupported {
    border-style: dashed;
    border-color: var(--yellow);
  }
  .info-i {
    background: transparent;
    border: none;
    color: var(--muted);
    font-size: inherit;
    margin-left: 4px;
    padding: 0;
    cursor: help;
  }
  .info-i:hover,
  .info-i:focus-visible {
    color: var(--txt2);
  }

  /* Couleur/taille/interlettrage viennent de `.lbl-key` (global, harmonisation
     §chantier libellés) : ne reste ici que ce que `.lbl-key` ne couvre pas. */
  .opt-name {
    text-transform: uppercase;
  }
  /* Heure : 4e colonne de `.implicit`, à côté du vent (§8.6). Largeur figée
     pour rester compacte comme les autres champs de la ligne plutôt que de
     s'étirer en pleine largeur (son ancien emplacement, en rangée seule) —
     `Slider` prend toute la largeur qu'on lui donne, donc c'est cette
     largeur-là qui la fixe. */
  .time-imp {
    width: 140px;
  }
  .season-wrap {
    margin-top: 16px;
  }
  /* Mêmes cartes que la météo (.wcard/.wn ci-dessus), deux colonnes de plus
     pour garder les 5 options (Aucune + 4 saisons) et la date manuelle sur
     une seule ligne. Sélecteur composé pour primer sur .weather
     { grid-template-columns } quel que soit l'ordre des règles dans la
     feuille de style. */
  .weather.season-grid {
    grid-template-columns: repeat(6, 1fr);
  }
  /* Date manuelle (§8.6bis) : même encadré wcard que les cartes saison,
     contenu différent (champ natif au lieu d'une icône SVG). Le picker natif
     est blanc par défaut, hors charte sombre — `color-scheme: dark` sur
     .date-input bascule son rendu (icône + popup) en sombre, seule prise
     possible dessus (pas de pseudo-élément stylable autrement). */
  .date-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    cursor: default;
  }
  .date-input {
    width: 100%;
    background: var(--bg);
    border: 1px solid var(--line);
    color: var(--txt2);
    padding: 4px 3px;
    font-size: 9px;
    color-scheme: dark;
  }
</style>
