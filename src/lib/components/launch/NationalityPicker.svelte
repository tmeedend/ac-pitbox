<script lang="ts">
  // The country list of a nationality cell: a search field and the game's
  // nationalities, each with its flag, `Auto` first. Meant to sit in an
  // `AnchoredPopover`, and mounted afresh on each opening, so a search never
  // survives from one row to the next — that would pass for a short list.
  //
  // **Un popover maison et non un `<select>`.** Un `<option>` natif ne peut pas
  // porter d'image — le menu déroulé est dessiné par le système, pas par la
  // webview —, donc le drapeau n'y apparaissait jamais alors que c'est lui qui
  // permet de reconnaître un pays d'un coup d'œil. Le projet avait déjà buté
  // dessus (`ImageSelectDropdown`).
  //
  // Ce qu'on perd, c'est la recherche à la frappe du système ; ce qu'on gagne,
  // c'est un vrai champ de recherche — et sur **220 pays** il vaut mieux, parce
  // qu'il cherche n'importe où dans le nom et pas seulement au début : « guinea »
  // rend les trois Guinées, ce qu'une frappe initiale ne sait pas faire.
  import { previewSrc } from "$lib/library/library";
  import type { Nationality } from "$lib/launch/launch";
  import { t } from "$lib/i18n/index.svelte";

  interface Props {
    nationalityList: Nationality[];
    /** The country picked today, by its full English name; `null`: `Auto`. */
    current: string | null;
    /** What `Auto` stands for here — the livery's country on a grid row. */
    autoLabel: string;
    onpick: (name: string | null) => void;
  }
  const { nationalityList, current, autoLabel, onpick }: Props = $props();

  let query = $state("");
  const matches = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return q ? nationalityList.filter((n) => n.name.toLowerCase().includes(q)) : nationalityList;
  });
</script>

<div class="natpop">
  <!-- `autofocus` : le popover s'ouvre pour chercher, et sans lui il
       faut un second clic avant de pouvoir taper. -->
  <!-- svelte-ignore a11y_autofocus -->
  <input class="input natq" type="text" autofocus placeholder={t("launch.searchCountry")} bind:value={query} />
  <div class="natlist">
    <!-- `Auto` d'abord et toujours visible : c'est le seul choix qui ne
         se cherche pas par son nom. -->
    <button class="nat" type="button" onclick={() => onpick(null)}>
      <span class="flag flag-none"></span>
      <span class="nat-n">{autoLabel}</span>
    </button>
    {#each matches as n (n.code)}
      <button class="nat" class:on={current === n.name} type="button" onclick={() => onpick(n.name)}>
        {#if previewSrc(n.flag)}<img class="flag" src={previewSrc(n.flag)} alt="" />{:else}
          <span class="flag flag-none"></span>{/if}
        <span class="nat-n">{n.name}</span>
      </button>
    {/each}
    {#if !matches.length}
      <p class="nat-none">{t("common.noResults")}</p>
    {/if}
  </div>
</div>

<style>
  .natpop {
    display: flex;
    flex-direction: column;
    padding: 6px;
    gap: 6px;
  }
  .natq {
    font-size: 11px;
  }
  .natlist {
    display: flex;
    flex-direction: column;
    max-height: 300px;
    overflow-y: auto;
  }
  .nat {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 4px 6px;
    background: transparent;
    border: 1px solid transparent;
    color: var(--txt2);
    font-size: 11px;
    text-align: left;
  }
  .nat:hover {
    background: var(--raised);
    color: var(--txt);
  }
  .nat.on {
    border-color: var(--rosso-border);
    background: var(--rosso-dim);
    color: var(--rosso-bright);
  }
  .nat-n {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .nat-none {
    padding: 8px 6px;
    font-size: 11px;
    color: var(--muted);
  }
  /* 4:3 comme les PNG du jeu, et un filet : beaucoup de drapeaux ont du blanc
     sur un bord, qui se fondrait dans la ligne. */
  .flag {
    flex: none;
    width: 16px;
    height: 12px;
    object-fit: cover;
    border: 1px solid var(--line);
  }
  /* Place tenue quand la livrée ne déclare aucun pays : sans elle la colonne se
     replierait sur les lignes muettes et la grille danserait. */
  .flag-none {
    border-style: dashed;
  }
</style>
