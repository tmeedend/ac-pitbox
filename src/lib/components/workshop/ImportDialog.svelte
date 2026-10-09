<script lang="ts">
  // Importing a library (EXPORT§7.3): the manifest read alone, the refusal
  // said with what to do, the parts to take, then the import — modal and not
  // dismissable while it runs: it replaces the base and the settings, and a
  // gesture elsewhere in the meantime could write over them. It ends on the
  // app reloaded, so that every screen reads what came in.
  import { t, i18n } from "$lib/i18n/index.svelte";
  import { fmtSize } from "$lib/format";
  import { errorText } from "$lib/errors";
  import { reloadUiPrefs } from "$lib/uiPrefs.svelte";
  import LoadingState from "$lib/components/ui/LoadingState.svelte";
  import Modal from "$lib/components/ui/Modal.svelte";
  import ProgressBar from "$lib/components/ui/ProgressBar.svelte";
  import TransferParts from "./TransferParts.svelte";
  import {
    effectiveParts,
    importLibrary,
    inspectExport,
    type ImportReport,
    type Inspection,
    type Part,
  } from "$lib/workshop/transfer";

  interface Props {
    /** The `.pitbox` file chosen. */
    path: string;
    onclose: () => void;
  }
  let { path, onclose }: Props = $props();

  let inspection = $state<Inspection | null>(null);
  let selected = $state<Part[]>([]);
  let running = $state(false);
  let report = $state<ImportReport | null>(null);
  let error = $state("");

  $effect(() => {
    const file = path;
    inspectExport(file)
      .then((i) => {
        inspection = i;
        selected = [...i.manifest.parts];
      })
      .catch((e) => (error = errorText(e)));
  });

  const manifest = $derived(inspection?.manifest ?? null);
  const refusal = $derived(inspection?.refusal ?? null);
  const parts = $derived(effectiveParts(selected));

  async function go(): Promise<void> {
    running = true;
    error = "";
    try {
      report = await importLibrary(path, parts);
      // Before anything else can write a setting from the old copy.
      await reloadUiPrefs();
    } catch (e) {
      error = errorText(e);
    } finally {
      running = false;
    }
  }

  function exportedOn(m: NonNullable<typeof manifest>): string {
    return t("transfer.exportedOn", {
      date: new Date(m.exported_at).toLocaleDateString(i18n.locale),
      version: m.app_version,
    });
  }
</script>

<Modal title={t("transfer.importTitle")} onclose={running || report ? undefined : onclose} width="540px">
  {#if report}
    <p class="line">{t("transfer.importedMods", { count: report.mods })}</p>
    {#if report.apps + report.others}
      <p class="line">{t("transfer.importedAddons", { count: report.apps + report.others })}</p>
    {/if}
    {#if report.stock_applied}
      <p class="line">{t("transfer.stockApplied", { count: report.stock_applied })}</p>
    {/if}
    {#if report.stock_missing.length}
      <p class="line">
        {t("transfer.stockMissing", { count: report.stock_missing.length })}
        <span class="mono ids">{report.stock_missing.join(", ")}</span>
      </p>
    {/if}
    {#if report.presets_renamed}
      <p class="line">{t("transfer.presetsRenamed", { count: report.presets_renamed })}</p>
    {/if}
    {#if report.active_at_export}
      <p class="line muted">{t("transfer.activeAtExport", { count: report.active_at_export })}</p>
    {/if}
  {:else if error && !inspection}
    <div class="errbox">{error}</div>
  {:else if !inspection || !manifest}
    <LoadingState />
  {:else if refusal}
    <p class="refusal">{t(refusal.key, { count: refusal.count ?? 0, version: refusal.version ?? "" })}</p>
  {:else}
    <p class="line">{exportedOn(manifest)}</p>
    {#if manifest.library_bytes_at_export}
      <p class="line muted">
        {t("transfer.weighed", { size: fmtSize(manifest.library_bytes_at_export), file: fmtSize(inspection.file_bytes) })}
      </p>
    {/if}
    <TransferParts available={manifest.parts} counts={manifest.counts} bind:selected disabled={running} />
    {#if running}
      <ProgressBar ratio={null} phase={t("transfer.importing")} />
    {/if}
    {#if error}<div class="errbox">{error}</div>{/if}
  {/if}
  {#snippet footer()}
    {#if report}
      <button class="btn btn-primary" type="button" onclick={() => location.reload()}>{t("transfer.openLibrary")}</button>
    {:else}
      <button class="btn" type="button" disabled={running} onclick={onclose}>{t("common.cancel")}</button>
      {#if inspection && !refusal}
        <button class="btn btn-primary" type="button" disabled={running || parts.length === 0} onclick={go}>
          {t("transfer.importGo")}
        </button>
      {/if}
    {/if}
  {/snippet}
</Modal>

<style>
  .line {
    color: var(--txt2);
    font-size: 12px;
    line-height: 1.45;
  }
  .line.muted {
    color: var(--muted);
    font-size: 11.5px;
  }
  .ids {
    display: block;
    color: var(--muted);
    font-size: 11px;
    overflow-wrap: anywhere;
  }
  .refusal {
    color: var(--txt2);
    font-size: 12px;
    line-height: 1.5;
  }
</style>
