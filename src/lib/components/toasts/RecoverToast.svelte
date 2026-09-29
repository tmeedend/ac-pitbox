<script lang="ts">
  // Progress and report of a recovery lot (ESPACE§7.2), in the notification
  // stack like the other lots. The report is the list of what is left to do
  // by hand — each line with its gesture —, since that is what the user reads
  // it for; what came back is a count.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import Toast from "./Toast.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { openInSection } from "$lib/shell/nav.svelte";
  import { cancelRecovery, dismissRecovery, recovery, type RecoveryResult } from "$lib/library/recovery.svelte";

  const recovered = $derived(recovery.results?.filter((r) => r.status === "recovered").length ?? 0);
  const left = $derived(recovery.results?.filter((r) => r.status !== "recovered") ?? []);

  function openFiche(r: RecoveryResult) {
    void openInSection(r.kind === "Track" ? "tracks" : "cars", r.id);
  }

  function openPage(url: string) {
    openUrl(url).catch((e) => console.error("open registry page", e));
  }
</script>

{#if recovery.running}
  <Toast title={t("showcase.recoveringMany", { index: recovery.index, total: recovery.total })} truncate>
    {#snippet actions()}
      <button class="btn-ghost r-cancel" type="button" onclick={cancelRecovery} disabled={recovery.cancelling}>
        {recovery.cancelling ? t("bulk.cancelling") : t("bulk.cancel")}
      </button>
    {/snippet}
    {#if recovery.current}<div class="r-cur mono">{recovery.current}</div>{/if}
  </Toast>
{:else if recovery.results}
  <Toast title={t("showcase.recoveredMany", { count: recovered })} onclose={dismissRecovery}>
    {#each left as r (r.id)}
      <div class="r-row">
        <span class="r-name">{r.name}</span>
        {#if r.status === "page"}
          <span class="r-why">{t("showcase.recoverOnPage")}</span>
          <button class="btn-ghost r-go" type="button" onclick={() => openPage(r.url)}>{t("showcase.open")}</button>
        {:else if r.status === "failed"}
          <span class="r-why err">{r.error}</span>
        {:else}
          <span class="r-why">{t("showcase.recoverManual")}</span>
          <button class="btn-ghost r-go" type="button" onclick={() => openFiche(r)}>{t("modpanel.ctxOpenDetail")}</button>
        {/if}
      </div>
    {/each}
  </Toast>
{/if}

<style>
  .r-cancel {
    font-size: 11px;
  }
  .r-cancel:disabled {
    color: var(--muted);
    cursor: default;
  }
  .r-cur {
    color: var(--txt2);
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .r-row {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 2px 0;
    font-size: 11px;
  }
  .r-name {
    flex: none;
    color: var(--txt2);
  }
  .r-why {
    flex: 1;
    min-width: 0;
    color: var(--muted);
  }
  .r-why.err {
    color: var(--rosso-bright);
    overflow-wrap: anywhere;
  }
  .r-go {
    flex: none;
    font-size: 11px;
  }
</style>
