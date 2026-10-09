<script lang="ts">
  // The strip of an app's or an "other" mod's fiche when it has no files
  // (ESPACE§5.6): it arrived with an imported library export. Same family as
  // the car's `ShowcaseBanner`, smaller: an app has no registry entry, no pack
  // page and no author address to offer — its archive's name and a web search
  // are all there is, and importing that archive brings it back in its row.
  import { t } from "$lib/i18n/index.svelte";
  import { openExternal } from "$lib/links";
  import { webSearchUrl } from "$lib/webSearch";

  interface Props {
    /** The archive it came from, the best search key there is (ESPACE§8.2). */
    archive: string | null;
    /** What to search for without one. */
    name: string;
  }
  let { archive, name }: Props = $props();
</script>

<div class="vit">
  <div class="line">
    <span class="what">{t("showcase.explainAddon")}</span>
    <button class="btn" type="button" onclick={() => openExternal(webSearchUrl(archive ?? name))}>
      {t("showcase.searchWeb")}
    </button>
  </div>
  <div class="foot">
    {#if archive}
      <span>{t("showcase.fileName")}</span>
      <span class="mono file">{archive}</span>
    {/if}
    <span>{t("showcase.dropHint")}</span>
  </div>
</div>

<style>
  .vit {
    margin-bottom: 14px;
    padding: 8px 10px;
    background: var(--panel2);
    border: 1px solid var(--line);
    font-size: 11.5px;
  }
  .line {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .what {
    flex: 1;
    min-width: 0;
    color: var(--txt2);
    line-height: 1.45;
  }
  .foot {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 6px;
    color: var(--muted);
  }
  /* The name is the search key: it must be selectable, whole. */
  .file {
    color: var(--txt2);
    user-select: all;
    overflow-wrap: anywhere;
  }
</style>
