<script lang="ts">
  // Bloc « Météo & saison » de l'écran Lancement (§8.5/§8.6/§8.6bis) : cartes
  // météo en icônes SVG, température/vent implicites (jamais saisis
  // manuellement, seulement corrigeables), heure de la session, saison
  // optionnelle. Vue de présentation : le déclenchement des calculs
  // (weatherConditions, mémorisation de l'override) reste dans Launch.svelte,
  // qui pilote aussi ce même état depuis d'autres sources (presets, sessions
  // sauvegardées) — ce bloc ne fait qu'afficher et notifier.
  import type { RaceSetup, Season, TrackSun, WeatherOption } from "$lib/launch";
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
    sun,
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
    /** Course du soleil du circuit (§8.6ter), ou `null` si sa position est
     * inconnue — la bande jour/nuit ne s'affiche alors pas du tout. */
    sun: TrackSun | null;
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

  // Arrondi sur les minutes totales, pas sur les minutes seules : un lever à
  // 4,999 h donnait « 04:60 » (heure tronquée d'un côté, minutes arrondies de
  // l'autre). Invisible tant que le curseur ne produisait que des demi-heures,
  // pas depuis que les repères de lever/coucher affichent l'heure réelle.
  function fmtTime(h: number): string {
    const total = Math.round(h * 60);
    const hh = Math.floor(total / 60) % 24, mm = total % 60;
    return `${String(hh).padStart(2, "0")}:${String(mm).padStart(2, "0")}`;
  }

  // --- Bande jour/nuit sous le curseur d'heure (§8.6ter) ---
  // Borne haute du curseur : la bande couvre exactement la course du pouce,
  // sinon un repère de coucher tombe à côté de l'heure qu'il désigne.
  const BAND_MAX = 23.5;
  const NIGHT = "var(--sky-night)";
  const TWILIGHT = "var(--sky-twilight)";
  const DAY = "var(--sky-day)";
  /** Position d'une heure sur la bande, en pourcentage de la course du pouce. */
  function bandPct(h: number): number {
    return Math.max(0, Math.min(100, (h / BAND_MAX) * 100));
  }
  /** Dégradé du ciel : nuit, crépuscule civil, plein jour, et retour. Le
   * crépuscule civil (soleil à -6°) est ce qui rend la bande lisible — entre
   * lui et le lever, il y a la demi-heure où l'on voit sans phares, qu'une
   * simple coupure nuit/jour ferait disparaître. */
  function skyGradient(s: TrackSun): string {
    if (s.sunrise === null || s.sunset === null) {
      // Nuit polaire ou soleil de minuit : une seule couleur, et aucun repère.
      return s.polarNight ? NIGHT : DAY;
    }
    const dawn = s.dawn ?? s.sunrise - 0.5;
    const dusk = s.dusk ?? s.sunset + 0.5;
    const at = (color: string, h: number) => `${color} ${bandPct(h)}%`;
    // Le jour peut enjamber minuit très au nord (lever à 01:30, coucher à
    // 23:40) : la bande commence alors en plein jour et la nuit est au milieu.
    const stops =
      s.sunset < s.sunrise
        ? [
            `${DAY} 0%`,
            at(DAY, s.sunset - 0.7),
            at(TWILIGHT, s.sunset),
            at(NIGHT, dusk),
            at(NIGHT, dawn),
            at(TWILIGHT, s.sunrise),
            at(DAY, s.sunrise + 0.7),
            `${DAY} 100%`,
          ]
        : [
            `${NIGHT} 0%`,
            at(NIGHT, dawn),
            at(TWILIGHT, s.sunrise),
            at(DAY, Math.min(s.sunrise + 0.7, s.solarNoon)),
            at(DAY, Math.max(s.sunset - 0.7, s.solarNoon)),
            at(TWILIGHT, s.sunset),
            at(NIGHT, dusk),
            `${NIGHT} 100%`,
          ];
    return `linear-gradient(to right, ${stops.join(", ")})`;
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
        {#if sun}
          <!-- Bande jour/nuit (§8.6ter) : la course du soleil sur ce circuit,
               à la date que CSP utilisera. Alignée sur la course du pouce du
               curseur (marges de 5px = demi-largeur du pouce), pour qu'un
               repère de coucher désigne bien l'heure qu'il affiche. Les deux
               repères sont cliquables : c'est le geste utile — se poser pile
               au lever ou au coucher, ce qu'un curseur au pas d'une demi-heure
               ne permet pas d'atteindre. -->
          <div
            class="sky"
            title={t("launch.sunBandTitle", { date: sun.date }) +
              (sun.source === "geotags" ? ` — ${t("launch.sunApprox")}` : "")}
          >
            <div
              class="sky-band"
              class:approx={sun.source === "geotags"}
              style:background-image={skyGradient(sun)}
            ></div>
            {#if sun.sunrise !== null && sun.sunset !== null}
              {@const rise = sun.sunrise}
              {@const fall = sun.sunset}
              <button
                class="sun-mark"
                type="button"
                style:left="{bandPct(rise)}%"
                title={t("launch.sunrise")}
                onclick={() => (setup.time_hours = rise)}
              >
                <span class="sun-tick"></span>
                <span class="sun-time mono">↑{fmtTime(rise)}</span>
              </button>
              <button
                class="sun-mark"
                type="button"
                style:left="{bandPct(fall)}%"
                title={t("launch.sunset")}
                onclick={() => (setup.time_hours = fall)}
              >
                <span class="sun-tick"></span>
                <span class="sun-time mono">↓{fmtTime(fall)}</span>
              </button>
            {/if}
          </div>
        {/if}
      </div>
    </div>
    <p class="implicit-note">{t("launch.implicitNote")}</p>
  {/if}
  {#if currentWeather?.wet && !trackSupportsRain}
    <p class="warnbox spaced">
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
      <!-- Alerte, pas note de bas de page : elle prévient que le réglage
           qu'on vient de choisir n'aura aucun effet ici. Elle s'affichait en
           8px gris, la même taille que la note d'information voisine, donc
           illisible et indistinguable de ce qui n'est qu'un commentaire. -->
      <p class="warnbox spaced">
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
  /* Note d'information — ce qui explique, sans rien signaler. Passée de 8 à
     10px : à 8 elle n'était pas discrète, elle était illisible. */
  .implicit-note {
    color: var(--muted);
    font-size: 10px;
    margin-top: 6px;
  }
  /* L'encadré vient de `.warnbox` (global.css) ; seule la marge est locale. */
  .spaced {
    margin-top: 10px;
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
  /* Heure : sur sa propre ligne dans `.implicit`, sous les températures et le
     vent (§8.6ter). Elle tenait dans 140px tant qu'elle n'était qu'un
     curseur ; la bande jour/nuit en dessous porte deux heures lisibles et des
     repères à placer au pixel, ce qui demande la largeur du bloc. */
  .time-imp {
    flex-basis: 100%;
  }
  /* Marges de 5px = demi-largeur du pouce du curseur (voir Slider.svelte) :
     un `input[type=range]` réserve cette moitié à chaque bout, donc une bande
     posée bord à bord désignerait des heures décalées aux extrémités. */
  .sky {
    position: relative;
    margin: 7px 5px 0;
    /* Couleurs du ciel, pas des couleurs d'interface : elles ne sortent pas
       d'ici et n'ont donc rien à faire dans la palette globale. */
    --sky-night: #0a0f1e;
    --sky-twilight: #b5622c;
    --sky-day: #5f8fb8;
  }
  .sky-band {
    height: 14px;
    border: 1px solid var(--line);
  }
  /* Même code visuel que `.wcard.unsupported` juste au-dessus : pointillé
     jaune = ce n'est pas garanti. Ici, la position vient du mod et non de la
     table de CSP, donc le fuseau n'est qu'approché — l'infobulle le dit, mais
     une infobulle ne se survole que si quelque chose invite à le faire. */
  .sky-band.approx {
    border-style: dashed;
    border-color: var(--yellow);
  }
  /* Repère cliquable : un trait sur la bande, l'heure dessous. Le bouton
     lui-même est transparent — seul le trait et le texte se voient. */
  .sun-mark {
    position: absolute;
    top: 0;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 0;
    background: transparent;
    border: none;
    cursor: pointer;
  }
  .sun-tick {
    width: 1px;
    height: 16px;
    background: rgba(255, 255, 255, 0.75);
  }
  .sun-time {
    margin-top: 2px;
    font-size: 9.5px;
    color: var(--muted);
    white-space: nowrap;
  }
  .sun-mark:hover .sun-time,
  .sun-mark:focus-visible .sun-time {
    color: var(--txt2);
  }
  .sun-mark:hover .sun-tick,
  .sun-mark:focus-visible .sun-tick {
    background: var(--rosso-bright);
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
