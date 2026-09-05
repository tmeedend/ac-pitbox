<script lang="ts">
  // Barre de filtres de la bibliothèque (§6.3).
  //
  // **Une seule ligne** : recherche, « + Filtre », puces et « Tout effacer »
  // dans une coulée qui passe à la ligne, plus un bloc calé à droite pour le
  // décompte et ce que l'écran appelant y range. Chaque filtre posé est une
  // puce ; son éditeur vit dans un popover accroché à elle, jamais dans la
  // barre. C'est ce qui fait qu'un filtre à jetons multiples avec opérateur ne
  // coûte pas un pixel de plus qu'une case à cocher — l'ancienne barre
  // affichait onze contrôles en permanence, sur environ 200 px de hauteur,
  // pour quelqu'un qui en emploie un ou deux.
  //
  // Deux règles gouvernent tous les gestes :
  //   — **le clic modifie, la croix retire**, sans exception ni type de puce
  //     particulier. Une puce booléenne s'inverse indéfiniment au clic et ne
  //     s'évapore jamais au troisième ;
  //   — **la croix d'une puce épinglée la ramène à l'état fantôme** au lieu de
  //     la faire disparaître. Même geste, deux résultats, mais les deux
  //     répondent à « annule ce que je viens de faire ».
  import { tick, type Snippet } from "svelte";
  import AnchoredPopover from "./AnchoredPopover.svelte";
  import FilterAddMenu from "./FilterAddMenu.svelte";
  import FilterEditor from "./FilterEditor.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import {
    blankState,
    chipSummary,
    ariaSummary,
    isBlank,
    type FilterDef,
    type FilterMap,
    type FilterOption,
    type FilterState,
    type Sign,
  } from "$lib/filters";

  interface Props {
    defs: FilterDef[];
    filters: FilterMap;
    pinned: string[];
    query: string;
    /** Valeurs proposées pour un filtre `val`, décompte compris. */
    optionsFor: (key: string) => FilterOption[];
    /** Raccourcis de décennie du filtre d'année, déduits de la bibliothèque. */
    presets: { label: string; min: number; max: number }[];
    resultCount: number;
    /** Ce que l'écran range dans le bloc de droite (colonnes, vue). */
    end?: Snippet;
  }
  let {
    defs,
    filters = $bindable(),
    pinned = $bindable(),
    query = $bindable(),
    optionsFor,
    presets,
    resultCount,
    end,
  }: Props = $props();

  const defOf = (key: string) => defs.find((d) => d.key === key);

  /** Épinglés d'abord, dans leur ordre, puis les filtres posés à la volée dans
   * l'ordre du catalogue — pas dans celui où ils ont été ajoutés, qui ferait
   * danser la rangée d'une session à l'autre. */
  const chipKeys = $derived([
    ...pinned.filter((k) => defs.some((d) => d.key === k)),
    ...defs.filter((d) => !pinned.includes(d.key) && filters[d.key]).map((d) => d.key),
  ]);

  /** Un filtre présent mais sans valeur ne compte pas : il se lit comme un
   * fantôme et n'écarte rien. */
  const activeCount = $derived(Object.entries(filters).filter(([, st]) => !isBlank(st)).length);
  const canClear = $derived(activeCount > 0 || query.trim() !== "");

  let addEl = $state<HTMLElement | null>(null);
  let menuOpen = $state(false);
  let openKey = $state<string | null>(null);
  let openAnchor = $state<HTMLElement | null>(null);
  // Volontairement hors `$state` : ces références ne pilotent aucun rendu,
  // elles ne servent qu'à donner une ancre au popover au moment où on l'ouvre.
  const chipEls: Record<string, HTMLElement | null> = {};

  const openDef = $derived(openKey ? defOf(openKey) : undefined);
  const openState = $derived(openKey ? filters[openKey] : undefined);

  function closeEditor() {
    // R2 : l'inactivité est l'ABSENCE de l'entrée, jamais une valeur vide
    // qu'on garderait en mémoire. Un éditeur ouvert puis vidé rend donc sa
    // clé — la puce redevient fantôme si elle est épinglée, disparaît sinon.
    if (openKey && filters[openKey] && isBlank(filters[openKey])) drop(openKey);
    openKey = null;
    openAnchor = null;
  }

  function drop(key: string) {
    const next = { ...filters };
    delete next[key];
    filters = next;
  }

  function update(key: string, st: FilterState) {
    filters = { ...filters, [key]: st };
  }

  async function openEditor(key: string) {
    if (openKey === key) {
      closeEditor();
      return;
    }
    closeEditor();
    if (!filters[key]) {
      const def = defOf(key);
      if (!def) return;
      update(key, blankState(def));
      await tick();
    }
    openKey = key;
    openAnchor = chipEls[key] ?? null;
  }

  /** Corps de puce cliqué. Un booléen s'inverse (ou se pose à sa polarité par
   * défaut s'il était fantôme) ; tout le reste ouvre son éditeur. */
  function onChipClick(def: FilterDef) {
    if (def.type === "bool") {
      const st = filters[def.key];
      if (st && st.type === "bool") update(def.key, { type: "bool", sign: (st.sign * -1) as Sign });
      else update(def.key, blankState(def));
      return;
    }
    void openEditor(def.key);
  }

  function onChipKey(e: KeyboardEvent, def: FilterDef) {
    if (e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      removeChip(def.key);
    }
  }

  function removeChip(key: string) {
    if (openKey === key) {
      openKey = null;
      openAnchor = null;
    }
    drop(key);
  }

  function clearAll() {
    filters = {};
    query = "";
    closeEditor();
  }

  async function pickFromMenu(key: string) {
    const def = defOf(key);
    if (!def) return;
    menuOpen = false;
    if (def.type === "bool") {
      // Pas d'éditeur : la puce est posée directement avec sa polarité par
      // défaut, et se manipule ensuite au clic.
      if (!filters[key]) update(key, blankState(def));
      return;
    }
    if (!filters[key]) update(key, blankState(def));
    await tick();
    openKey = key;
    openAnchor = chipEls[key] ?? null;
  }

  function togglePin(key: string) {
    pinned = pinned.includes(key) ? pinned.filter((k) => k !== key) : [...pinned, key];
  }

  /** Libellé complet d'une puce, pour le lecteur d'écran et l'infobulle : les
   * `+N` masquent des valeurs qui doivent rester lisibles autrement. */
  function chipLabel(def: FilterDef, st: FilterState | undefined): string {
    return st && !isBlank(st) ? ariaSummary(def, st) : t(def.labelKey);
  }
