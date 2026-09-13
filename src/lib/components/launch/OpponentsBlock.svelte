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
  import { formatRatio } from "$lib/carSpecs";
  import type { CardIndex, FilterDef, FilterMap } from "$lib/filters";
  import { chipAvailable, isChipOn, toggleChip, type ChipKind } from "$lib/opponentPool";
  import {
    AGGRESSION_MAX,
    AGGRESSION_MIN,
    AGGRESSION_STEP,
    AI_LEVEL_MAX,
    AI_LEVEL_MIN,
    type Opponent,
    type RaceSetup,
    type SkinItem,
  } from "$lib/launch";
  import { previewSrc, type ModCard } from "$lib/library";
  import { t } from "$lib/i18n/index.svelte";
  import FilterBar from "../filters/FilterBar.svelte";
  import ColumnsMenu from "../ColumnsMenu.svelte";
  import Tooltip from "../Tooltip.svelte";
  import NumberStepper from "../NumberStepper.svelte";
  import CenterSpread from "../CenterSpread.svelte";

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
    columns = $bindable(),
    wide = $bindable(),
    oncountchange,
    onfill,
    onchoose,
    onregenerate,
    onremove,
    onduplicate,
    onsetlevel,
    onsetcell,
    onsavegrid,
    onloadgrid,
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
    /** Colonnes optionnelles affichées, et plateau élargi (§4.2/§4.3). */
    columns: string[];
    wide: boolean;
    oncountchange: (n: number) => void;
    onfill: () => void;
    onchoose: () => void;
    onregenerate: () => void;
    onremove: (index: number) => void;
    onduplicate: (index: number) => void;
    onsetlevel: (index: number, level: number | null) => void;
    onsetcell: (index: number, patch: Partial<Opponent>) => void;
    onsavegrid: () => void;
    onloadgrid: () => void;
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

  // Bornes de la force d'une ligne du plateau : celles du curseur de Content
  // Manager (`launch.ts`). La fourchette elle-même est passée en centre ± écart
  // et vit dans `CenterSpread` (§2.9).
  const RANGE_MIN = AI_LEVEL_MIN;
  const RANGE_MAX = AI_LEVEL_MAX;
  // --- Columns (§4.2) ------------------------------------------------------
  //
  // The car is fixed — it IS the row. The other six are a preference, and they
  // go through the same menu as the library's table view: same gesture, same
  // component (`ColumnsMenu`).
  //
  // **No "how many fit" note.** §4.3 asks the menu to say how many more columns
  // the current width takes, which means measuring the grid in pixels — and a
  // pixel read back into a layout is precisely what the interface zoom breaks
  // (§13). What the width does instead is squeeze the name column, which says
  // the same thing without a number and without a legend: `⤢` gives the room
  // back. Worth revisiting with a measurement whose zoom behaviour has been
  // checked in the app.
  const COLUMNS = $derived([
    { key: "car", label: t("columns.name"), fixed: true },
    { key: "ratio", label: t("launch.colRatio") },
    { key: "strength", label: t("launch.colStrength") },
    { key: "driver", label: t("launch.colDriver") },
    { key: "nationality", label: t("launch.colNationality") },
    { key: "ballast", label: t("launch.colBallast") },
    { key: "restrictor", label: t("launch.colRestrictor") },
  ]);
  const shows = (key: string) => columns.includes(key);
  function toggleColumn(key: string) {
    columns = columns.includes(key) ? columns.filter((k) => k !== key) : [...columns, key];
  }

  /** kg/bhp d'un adversaire, `—` quand sa fiche est illisible — jamais estimé.
   * Lu dans l'index du vivier, donc analysé une fois par chargement de liste et
   * pas une fois par ligne rendue. */
  function opponentRatio(carId: string): string {
    const card = carPool.find((c) => c.id_interne === carId);
    return formatRatio(card ? index.ctx.ratioOf(card) : null);
  }

  /** Une saisie vidée rend la cellule à `Auto` (§4.1). */
  const orAuto = (v: string): string | null => (v.trim() === "" ? null : v.trim());

  /** La livrée d'une ligne, quand elle est connue. */
  function skinOf(opp: Opponent): SkinItem | undefined {
    return opp.car_skin ? skinsByCarId[opp.car_id]?.find((sk) => sk.id === opp.car_skin) : undefined;
  }

  /** Ce qu'une cellule `Auto` vaut **réellement** : ce que la livrée déclare,
   * et donc ce que le jeu mettra. Afficher un « Auto » creux là où la donnée
   * existe cachait au lecteur le nom qui allait s'afficher en course — y
   * compris quand deux lignes portaient le même. */
  function autoDriver(opp: Opponent): string {
    const sk = skinOf(opp);
    const number = sk?.number ? `${sk.number} ` : "";
    return sk?.driver ? `${number}${sk.driver}` : t("launch.autoCell");
  }
  function autoNationality(opp: Opponent): string {
    return skinOf(opp)?.country ?? t("launch.autoCell");
  }

  /** Deux pilotes sous la même identité (§1.9). La génération l'évite ; ceci
   * n'attrape que ce que l'utilisateur a forcé à la main, et le dit plutôt que
   * de le corriger dans son dos. */
  const duplicateDrivers = $derived.by(() => {
    const seen = new Set<string>();
    for (const opp of setup.opponents) {
      const sk = skinOf(opp);
      const key = `${opp.driver_name ?? sk?.number ?? ""}|${opp.driver_name ?? sk?.driver ?? ""}`.trim().toLowerCase();
      if (key === "|") continue;
      if (seen.has(key)) return true;
      seen.add(key);
    }
    return false;
  });

  /** Lest et bride : 0 à 100, jamais d'`Auto` — « rien » s'y dit par 0. */
  const clamp100 = (v: string): number => Math.max(0, Math.min(100, Math.round(Number(v) || 0)));

  /** Les quatre colonnes optionnelles portent des valeurs qu'aucune ligne ne
   * nomme d'elle-même : dès que l'une est là, l'en-tête devient nécessaire. */
  const headerNeeded = $derived(
    columns.some((k) => k === "driver" || k === "nationality" || k === "ballast" || k === "restrictor"),
  );

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
<section class="blk oppo-blk">
  <header class="blk-h"><span class="blk-t">{t("launch.opponentsLabel")}</span></header>
  <div class="blk-b">
  <!-- Tout ce qui n'est pas la grille reste à sa largeur de repos, même en
       mode élargi : un champ de recherche et trois puces étalés sur 1200 px
       ne gagnent rien, et la barre de filtres doit rester le même objet que
       dans la bibliothèque (§2.3). -->
  <div class="head-cap" class:capped={wide}>

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

    <CenterSpread
      label={t("launch.aiRangeLabel")}
      center={setup.ai_level}
      spread={setup.ai_spread}
      min={AI_LEVEL_MIN}
      max={AI_LEVEL_MAX}
      onchange={(c, sp) => {
        setup.ai_level = c;
        setup.ai_spread = sp;
      }}
    >
      {#snippet info()}
        <!-- L'explication permanente vit dans le ⓘ, jamais dans un encart
             jaune : celui-ci est réservé à ce qui appelle une action. -->
        <Tooltip text={t("launch.explicitStrengthNote")} align="left"
          ><button type="button" class="info-i">ⓘ</button></Tooltip
        >
      {/snippet}
    </CenterSpread>

    <!-- Même composant, mêmes gestes : ce sont les deux réglages qui décident
         du caractère de la course, ils ne peuvent pas se manipuler autrement
         l'un que l'autre. -->
    <CenterSpread
      label={t("launch.aggressionLabel")}
      center={setup.aggression}
      spread={setup.aggression_spread}
      min={AGGRESSION_MIN}
      max={AGGRESSION_MAX}
      onchange={(c, sp) => {
        setup.aggression = c;
        setup.aggression_spread = sp;
      }}
    />
  </div>

  <!-- A configuration problem calling for an action, so yellow is right here —
       unlike the strength explanation, which moved to an ⓘ. -->
  {#if duplicateDrivers}
    <p class="warnbox thin">{t("launch.duplicateDrivers")}</p>
  {/if}

  <!-- A statement, not a block: the grid is still playable, so no red (§3.5). -->
  {#if poolCount > 0 && poolCount < opponentCount}
    <p class="warnbox thin">
      {poolCount === 1 ? t("launch.poolThinOne") : t("launch.poolThinFew", { count: poolCount })}
    </p>
  {:else if poolCount === 0}
    <p class="warnbox thin">{t("launch.poolEmpty")}</p>
  {/if}
  </div>

  <div class="oppo" class:wide>
    <!-- `Regenerate` is NOT `Fill` with another name: it keeps the cars and
         re-rolls what was drawn on them (skin, strength), where `Fill` draws
         the cars themselves. Two gestures one actually wants separately — the
         grid is right but the liveries repeat, versus the grid is wrong. -->
    <div class="oppo-h lbl">
      <span>{t("launch.gridHeader", { count: setup.opponents.length })}</span>
      <!-- La position de départ a rejoint SESSION OPTIONS (§2.5) : elle dépend
           du type de session, et tout ce qui en dépend vit là-bas. -->
      <span class="oppo-sp"></span>
      <ColumnsMenu size="header" items={COLUMNS} visible={columns} ontoggle={toggleColumn} />
      <button
        class="oppo-regen"
        type="button"
        aria-pressed={wide}
        class:on={wide}
        title={t("launch.widenGrid")}
        onclick={() => (wide = !wide)}>⤢</button
      >
      <button class="oppo-regen" type="button" disabled={!setup.opponents.length} onclick={onregenerate}
        >{t("launch.regenerateGrid")}</button
      >
    </div>

    <!-- En-tête de colonnes seulement quand il y a plus que la voiture à
         nommer : sur trois colonnes, la ligne se lit sans légende. -->
    {#if headerNeeded}
      <div class="oppo-row oppo-th">
        <span class="oppo-img th-img"></span>
        <!-- Read-only header in the dimmer grey, editable ones in the lighter:
             two greys, no third level, and the row says what can be typed into
             before one tries. -->
        <span class="oppo-n lbl-key ro">{t("columns.name")}</span>
        {#if shows("driver")}<span class="oppo-driver lbl-key">{t("launch.colDriver")}</span>{/if}
        <!-- Abréviations levées en mode élargi (§2.4) : la place gagnée sert
             d'abord à nommer les colonnes en entier. `Restrictor` y reprend le
             mot exact de la carte voiture du joueur, pour que le lien entre
             les deux réglages se voie. -->
        {#if shows("nationality")}<span class="oppo-nat lbl-key">{wide ? t("launch.colNationality") : t("launch.colNatShort")}</span>{/if}
        {#if shows("ratio")}<span class="oppo-ratio lbl-key">{t("launch.colRatio")}</span>{/if}
        {#if shows("strength")}<span class="oppo-force lbl-key">{wide ? t("launch.colStrength") : t("launch.colStrShort")}</span>{/if}
        {#if shows("ballast")}<span class="oppo-bal lbl-key">{t("launch.colBallast")}</span>{/if}
        {#if shows("restrictor")}<span class="oppo-res lbl-key">{wide ? t("launch.colRestrictor") : t("launch.colResShort")}</span>{/if}
        <span class="th-act"></span>
      </div>
    {/if}
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
        {#if shows("driver")}
          <!-- Vide = `Auto` : le nom vient alors du `ui_skin.json` de la livrée,
               ce que le jeu fait déjà. Rien à voir avec les mods de tenue de
               pilote, qui sont une apparence 3D sur leur propre axe. -->
          <input
            class="oppo-driver cell"
            class:is-auto={opp.driver_name == null}
            type="text"
            placeholder={autoDriver(opp)}
            value={opp.driver_name ?? ""}
            onclick={(e) => e.stopPropagation()}
            onchange={(e) => onsetcell(i, { driver_name: orAuto(e.currentTarget.value) })}
          />
        {/if}
        {#if shows("nationality")}
          <!-- Un nom de pays anglais entier, jamais un code : c'est ce que CM
               écrit, relevé sur un preset réel (« Brunei Darussalam »). -->
          <input
            class="oppo-nat cell mono"
            class:is-auto={opp.nationality == null}
            type="text"
            placeholder={autoNationality(opp)}
            value={opp.nationality ?? ""}
            onclick={(e) => e.stopPropagation()}
            onchange={(e) => onsetcell(i, { nationality: orAuto(e.currentTarget.value) })}
          />
        {/if}
        {#if shows("ratio")}
          <span class="oppo-ratio mono">{opponentRatio(opp.car_id)}</span>
        {/if}
        {#if shows("strength")}
        <!-- `Auto` is not a value we draw: it is the absence of an override,
             and the game draws inside the global range. Hence a placeholder
             rather than a number — emptying the field is the gesture that puts
             the cell back to `Auto`, which is why no menu is needed here. -->
        <input
          class="oppo-force mono"
          class:is-auto={opp.ai_level == null}
          type="number"
          min={RANGE_MIN}
          max={RANGE_MAX}
          placeholder={t("launch.autoCell")}
          value={opp.ai_level ?? ""}
          title={t("launch.opponentLevelTooltip")}
          onclick={(e) => e.stopPropagation()}
          onchange={(e) => onsetlevel(i, e.currentTarget.value.trim() === "" ? null : Number(e.currentTarget.value))}
        />
        {/if}
        {#if shows("ballast")}
          <input
            class="oppo-bal cell mono"
            class:is-zero={!opp.ballast}
            type="number"
            min="0"
            max="100"
            value={opp.ballast}
            onclick={(e) => e.stopPropagation()}
            onchange={(e) => onsetcell(i, { ballast: clamp100(e.currentTarget.value) })}
          />
        {/if}
        {#if shows("restrictor")}
          <input
            class="oppo-res cell mono"
            class:is-zero={!opp.restrictor}
            type="number"
            min="0"
            max="100"
            value={opp.restrictor}
            onclick={(e) => e.stopPropagation()}
            onchange={(e) => onsetcell(i, { restrictor: clamp100(e.currentTarget.value) })}
          />
        {/if}
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
      >{poolCount === 1 ? t("launch.chooseFromPoolOne") : t("launch.chooseFromPool", { count: poolCount })}</button
    >
    <!-- A grid is worth saving on its own, apart from the session that holds
         it: the same GT3 field on ten tracks (§5). -->
    <div class="oppo-foot">
      <button class="oppo-regen" type="button" disabled={!setup.opponents.length} onclick={onsavegrid}
        >{t("launch.saveGrid")}</button
      >
      <button class="oppo-regen" type="button" onclick={onloadgrid}>{t("launch.loadGrid")}</button>
    </div>
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
  /* Même ⓘ que SIMULATION : l'explication permanente vit là, jamais dans un
     encart jaune. */
  .info-i {
    background: transparent;
    border: none;
    padding: 0 0 0 4px;
    color: var(--muted2);
    font-size: 10px;
    line-height: 1;
  }
  .info-i:hover {
    color: var(--txt2);
  }
  /* Le second des deux gris, et il n'y en a pas de troisième : une colonne en
     lecture seule s'annonce plus éteinte que celles qui se saisissent. */
  .oppo-th .ro {
    color: var(--faint);
  }
  /* 600 px : la largeur que le bloc reçoit réellement en mode normal, et celle
     sur laquelle le plateau a été dessiné. */
  .head-cap.capped {
    max-width: 600px;
  }
  .oppo-foot {
    display: flex;
    gap: 7px;
    padding: 7px 10px;
    border-top: 1px solid var(--line);
  }
  /* Pushes the start position and `Regenerate` to the right of the title. */
  .oppo-sp {
    flex: 1;
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
  .oppo-regen:hover:not(:disabled) {
    background: var(--panel2);
    color: var(--txt2);
  }
  .oppo-regen:disabled {
    color: var(--faint2);
    cursor: not-allowed;
  }
  /* Niveau 2 du barème (§7.2ter) : le plateau élargi est un état qu'on a
     demandé, et le bouton le dit. */
  .oppo-regen.on {
    border-color: var(--rosso-border);
    background: var(--rosso-dim);
    color: var(--rosso-bright);
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
  /* Plafonnée en mode élargi (§2.4) : sans ce cap, la largeur gagnée allait
     toute au nom, et l'écart entre lui et `Driver name` devenait assez grand
     pour qu'on perde la ligne en la parcourant des yeux. Ce qu'on est venu
     chercher en élargissant, ce sont des colonnes de plus, pas une colonne
     plus large. */
  .oppo-n {
    font-size: 10.5px;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .oppo.wide .oppo-n {
    flex: 0 1 300px;
  }
  /* La place restante va au vide en fin de ligne plutôt qu'à une colonne
     arbitraire : chaque cellule garde la largeur qui la rend lisible. */
  .oppo.wide .oppo-row::after {
    content: "";
    flex: 1;
  }
  .oppo.wide .oppo-driver {
    width: 150px;
  }
  .oppo.wide .oppo-nat {
    width: 110px;
  }
  .oppo.wide .oppo-bal,
  .oppo.wide .oppo-res {
    width: 72px;
  }
  .oppo-skin {
    color: var(--muted);
  }
  /* Toutes les cellules optionnelles ont une largeur FIXE et le nom prend ce
     qui reste : ajouter une colonne serre donc le nom, et c'est `⤢` qui lui
     rend sa place. Aucune ne s'étire, sans quoi l'alignement d'une colonne à
     l'autre se perdrait d'une ligne à la suivante. */
  .oppo-ratio {
    width: 52px;
    flex: none;
    font-size: 9px;
    color: var(--muted);
    text-align: right;
  }
  .oppo-driver {
    width: 104px;
  }
  .oppo-nat {
    width: 74px;
  }
  .oppo-bal,
  .oppo-res {
    width: 44px;
    text-align: right;
  }
  /* Les quatre champs de cellule partagent la même discrétion que la force :
     pas de cadre au repos, il apparaît au survol de la ligne — sans quoi
     chaque ligne devient un formulaire. */
  .cell {
    background: transparent;
    border: 1px solid transparent;
    color: var(--txt);
    font-size: 9.5px;
    padding: 2px 3px;
    flex: none;
    min-width: 0;
    appearance: textfield;
  }
  .oppo-row:hover .cell {
    background: var(--bg);
    border-color: var(--line);
  }
  .cell:focus {
    background: var(--bg);
    border-color: var(--line);
  }
  .cell::-webkit-outer-spin-button,
  .cell::-webkit-inner-spin-button {
    appearance: none;
    margin: 0;
  }
  /* `Auto` et 0 se lisent comme « non décidé » et « rien » : éteints tous les
     deux, mais le premier est un texte de substitution et le second une vraie
     valeur — d'où deux règles et non une. */
  .cell.is-auto::placeholder {
    color: var(--faint);
  }
  .cell.is-zero {
    color: var(--faint);
  }
  /* Rangée d'en-tête : une ligne du plateau sans ses gestes. */
  .oppo-th {
    cursor: default;
    background: var(--bg);
    padding-top: 3px;
    padding-bottom: 3px;
  }
  .oppo-th:hover {
    background: var(--bg);
  }
  .th-img {
    width: 48px;
    border: 0;
    background: transparent;
    height: auto;
  }
  .th-act {
    width: 44px;
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
  /* Dimmed like any `Auto` cell — the placeholder is what shows, and it must
     read as "not decided", not as a value. */
  .oppo-force.is-auto::placeholder {
    color: var(--faint);
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

  /* Bulle de survol : la même image, en grand.
     **Elle déborde vers le BAS, jamais vers le haut.** Elle remontait depuis le
     haut de la ligne et recouvrait donc les lignes précédentes — précisément
     celles qu'on est en train de comparer à celle qu'on survole. Ancrée sur le
     bord supérieur de sa ligne, elle ne cache que ce qui suit, qu'on n'a pas
     encore lu.
     Le repli en fin de liste est un `margin-bottom` négatif sur la dernière
     ligne : la seule façon de remonter la bulle « du strict nécessaire » sans
     relever une position à l'écran, donc sans repasser par des pixels que le
     zoom d'interface multiplierait une seconde fois (§13).
     Aucune transition sous `prefers-reduced-motion` (règle globale). */
  .oppo-bubble {
    position: absolute;
    left: 56px;
    top: 0;
    z-index: 20;
    width: 240px;
    max-height: 240px;
    overflow: hidden;
    padding: 4px;
    border: 1px solid var(--line);
    background: var(--bg);
    box-shadow: 0 12px 34px rgb(0 0 0 / 60%);
    opacity: 0;
    visibility: hidden;
    transition: opacity 0.1s;
  }
  /* Les deux dernières lignes n'ont pas 240 px sous elles : la bulle y remonte
     pour rester dans le bloc plutôt que de le déborder. */
  .oppo-row:nth-last-of-type(-n + 2) .oppo-bubble {
    top: auto;
    bottom: 0;
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
</style>
