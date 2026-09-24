<script lang="ts">
  // Éditeur d'un filtre (§6.3) — un seul composant, contenu variable selon le
  // type. Il vit dans un popover accroché à la puce du filtre : c'est ce qui
  // permet à un champ à jetons multiples avec opérateur d'occuper les mêmes
  // 28 px qu'une case à cocher dans la barre.
  //
  // Aucun bouton de validation : chaque modification s'applique tout de suite
  // et le décompte de résultats bouge à la frappe. Un « Appliquer » ferait
  // douter que le compteur parle bien de ce qui est affiché.
  //
  // Pas de branche `bool` : une puce booléenne s'inverse au clic, elle n'ouvre
  // jamais d'éditeur.
  import { untrack } from "svelte";
  import NumberStepper from "$lib/components/ui/NumberStepper.svelte";
  import Slider from "$lib/components/ui/Slider.svelte";
  import { flagFor } from "$lib/flags.svelte";
  import Emblem from "$lib/components/ui/Emblem.svelte";
  import { logoOf } from "$lib/library/brandLogos.svelte";
  import { previewSrc } from "$lib/library/library";
  import { t } from "$lib/i18n/index.svelte";
  import { clampPerfPct, formatRatio, perfBand, PERF_MAX_PCT, PERF_MIN_PCT, PERF_STEP } from "$lib/detail/carSpecs";
  import { valueLabel, type FilterDef, type FilterOption, type FilterState, type PerfRef, type Sign } from "$lib/library/filters";

  interface Props {
    def: FilterDef;
    st: FilterState;
    /** Valeurs proposées, avec leur décompte (`val` seulement). */
    options: FilterOption[];
    /** Raccourcis de décennie (`range` seulement). */
    presets: { label: string; min: number; max: number }[];
    /** Voiture de référence de la bande de performance (`perf` seulement) et
     * nombre de voitures que leurs specs illisibles écartent d'office. */
    perfRef?: PerfRef | null;
    perfUnreadable?: number;
    onupdate: (next: FilterState) => void;
  }
  let { def, st, options, presets, perfRef = null, perfUnreadable = 0, onupdate }: Props = $props();

  // Bande de performance (CIBLE§3.4) : les bornes réelles en kg/bhp, pas seulement
  // le pourcentage. Un « ±15 % » ne dit pas ce qu'il laisse passer ; « 3,0 à
  // 4,1 kg/bhp » le dit, et c'est la seule façon de comprendre pourquoi une
  // voiture précise entre ou n'entre pas.
  const perfPct = $derived(st.type === "perf" ? clampPerfPct(st.pct) : 0);
  const perfBounds = $derived(perfRef?.ratio != null ? perfBand(perfRef.ratio, perfPct) : null);

  // « Rien de ce côté » côté NumberStepper : une sentinelle plutôt qu'un
  // `null`, `value` restant un `number` pour tous ses appelants (voir son
  // `emptyValue`). Le modèle de filtre, lui, dit `null`.
  const NO_YEAR = 0;
  const YEAR_START_MIN = 1950;
  const YEAR_START_MAX = new Date().getFullYear();

  let text = $state("");
  let inputEl = $state<HTMLInputElement | null>(null);
  let activeIndex = $state(0);

  /** Un `-` en tête bascule la saisie en exclusion — l'un des deux gestes qui
   * posent une exclusion sans passer par le survol d'une suggestion. */
  const excMode = $derived(text.trimStart().startsWith("-"));
  const needle = $derived(text.trim().replace(/^[-+]/, "").trim().toLowerCase());

  const posed = $derived(st.type === "val" ? st.values : []);
  /** Sign posed on each value, absent when the value is not in the filter.
   * A map rather than a scan: every suggestion row reads it. */
  const posedSign = $derived(new Map(posed.map((v) => [v.value, v.sign])));
  // The list keeps EVERY value matching the search, posed ones included.
  // Dropping a row the moment it was clicked pulled the next one up under the
  // cursor and lost the place being read - reported, and worse still on a long
  // scrolled list. A posed row shows its sign instead, and clicking the same
  // sign again takes it back.
  const shown = $derived(options.filter((o) => o.label.toLowerCase().includes(needle)));
  // L'index actif ne doit jamais dépasser une liste qui vient de rétrécir sous
  // la frappe : sans ça, Entrée valide `undefined` et ne fait rien.
  const cursor = $derived(Math.min(activeIndex, Math.max(shown.length - 1, 0)));

  /** Inclusions d'abord : c'est l'ordre dans lequel on relit ce qu'on a posé,
   * et il ne doit pas dépendre de l'ordre dans lequel on l'a posé. */
  const ordered = $derived(
    posed
      .map((v, i) => ({ ...v, i }))
      .sort((a, b) => b.sign - a.sign),
  );

  const defaultSign: Sign = $derived(def.defaultSign ?? 1);
  /** Ordre des deux boutons d'une suggestion : le geste le plus probable en
   * premier sous le curseur. « Contenu de base » se cherche presque toujours
   * en négatif, d'où `[−] [+]` quand la polarité par défaut est l'exclusion. */
  const signOrder: Sign[] = $derived(defaultSign < 0 ? [-1, 1] : [1, -1]);

  function setValues(values: { value: string; sign: Sign }[]) {
    if (st.type !== "val") return;
    onupdate({ type: "val", values, op: st.op });
  }

  /** Poses `value` with the given sign, flips it when it was posed the other
   * way, takes it back when the same sign is asked again. One path for the
   * three gestures: the row never moves, only its state changes. */
  function setSign(value: string, sign: Sign) {
    if (st.type !== "val") return;
    const at = st.values.findIndex((v) => v.value === value);
    if (at < 0) setValues([...st.values, { value, sign }]);
    else if (st.values[at].sign === sign) setValues(st.values.filter((_, i) => i !== at));
    else setValues(st.values.map((v, i) => (i === at ? { ...v, sign } : v)));
    inputEl?.focus();
  }

  function removeAt(i: number) {
    if (st.type !== "val") return;
    setValues(st.values.filter((_, idx) => idx !== i));
    inputEl?.focus();
  }

  function flipAt(i: number) {
    if (st.type !== "val") return;
    setValues(st.values.map((v, idx) => (idx === i ? { ...v, sign: (v.sign * -1) as Sign } : v)));
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      activeIndex = Math.min(cursor + 1, shown.length - 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      activeIndex = Math.max(cursor - 1, 0);
    } else if (e.key === "Enter") {
      const pick = shown[cursor];
      if (!pick) return;
      e.preventDefault();
      setSign(pick.value, e.altKey || excMode ? -1 : defaultSign);
      // Cleared HERE only: typing was the way this value was named, and the
      // next one gets typed too. A click is the opposite - the search text is
      // what the list is being read through, so nothing may touch it.
      text = "";
      activeIndex = 0;
    } else if (e.key === "Backspace" && text === "" && posed.length) {
      // Retirer le dernier jeton à la touche retour, comme un champ de
      // destinataires d'e-mail : le geste est acquis, il n'a pas à s'expliquer.
      removeAt(posed.length - 1);
    }
  }

  // Bornes tenues localement et poussées à chaque changement, plutôt que lues
  // depuis `st` à chaque rendu : `NumberStepper` écrit dans sa propre prop
  // `value`, et lui rendre une valeur qu'il vient d'écrire le ferait lutter
  // contre lui-même. L'éditeur est détruit à la fermeture du popover, donc
  // cette copie ne peut pas se désynchroniser — rien d'autre ne touche au
  // filtre pendant qu'il est ouvert.
  // `untrack` : la capture de la valeur initiale est voulue, pas un oubli.
  let yMin = $state(untrack(() => (st.type === "range" ? (st.min ?? NO_YEAR) : NO_YEAR)));
  let yMax = $state(untrack(() => (st.type === "range" ? (st.max ?? NO_YEAR) : NO_YEAR)));
  function pushRange() {
    onupdate({ type: "range", min: yMin === NO_YEAR ? null : yMin, max: yMax === NO_YEAR ? null : yMax });
  }
  function applyPreset(p: { min: number; max: number }) {
    yMin = p.min;
    yMax = p.max;
    pushRange();
  }
