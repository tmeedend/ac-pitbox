<script lang="ts">
  // "A newer version exists" on the fiche of a car or a track (§4.7).
  //
  // A strip under the tabs rather than a control in the header: `FicheHeader`
  // keeps a single visible control on purpose, and an update is a notice with
  // two answers, not a permanent action. It sits with the other notices of
  // the fiche (export done, error) and shows on every tab — whoever opens the
  // fiche of an outdated mod should see it, whatever tab they land on.
  //
  // The changelog is fetched only when asked for: the registry list carries
  // version numbers, not texts, and a request per fiche opened would be one
  // per mod browsed.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { t } from "$lib/i18n/index.svelte";
  import { errorText } from "$lib/errors";
  import type { ModKind } from "$lib/library/library";
  import {
    modUpdates,
    updateFor,
    updateKey,
    installUpdate,
    ignoreUpdate,
    canStartUpdate,
    modUpdateDetails,
    type UpdateDetails,
  } from "$lib/library/modUpdates.svelte";

  const { kind, id }: { kind: ModKind; id: string } = $props();

  const update = $derived(updateFor(kind, id));
  const mine = $derived(modUpdates.busy?.kind === kind && modUpdates.busy?.id === id ? modUpdates.busy : null);
  const outcome = $derived(update ? modUpdates.outcome[updateKey(update)] : undefined);

  let showDetails = $state(false);
  let details = $state<UpdateDetails | null>(null);
  let detailsError = $state<string | null>(null);
  let loadingDetails = $state(false);

  // Another mod, or another version of this one: what was loaded no longer
  // describes it. Keyed on a string, not on `update`: a daily re-check builds
  // a new object for the same version, and must not fold an open changelog.
  const signature = $derived(update ? `${updateKey(update)}@${update.available}` : "");
  $effect(() => {
    void signature;
    showDetails = false;
    details = null;
    detailsError = null;
  });

  async function toggleDetails(): Promise<void> {
    showDetails = !showDetails;
    if (!showDetails || details || !update) return;
    loadingDetails = true;
    detailsError = null;
    try {
      details = await modUpdateDetails(update);
    } catch (e) {
      detailsError = errorText(e);
    } finally {
      loadingDetails = false;
    }
  }
</script>

{#if update}
  <div class="upd">
    <div class="line">
      <span class="what">
        {t("modUpdates.available", { version: update.available })}
        {#if update.installed}<span class="inst mono">{t("modUpdates.installed", { version: update.installed })}</span>{/if}
      </span>
      {#if mine}
        <span class="busy">{mine.phase === "download" ? t("modUpdates.phaseDownload") : t("modUpdates.phaseImport")}</span>
      {:else}
        <button class="btn btn-ghost" type="button" onclick={toggleDetails} aria-expanded={showDetails}>
          {t("modUpdates.whatsNew")}
        </button>
        <button class="btn btn-ghost" type="button" onclick={() => ignoreUpdate(update)} title={t("modUpdates.ignoreHint")}>
          {t("modUpdates.ignore")}
        </button>
        <button
          class="btn btn-primary"
          type="button"
          disabled={!canStartUpdate()}
          onclick={() => void installUpdate(update)}
          title={update.limited ? t("modUpdates.limitedHint") : undefined}
        >
          {t("modUpdates.update")}
        </button>
      {/if}
    </div>
    {#if outcome && "error" in outcome}
      <div class="note err">{outcome.error}</div>
    {:else if outcome}
      <div class="note">{t("modUpdates.inBrowser")}</div>
    {/if}
    {#if showDetails}
      <div class="details">
        {#if loadingDetails}
          <span class="note">{t("common.loading")}</span>
        {:else if detailsError}
          <span class="note err">{detailsError}</span>
        {:else if details}
          {#if details.changelog}
            <pre class="changelog">{details.changelog}</pre>
          {:else}
            <span class="note">{t("modUpdates.noChangelog")}</span>
          {/if}
          {#if details.informationUrl}
            <button class="link" type="button" onclick={() => openUrl(details!.informationUrl!).catch(() => {})}>
              {details.author ? t("modUpdates.authorPage", { author: details.author }) : t("modUpdates.authorPageAnon")}
            </button>
          {/if}
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  .upd {
    margin: 10px 18px 0;
    padding: 8px 10px;
    background: var(--blue-dim);
    border: 1px solid var(--blue-border);
    font-size: 11.5px;
  }
  .line {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .what {
    flex: 1;
    min-width: 0;
    color: var(--blue);
    font-weight: 600;
  }
  .inst {
    margin-left: 8px;
    color: var(--muted);
    font-weight: 400;
    font-size: 11px;
  }
  .busy {
    color: var(--muted);
  }
  .note {
    display: block;
    margin-top: 6px;
    font-size: 11px;
    color: var(--muted);
  }
  .note.err {
    color: var(--rosso-bright);
  }
  .details {
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--blue-border);
  }
  /* The author's text as they typed it: line breaks are the list. */
  .changelog {
    margin: 0;
    white-space: pre-wrap;
    font-family: inherit;
    color: var(--txt2);
    line-height: 1.5;
    max-height: 220px;
    overflow-y: auto;
  }
  .link {
    margin-top: 6px;
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    color: var(--blue);
    cursor: pointer;
  }
  .link:hover,
  .link:focus-visible {
    text-decoration: underline;
  }
</style>
