<script lang="ts">
  // The summary of the music assigned to terms (§16.8): read-only — a term's
  // music is chosen on its line in Classement, where `›` leads. What the
  // summary adds is the view of all of them at once, and the orphans: an
  // assignment whose term no longer exists is kept (§16.4), and this is the
  // one place it shows, with the one button that removes it.
  import { onMount } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import Emblem from "$lib/components/ui/Emblem.svelte";
  import TrackCategoryIcon from "$lib/components/library/TrackCategoryIcon.svelte";
  import { countryLabel, flagFor, gameCountry, loadFlags } from "$lib/flags.svelte";
  import { isFictional } from "$lib/flags";
  import { logoOf } from "$lib/library/brandLogos.svelte";
  import { familyIcon } from "$lib/library/familyIcons";
  import { categoryFamilies, loadFamilies } from "$lib/library/familyTable.svelte";
  import { previewSrc } from "$lib/library/library";
  import { libraryVersion } from "$lib/library/libraryVersion.svelte";
  import { cardsAt } from "$lib/music/context.svelte";
  import { loadTrackCategoryLooks, trackCategoryName } from "$lib/library/trackCategoryTable.svelte";
  import { getRulesView } from "$lib/workshop/rules";
  import { focusTerm } from "$lib/workshop/termFocus.svelte";
  import { scanMusicSource, sourceName } from "$lib/music/music";
  import { assignmentRows, type Assignments, type Taxonomy } from "$lib/music/resolve";

  let { assignments, onremove }: { assignments: Assignments; onremove: (taxonomy: Taxonomy, id: string) => void } =
    $props();

  /** The terms that exist now, per taxonomy, lowercased; `null` until read —
   * nothing is called an orphan before we know. */
  let known = $state<Record<Taxonomy, Set<string>> | null>(null);
  let counts = $state<Record<string, number | null>>({});

  onMount(async () => {
    try {
      const [cards, rules] = await Promise.all([
        cardsAt(libraryVersion()),
        getRulesView(),
        loadFamilies(),
        loadFlags(),
        loadTrackCategoryLooks(),
      ]);
      const lc = (s: string) => s.toLowerCase();
      known = {
        brand: new Set(cards.flatMap((c) => (c.kind === "Car" && c.brand ? [lc(c.brand)] : []))),
        family: new Set(categoryFamilies().map((f) => lc(f.id))),
        track_category: new Set(rules.track_categories.map((r) => lc(r.name))),
        country: new Set(cards.flatMap((c) => (c.country ? [lc(c.country)] : []))),
      };
    } catch (e) {
      console.warn("music: terms not read for the assignments summary", e);
    }
  });

  const rows = $derived(assignmentRows(assignments).filter((r) => r.taxonomy !== "screen"));

  $effect(() => {
    for (const r of rows) {
      if (r.source in counts) continue;
      counts[r.source] = null;
      scanMusicSource(r.source)
        .then((i) => (counts[r.source] = i.missing ? -1 : i.track_count))
        .catch((e) => console.warn("music: source not scanned", e));
    }
  });

  function orphan(taxonomy: Taxonomy, id: string): boolean {
    if (!known) return false;
    if (taxonomy === "country" && (gameCountry(id) || isFictional(id))) return false;
    return !known[taxonomy].has(id.toLowerCase());
  }

  function nameOf(taxonomy: Taxonomy, id: string): string {
    switch (taxonomy) {
      case "family": {
        const f = categoryFamilies().find((x) => x.id === id);
        return f?.name ?? t(`families.${id}`);
      }
      case "track_category":
        return trackCategoryName(id);
      case "country":
        return countryLabel(id);
      default:
        return id;
    }
  }

  const TAB: Record<Taxonomy, string> = {
    brand: "brands",
    family: "categories",
    track_category: "categories",
    country: "countries",
  };
  const KIND: Record<Taxonomy, string> = {
    brand: "nav.brands",
    family: "nav.categories",
    track_category: "music.trackCategory",
    country: "nav.countries",
  };
</script>

{#if rows.length}
  <ul class="rows">
    {#each rows as r (`${r.taxonomy}:${r.id}`)}
      {@const tx = r.taxonomy as Taxonomy}
      {@const lost = orphan(tx, r.id)}
      {@const n = counts[r.source]}
      <li class:lost>
        <span class="emb" aria-hidden="true">
          {#if tx === "brand"}
            {@const logo = logoOf(r.id)}
            {@const src = logo ? previewSrc(logo.path) : null}
            {#if logo && src}<Emblem {src} plaque={logo.plaque} size={20} />{/if}
          {:else if tx === "family"}
            <svg class="ico" viewBox="0 0 120 52"
              >{@html familyIcon(categoryFamilies().find((f) => f.id === r.id)?.icon)}</svg
            >
          {:else if tx === "track_category"}
            <TrackCategoryIcon tag={r.id} size={18} />
          {:else}
            {@const flag = flagFor(r.id)}
            {#if flag}<img class="flag" src={flag} alt="" />{/if}
          {/if}
        </span>
        <span class="nm" title={nameOf(tx, r.id)}>
          {nameOf(tx, r.id)}
          <span class="kind">{t(KIND[tx])}</span>
        </span>
        <span class="src mono" title={r.source}>{sourceName(r.source)}</span>
        <span class="ct mono" class:missing={n === -1}
          >{n == null ? "" : n === -1 ? t("music.missing") : t("music.trackCount", { count: n })}</span
        >
        {#if lost}
          <span class="orphan">{t("music.orphan")}</span>
          <button type="button" class="btn" onclick={() => onremove(tx, r.id)}>{t("music.remove")}</button>
        {:else}
          <button
            type="button"
            class="go"
            title={t("music.openInSorting")}
            aria-label={t("music.openInSorting")}
            onclick={() => void focusTerm(TAB[tx], r.id, tx === "track_category")}>›</button
          >
        {/if}
      </li>
    {/each}
  </ul>
{:else}
  <p class="empty">{t("music.noAssignment")}</p>
{/if}

<style>
  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
    border-top: 1px solid var(--line);
  }
  .rows li {
    display: grid;
    grid-template-columns: 28px minmax(0, 1fr) minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 12px;
    padding: 6px 8px;
    border-bottom: 1px solid var(--line);
    font-size: 12.5px;
  }
  .rows li:has(.orphan) {
    grid-template-columns: 28px minmax(0, 1fr) minmax(0, 1fr) auto auto auto;
  }
  .emb {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--txt2);
  }
  .ico {
    height: 16px;
    width: auto;
    fill: var(--txt2);
  }
  .ico :global(.cut) {
    fill: var(--panel);
  }
  .flag {
    width: 24px;
    height: 16px;
    object-fit: cover;
    border-radius: 2px;
  }
  .nm {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .kind {
    margin-left: 8px;
    font-size: 11px;
    color: var(--muted2);
  }
  .src {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--txt2);
    font-size: 11.5px;
  }
  .ct {
    font-size: 11px;
    color: var(--muted2);
    white-space: nowrap;
  }
  .ct.missing,
  .orphan {
    color: var(--yellow);
    font-size: 11px;
  }
  li.lost .nm {
    color: var(--muted);
  }
  .go {
    width: 22px;
    height: 22px;
    background: none;
    color: var(--muted2);
    font: inherit;
  }
  .go:hover {
    color: var(--txt);
    background: var(--raised);
  }
  .go:focus-visible {
    outline: 2px solid var(--rosso);
    outline-offset: -2px;
  }
  .empty {
    color: var(--muted2);
    font-size: 12px;
  }
</style>
