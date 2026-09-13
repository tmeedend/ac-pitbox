<script lang="ts">
  // Opponents block of the session settings screen (§3).
  //
  // **The filter replaced the tabs.** `Same car` / `By category` / `Free` did
  // two jobs at once: define a set of cars — which the filter bar already does,
  // better — and generate a grid out of that set. Only the second justified
  // them, and the duplication of the first is what forced the reconciliation
  // rules ("removing a chip does not change the tab", "the grid goes manual").
  // The block now carries the SAME filter bar as the library, plus three
  // shortcut chips that pose a token and nothing else (`opponentPool.ts`).
  //
  // **The filter defines the pool, never the grid.** Between the two there is
  // always a gesture, and there are exactly two, both working on the filtered
  // set: `Fill N at random` REPLACES the grid — the path of whoever wants to
  // drive now — and `Choose from the N…` opens the picker on that same filter
  // and ADDS at the end — the path of whoever wants to decide. The second
  // carries the pool count in its label, because that is what says what the
  // filter bought: 42 rows to read instead of 600.
  //
  // Presentation only: generation (`generateOpponents`, the skin cache) stays
  // in `Launch.svelte`, which triggers it from other sources too (presets, the
  // library's "set as opponents"). This block shows the result and reports the
  // local gestures.
  import type { CardIndex, FilterDef, FilterMap } from "$lib/filters";
  import { chipAvailable, isChipOn, toggleChip, type ChipKind } from "$lib/opponentPool";
  import type { Opponent, RaceSetup, SkinItem } from "$lib/launch";
  import { previewSrc, type ModCard } from "$lib/library";
  import { t } from "$lib/i18n/index.svelte";
  import FilterBar from "../filters/FilterBar.svelte";
  import NumberStepper from "../NumberStepper.svelte";

  let {
    setup,
    opponentCount,
    carPool,
    skinsByCarId,
    defs,
    filters = $bindable(),
    pinned = $bindable(),
    query = $bindable(),
    index,
    poolCount,
    playerCard,
    oncountchange,
    onfill,
    onchoose,
    onregenerate,
    onremove,
    onduplicate,
    onsetlevel,
    onopenpicker,
  }: {
    setup: RaceSetup;
    opponentCount: number;
    carPool: ModCard[];
    skinsByCarId: Record<string, SkinItem[]>;
    defs: FilterDef[];
    filters: FilterMap;
    pinned: string[];
    query: string;
    index: CardIndex;
    /** Distinct cars the filter keeps — the number both gestures work on. */
    poolCount: number;
    /** The car being driven: what the three chips take their value from. */
    playerCard: ModCard | null;
    oncountchange: (n: number) => void;
    onfill: () => void;
    onchoose: () => void;
    onregenerate: () => void;
    onremove: (index: number) => void;
    onduplicate: (index: number) => void;
    onsetlevel: (index: number, level: number) => void;
    onopenpicker: (index: number) => void;
  } = $props();

  const CHIPS: { kind: ChipKind; labelKey: string }[] = [
    { kind: "model", labelKey: "launch.chipSameCar" },
    { kind: "category", labelKey: "launch.chipSameCategory" },
    { kind: "performance", labelKey: "launch.chipSamePerformance" },
  ];
  const refRatio = $derived(index.ctx.perfRef?.ratio ?? null);

  /** Why a chip cannot be clicked. Three causes, three sentences: "no car
   * chosen" and "this car declares no category" are not the same problem, and
   * one of them the user can do something about. */
  function chipReason(kind: ChipKind): string {
    if (!playerCard) return t("launch.chipNoCar");
    if (kind === "performance") return t("launch.chipNoPerf");
    return t("launch.chipNoCategory");
  }

  // --- AI level range (two handles, §8.6) ---
  const RANGE_MIN = 60;
  const RANGE_MAX = 100;
  function clampAiMin() {
    if (setup.ai_level_min > setup.ai_level_max) setup.ai_level_min = setup.ai_level_max;
  }
  function clampAiMax() {
    if (setup.ai_level_max < setup.ai_level_min) setup.ai_level_max = setup.ai_level_min;
  }
  const aiMinPct = $derived(((setup.ai_level_min - RANGE_MIN) / (RANGE_MAX - RANGE_MIN)) * 100);
  const aiMaxPct = $derived(((setup.ai_level_max - RANGE_MIN) / (RANGE_MAX - RANGE_MIN)) * 100);
  // Both values hang off their own handle (§9.3): laid at the ends of the
  // track, they said the range without saying which handle one was dragging.
  //
  // Under 20 points apart the two labels no longer fit side by side on a track
  // this wide — and at that distance the handles themselves stop being
  // distinguishable, so separating the labels would teach nothing. One label
  // then, the range end to end. That is the default setting (92-98).
  //
  // Placement is a PERCENTAGE OF THE TRACK, never a measurement taken off the
  // screen: a `getBoundingClientRect` would return window pixels already
  // multiplied by the interface zoom, which a `left` in CSS pixels would
  // multiply a second time (§13). The overflow at the very edges is caught in
  // CSS by a `clamp()`, which knows the real width of the track where this
  // file does not.
  const aiLabelsMerged = $derived(aiMaxPct - aiMinPct < 20);

  function opponentName(carId: string): string {
    return carPool.find((c) => c.id_interne === carId)?.display_name ?? carId;
  }
  /** Vignette de l'adversaire : `preview.jpg` du skin d'abord, `livery.png`
   * seulement en dernier recours.
   *
   * C'est l'inverse de l'ordre du sélecteur de livrée de la barre latérale, et
   * l'inversion est **locale à ce composant** : les deux ne posent pas la même
   * question. Là-bas c'est « quelle peinture ? », et une pastille de couleur y
   * répond ; ici c'est « quelle voiture ? », et quatre pastilles de couleur n'y
   * répondent pas. La preview montre la voiture *portant* le skin, donc elle
   * répond aux deux à la fois — y compris pour deux adversaires « même
   * voiture », qui doivent rester distinguables. */
  function opponentPreview(opp: Opponent): string | null {
    const skin = opp.car_skin ? skinsByCarId[opp.car_id]?.find((s) => s.id === opp.car_skin) : null;
    const modPreview = carPool.find((c) => c.id_interne === opp.car_id)?.preview ?? null;
    return previewSrc(skin?.preview ?? modPreview ?? skin?.livery ?? null);
  }
  function opponentSkinName(opp: Opponent): string | undefined {
    return opp.car_skin ? skinsByCarId[opp.car_id]?.find((s) => s.id === opp.car_skin)?.name : undefined;
  }
