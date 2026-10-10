<script lang="ts">
  import { onMount } from "svelte";
  import { analyzeBulkImport, type BulkAddon, type BulkEntry } from "$lib/library/library";
  import { importState, runBulkImport } from "$lib/workshop/importState.svelte";
  import { bulkCounts, buildExecItems, importCount, isOtherContent } from "$lib/workshop/bulkImport";
  import { t } from "$lib/i18n/index.svelte";

  import { errorText } from "$lib/errors";
  import Seg from "$lib/components/ui/Seg.svelte";
  interface Props {
    parent: string;
    copy: boolean;
    onclose: () => void;
  }
  let { parent, copy, onclose }: Props = $props();

  let entries = $state<BulkEntry[]>([]);
  let loading = $state(true);
  let error = $state("");
  let skipDuplicates = $state(true);
  // Décisions d'arbitrage des cas ambigus : id → "keep_both" | "replace".
  let decisions = $state<Record<string, "keep_both" | "replace">>({});

  const parentName = $derived(parent.split(/[\\/]/).filter(Boolean).pop() ?? parent);

  const counts = $derived(bulkCounts(entries));

  const ambiguousMods = $derived(entries.flatMap((e) => e.mods.filter((m) => m.status === "ambiguous")));

  const toImport = $derived(importCount(entries, skipDuplicates));

  onMount(async () => {
    try {
      entries = await analyzeBulkImport(parent);
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  });

  function setAllAmbiguous(action: "keep_both" | "replace") {
    const d: Record<string, "keep_both" | "replace"> = {};
    for (const m of ambiguousMods) d[m.id] = action;
    decisions = d;
  }

  // The batch goes on in the progress toast (§4.2bis), like any import: the
  // dialog has nothing left to show, and keeping it open blocked the app.
  function execute() {
    void runBulkImport(parentName, buildExecItems(entries, skipDuplicates, decisions), copy);
    onclose();
  }

  function statusLabel(status: string): string {
    switch (status) {
      case "new": return t("bulkImport.statusNew");
      case "update": return t("bulkImport.statusUpdate");
      case "duplicate": return t("bulkImport.statusDuplicate");
      case "ambiguous": return t("bulkImport.statusAmbiguous");
      case "rehydrate": return t("bulkImport.statusRehydrate");
      default: return status;
    }
  }

  function addonLabel(kind: BulkAddon["kind"]): string {
    switch (kind) {
      case "sound": return t("bulkImport.addonSound");
      case "skin": return t("bulkImport.addonSkin");
      case "app": return t("bulkImport.addonApp");
    }
  }
</script>

<div class="backdrop">
  <div class="modal">
    <header>
      <div>
        <h2>{t("import.massTitle")}</h2>
        <div class="dialog-sub mono">{parentName}</div>
      </div>
      <button class="btn-ghost close" type="button" onclick={onclose}>✕</button>
    </header>

    {#if loading}
      <div class="state">{t("bulkImport.analyzing")}</div>
    {:else if error && !entries.length}
      <div class="errbox">{error}</div>
    {:else}
      <!-- Récapitulatif -->
      <div class="counts">
        <span class="ct new">{t("bulkImport.countNew", { count: counts.new })}</span>
        <span class="ct upd">{t("bulkImport.countUpdate", { count: counts.update })}</span>
        <span class="ct dup">{t("bulkImport.countDuplicate", { count: counts.duplicate })}</span>
        <span class="ct amb">{t("bulkImport.countAmbiguous", { count: counts.ambiguous })}</span>
        {#if counts.rehydrate > 0}<span class="ct upd">{t("bulkImport.countRehydrate", { count: counts.rehydrate })}</span>{/if}
        {#if counts.addon > 0}<span class="ct new">{t("bulkImport.countAddon", { count: counts.addon })}</span>{/if}
        {#if counts.other > 0}<span class="ct new">{t("bulkImport.countOther", { count: counts.other })}</span>{/if}
        <span class="ct ign">{t("bulkImport.countIgnored", { count: counts.ignored })}</span>
      </div>

      <!-- Arbitrage groupé -->
      <div class="controls">
        <label class="chk">
          <input type="checkbox" bind:checked={skipDuplicates} />
          <span>{t("bulkImport.skipDuplicates")}</span>
        </label>
        {#if ambiguousMods.length}
          <div class="amb-actions">
            <span class="amb-lbl">{t("bulkImport.ambiguousLabel")}</span>
            <button class="btn-sm" type="button" onclick={() => setAllAmbiguous("keep_both")}>{t("bulkImport.keepAll")}</button>
            <button class="btn-sm" type="button" onclick={() => setAllAmbiguous("replace")}>{t("bulkImport.replaceAll")}</button>
          </div>
        {/if}
      </div>

      <!-- Liste -->
      <div class="list">
        {#each entries as e (e.path)}
          <div class="entry" class:ignored={e.ignored}>
            <div class="e-name">{e.subfolder}</div>
            {#if e.ignored}
              <span class="badge ign">{t("bulkImport.ignoredBadge")}</span>
            {:else if isOtherContent(e)}
              <span class="badge new">{t("bulkImport.statusOther")}</span>
            {:else}
              <div class="e-mods">
                {#each e.mods as m (m.id)}
                  <div class="mod">
                    <span class="badge {m.status}">{statusLabel(m.status)}</span>
                    <span class="m-name">{m.name ?? m.id}</span>
                    {#if m.status === "ambiguous"}
                      <span class="m-conflict">≈ {m.existing_name ?? m.existing_id}</span>
                      <span class="decision">
                        <Seg
                          size="mini"
                          tone="neutral"
                          value={decisions[m.id] ?? "keep_both"}
                          onselect={(v) => (decisions[m.id] = v as "keep_both" | "replace")}
                          items={[
                            { value: "keep_both", label: t("bulkImport.keepBoth") },
                            { value: "replace", label: t("bulkImport.replace") },
                          ]}
                        />
                      </span>
                    {/if}
                  </div>
                {/each}
                {#each e.addons as a, i (i)}
                  <div class="mod">
                    <span class="badge new">{addonLabel(a.kind)}</span>
                    <span class="m-name mono">{a.target}</span>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      </div>

      {#if error}<div class="errbox">{error}</div>{/if}

      <footer>
        <span class="mode mono">{copy ? t("import.copy") : t("import.move")}</span>
        <div class="f-actions">
          <button class="btn" type="button" onclick={onclose}>{t("common.cancel")}</button>
          <button class="btn btn-primary" type="button" onclick={execute} disabled={importState.importing || toImport === 0}>
            {t("bulkImport.importButton", { count: toImport })}
          </button>
        </div>
      </footer>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 60;
    padding: 24px;
  }
  .modal {
    width: 720px;
    max-width: 100%;
    max-height: calc(90 * var(--vh));
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--rosso);
  }
  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    padding: 16px 18px;
    border-bottom: 1px solid var(--line);
  }
  h2 {
    font-size: 15px;
    font-weight: 600;
  }
  .dialog-sub {
    color: var(--muted2);
    font-size: 11px;
    margin-top: 3px;
  }
  .close {
    font-size: 14px;
    padding: 4px 8px;
  }
  .state {
    padding: 40px;
    text-align: center;
    color: var(--muted);
  }
  .counts {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding: 14px 18px;
    border-bottom: 1px solid var(--line);
  }
  .ct {
    font-size: 11px;
    padding: 3px 9px;
    border: 1px solid var(--line);
    font-family: var(--mono);
  }
  .ct.new { color: var(--green); border-color: var(--green-border); }
  .ct.upd { color: var(--yellow); border-color: #4a4426; }
  .ct.dup { color: var(--muted); }
  .ct.amb { color: var(--rosso-bright); border-color: var(--rosso-border); }
  .ct.ign { color: var(--faint); }
  .controls {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 12px 18px;
    border-bottom: 1px solid var(--line);
    flex-wrap: wrap;
  }
  .chk {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 12.5px;
    color: var(--txt2);
    cursor: pointer;
  }
  .amb-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .amb-lbl {
    font-size: 11px;
    color: var(--muted);
  }
  .btn-sm {
    background: var(--raised);
    border: 1px solid var(--line);
    color: var(--txt2);
    font-size: 10.5px;
    padding: 4px 8px;
  }
  .btn-sm:hover {
    border-color: var(--faint);
  }
  .list {
    overflow-y: auto;
    padding: 8px 18px;
  }
  .entry {
    padding: 8px 0;
    border-bottom: 1px solid var(--line);
  }
  .entry.ignored {
    opacity: 0.55;
  }
  .e-name {
    font-size: 12px;
    font-weight: 600;
    color: var(--txt);
    margin-bottom: 4px;
  }
  .e-mods {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .mod {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }
  .badge {
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    padding: 1px 6px;
    border: 1px solid var(--line);
    flex: none;
  }
  .badge.new { color: var(--green); border-color: var(--green-border); }
  .badge.update { color: var(--yellow); border-color: #4a4426; }
  .badge.duplicate { color: var(--muted); }
  .badge.ambiguous { color: var(--rosso-bright); border-color: var(--rosso-border); }
  .badge.ign { color: var(--faint); }
  .m-name {
    color: var(--txt2);
  }
  .m-conflict {
    color: var(--yellow);
    font-size: 11px;
  }
  .decision {
    margin-left: auto;
  }
  .errbox {
    margin: 12px 18px;
  }
  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 18px;
    border-top: 1px solid var(--line);
  }
  .mode {
    color: var(--muted);
    font-size: 11px;
    text-transform: uppercase;
  }
  .f-actions {
    display: flex;
    gap: 10px;
  }
</style>
