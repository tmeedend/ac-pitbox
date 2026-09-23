<script lang="ts">
  // The index of the library (INDEX§1): what the Cars and Tracks screens
  // show when no chip is posed and the search box is empty, instead of a wall
  // of three hundred thumbnails sorted alphabetically.
  //
  // **A tile poses a chip, and that is all it does** (INDEX§2). It opens
  // no mode and holds no selection of its own: `onpose` writes into the one
  // filter state of the screen, exactly as the chip editor would, and the list
  // appears because a filter is now active. Removing the last chip brings the
  // index back for the same reason — there is no "back to the index" button,
  // and there must not be one.
  //
  // Tracks get ONE index, by country: a circuit is its place. Cars get two,
  // families then brands, and deliberately no country (INDEX§6.3): three
  // stacked grids are the wall again, and "Japan" on a car only says less well
  // what `#jdm` says.
  import { flagFor, loadFlags } from "$lib/flags.svelte";
  import { localizedCountry } from "$lib/flags";
  import { i18n, t } from "$lib/i18n/index.svelte";
  import { brandTiles, indexTiles, initials, type IndexTile } from "$lib/library/browseIndex";
  import type { CategoryFamily } from "$lib/library/families";
  import { familyIcon, NEUTRAL_ICON } from "$lib/library/familyIcons";
  import type { FilterOption } from "$lib/library/filters";
  import type { ModKind } from "$lib/library/library";

  interface Props {
    kind: ModKind;
    /** The options of the filter editors — same values, same counts. */
    optionsFor: (key: string) => FilterOption[];
    /** Size of the library of this kind: what the brands have to cover. */
    total: number;
    families: CategoryFamily[];
    /** URL of the logo drawn on a brand tile, or `null` for initials. */
    badgeOf: (brand: string) => string | null;
    onpose: (key: string, value: string) => void;
    /** "See all tracks": the unfiltered list, the one way to it that poses no
     * chip (INDEX§5.1). */
    onshowall: () => void;
  }
  let { kind, optionsFor, total, families, badgeOf, onpose, onshowall }: Props = $props();

  const isCar = $derived(kind === "Car");

  // Flags are only read here for tracks; the table is loaded by whoever shows
  // one, not at startup (see `flags.svelte.ts`).
  $effect(() => {
    if (!isCar) void loadFlags();
  });

  const countries = $derived(isCar ? [] : indexTiles(optionsFor("country")));
  const familyTiles = $derived(isCar && families.length ? indexTiles(optionsFor("family")) : []);
  /** Not remembered, on purpose (INDEX§6.2): folded again next time. */
  let allBrands = $state(false);
  const brandAll = $derived(isCar ? indexTiles(optionsFor("brand")) : []);
  const brands = $derived(brandTiles(brandAll, total, allBrands));

  const iconOf = (id: string) => familyIcon(families.find((f) => f.id === id)?.icon);

  function countText(n: number): string {
    const unit = isCar ? (n === 1 ? "index.carOne" : "index.cars") : n === 1 ? "index.trackOne" : "index.tracks";
    return t(unit, { count: n });
  }

  /** The name ON the tile. A country is translated from its code
   * (INDEX§11); a brand never is, and a family or "Not set" already comes
   * translated from the filter's own vocabulary. */
  function tileName(key: string, tile: IndexTile): string {
    return key === "country" && !tile.unset ? localizedCountry(tile.label, i18n.locale) : tile.label;
  }

  /**
   * Arrows move between tiles like in a grid of cards (INDEX§9). The
   * column count is read off the grid as laid out — `auto-fill` decides it,
   * and it changes with the width of the window and the UI zoom.
   */
  function onTileKey(e: KeyboardEvent) {
    const step = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -2, ArrowDown: 2 }[e.key];
    if (step === undefined) return;
    const tile = e.currentTarget as HTMLElement;
    const grid = tile.parentElement;
    if (!grid) return;
    const tiles = [...grid.querySelectorAll<HTMLElement>(":scope > .tile")];
    const cols = Math.max(1, getComputedStyle(grid).gridTemplateColumns.split(" ").filter(Boolean).length);
    const i = tiles.indexOf(tile);
    const next = Math.abs(step) === 1 ? i + step : i + Math.sign(step) * cols;
    if (next < 0 || next >= tiles.length) return;
    e.preventDefault();
    tiles[next].focus();
  }