</script>

<!-- Opponents (race and track day only, §3) -->
<section class="blk">
  <header class="blk-h"><span class="blk-t">{t("launch.opponentsLabel")}</span></header>
  <div class="blk-b">

  <!-- The library's own filter bar, third consumer. The chips row below is not
       a second way of filtering: it poses tokens INTO this bar, which is why a
       chip lights up when the matching token is posed by hand. -->
  <FilterBar
    {defs}
    bind:filters
    bind:pinned
    bind:query
    optionsFor={index.optionsFor}
    presets={index.yearPresets}
    resultCount={poolCount}
    countKey="launch.poolCount"
    perfRef={index.ctx.perfRef}
    perfUnreadable={index.perfUnreadable}
  />

  <!-- Three shortcuts, not three modes. They exist for the one-click gesture
       and for what tokens alone do not offer: reading the pool at a glance. -->
  <div class="chips">
    {#each CHIPS as chip (chip.kind)}
      {@const on = isChipOn(filters, chip.kind, playerCard)}
      {@const can = chipAvailable(chip.kind, playerCard, refRatio)}
      <!-- `aria-disabled`, not `disabled`: §3.4 asks the chip to EXPLAIN why it
           cannot be clicked, and a disabled button fires no mouse event, so its
           tooltip never appears and the keyboard cannot reach it either. It
           stays focusable and says its reason; the click is what is refused. -->
      <button
        type="button"
        class="chip"
        class:on
        class:off={!can}
        aria-disabled={!can}
        title={can ? undefined : chipReason(chip.kind)}
        onclick={() => can && (filters = toggleChip(filters, chip.kind, playerCard))}>{t(chip.labelKey)}</button
      >
    {/each}
  </div>

  <!-- How many, the random draw, and the difficulty the draw spreads over. -->
  <div class="adv-row">
    <label class="grid-fields">
      <NumberStepper min={0} max={30} value={opponentCount} onchange={(v) => oncountchange(v)} />
      <span class="fk lbl-key">{t("launch.aiCount")}</span>
    </label>

    <!-- The only red of the block, and it is level 2 of the scale (§7.2ter):
         filled red stays on the launch button alone. -->
    <button class="fill" type="button" disabled={poolCount === 0 || opponentCount === 0} onclick={onfill}
      >{t("launch.fillAtRandom", { count: opponentCount })}</button
    >

    <div class="ai-range-field">
      <span class="fk lbl-key">{t("launch.aiRangeLabel")}</span>
      <div class="dual-range">
        <div class="dr-track"></div>
        <div class="dr-fill" style="left:{aiMinPct}%; right:{100 - aiMaxPct}%"></div>
        <!-- « min » et « max » ont quitté les libellés : la position de la
             poignée le dit déjà, et ils doublaient la largeur d'une valeur là
             où c'est précisément la largeur qui manque. Ils restent sur les
             curseurs comme nom accessible, qui n'en a pas d'autre. -->
        <input
          type="range"
          min={RANGE_MIN}
          max={RANGE_MAX}
          aria-label={t("launch.aiMin", { level: setup.ai_level_min })}
          bind:value={setup.ai_level_min}
          oninput={clampAiMin}
        />
        <input
          type="range"
          min={RANGE_MIN}
          max={RANGE_MAX}
          aria-label={t("launch.aiMax", { level: setup.ai_level_max })}
          bind:value={setup.ai_level_max}
          oninput={clampAiMax}
        />
        {#if aiLabelsMerged}
          <span class="dr-v pair mono" style="--at:{(aiMinPct + aiMaxPct) / 2}%"
            >{setup.ai_level_min}–{setup.ai_level_max}%</span
          >
        {:else}
          <span class="dr-v mono" style="--at:{aiMinPct}%">{setup.ai_level_min}%</span>
          <span class="dr-v mono" style="--at:{aiMaxPct}%">{setup.ai_level_max}%</span>
        {/if}
      </div>
    </div>
  </div>

  <!-- A statement, not a block: the grid is still playable, so no red (§3.5). -->
  {#if poolCount > 0 && poolCount < opponentCount}
    <p class="warnbox thin">
      {poolCount === 1 ? t("launch.poolThinOne") : t("launch.poolThinFew", { count: poolCount })}
    </p>
  {:else if poolCount === 0}
    <p class="warnbox thin">{t("launch.poolEmpty")}</p>
  {/if}

  <div class="oppo">
    <!-- `Regenerate` is NOT `Fill` with another name: it keeps the cars and
         re-rolls what was drawn on them (skin, strength), where `Fill` draws
         the cars themselves. Two gestures one actually wants separately — the
         grid is right but the liveries repeat, versus the grid is wrong. -->
    <div class="oppo-h lbl">
      <span>{t("launch.gridHeader", { count: setup.opponents.length })}</span>
      <button class="oppo-regen" type="button" disabled={!setup.opponents.length} onclick={onregenerate}
        >{t("launch.regenerateGrid")}</button
      >
    </div>
    {#each setup.opponents as opp, i}
      {@const prev = opponentPreview(opp)}
      <div
        class="oppo-row"
        role="button"
        tabindex="0"
        title={t("launch.opponentEditTooltip")}
        onclick={() => onopenpicker(i)}
        onkeydown={(e) => (e.key === "Enter" || e.key === " ") && onopenpicker(i)}
      >
        <div class="oppo-img">{#if prev}<img src={prev} alt="" />{:else}<span class="mono">🏎</span>{/if}</div>
        <span class="oppo-n">{opponentName(opp.car_id)}{#if opponentSkinName(opp)}<span class="oppo-skin"> · {opponentSkinName(opp)}</span>{/if}</span>
        <!-- Rapport poids/puissance (L3) : colonne tenue vide pour que rien ne
             se déplace quand elle se remplira. -->
        <span class="oppo-ratio"></span>
        <input
          class="oppo-force mono"
          type="number"
          min={RANGE_MIN}
          max={RANGE_MAX}
          value={opp.ai_level}
          title={t("launch.opponentLevelTooltip")}
          onclick={(e) => e.stopPropagation()}
          onchange={(e) => onsetlevel(i, Number(e.currentTarget.value))}
        />
        <button
          class="oppo-dup"
          type="button"
          title={t("launch.opponentDuplicateTooltip")}
          onclick={(e) => { e.stopPropagation(); onduplicate(i); }}
        >+</button>
        <button class="oppo-x" type="button" title={t("common.remove")} onclick={(e) => { e.stopPropagation(); onremove(i); }}>✕</button>
        <!-- La vignette de ligne dit quelle voiture ; en grand, elle dit
             laquelle exactement. Le JPEG est déjà sur disque et déjà chargé
             par la vignette : aucun appel backend, jamais l'aperçu 3D.
             Positionnement **absolu** dans la ligne, pas `fixed` : une bulle
             fixe se place à partir d'un `getBoundingClientRect`, donc des
             pixels de fenêtre déjà multipliés par le zoom d'interface (§13) —
             et il faudrait la refermer au défilement de chaque ancêtre. Ici
             elle suit le contenu toute seule. -->
        {#if prev}
          <span class="oppo-bubble"><img src={prev} alt="" /></span>
        {/if}
      </div>
    {/each}
    <!-- The pool count in the label is the point: it says what the filter
         bought — this many rows to read instead of the whole library. -->
    <button class="oppo-add" type="button" disabled={poolCount === 0} onclick={onchoose}
      >{t("launch.chooseFromPool", { count: poolCount })}</button
    >
  </div>
  </div>
</section>


<style>
  /* Three shortcuts under the token row, close enough to read as its
     complement rather than as a control bar of their own. */
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 9px;
  }
  /* Level 2 of the red scale when active (§7.2ter): red text, dimmed ground,
     border — never a filled red, which stays the launch button's alone. A chip
     at rest introduces no red, and neither does hovering it. */
  .chip {
    border: 1px solid var(--line);
    background: var(--panel2);
    color: var(--txt2);
    font-size: 10.5px;
    padding: 4px 9px;
  }
  .chip:hover:not(.off) {
    background: var(--raised);
    color: var(--txt);
  }
  .chip.on {
    border-color: var(--rosso-border);
    background: var(--rosso-dim);
    color: var(--rosso-bright);
  }
  .chip.off {
    color: var(--faint);
    cursor: not-allowed;
  }
  /* The draw. Same level 2 as an active chip: it is the gesture the block is
     built around, not a destructive action. */
  .fill {
    border: 1px solid var(--rosso-border);
    background: var(--rosso-dim);
    color: var(--rosso-bright);
    font-size: 9.5px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    padding: 6px 12px;
    white-space: nowrap;
  }
  .fill:hover:not(:disabled) {
    border-color: var(--rosso-bright);
    color: var(--rosso-bright);
  }
  .fill:disabled {
    border-color: var(--line);
    background: transparent;
    color: var(--faint);
    cursor: not-allowed;
  }
  /* `.warnbox` carries the colours; only the spacing is local. */
  .thin {
    margin-top: 10px;
  }
  /* Compteur d'adversaires, tirage et difficulté : tout sur une ligne (retombe
     seulement si la largeur manque). */
  .adv-row {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: 16px 20px;
    margin: 13px 0 0;
  }
  .ai-range-field {
    display: flex;
    flex-direction: column;
    gap: 5px;
    /* Largeur fixe pour tenir à côté du compteur et des champs année, plutôt
       que de s'étirer sur toute la largeur restante comme sur son ancien
       emplacement en pleine rubrique. */
    width: 170px;
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
  .oppo {
    border: 1px solid var(--line);
    margin-top: 12px;
  }
  /* Couleur/taille/interlettrage/majuscules viennent de `.lbl` (global,
     harmonisation §chantier libellés) : ne reste ici que le fond en bandeau
     et l'annulation de la marge basse (`.lbl` en prévoit une pour une
     rubrique de carte, pas pour un bandeau suivi directement des lignes). */
  .oppo-h {
    background: var(--raised);
    padding: 6px 10px;
    margin-bottom: 0;
    /* `.lbl` est déjà en flex : reste à écarter les deux bouts. L'espace du
       milieu attend la position de départ (L3). */
    justify-content: space-between;
    gap: 10px;
  }
  /* Bouton secondaire neutre : régénérer le plateau n'est pas ce qui est
     retenu pour la session, donc pas de rouge (§7.2ter). L'action existait
     déjà comme effet de bord d'un changement de voiture — sans aucun moyen de
     la demander. */
  .oppo-regen {
    background: transparent;
    border: 1px solid var(--line);
    color: var(--muted);
    font-size: 8.5px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    padding: 3px 8px;
  }
  .oppo-regen:hover {
    background: var(--panel2);
    color: var(--txt2);
  }
  /* `relative` porte la bulle de survol, et son absence ne se voit pas comme
     un défaut de style : un enfant `absolute` se cale sur le premier ancêtre
     positionné, ici le conteneur de défilement de tout l'écran — la bulle
     partait donc en bas de la page, hors champ, et le survol semblait n'avoir
     aucun effet. */
  .oppo-row {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 6px 10px;
    border-top: 1px solid var(--line);
    background: var(--panel2);
    cursor: pointer;
    position: relative;
  }
  .oppo-row:hover {
    background: var(--raised);
  }
  /* 16:9, le format de `preview.jpg`. La ligne ne grandit que de quelques
     pixels et la surface double : à 34x22 on voyait une couleur, pas une
     voiture. Recadrage plutôt que dézoom — le cadrage Kunos est constant et la
     plupart des mods le reprennent, la voiture est donc toujours au même
     endroit dans l'image. */
  .oppo-img {
    width: 48px;
    height: 27px;
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
    object-position: center 60%;
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
    color: var(--muted);
  }
  /* Vide jusqu'au L3 (rapport poids/puissance). Largeur d'un « 412 ch/t » en
     mono 9px, pour que le nom ne se réétale pas le jour où elle se remplit. */
  .oppo-ratio {
    width: 46px;
    flex: none;
  }
  /* Blanche, et sans cadre au repos : le vert était la seule occurrence de
     cette couleur dans un contrôle, et un cadre permanent faisait de chaque
     ligne un formulaire. Le cadre apparaît au survol de la ligne — c'est là
     qu'il faut savoir que la valeur s'édite, pas avant. */
  .oppo-force {
    width: 34px;
    height: 20px;
    background: transparent;
    border: 1px solid transparent;
    color: var(--txt);
    font-size: 9px;
    text-align: center;
    flex: none;
    appearance: textfield;
  }
  .oppo-row:hover .oppo-force {
    background: var(--bg);
    border-color: var(--line);
  }
  .oppo-force::-webkit-outer-spin-button,
  .oppo-force::-webkit-inner-spin-button {
    appearance: none;
    margin: 0;
  }
  /* Le focus reste jaune, celui de `global.css` : le rouge qu'il portait ici
     ne se rattache à aucun niveau du barème (§7.2ter). */
  .oppo-force:focus {
    background: var(--bg);
    border-color: var(--line);
  }
  .oppo-dup {
    background: transparent;
    color: var(--muted2);
    font-size: 13px;
    line-height: 1;
    padding: 2px 5px;
    flex: none;
  }
  .oppo-dup:hover {
    background: transparent;
    color: var(--txt);
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
  /* Action secondaire : aucun niveau du barème ne couvre un libellé rouge
     (§7.2ter), et le rouge de cet écran doit rester au bouton de lancement. */
  .oppo-add {
    background: var(--panel2);
    padding: 7px 10px;
    border-top: 1px solid var(--line);
    color: var(--txt2);
    font-size: 9.5px;
    text-align: left;
    width: 100%;
  }
  .oppo-add:hover {
    background: var(--raised);
  }

  /* Fourchettes (année + IA, deux curseurs) */
  .dual-range {
    position: relative;
    height: 28px;
    /* Les valeurs vivent au-dessus de la piste, dans la place que leur ancienne
       ligne occupait en dessous. */
    margin-top: 12px;
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
  /* Bulle de survol : la même image, en grand. Aucune transition sous
     `prefers-reduced-motion` (règle globale de `global.css`). */
  .oppo-bubble {
    position: absolute;
    left: 56px;
    bottom: calc(100% - 10px);
    z-index: 20;
    width: 240px;
    padding: 4px;
    border: 1px solid var(--line);
    background: var(--bg);
    box-shadow: 0 12px 34px rgb(0 0 0 / 60%);
    opacity: 0;
    visibility: hidden;
    transition: opacity 0.1s;
  }
  .oppo-row:hover .oppo-bubble,
  .oppo-row:focus-visible .oppo-bubble {
    opacity: 1;
    visibility: visible;
  }
  .oppo-bubble img {
    display: block;
    width: 100%;
    height: auto;
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
    width: 10px;
    height: 20px;
    border-radius: 2px;
    background: var(--rosso);
    border: 2px solid var(--panel);
    cursor: pointer;
    margin-top: 4px;
  }
  /* Accrochée à sa poignée, au-dessus de la piste. `--at` est un pourcentage
     de la piste, pas une mesure relevée à l'écran : rien à diviser par le zoom
     (§13).
     Le `clamp` retient la valeur dans la piste quand la poignée arrive au bord
     — sans lui, la valeur maximale d'une fourchette haute passait par-dessus
     le champ voisin (constaté à l'écran, « année min » recouvert). Il vaut une
     demi-largeur de libellé, la seule mesure que le CSS connaisse ici et que
     le composant ignore. */
  .dr-v {
    --pad: 16px;
    position: absolute;
    bottom: calc(100% - 4px);
    left: clamp(var(--pad), var(--at), calc(100% - var(--pad)));
    transform: translateX(-50%);
    white-space: nowrap;
    font-size: 8.5px;
    color: var(--txt2);
    pointer-events: none;
  }
  .dr-v.pair {
    --pad: 26px;
  }
</style>