</script>

<!-- UNE seule coulée : recherche, bouton, puces et « Tout effacer » passent à
     la ligne ensemble. Le décompte et la bascule de vue sont dans un bloc à
     part, calé en haut à droite — c'est ce qui les empêche de descendre avec
     les puces quand la coulée grandit. -->
<div class="filters">
  <div class="flow">
  <label class="search">
    <span class="sr">{t("library.search")}</span>
    <span class="field">
      <input class="input" placeholder={t("library.searchPlaceholder")} bind:value={query} />
      {#if query}
        <button type="button" class="wipe" title={t("filters.clearSearch")} onclick={() => (query = "")}>×</button>
      {/if}
    </span>
  </label>

  <button
    bind:this={addEl}
    type="button"
    class="add"
    aria-expanded={menuOpen}
    onclick={() => {
      closeEditor();
      menuOpen = !menuOpen;
    }}><span class="plus" aria-hidden="true">+</span>{t("filters.filter")}</button
  >

  <!-- Filet de 1 px : il sépare l'outil de son résultat sans boîte ni fond,
       et c'est lui qui empêche de lire « le bouton fait partie des puces ».
       Rien à séparer quand il n'y a pas de puce, donc il n'apparaît pas. -->
  {#if chipKeys.length}<span class="rule" aria-hidden="true"></span>{/if}

  {#each chipKeys as key (key)}
    {@const def = defOf(key)}
    {#if def}
      {@const st = filters[key]}
      {@const ghost = !st || isBlank(st)}
      {@const sum = st && !ghost ? chipSummary(def, st) : null}
      <span bind:this={chipEls[key]} class="chip" class:ghost class:open={openKey === key}>
        <button
          type="button"
          class="body"
          title={ghost ? t(def.labelKey) : chipLabel(def, st)}
          aria-label={chipLabel(def, st)}
          onclick={() => onChipClick(def)}
          onkeydown={(e) => onChipKey(e, def)}
        >
          {#if !st || isBlank(st)}
            <span class="pin" aria-hidden="true">◈</span>{t(def.labelKey)}
          {:else if st.type === "bool"}
            <span class:neg={st.sign < 0}>{st.sign > 0 ? t(def.labelKey) : t(def.negLabelKey ?? def.labelKey)}</span>
          {:else if sum}
            <span class="k">{t(def.labelKey)} :</span>
            {#if sum.op}
              <span class="op">{sum.op === "and" ? t("filters.opAnd") : t("filters.opOr")}</span>
            {/if}
            <span class="v">
              {#if sum.plain !== undefined}
                {sum.plain}
              {:else}
                {#if sum.inc.length}<span>{sum.inc.join(", ")}{#if sum.incMore}&nbsp;+{sum.incMore}{/if}</span>{/if}
                {#if sum.exc.length}
                  {#if sum.inc.length}<span class="sep"> · </span>{/if}
                  <span class="neg"
                    >{t("filters.except")}
                    {sum.exc.join(", ")}{#if sum.excMore}&nbsp;+{sum.excMore}{/if}</span
                  >
                {/if}
              {/if}
            </span>
          {/if}
        </button>
        {#if !ghost}
          <button
            type="button"
            class="x"
            title={pinned.includes(key) ? t("filters.chipResetPinned") : t("filters.chipRemove")}
            onclick={() => removeChip(key)}>×</button
          >
        {/if}
      </span>
    {/if}
  {/each}

  {#if canClear}
    <button type="button" class="clear" onclick={clearAll}>{t("filters.clearAll")}</button>
  {/if}
  </div>

  <div class="aside">
    <span class="count mono">{t("library.results", { count: resultCount })}</span>
    {@render end?.()}
  </div>
</div>

{#if menuOpen && addEl}
  <AnchoredPopover anchor={addEl} minWidth={250} onclose={() => (menuOpen = false)}>
    <FilterAddMenu {defs} {pinned} onpick={pickFromMenu} ontogglePin={togglePin} />
  </AnchoredPopover>
{/if}

{#if openKey && openAnchor && openDef && openState}
  <AnchoredPopover anchor={openAnchor} minWidth={openDef.type === "val" ? 294 : 200} onclose={closeEditor}>
    <FilterEditor
      def={openDef}
      st={openState}
      options={optionsFor(openKey)}
      {presets}
      onupdate={(next) => update(openDef.key, next)}
    />
  </AnchoredPopover>
{/if}

<style>
  /* `flex-start` sur le conteneur, et c'est le détail qui compte : sans lui,
     le bloc de droite se centrerait sur toute la hauteur de la coulée et
     descendrait avec elle dès que les puces passent à la ligne. */
  .filters {
    /* Deux hauteurs, deux natures : on AGIT sur un contrôle, une puce est
       l'état qui en résulte. L'écart se lit sans avoir à l'écrire. */
    --ctl-h: 32px;
    --chip-h: 26px;
    display: flex;
    align-items: flex-start;
    gap: 14px;
    margin-bottom: 14px;
  }
  .flow {
    flex: 1 1 auto;
    min-width: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .aside {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    height: var(--ctl-h);
  }
  /* Largeur fixe, pas de `flex: 1` : à s'étirer, la recherche mangeait toute
     la ligne et refoulait la première puce au rang suivant — c'est-à-dire
     qu'elle produisait exactement la deuxième rangée qu'on cherche à éviter. */
  .search {
    flex: none;
    width: 300px;
    max-width: 100%;
  }
  /* Le filet ne porte aucun fond ni cadre : il sépare l'outil de son
     résultat, il ne les met pas en boîte. */
  .rule {
    flex: none;
    width: 1px;
    height: 18px;
    background: var(--line);
    margin: 0 4px;
  }
  /* Le libellé « Recherche » ne s'affiche plus : le placeholder dit déjà ce
     que le champ cherche, et la barre n'a qu'une ligne de haut. Il
     reste pour le lecteur d'écran, qui n'a pas le placeholder. */
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
  .field {
    position: relative;
    display: block;
  }
  .field .input {
    width: 100%;
    height: var(--ctl-h);
    padding-right: 26px;
  }
  .wipe {
    position: absolute;
    right: 2px;
    top: 50%;
    transform: translateY(-50%);
    width: 20px;
    height: 20px;
    background: none;
    border: 0;
    color: var(--muted2);
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
  }
  .wipe:hover {
    color: var(--txt);
  }

  .add {
    display: flex;
    align-items: center;
    gap: 7px;
    height: var(--ctl-h);
    padding: 0 12px;
    background: none;
    border: 1px dashed var(--faint2);
    color: var(--muted);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
    white-space: nowrap;
  }
  .add:hover,
  .add[aria-expanded="true"] {
    border-color: var(--rosso-border);
    color: var(--txt);
  }
  .add .plus {
    color: var(--muted2);
    font-size: 14px;
    line-height: 1;
  }
  .count {
    font-size: 11px;
    color: var(--faint);
    white-space: nowrap;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    height: var(--chip-h);
    border: 1px solid var(--rosso-border);
    background: var(--panel2);
    max-width: 100%;
  }
  .chip:hover {
    border-color: var(--rosso);
  }
  .chip.open {
    border-color: var(--rosso);
    background: var(--rosso-dim);
  }
  /* Fantôme : le filtre est épinglé mais ne dit encore rien. Pointillés et pas
     de croix — il n'y a rien à retirer. */
  .chip.ghost {
    border-style: dashed;
    border-color: var(--faint2);
    background: none;
  }
  .chip.ghost:hover {
    border-color: var(--muted2);
  }
  .chip .body {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: none;
    border: 0;
    color: var(--txt);
    font: inherit;
    font-size: 12px;
    padding: 0 4px 0 9px;
    height: 100%;
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
  }
  .chip.ghost .body {
    color: var(--muted);
    padding-right: 9px;
  }
  .chip.ghost:hover .body {
    color: var(--txt2);
  }
  .chip .pin {
    color: var(--faint);
    font-size: 10px;
  }
  .chip .k {
    color: var(--muted);
  }
  .chip .v {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .chip .op {
    font-size: 9.5px;
    letter-spacing: 0.1em;
    color: var(--rosso-bright);
    border: 1px solid var(--rosso-border);
    padding: 0 4px;
  }
  /* Une exclusion se dit par le mot (« sauf », « Hors… »), pas par une couleur
     qui lui serait propre : le jaune est déjà l'alerte et le rouge le
     destructif. */
  .chip .neg {
    color: var(--muted);
  }
  .chip .sep {
    color: var(--faint2);
  }
  .chip .x {
    width: 18px;
    height: 18px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: 0;
    color: var(--muted2);
    font-size: 13px;
    cursor: pointer;
    margin-right: 4px;
  }
  .chip .x:hover {
    background: var(--raised);
    color: var(--txt);
  }
  .clear {
    height: var(--chip-h);
    padding: 0 6px;
    background: none;
    border: 0;
    color: var(--muted2);
    font: inherit;
    font-size: 11.5px;
    cursor: pointer;
  }
  .clear:hover {
    color: var(--rosso-bright);
  }
</style>