</script>

<!-- The emblem kind is NOT a class of the tile: `flag` is also the class of
     the flag itself, and the tile took its 44×29 box and `overflow: hidden` -
     a squashed tile showing only its count. -->
{#snippet tile(key: string, tl: IndexTile, emblem: "flag" | "family" | "brand")}
  {@const name = tileName(key, tl)}
  <button
    type="button"
    class="tile"
    class:brand={emblem === "brand"}
    class:unset={tl.unset}
    aria-label="{name}, {countText(tl.count)}"
    onclick={() => onpose(key, tl.value)}
    onkeydown={onTileKey}
  >
    {#if emblem === "flag"}
      {@const flag = tl.unset ? null : flagFor(tl.label)}
      <span class="flag">
        {#if flag}<img src={flag} alt="" />{:else}<span class="none" aria-hidden="true">?</span>{/if}
      </span>
    {:else if emblem === "family"}
      <svg class="ico" viewBox="0 0 120 52" aria-hidden="true">{@html tl.unset ? NEUTRAL_ICON : iconOf(tl.value)}</svg>
    {:else}
      {@const logo = badgeOf(tl.value)}
      <span class="logo">
        {#if logo}<img src={logo} alt="" />{:else}<span aria-hidden="true">{initials(name)}</span>{/if}
      </span>
    {/if}
    <span class="nm">{name}</span>
    <span class="ct">{countText(tl.count)}</span>
  </button>
{/snippet}

<div class="index">
  {#if !isCar}
    <section class="sect">
      <div class="sh">
        <h2 class="lbl">{t("index.byCountry")}</h2>
        <span class="s">{t("index.countries", { count: countries.filter((c) => !c.unset).length })}</span>
        <button type="button" class="more" onclick={onshowall}>{t("index.allTracks")}</button>
      </div>
      <div class="tiles countries">
        {#each countries as tl (tl.value)}{@render tile("country", tl, "flag")}{/each}
      </div>
    </section>
  {:else}
    <!-- Families first: coarser, fewer, above the fold - and the most
         frequent question ("I want to drive a GT tonight"). -->
    {#if familyTiles.length}
      <section class="sect">
        <div class="sh">
          <h2 class="lbl">{t("index.byFamily")}</h2>
          <!-- Without it, whoever adds the counters up takes the overlap for a
               bug (INDEX§6.1): a 250 GTO is Classic, Sportscars AND
               Race. -->
          <span class="s">{t("index.familiesOverlap")}</span>
        </div>
        <div class="tiles families">
          {#each familyTiles as tl (tl.value)}{@render tile("family", tl, "family")}{/each}
        </div>
      </section>
    {/if}
    {#if brandAll.length}
      <section class="sect">
        <div class="sh">
          <h2 class="lbl">{t("index.byBrand")}</h2>
          <span class="s">{t("index.brands", { count: brandAll.length })}</span>
          {#if allBrands || brands.hidden}
            <button type="button" class="more" aria-expanded={allBrands} onclick={() => (allBrands = !allBrands)}>
              {allBrands ? t("index.fewerBrands") : t("index.allBrands", { count: brandAll.length })}
            </button>
          {/if}
        </div>
        <div class="tiles brands">
          {#each brands.shown as tl (tl.value)}{@render tile("brand", tl, "brand")}{/each}
        </div>
      </section>
    {/if}
  {/if}
</div>

<style>
  /* Tokens of the app, not those of the mock-ups (see CLAUDE.md, "le design
     system fait foi"): the tile is a card of the grid (`--cell`), its hover
     climbs to the dimmed red and its focus to the red stroke - no red at
     rest, the accent scale of §7.2ter applies without exception. */
  .index {
    padding-top: 2px;
  }
  .sect {
    border-top: 1px solid var(--line);
    padding-top: 12px;
  }
  .sect + .sect {
    margin-top: 24px;
  }
  .sh {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 6px;
  }
  .sh .s {
    font-size: 11.5px;
    color: var(--muted2);
    margin-bottom: 8px;
  }
  /* Aligned on the `.lbl` margin, so the three sit on one line. */
  .more {
    margin: 0 0 8px auto;
    background: none;
    border: 1px solid var(--line);
    color: var(--muted);
    font: inherit;
    font-size: 11.5px;
    padding: 5px 11px;
    cursor: pointer;
  }
  .more:hover {
    border-color: var(--faint2);
    color: var(--txt);
  }
  .more:focus-visible {
    outline: 2px solid var(--rosso);
    outline-offset: 2px;
  }

  .tiles {
    display: grid;
    gap: 9px;
  }
  /* Widths per grid (INDEX§4.2): a family name can be long and its
     silhouette is wide; "Royaume-Uni" and "Vereinigtes Königreich" must both
     fit in two lines; brands are more numerous and their emblem smaller. */
  .tiles.families {
    grid-template-columns: repeat(auto-fill, minmax(134px, 1fr));
  }
  .tiles.countries {
    grid-template-columns: repeat(auto-fill, minmax(124px, 1fr));
  }
  .tiles.brands {
    grid-template-columns: repeat(auto-fill, minmax(112px, 1fr));
  }

  .tile {
    --tile-bg: var(--cell);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 13px 10px 11px;
    background: var(--tile-bg);
    border: 1px solid var(--line);
    border-radius: 2px;
    color: var(--txt);
    font: inherit;
    cursor: pointer;
    transition:
      border-color 0.12s,
      background 0.12s;
  }
  .tile:hover {
    --tile-bg: var(--mat);
    border-color: var(--rosso-border);
  }
  .tile:focus-visible {
    outline: 2px solid var(--rosso);
    outline-offset: 2px;
  }
  /* The absent value: last, dashed, neutral emblem (INDEX§4.3). It makes
     the hole in the data visible instead of hiding it. */
  .tile.unset {
    border-style: dashed;
    border-color: var(--faint2);
  }
  .tile.unset:hover {
    border-color: var(--rosso-border);
  }
  .nm {
    font-size: 12px;
    line-height: 1.25;
    text-align: center;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
  }
  .ct {
    font-size: 10.5px;
    color: var(--muted2);
  }

  .flag {
    width: 44px;
    height: 29px;
    border-radius: 2px;
    overflow: hidden;
    box-shadow: 0 1px 5px rgba(0, 0, 0, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .flag img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .flag .none {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: repeating-linear-gradient(135deg, var(--raised), var(--raised) 5px, var(--mat) 5px, var(--mat) 10px);
    color: var(--faint);
    font-size: 14px;
  }

  .ico {
    height: 34px;
    width: auto;
    fill: var(--txt2);
    transition: fill 0.12s;
  }
  .tile:hover .ico {
    fill: var(--txt);
  }
  .tile.unset .ico {
    fill: var(--faint);
  }
  /* Windows and tyres are holes in the colour of the tile: they follow its
     hover background instead of freezing one grey. */
  .ico :global(.cut) {
    fill: var(--tile-bg);
    transition: fill 0.12s;
  }
  .ico :global(.smoke) {
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    opacity: 0.5;
  }

  .logo {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    background: var(--raised);
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    font-size: 10px;
    letter-spacing: 0.04em;
    color: var(--muted);
  }
  .logo img {
    width: 24px;
    height: 24px;
    object-fit: contain;
  }
  .tile.brand .nm {
    font-size: 11.5px;
  }
</style>
