<script lang="ts">
  // Exporting the library (EXPORT§7.2): the parts with what each holds, the
  // weight before anything is written, what will not leave, then the file.
  // The writing itself runs in the background (`transferState`): the dialog
  // closes as soon as the file is chosen.
  import { save } from "@tauri-apps/plugin-dialog";
  import { t } from "$lib/i18n/index.svelte";
  import { fmtSize } from "$lib/format";
  import { errorText } from "$lib/errors";
  import LoadingState from "$lib/components/ui/LoadingState.svelte";
  import Modal from "$lib/components/ui/Modal.svelte";
  import TransferParts from "./TransferParts.svelte";
  import {
    effectiveParts,
    estimateTransfer,
    exportFileName,
    PARTS,
    type Estimate,
    type Part,
  } from "$lib/workshop/transfer";
  import { runExport } from "$lib/workshop/transferState.svelte";

  interface Props {
    onclose: () => void;
  }
  let { onclose }: Props = $props();

  let estimate = $state<Estimate | null>(null);
  let error = $state("");
  let selected = $state<Part[]>([...PARTS]);

  const parts = $derived(effectiveParts(selected));
  const size = $derived(parts.reduce((n, p) => n + (estimate?.bytes[p] ?? 0), 0));

  $effect(() => {
    estimateTransfer()
      .then((e) => (estimate = e))
      .catch((e) => (error = errorText(e)));
  });

  async function go(): Promise<void> {
    const path = await save({
      title: t("transfer.exportTitle"),
      defaultPath: exportFileName(new Date()),
      filters: [{ name: t("transfer.fileFilter"), extensions: ["pitbox"] }],
    });
    if (!path) return;
    onclose();
    await runExport(path, parts);
  }
</script>

<Modal title={t("transfer.exportTitle")} {onclose} width="540px">
  {#if error}
    <div class="errbox">{error}</div>
  {:else if !estimate}
    <LoadingState />
  {:else}
    <TransferParts available={PARTS} counts={estimate.counts} bytes={estimate.bytes} bind:selected />
    <p class="size">{t("transfer.estimate", { size: fmtSize(size) })}</p>
    <p class="note">{t("transfer.notExported")}</p>
    <p class="note muted">{t("transfer.contentNote")}</p>
  {/if}
  {#snippet footer()}
    <button class="btn" type="button" onclick={onclose}>{t("common.cancel")}</button>
    <button class="btn btn-primary" type="button" disabled={!estimate || parts.length === 0} onclick={go}>
      {t("transfer.exportGo")}
    </button>
  {/snippet}
</Modal>

<style>
  .size {
    color: var(--txt2);
    font-size: 12px;
  }
  .note {
    color: var(--txt2);
    font-size: 11.5px;
    line-height: 1.45;
  }
  .note.muted {
    color: var(--muted);
  }
</style>
