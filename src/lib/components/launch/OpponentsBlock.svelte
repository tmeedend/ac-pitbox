<script lang="ts">
  // Le **vivier** d'adversaires : à qui la grille a le droit de piocher, combien
  // de voitures, et le caractère de la course.
  //
  // **Le filtre a remplacé les onglets.** `Même voiture` / `Par catégorie` /
  // `Libre` faisaient deux choses à la fois — définir un ensemble de voitures,
  // ce que la barre de filtres fait déjà et mieux, et engendrer un plateau à
  // partir de lui. Seule la seconde les justifiait, et le doublon de la première
  // est ce qui obligeait aux règles de réconciliation. Ce bloc porte donc la
  // **même** barre de filtres que la bibliothèque, plus trois puces qui posent
  // un jeton et rien d'autre (`opponentPool.ts`).
  //
  // **Le filtre définit le vivier, jamais le plateau.** Entre les deux il y a
  // toujours un geste, et il y en a exactement deux, travaillant sur le même
  // ensemble filtré : `Tirer N au hasard` **remplace** le plateau, et
  // `Choisir dans le vivier` — qui vit avec le plateau, en dessous — **ajoute**.
  //
  // Le plateau lui-même est **la suite de cette page** (`GridBlock`) et non un
  // cadre imbriqué : le générateur est en haut, sa sortie en dessous, et rien
  // ne les sépare (lot 5 §5.1). Une gouttière entre les deux rendait invisibles
  // trois liens réels — la bannière de vivier explique le contenu du plateau,
  // `Tirer au hasard` et `Régénérer` font des choses voisines, et la colonne
  // « Force » réagit à un curseur hors de vue.
  import type { CardIndex, FilterDef, FilterMap } from "$lib/filters";
  import { chipAvailable, isChipOn, toggleChip, type ChipKind } from "$lib/opponentPool";
  import {
    AGGRESSION_MAX,
    AGGRESSION_MIN,
    AI_LEVEL_MAX,
    AI_LEVEL_MIN,
    type RaceSetup,
  } from "$lib/launch";
  import type { ModCard } from "$lib/library";
  import { t } from "$lib/i18n/index.svelte";
  import FilterBar from "../filters/FilterBar.svelte";
  import Tooltip from "../Tooltip.svelte";
  import NumberStepper from "../NumberStepper.svelte";
  import CenterSpread from "../CenterSpread.svelte";

  let {
    setup,
    opponentCount,
    defs,
    filters = $bindable(),
    pinned = $bindable(),
    query = $bindable(),
    index,
    poolCount,
    playerCard,
    oncountchange,
    onfill,
  }: {
    setup: RaceSetup;
    opponentCount: number;
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
</script>

<!-- Ni carte ni en-tête : le titre de l'écran dit déjà « Course · Adversaires »,
     et un second cadre autour du générateur le détacherait du plateau qu'il
     alimente. -->
<div class="oppo-setup">

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
    <!-- Libellé AU-DESSUS, comme les deux réglages voisins : trois contrôles
         d'une même rangée avec deux placements de libellé se lisent en zigzag,
         et c'est ce qui donnait à la rangée son air désaligné. -->
    <label class="field">
      <span class="fk lbl-key">{t("launch.aiCount")}</span>
      <NumberStepper min={0} max={30} value={opponentCount} onchange={(v) => oncountchange(v)} />
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

  <!-- A statement, not a block: the grid is still playable, so no red (§3.5). -->
  {#if poolCount > 0 && poolCount < opponentCount}
    <p class="warnbox thin">
      {poolCount === 1 ? t("launch.poolThinOne") : t("launch.poolThinFew", { count: poolCount })}
    </p>
  {:else if poolCount === 0}
    <p class="warnbox thin">{t("launch.poolEmpty")}</p>
  {/if}
</div>

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
  /* `.warnbox` carries the colours; only the spacing is local. */
  .thin {
    margin-top: 10px;
  }
  /* Compteur d'adversaires, tirage et difficulté : tout sur une ligne (retombe
     seulement si la largeur manque). */
  /* `flex-end` et non `flex-start` : ces réglages ne portent pas le même
     nombre de lignes au-dessus de leur contrôle — un libellé seul pour le
     compteur, un libellé **et** la valeur lue pour les deux fourchettes. Alignés
     par le haut, le compteur et le bouton flottaient donc une soixantaine de
     pixels au-dessus des curseurs. **Ce que l'œil aligne, c'est la rangée de
     contrôles**, pas le coin supérieur des blocs — même leçon que le bloc
     Simulation, qui l'avait déjà apprise sur ses cases à cocher. */
  .adv-row {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 16px 20px;
    margin: 13px 0 0;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  /* Couleur/taille/interlettrage viennent de `.lbl-key` (global, harmonisation
     §chantier libellés) : ne reste ici que ce que `.lbl-key` ne couvre pas. */
  .fk {
    text-transform: uppercase;
  }

</style>
