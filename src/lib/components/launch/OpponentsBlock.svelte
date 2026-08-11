<script lang="ts">
  // Bloc « Adversaires » de l'écran Lancement (§8.6/§8.6ter) : mode de
  // plateau, fourchette d'année du vivier, liste générée (avec picker de
  // réglage fin), fourchette de niveau IA. Vue de présentation : la
  // génération du plateau (poolForMode/generateOpponents/regenerateGrid,
  // cache de skins) reste dans Launch.svelte, qui la déclenche aussi depuis
  // d'autres sources (presets, resynchronisation voiture/circuit) — ce bloc
  // ne fait qu'afficher le résultat et notifier les actions locales
  // (ajouter/dupliquer/retirer une ligne, régler un niveau, ouvrir le picker).
  import type { GridMode, Opponent, RaceSetup, SkinItem } from "$lib/launch";
  import { previewSrc, type ModCard } from "$lib/library";
  import { t } from "$lib/i18n/index.svelte";
  import NumberStepper from "../NumberStepper.svelte";
  import OpponentPicker from "../OpponentPicker.svelte";

  let {
    setup,
    gridMode,
    opponentCount,
    carPool,
    skinsByCarId,
    yearRangeMax,
    pickerPool,
    pickerIndex,
    onselectmode,
    oncountchange,
    onremove,
    onadd,
    onduplicate,
    onsetlevel,
    onopenpicker,
    onclosepicker,
    onconfirmpicker,
  }: {
    setup: RaceSetup;
    gridMode: GridMode;
    opponentCount: number;
    carPool: ModCard[];
    skinsByCarId: Record<string, SkinItem[]>;
    yearRangeMax: number;
    pickerPool: ModCard[];
    pickerIndex: number | null;
    onselectmode: (mode: GridMode) => void;
    oncountchange: (n: number) => void;
    onremove: (index: number) => void;
    onadd: () => void;
    onduplicate: (index: number) => void;
    onsetlevel: (index: number, level: number) => void;
    onopenpicker: (index: number) => void;
    onclosepicker: () => void;
    onconfirmpicker: (carId: string, skinId: string | null) => void;
  } = $props();

  const gridModes: { id: GridMode; labelKey: string }[] = [
    { id: "same_car", labelKey: "launch.gridSameCar" },
    { id: "same_category", labelKey: "launch.gridSameCategory" },
    { id: "free", labelKey: "launch.gridFree" },
  ];

  // --- Fourchette de niveau IA (deux curseurs, §8.6) ---
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
</script>

