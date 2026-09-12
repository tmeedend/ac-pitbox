<script lang="ts">
  // Bloc « Adversaires » de l'écran Lancement (§8.6/§8.6ter) : mode de
  // plateau, fourchette d'année du vivier, liste générée (avec picker de
  // réglage fin), fourchette de niveau IA. Vue de présentation : la
  // génération du plateau (poolForMode/generateOpponents/regenerateGrid,
  // cache de skins) reste dans Launch.svelte, qui la déclenche aussi depuis
  // d'autres sources (presets, resynchronisation voiture/circuit) — ce bloc
  // ne fait qu'afficher le résultat et notifier les actions locales
  // (ajouter/dupliquer/retirer une ligne, régler un niveau, ouvrir le picker).
  import { SAME_CATEGORY, type GridMode, type Opponent, type RaceSetup, type SkinItem } from "$lib/launch";
  import { previewSrc, type ModCard } from "$lib/library";
  import { t } from "$lib/i18n/index.svelte";
  import NumberStepper from "../NumberStepper.svelte";
  import OpponentPicker from "../OpponentPicker.svelte";
  import Seg from "../Seg.svelte";

  let {
    setup,
    gridMode,
    opponentCount,
    carPool,
    skinsByCarId,
    categorySelection,
    categoryOptions,
    pickerPool,
    pickerIndex,
    onselectmode,
    onselectcategory,
    oncountchange,
    onremove,
    onadd,
    onduplicate,
    onsetlevel,
    onopenpicker,
    onclosepicker,
    onconfirmpicker,
    onregenerate,
  }: {
    setup: RaceSetup;
    gridMode: GridMode;
    opponentCount: number;
    carPool: ModCard[];
    skinsByCarId: Record<string, SkinItem[]>;
    categorySelection: string;
    categoryOptions: string[];
    pickerPool: ModCard[];
    pickerIndex: number | null;
    onselectmode: (mode: GridMode) => void;
    onselectcategory: (category: string) => void;
    oncountchange: (n: number) => void;
    onremove: (index: number) => void;
    onadd: () => void;
    onduplicate: (index: number) => void;
    onsetlevel: (index: number, level: number) => void;
    onopenpicker: (index: number) => void;
    onclosepicker: () => void;
    onconfirmpicker: (carId: string, skinId: string | null) => void;
    onregenerate: () => void;
  } = $props();

  const gridModes = $derived([
    { value: "same_car", label: t("launch.gridSameCar") },
    { value: "same_category", label: t("launch.gridSameCategory") },
    { value: "free", label: t("launch.gridFree") },
  ]);

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
  // Les deux valeurs sont accrochées à leur poignée (§9.3) : posées aux
  // extrémités de la piste, elles disaient la fourchette sans dire laquelle
  // des deux poignées on était en train de bouger.
  //
  // Sous 20 points d'écart, les deux libellés ne tiennent plus côte à côte sur
  // une piste de cette largeur — et à cette distance les poignées elles-mêmes
  // ne se distinguent plus, donc les séparer n'apprendrait rien. Une seule
  // valeur alors, la fourchette d'un bout à l'autre. C'est le cas du réglage
  // par défaut (92-98).
  //
  // Le placement est en **pourcentage de la piste**, jamais en pixels relevés
  // à l'écran — un `getBoundingClientRect` rendrait des pixels de fenêtre déjà
  // multipliés par le zoom d'interface, qu'un `left` en pixels CSS
  // multiplierait une seconde fois (§13). Le débordement au ras des bords est
  // rattrapé en CSS par un `clamp()`, qui connaît la largeur réelle de la
  // piste là où ce fichier ne la connaît pas.
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

