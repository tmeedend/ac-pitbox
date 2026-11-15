<script lang="ts">
  // The question of GRILLE§6.5: the thumbnail engine changed, and the images
  // shown come from the previous version. Redone only on the user's word —
  // redoing them by itself cost four hundred renders at every launch while the
  // engine was being worked on.
  import Toast from "./Toast.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { regenerateStaleThumbs } from "$lib/gridthumbs/gridThumbs.svelte";
  import { dismissStale, staleThumbs } from "$lib/gridthumbs/staleThumbs.svelte";

  const s = $derived(staleThumbs());
</script>

{#if s.count > 0 && !s.accepted && !s.dismissed}
  <Toast title={t("gridThumbs.staleTitle")} tone="info" onclose={dismissStale} closeLabel={t("gridThumbs.staleLater")}>
    <div class="st-body">{t("gridThumbs.staleBody", { count: String(s.count) })}</div>
    <button type="button" class="btn st-redo" onclick={regenerateStaleThumbs}>{t("gridThumbs.staleRedo")}</button>
  </Toast>
{/if}

<style>
  .st-body {
    font-size: 11px;
    color: var(--txt2);
  }
  .st-redo {
    margin-top: 8px;
  }
</style>
