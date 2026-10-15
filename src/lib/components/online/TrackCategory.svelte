<script lang="ts">
  // The panel's category line (SPEC-play-online.md, "Catégorie de circuit"):
  // what the track's category is and who says so, what the server's own words
  // add ("≈"), and — the track being unknown or wrongly known — a choice made
  // here, kept by track id until the track is installed and after.
  //
  // A toggle saves at once: the choice is a file write on the Rust side,
  // short, and the filter of the list follows it without anything to confirm.
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import Tooltip from "$lib/components/ui/Tooltip.svelte";
  import TrackCategoryIcon from "$lib/components/library/TrackCategoryIcon.svelte";
  import { trackCategoryName } from "$lib/library/trackCategoryTable.svelte";
  import type { Looks } from "$lib/online/looks";
  import type { ServerSummary } from "$lib/online/online";
  import { serverCategories, type CategorySource, type Hint } from "$lib/online/categories";
  import { chooseTrackCategory, trackCategories } from "$lib/online/trackCategories.svelte";

  interface Props {
    server: ServerSummary;
    looks: Looks;
    /** The server's description, once its details are read. */
    description: string | null;
  }
  let { server, looks, description }: Props = $props();

  const cats = $derived(serverCategories(server, looks, trackCategories.knowledge, description));
  const vocabulary = $derived(trackCategories.knowledge.vocabulary);

  let picking = $state(false);
  let saving = $state(false);
  let error = $state("");

  // Another track, the picker opened for the last one closes. Through a
  // derived id: a refresh of the list hands a new object for the same server.
  const trackId = $derived(server.track.id);
  $effect(() => {
    void trackId;
    picking = false;
    error = "";
  });

  const SOURCE_KEYS: Record<CategorySource, string> = {
    chosen: "online.categoryFromChoice",
    library: "online.categoryFromLibrary",
    catalog: "online.categoryFromCatalog",
  };

  function hintText(h: Hint): string {
    const where = {
      name: "online.categoryHintName",
      description: "online.categoryHintDescription",
      track: "online.categoryHintTrack",
    }[h.source];
    return t(where, { category: trackCategoryName(h.category), word: h.word });
  }

  async function toggle(category: string) {
    const current = cats.source === "chosen" ? cats.base : [];
    const next = current.includes(category) ? current.filter((c) => c !== category) : [...current, category];
    saving = true;
    error = "";
    try {
      await chooseTrackCategory(trackId, next);
    } catch (e) {
      error = errorText(e);
    } finally {
      saving = false;
    }
  }
</script>

<div class="category">
  <span class="lbl-key">{t("online.category")}</span>
  {#if cats.base.length}
    <Tooltip text={t(SOURCE_KEYS[cats.source ?? "library"])} align="left" side="bottom">
      <span class="values">{cats.base.map(trackCategoryName).join(", ")}</span>
    </Tooltip>
  {:else if !cats.hints.length}
    <span class="unknown">{t("online.categoryUnknown")}</span>
  {/if}
  {#each cats.hints as h (h.category)}
    <Tooltip text={hintText(h)} align="left" side="bottom">
      <span class="hint">≈ {trackCategoryName(h.category)}</span>
    </Tooltip>
  {/each}
  {#if vocabulary.length}
    ·
    <button class="link" type="button" aria-expanded={picking} onclick={() => (picking = !picking)}>
      {picking ? t("common.close") : cats.source === "chosen" ? t("online.categoryChange") : t("online.categoryChoose")}
    </button>
  {/if}
</div>
{#if picking}
  <div class="picker">
    {#each vocabulary as c (c)}
      {@const on = cats.source === "chosen" && cats.base.includes(c)}
      <button class="pill" class:on type="button" aria-pressed={on} disabled={saving} onclick={() => toggle(c)}
        ><TrackCategoryIcon tag={c} size={11} />{trackCategoryName(c)}</button
      >
    {/each}
  </div>
  {#if error}<p class="errbox">{error}</p>{/if}
{/if}

<style>
  .category {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 8px;
    padding: 0 16px 10px;
    font-size: 11px;
    color: var(--muted);
  }
  .values {
    color: var(--txt2);
  }
  .unknown {
    color: var(--muted2);
  }
  .hint {
    color: var(--muted);
    cursor: help;
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--blue);
    font: inherit;
    cursor: pointer;
  }
  .picker {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 0 16px 10px;
  }
  .picker .pill {
    --tile-bg: var(--panel2);
    background: none;
    cursor: pointer;
  }
  .picker .pill:hover {
    color: var(--txt2);
  }
  .picker .pill.on {
    --tile-bg: var(--blue-dim);
    border-color: var(--blue-border);
    background: var(--blue-dim);
    color: var(--blue);
  }
  .errbox {
    margin: 0 16px 10px;
  }
</style>