<!-- Adversaires (Course uniquement, §8.6) -->
<section class="blk">
  <header class="blk-h"><span class="blk-t">{t("launch.opponentsLabel")}</span></header>
  <div class="blk-b">
  <div class="modes">
    {#each gridModes as m}
      <button class="mode" class:on={gridMode === m.id} type="button" onclick={() => onselectmode(m.id)}>
        <div class="mt">{t(m.labelKey)}</div>
      </button>
    {/each}
  </div>

  <!-- Nombre d'adversaires, difficulté et fourchette d'année du vivier, tout
       sur une ligne (§8.6). Année min/max : 0 ou vide = pas de filtre sur ce
       bord (`inYearRange` côté Launch.svelte) — remplace l'ancienne double
       glissière, ces deux champs se tapent directement. -->
  <div class="adv-row">
    <label class="grid-fields">
      <NumberStepper min={0} max={30} value={opponentCount} onchange={(v) => oncountchange(v)} />
      <span class="fk lbl-key">{t("launch.aiCount")}</span>
    </label>

    <div class="ai-range-field">
      <span class="fk lbl-key">{t("launch.aiRangeLabel")}</span>
      <div class="dual-range">
        <div class="dr-track"></div>
        <div class="dr-fill" style="left:{aiMinPct}%; right:{100 - aiMaxPct}%"></div>
        <input type="range" min={RANGE_MIN} max={RANGE_MAX} bind:value={setup.ai_level_min} oninput={clampAiMin} />
        <input type="range" min={RANGE_MIN} max={RANGE_MAX} bind:value={setup.ai_level_max} oninput={clampAiMax} />
      </div>
      <div class="dr-vals mono">
        <span>{t("launch.aiMin", { level: setup.ai_level_min })}</span>
        <span>{t("launch.aiMax", { level: setup.ai_level_max })}</span>
      </div>
    </div>

    {#if gridMode !== "same_car"}
      <label class="grid-fields">
        <NumberStepper width={70} min={0} max={yearRangeMax} bind:value={setup.year_min} />
        <span class="fk lbl-key">{t("launch.yearMinLabel")}</span>
      </label>
      <label class="grid-fields">
        <NumberStepper width={70} min={0} max={yearRangeMax} bind:value={setup.year_max} />
        <span class="fk lbl-key">{t("launch.yearMaxLabel")}</span>
      </label>
    {/if}
  </div>

  <div class="oppo">
    <div class="oppo-h lbl">{t("launch.gridGenerated", { count: setup.opponents.length })}</div>
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
      </div>
    {/each}
    <button class="oppo-add" type="button" onclick={onadd}>+ {t("launch.addOpponent")}</button>
  </div>
  </div>
</section>

{#if pickerIndex != null}
  <OpponentPicker
    pool={pickerPool}
    currentCarId={setup.opponents[pickerIndex].car_id}
    currentSkinId={setup.opponents[pickerIndex].car_skin}
    onpick={onconfirmpicker}
    onclose={onclosepicker}
  />
{/if}

<style>
  .modes {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 1px;
    background: var(--line);
    border: 1px solid var(--line);
    margin-bottom: 12px;
  }
  /* Compteur d'adversaires, difficulté, fourchette d'année : tout sur une
     ligne (retombe seulement si la largeur manque). */
  .adv-row {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: 16px 20px;
    margin-bottom: 12px;
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
  .ai-range-field .dr-vals {
    font-size: 8.5px;
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
  /* Même taille que .seg button (Practice/Hotlap/Course, Launch.svelte) et
     .seg-v button (Faux départ/Grip, Launch.svelte) : ce sont le même rôle —
     le libellé d'une option cliquable — qui n'a aucune raison de changer de
     taille selon l'écran. */
  .mode .mt {
    font-size: 11px;
    color: var(--txt2);
  }
  .mode.on .mt {
    color: var(--rosso-bright);
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
    color: var(--muted);
  }
  .oppo-force {
    width: 34px;
    height: 20px;
    background: var(--bg);
    border: 1px solid var(--line);
    color: var(--green);
    font-size: 9px;
    text-align: center;
    flex: none;
    appearance: textfield;
  }
  .oppo-force::-webkit-outer-spin-button,
  .oppo-force::-webkit-inner-spin-button {
    appearance: none;
    margin: 0;
  }
  .oppo-force:hover,
  .oppo-force:focus {
    border-color: var(--rosso-border);
    outline: none;
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

  /* Fourchettes (année + IA, deux curseurs) */
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
    width: 10px;
    height: 20px;
    border-radius: 2px;
    background: var(--rosso);
    border: 2px solid var(--panel);
    cursor: pointer;
    margin-top: 4px;
  }
  .dr-vals {
    display: flex;
    justify-content: space-between;
    font-size: 9.5px;
    color: var(--txt2);
    margin-top: 4px;
  }
  /* `.dr-vals` sert aux deux fourchettes : année (3 spans, avec séparateur)
     et niveau IA (2 spans, sans — la sienne a été retirée). `:not(:last-child)`
     évite qu'à 2 spans la règle n'atteigne le max au lieu d'un séparateur
     disparu. */
  .dr-vals span:nth-child(2):not(:last-child) {
    color: var(--muted);
  }
</style>
