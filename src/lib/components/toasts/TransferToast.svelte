<script lang="ts">
  // The library export in the notification stack (EXPORT§7.2): running (no
  // percentage — the backend walks the library without counting ahead), then
  // what it wrote, with the way to the file. Kept until closed, like the
  // survey's.
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import Toast from "./Toast.svelte";
  import ProgressBar from "$lib/components/ui/ProgressBar.svelte";
  import { dismissExport, exportState } from "$lib/workshop/transferState.svelte";
  import { fmtSize } from "$lib/format";
  import { t } from "$lib/i18n/index.svelte";

  function reveal(path: string): void {
    revealItemInDir(path).catch((e) => console.warn("reveal export", e));
  }
</script>

{#if exportState.running}
  <Toast title={t("transfer.exportTitle")}>
    <ProgressBar ratio={null} phase={t("transfer.exporting")} />
  </Toast>
{:else if exportState.result}
  {@const r = exportState.result}
  <Toast title={t("transfer.exportDone", { size: fmtSize(r.bytes) })} onclose={dismissExport}>
    <div class="acts">
      <button class="btn" type="button" onclick={() => reveal(r.path)}>{t("transfer.openFolder")}</button>
    </div>
  </Toast>
{:else if exportState.error}
  <Toast tone="warn" title={t("transfer.exportTitle")} onclose={dismissExport}>
    <div class="err">{exportState.error}</div>
  </Toast>
{/if}

<style>
  .acts {
    display: flex;
    margin-top: 8px;
  }
  .err {
    font-size: 12px;
    color: var(--txt2);
  }
</style>
