<script lang="ts">
  // Thumbnails rendered by the game, in the notification stack
  // (docs/SPEC-grille.md GRILLE§7.1). A session takes about a minute and opens
  // the game's window: the card says which car, where the session is, and the
  // game will close by itself — without it, a window opening on its own reads
  // as a bug. The bar does not move during a session: the game reports
  // nothing until it is done.
  import Toast from "./Toast.svelte";
  import ProgressBar from "$lib/components/ui/ProgressBar.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import { dismissInGameReport, inGameProgress, showInGameFailures } from "$lib/gridthumbs/inGame.svelte";

  const p = $derived(inGameProgress());
  const phase = $derived(p.phase === "ingame" ? t("gridThumbs.inGameShooting") : t("gridThumbs.inGameLaunching"));

  const report = $derived.by(() => {
    const parts: string[] = [];
    if (p.done > 0) parts.push(t("gridThumbs.inGameReportDone", { count: String(p.done) }));
    if (p.failed > 0) parts.push(t("gridThumbs.inGameReportFailed", { count: String(p.failed) }));
    return parts.join(" · ");
  });
</script>

{#if p.running}
  <Toast title={t("gridThumbs.inGameTitle")}>
    <ProgressBar ratio={null} {phase}>
      {#snippet detail()}{p.current
          ? p.others > 0
            ? t("gridThumbs.inGameAndMore", { name: p.current, count: String(p.others) })
            : p.current
          : ""}{/snippet}
    </ProgressBar>
    {#if p.total > 1}
      <div class="ig-count"><span class="mono">{t("gridThumbs.taskCount", { done: String(p.done + p.failed), total: String(p.total) })}</span></div>
      <ProgressBar ratio={(p.done + p.failed) / p.total} thin />
    {/if}
  </Toast>
{:else if p.finished}
  <Toast title={report} tone={p.failed > 0 ? "warn" : "default"} onclose={dismissInGameReport}>
    {#if p.lastError}<div class="ig-error">{errorText(p.lastError)}</div>{/if}
    {#if p.failures.length > 0}
      <button type="button" class="btn ig-see" onclick={() => void showInGameFailures()}>
        {t("gridThumbs.inGameSeeFailures")}
      </button>
    {/if}
  </Toast>
{/if}

<style>
  .ig-count {
    margin-top: 7px;
    font-size: 11px;
    color: var(--txt2);
  }
  .ig-error {
    font-size: 11px;
    color: var(--txt2);
  }
  .ig-see {
    margin-top: 8px;
  }
</style>