<!-- Adversaires (Course uniquement, §8.6) -->
<section class="blk">
  <header class="blk-h"><span class="blk-t">{t("launch.opponentsLabel")}</span></header>
  <div class="blk-b">
  <!-- Segmenté partagé : ces trois modes avaient leur propre copie, avec ses
       divs cliquables et son marquage de l'actif — lequel se trouvait être
       déjà le bon (rouge éteint + filet), le seul de l'écran à l'être. -->
  <div class="modes">
    <Seg value={gridMode} onselect={(v) => onselectmode(v as GridMode)} items={gridModes} />
  </div>

  <!-- Catégorie du vivier, nombre d'adversaires, difficulté et fourchette
       d'année, tout sur une ligne (§8.6). Catégorie : par défaut « Même
       catégorie » (en tête de liste) suit automatiquement la voiture pilotée,
       comportement d'origine ; une catégorie fixée à la main reste choisie
       même si on change de voiture. Année min/max : 0 ou vide = pas de filtre
       sur ce bord (`inYearRange` côté Launch.svelte) — remplace l'ancienne
       double glissière, ces deux champs se tapent directement. -->
  <div class="adv-row">
    {#if gridMode === "same_category"}
      <label class="cat-field">
        <span class="fk lbl-key">{t("launch.gridCategoryLabel")}</span>
        <select
          class="input cat-select"
          value={categorySelection}
          onchange={(e) => onselectcategory(e.currentTarget.value)}
        >
          <option value={SAME_CATEGORY}>{t("launch.gridCategorySame")}</option>
          {#each categoryOptions as cat}<option value={cat}>{cat}</option>{/each}
        </select>
      </label>
    {/if}

    <label class="grid-fields">
      <NumberStepper min={0} max={30} value={opponentCount} onchange={(v) => oncountchange(v)} />
      <span class="fk lbl-key">{t("launch.aiCount")}</span>
    </label>

    <div class="ai-range-field">
      <span class="fk lbl-key">{t("launch.aiRangeLabel")}</span>
      <div class="dual-range">
        <div class="dr-track"></div>
        <div class="dr-fill" style="left:{aiMinPct}%; right:{100 - aiMaxPct}%"></div>
        <!-- Les mots « min » et « max » ont quitté les libellés : la position
             de la poignée le dit déjà, et ils doublaient la largeur d'une
             valeur là où c'est précisément la largeur qui manque. Ils restent
             sur les curseurs comme nom accessible, qui n'en a pas d'autre. -->
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

    {#if gridMode !== "same_car"}
      <!-- Pas de plafond à l'année courante : un vivier peut légitimement
         viser une voiture concept ou un DLC annoncé pas encore sorti. -->
      <label class="grid-fields">
        <NumberStepper width={70} min={0} bind:value={setup.year_min} />
        <span class="fk lbl-key">{t("launch.yearMinLabel")}</span>
      </label>
      <label class="grid-fields">
        <NumberStepper width={70} min={0} bind:value={setup.year_max} />
        <span class="fk lbl-key">{t("launch.yearMaxLabel")}</span>
      </label>
    {/if}
  </div>

  <div class="oppo">
    <!-- « généré » devenait faux dès qu'une ligne avait été posée à la main.
         L'espace laissé libre entre le titre et le bouton est celui de la
         position de départ (L3) : il est tenu vide exprès. -->
    <div class="oppo-h lbl">
      <span>{t("launch.gridHeader", { count: setup.opponents.length })}</span>
      <button class="oppo-regen" type="button" onclick={onregenerate}>{t("launch.regenerateGrid")}</button>
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
    margin-bottom: 12px;
  }
  /* Même patron que `.ai-range-field` : libellé au-dessus, largeur fixe pour
     tenir dans la ligne plutôt que de s'étirer sur toute la largeur restante. */
  .cat-field {
    display: flex;
    flex-direction: column;
    gap: 5px;
    width: 150px;
  }
  .cat-select {
    width: 100%;
  }
  /* Catégorie (si « Par catégorie »), compteur d'adversaires, difficulté,
     fourchette d'année : tout sur une ligne (retombe seulement si la largeur
     manque). */
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
