<script lang="ts">
  // Mod updates (§4.7) in the bottom-right stack: the announcement of new
  // versions, and the download of the one being installed.
  //
  // Two toasts rather than one, because they do not live for the same time.
  // The announcement is closed once read and does not come back for the same
  // versions; the download can be started from a fiche, and must stay visible
  // after the fiche is closed — the same reason the import progress lives in
  // the stack and not on the Import screen.
  import Toast from "./Toast.svelte";
  import ProgressBar from "$lib/components/ui/ProgressBar.svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { openInSection } from "$lib/shell/nav.svelte";
  import {
    modUpdates,
    unannounced,
    updateKey,
    installUpdate,
    ignoreUpdate,
    markAnnounced,
    cancelUpdateDownload,
    canStartUpdate,
    type ModUpdate,
  } from "$lib/library/modUpdates.svelte";

  const fresh = $derived(unannounced());

  function mb(bytes: number): string {
    return (bytes / (1024 * 1024)).toFixed(bytes < 10 * 1024 * 1024 ? 1 : 0);
  }

  function open(u: ModUpdate): void {
    void openInSection(u.kind === "Track" ? "tracks" : "cars", u.id);
  }
</script>

<!-- Same anatomy as the import progress (`ImportToasts`), on purpose: the
     download hands over to the import, and the two toasts follow each other in
     the same corner — title = what, phase line + shared bar, "Stop" button. -->
{#if modUpdates.busy?.phase === "download"}
  {@const b = modUpdates.busy}
  <Toast title={b.name} truncate>
    {#snippet actions()}
      <button class="btn-ghost p-cancel" type="button" disabled={modUpdates.cancelling} onclick={cancelUpdateDownload}>
        {modUpdates.cancelling ? t("importOverlay.cancelling") : t("importOverlay.cancel")}
      </button>
    {/snippet}
    <ProgressBar ratio={b.total ? b.received / b.total : null} phase={t("modUpdates.downloadPhase")}>
      {#snippet detail()}
        <span class="mono">
          {b.total
            ? t("modUpdates.sizeOf", { done: mb(b.received), total: mb(b.total) })
            : t("modUpdates.size", { done: mb(b.received) })}
        </span>
      {/snippet}
    </ProgressBar>
  </Toast>
{/if}

{#if fresh.length}
  <Toast
    tone="info"
    icon="⬆"
    title={fresh.length > 1 ? t("modUpdates.toastMany", { n: fresh.length }) : t("modUpdates.toastOne")}
    onclose={markAnnounced}
    closeLabel={t("modUpdates.later")}
  >
    {#each fresh as u (updateKey(u))}
      {@const outcome = modUpdates.outcome[updateKey(u)]}
      <div class="row">
        <button class="name" type="button" onclick={() => open(u)} title={t("modUpdates.openFiche")}>
          {u.name ?? u.id}
        </button>
        <span class="ver mono">{u.installed ?? "?"} → {u.available}</span>
        <button
          class="btn btn-primary"
          type="button"
          disabled={!canStartUpdate()}
          onclick={() => void installUpdate(u)}
          title={u.limited ? t("modUpdates.limitedHint") : undefined}
        >
          {t("modUpdates.update")}
        </button>
        <button class="btn btn-ghost" type="button" onclick={() => ignoreUpdate(u)} title={t("modUpdates.ignoreHint")}>
          {t("modUpdates.ignore")}
        </button>
      </div>
      {#if outcome && "error" in outcome}
        <div class="note err">{outcome.error}</div>
      {:else if outcome}
        <div class="note">{t("modUpdates.inBrowser")}</div>
      {/if}
    {/each}
  </Toast>
{/if}

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 0;
    min-width: 0;
  }
  .name {
    flex: 1;
    min-width: 0;
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    color: var(--txt);
    text-align: left;
    cursor: pointer;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name:hover,
  .name:focus-visible {
    color: var(--rosso-bright);
  }
  .ver {
    flex: none;
    color: var(--muted);
    font-size: 11px;
  }
  .note {
    font-size: 11px;
    color: var(--muted);
    padding: 0 0 4px;
  }
  .note.err {
    color: var(--rosso-bright);
  }
  /* Same button as the import's "Stop". */
  .p-cancel {
    font-size: 11px;
  }
  .p-cancel:disabled {
    color: var(--muted);
    cursor: default;
  }
</style>
