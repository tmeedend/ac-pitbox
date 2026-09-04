<script lang="ts">
  // Menu « + Filtre » (§6.3) : la liste des filtres du catalogue, et l'épingle.
  //
  // L'épinglage se règle **ici** et pas dans les Réglages : l'intention
  // d'épingler se forme au moment où l'on ajoute Marque pour la quatrième fois
  // de la journée, pas dans un écran de configuration où il faudrait se
  // rappeler la liste des filtres hors contexte.
  //
  // La séquence clavier complète doit marcher sans souris : « + Filtre » →
  // trois lettres → Entrée → la valeur → Entrée.
  import { t } from "$lib/i18n/index.svelte";
  import type { FilterDef } from "$lib/filters";

  interface Props {
    defs: FilterDef[];
    pinned: string[];
    onpick: (key: string) => void;
    ontogglePin: (key: string) => void;
  }
  let { defs, pinned, onpick, ontogglePin }: Props = $props();

  let query = $state("");
  let activeIndex = $state(0);
  let inputEl = $state<HTMLInputElement | null>(null);

  const shown = $derived(defs.filter((d) => t(d.labelKey).toLowerCase().includes(query.trim().toLowerCase())));
  const cursor = $derived(Math.min(activeIndex, Math.max(shown.length - 1, 0)));

  // Focus dès l'ouverture : le menu ne s'ouvre que sur un geste explicite, et
  // la première chose qu'on veut faire dedans est de taper le nom du filtre.
  $effect(() => {
    inputEl?.focus();
  });

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
      onpick(pick.key);
    }
  }
</script>

<div class="menu">
  <div class="head">{t("filters.addFilter")}</div>
  <input
    bind:this={inputEl}
    bind:value={query}
    class="find"
    type="text"
    autocomplete="off"
    spellcheck="false"
    placeholder={t("filters.searchFilter")}
    oninput={() => (activeIndex = 0)}
    onkeydown={onKeydown}
  />
  <div class="list">
    {#each shown as def, i (def.key)}
      <div class="row" class:on={i === cursor}>
        <button type="button" class="pick" onclick={() => onpick(def.key)}>{t(def.labelKey)}</button>
        <button
          type="button"
          class="pin"
          class:set={pinned.includes(def.key)}
          title={t("filters.pinTooltip")}
          aria-pressed={pinned.includes(def.key)}
          onclick={() => ontogglePin(def.key)}>◈</button
        >
      </div>
    {:else}
      <div class="none">{t("filters.tokenNoMatch")}</div>
    {/each}
  </div>
  <div class="foot">{t("filters.pinHint")}</div>
</div>

<style>
  .menu {
    display: flex;
    flex-direction: column;
    min-width: 250px;
  }
  .head {
    font-size: 9.5px;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--muted);
    padding: 2px 2px 8px;
  }
  .find {
    width: 100%;
    height: 30px;
    background: var(--panel2);
    border: 1px solid var(--line);
    color: var(--txt);
    padding: 0 9px;
    outline: 0;
    font: inherit;
    font-size: 12.5px;
    margin-bottom: 6px;
  }
  .find:focus {
    border-color: var(--rosso-border);
  }
  .list {
    max-height: 300px;
    overflow-y: auto;
  }
  .row {
    display: flex;
    align-items: center;
    height: 28px;
  }
  .row.on,
  .row:hover {
    background: var(--raised);
  }
  .pick {
    flex: 1;
    text-align: left;
    background: none;
    border: 0;
    color: var(--txt2);
    font: inherit;
    font-size: 12.5px;
    padding: 0 8px;
    height: 100%;
    cursor: pointer;
  }
  .row.on .pick,
  .row:hover .pick {
    color: var(--txt);
  }
  /* L'épingle bascule **sans fermer le menu** et sans poser le filtre : c'est
     un réglage, pas une action sur la bibliothèque. */
  .pin {
    background: none;
    border: 0;
    color: var(--faint2);
    font-size: 11px;
    padding: 3px 7px;
    cursor: pointer;
    align-self: stretch;
  }
  .pin:hover {
    background: var(--panel2);
    color: var(--muted);
  }
  .pin.set {
    color: var(--rosso-bright);
  }
  .none {
    font-size: 11.5px;
    color: var(--faint);
    padding: 6px 8px;
  }
  .foot {
    border-top: 1px solid var(--line);
    margin-top: 7px;
    padding: 8px 4px 2px;
    font-size: 10.5px;
    color: var(--muted2);
    line-height: 1.5;
  }
</style>