</script>

<div class="ed" class:wide={def.type === "val"}>
  <div class="head">{t(def.labelKey)}</div>

  {#if st.type === "val"}
    <!-- Le champ ne pose pas de valeur libre : les valeurs proposées sont
         **dérivées de la bibliothèque** (marques, auteurs, tags…), donc une
         valeur inventée ne remonterait aucun mod par construction. Il ne sert
         qu'à retrouver une valeur dans une longue liste. -->
    <input
      bind:this={inputEl}
      bind:value={text}
      class="find"
      type="text"
      autocomplete="off"
      spellcheck="false"
      placeholder={t("filters.searchValue")}
      title={t("filters.tokenInputHint")}
      oninput={() => (activeIndex = 0)}
      onkeydown={onKeydown}
    />

    <div class="sugg">
      {#each shown as opt, i (opt.value)}
        {@const held = posedSign.get(opt.value)}
        <div class="opt" class:on={i === cursor} class:posed={held !== undefined} class:neg={held !== undefined && held < 0}>
          <!-- Le drapeau est un repère, pas une information : le nom reste à
               côté, et une case vide tient la place quand Assetto Corsa ne
               connaît pas ce pays — sans elle, les noms ne s'aligneraient plus
               d'une ligne à l'autre. -->
          {#if def.flags}
            {@const flag = flagFor(opt.value)}
            {#if flag}<img class="flag" src={flag} alt="" />{:else}<span class="flag flag-none"></span>{/if}
          {:else if def.logos}
            <!-- The brand's logo at 18 px (TAXO§10), a disc keeping the place
                 when it has none - the names stay aligned. -->
            {@const logo = logoOf(opt.value)}
            {@const src = logo ? previewSrc(logo.path) : null}
            {#if logo && src}<Emblem {src} plaque={logo.plaque} size={18} />{:else}<span class="logo-none"></span>{/if}
          {/if}
          <span class="oname" title={opt.label}>{opt.label}</span>
          <span class="ocount mono">{opt.count}</span>
          <span class="acts">
            {#each signOrder as sign (sign)}
              <button
                type="button"
                class="a"
                class:minus={sign < 0}
                class:held={held === sign}
                title={held === sign
                  ? t("filters.tokenRemove")
                  : sign < 0
                    ? t("filters.tokenExclude")
                    : t("filters.tokenInclude")}
                onclick={() => setSign(opt.value, sign)}>{sign < 0 ? "−" : "+"}</button
              >
            {/each}
          </span>
        </div>
      {:else}
        <div class="none">{t("filters.tokenNoMatch")}</div>
      {/each}
    </div>

    <!-- Récapitulatif SOUS la liste, et c'est tout l'intérêt : au-dessus, il
         poussait le champ et les suggestions d'une rangée entière au premier
         jeton posé. Il porte ce que la liste ne montre pas — une valeur posée
         puis masquée par la recherche en cours reste lisible et reprenable ici. -->
    {#if ordered.length}
      <div class="toks">
        {#each ordered as tk (tk.value)}
          <span class="tok" class:neg={tk.sign < 0}>
            <button type="button" class="body" title={t("filters.tokenToggle")} onclick={() => flipAt(tk.i)}>
              <span class="sign" aria-hidden="true">{tk.sign < 0 ? "−" : "+"}</span>
              {#if def.flags}
                {@const flag = flagFor(tk.value)}
                {#if flag}<img class="flag" src={flag} alt="" />{/if}
              {:else if def.logos}
                {@const logo = logoOf(tk.value)}
                {@const src = logo ? previewSrc(logo.path) : null}
                {#if logo && src}<Emblem {src} plaque={logo.plaque} size={14} />{/if}
              {/if}
              <span class="label">{valueLabel(def, tk.value)}</span>
            </button>
            <button type="button" class="rm" title={t("filters.tokenRemove")} onclick={() => removeAt(tk.i)}>×</button>
          </span>
        {/each}
      </div>
    {/if}

    {#if def.operator}
      <!-- Le libellé n'est pas décoratif : sans lui, on croit que l'opérateur
           gouverne aussi les exclusions. -->
      <div class="grouplbl">{t("filters.opBetweenIncluded")}</div>
      <div class="opsel">
        <button type="button" class:on={st.op === "and"} onclick={() => onupdate({ ...st, op: "and" })}>
          {t("filters.opAndGloss")}
        </button>
        <button type="button" class:on={st.op === "or"} onclick={() => onupdate({ ...st, op: "or" })}>
          {t("filters.opOrGloss")}
        </button>
      </div>
    {/if}
    <div class="hint">
      {#if def.operator}{t("filters.hintExclusionsAlways")}<br />{/if}
      {t("filters.hintFlipToken")}
    </div>
  {:else if st.type === "range"}
    <!-- Bornes indépendamment vides : une borne vide ne borne rien. Sans ce
         repli, vider « année min » ramenait le plafond de « année max » à
         zéro. `emptyStart` donne à ▲ et ▼ le même point de départ depuis un
         champ vide, exactement comme si on tapait cette année. -->
    <div class="rng">
      <NumberStepper
        width={82}
        max={yMax === NO_YEAR ? undefined : yMax}
        emptyValue={NO_YEAR}
        emptyStart={YEAR_START_MIN}
        bind:value={yMin}
        onchange={pushRange}
      />
      <span class="dash">–</span>
      <NumberStepper
        width={82}
        min={yMin === NO_YEAR ? undefined : yMin}
        emptyValue={NO_YEAR}
        emptyStart={YEAR_START_MAX}
        bind:value={yMax}
        onchange={pushRange}
      />
    </div>
    {#if presets.length}
      <div class="presets">
        {#each presets as p (p.min)}
          <button type="button" class="preset" onclick={() => applyPreset(p)}>{p.label}</button>
        {/each}
      </div>
    {/if}
  {:else if st.type === "perf"}
    <!-- Sans référence lisible, la bande ne peut pas être tracée : le dire,
         plutôt que d'afficher un curseur qui ne filtrerait rien en silence. -->
    {#if perfRef?.ratio == null}
      <p class="perf-none">
        {perfRef ? t("library.perfRefUnreadable", { name: perfRef.name }) : t("library.perfNoRef")}
      </p>
    {:else}
      <Slider
        compact
        label={t("library.perfTolerance")}
        min={PERF_MIN_PCT}
        max={PERF_MAX_PCT}
        step={PERF_STEP}
        value={perfPct}
        display={`±${perfPct}%`}
        oninput={(v) => onupdate({ type: "perf", pct: clampPerfPct(v) })}
      />
      <p class="perf-ref">
        {t("library.perfReference", {
          name: perfRef.name,
          bhp: perfRef.bhp ?? 0,
          kg: perfRef.kg ?? 0,
          ratio: formatRatio(perfRef.ratio),
        })}
      </p>
      {#if perfBounds}
        <p class="perf-band mono">{formatRatio(perfBounds.min)} – {formatRatio(perfBounds.max)} kg/bhp</p>
      {/if}
    {/if}
    {#if perfUnreadable > 0}
      <p class="perf-out">{t("library.perfUnreadable", { count: perfUnreadable })}</p>
    {/if}
  {:else if st.type === "text"}
    <input
      bind:this={inputEl}
      class="find solo"
      type="text"
      autocomplete="off"
      placeholder={t("library.filterDescriptionPlaceholder")}
      value={st.text}
      oninput={(e) => onupdate({ type: "text", text: e.currentTarget.value })}
    />
  {/if}
</div>

<style>
  /* Trois registres, trois poids : le réglage, ce sur quoi il porte, et ce
     qu'il laisse dehors. Sans cet écart les trois lignes se lisent comme un
     paragraphe et on ne trouve plus les bornes. */
  .perf-ref,
  .perf-none {
    margin: 8px 0 0;
    font-size: 11px;
    color: var(--muted);
    line-height: 1.45;
  }
  .perf-band {
    margin: 4px 0 0;
    font-size: 12px;
    color: var(--txt);
  }
  /* Jaune, pas rouge : une voiture sans specs lisibles n'est pas une erreur,
     c'est une donnée qui manque — même couleur que `.warnbox`, qui porte déjà
     l'alerte non bloquante de l'app. */
  .perf-out {
    margin: 8px 0 0;
    font-size: 11px;
    color: var(--yellow);
    line-height: 1.45;
  }

  .ed {
    display: flex;
    flex-direction: column;
  }
  .wide {
    width: 294px;
  }
  .head {
    font-size: 9.5px;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--muted);
    padding: 2px 2px 8px;
  }
  .toks {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-top: 9px;
  }
  .tok {
    display: inline-flex;
    align-items: center;
    height: 24px;
    border: 1px solid var(--rosso-border);
    background: var(--rosso-dim);
    font-size: 11.5px;
  }
  .tok .body {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: none;
    border: 0;
    color: var(--txt);
    font: inherit;
    padding: 0 4px 0 8px;
    cursor: pointer;
    max-width: 190px;
  }
  .tok .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tok .sign {
    color: var(--rosso-bright);
  }
  .tok .rm {
    background: none;
    border: 0;
    color: var(--muted2);
    font-size: 12px;
    padding: 0 5px 0 2px;
    cursor: pointer;
    align-self: stretch;
  }
  .tok .rm:hover {
    color: var(--txt);
  }
  /* Une exclusion n'a pas de couleur à elle : elle se dit par la rature et par
     le mot. Le jaune reste réservé aux vraies alertes, et le rouge à l'action
     destructive — les charger d'un second sens les viderait du premier. */
  .tok.neg {
    border-color: var(--line);
    background: var(--panel2);
  }
  .tok.neg .body {
    color: var(--muted);
    text-decoration: line-through;
    text-decoration-color: var(--faint2);
  }
  .tok.neg .sign {
    color: var(--muted2);
    text-decoration: none;
  }

  .find {
    width: 100%;
    height: 30px;
    background: var(--panel2);
    border: 1px solid var(--line);
    color: var(--txt);
    padding: 0 9px;
    /* Pas d'`outline: 0` : l'anneau jaune de `global.css` est le repère de
       focus de toute l'app, et le masquer ici obligeait à le remplacer par
       un filet rouge — que le barème §7.2ter n'autorise pas sur un champ. */
    font: inherit;
    font-size: 12.5px;
  }
  .find:focus {
    border-color: var(--faint);
  }
  .solo {
    margin-bottom: 2px;
  }

  .sugg {
    max-height: 178px;
    overflow-y: auto;
    margin-top: 6px;
  }
  .opt {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 28px;
    padding: 0 4px 0 8px;
    font-size: 12.5px;
    color: var(--txt2);
  }
  .opt.on,
  .opt:hover {
    background: var(--raised);
    color: var(--txt);
  }
  .oname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ocount {
    margin-left: auto;
    font-size: 10.5px;
    color: var(--faint);
  }
  /* Les deux boutons n'apparaissent qu'au survol ou sur la ligne active :
     affichés en permanence, ils font une colonne de signes qui masque la
     valeur, qui est la seule chose qu'on lit dans cette liste. */
  .acts {
    display: none;
    gap: 3px;
  }
  .opt.on .acts,
  .opt:hover .acts {
    display: flex;
  }
  /* Une valeur déjà posée garde ses deux boutons visibles en permanence : ils
     ne sont plus une action offerte au survol mais l'état de la ligne, et un
     état ne se lit pas seulement sous le curseur. */
  .opt.posed .acts {
    display: flex;
  }
  .a {
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--line);
    background: none;
    color: var(--muted);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .a:hover {
    border-color: var(--rosso-border);
    background: var(--rosso-dim);
    color: var(--rosso-bright);
  }
  .a.minus:hover {
    border-color: var(--faint2);
    background: var(--raised);
    color: var(--txt);
  }
  /* Le bouton du signe en cours est allé chercher l'apparence de son propre
     survol : c'est le même rouge éteint que la puce retenue, et il dit « c'est
     celui-là » sans introduire une troisième couleur. */
  .a.held {
    border-color: var(--rosso-border);
    background: var(--rosso-dim);
    color: var(--rosso-bright);
  }
  .a.minus.held {
    border-color: var(--faint2);
    background: var(--raised);
    color: var(--txt);
  }
  /* Même langage que le jeton du récapitulatif : une exclusion se dit par la
     rature et par le mot, jamais par une couleur à elle. */
  .opt.neg .oname {
    color: var(--muted);
    text-decoration: line-through;
    text-decoration-color: var(--faint2);
  }
  .none {
    font-size: 11.5px;
    color: var(--faint);
    padding: 6px 8px;
  }
  /* 4:3 comme les PNG du jeu, et un filet : beaucoup de drapeaux ont du blanc
     sur un bord, qui se fondrait dans la ligne. Mêmes valeurs que la colonne
     Nationalité du plateau — c'est la même table, à la même taille. */
  .flag {
    flex: none;
    width: 16px;
    height: 12px;
    object-fit: cover;
    border: 1px solid var(--line);
  }
  /* Place tenue quand le pays n'est pas dans la table du jeu : sans elle, les
     noms cesseraient de s'aligner d'une ligne à l'autre. */
  .flag-none {
    border-style: dashed;
  }
  .logo-none {
    flex: none;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--raised);
  }

  .grouplbl {
    font-size: 9px;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--faint);
    margin: 11px 2px 5px;
  }
  .opsel {
    display: flex;
    border: 1px solid var(--line);
  }
  .opsel button {
    flex: 1;
    background: var(--panel2);
    border: 0;
    border-right: 1px solid var(--line);
    color: var(--muted);
    height: 28px;
    cursor: pointer;
    font: inherit;
    font-size: 11px;
  }
  .opsel button:last-child {
    border-right: 0;
  }
  .opsel button.on {
    background: var(--raised);
    color: var(--txt);
  }
  .hint {
    font-size: 10.5px;
    color: var(--muted2);
    line-height: 1.55;
    margin-top: 8px;
  }

  .rng {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .dash {
    color: var(--muted2);
  }
  .presets {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-top: 9px;
    max-width: 260px;
  }
  .preset {
    font-size: 11px;
    border: 1px solid var(--line);
    background: none;
    color: var(--muted);
    padding: 3px 8px;
    cursor: pointer;
    font-family: inherit;
  }
  .preset:hover {
    border-color: var(--rosso-border);
    color: var(--txt);
  }
</style>
