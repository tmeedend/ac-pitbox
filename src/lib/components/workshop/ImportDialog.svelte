<script lang="ts">
  // Importing a library (EXPORT§7.3): the export read, the refusal said with
  // what to do, the parts to take and — into a library that exists — what the
  // merge will do and the choices it needs (EXPORT§7.4), then the import:
  // modal and not dismissable while it runs, since it writes the base and the
  // settings, and a gesture elsewhere in the meantime could write over them.
  // It ends on its report, then the app reloaded, so that every screen reads
  // what came in.
  import { t, i18n } from "$lib/i18n/index.svelte";
  import { fmtSize } from "$lib/format";
  import { errorText } from "$lib/errors";
  import { reloadUiPrefs } from "$lib/uiPrefs.svelte";
  import LoadingState from "$lib/components/ui/LoadingState.svelte";
  import Modal from "$lib/components/ui/Modal.svelte";
  import ProgressBar from "$lib/components/ui/ProgressBar.svelte";
  import TransferParts from "./TransferParts.svelte";
  import MergeChoices from "./MergeChoices.svelte";
  import ImportReportView from "./ImportReportView.svelte";
  import {
    defaultChoices,
    effectiveParts,
    importLibrary,
    initialImportParts,
    inspectExport,
    mergeToShow,
    type Choices,
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
  let choices = $state<Choices>(defaultChoices());
  let running = $state(false);
  let report = $state<ImportReport | null>(null);
  let error = $state("");

  $effect(() => {
    const file = path;
    inspectExport(file)
      .then((i) => {
        inspection = i;
        selected = initialImportParts(i.manifest.parts, i.plan?.merging ?? false);
      })
      .catch((e) => (error = errorText(e)));
  });

  const manifest = $derived(inspection?.manifest ?? null);
  const refusal = $derived(inspection?.refusal ?? null);
  const plan = $derived(inspection?.plan ?? null);
  const parts = $derived(effectiveParts(selected));

  async function go(): Promise<void> {
    running = true;
    error = "";
    try {
      report = await importLibrary(path, parts, choices);
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

<Modal title={t("transfer.importTitle")} onclose={running || report ? undefined : onclose} width="600px">
  {#if report}
    <ImportReportView {report} />
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
    {#if plan && mergeToShow(plan) && parts.includes("library")}
      <MergeChoices {plan} bind:choices disabled={running} />
    {/if}
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
  .refusal {
    color: var(--txt2);
    font-size: 12px;
    line-height: 1.5;
  }
</style>
