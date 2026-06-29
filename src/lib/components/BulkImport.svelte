<script lang="ts">
  import { onMount } from "svelte";
  import {
    analyzeBulkImport,
    executeBulkImport,
    type ArchiveResult,
    type BulkEntry,
    type BulkExecItem,
  } from "$lib/library";

  interface Props {
    parent: string;
    copy: boolean;
    onclose: () => void;
    ondone: (report: ArchiveResult[]) => void;
  }
  let { parent, copy, onclose, ondone }: Props = $props();

  let entries = $state<BulkEntry[]>([]);
  let loading = $state(true);
  let running = $state(false);
  let error = $state("");
  let skipDuplicates = $state(true);
  // Décisions d'arbitrage des cas ambigus : id → "keep_both" | "replace".
  let decisions = $state<Record<string, "keep_both" | "replace">>({});

  const parentName = $derived(parent.split(/[\\/]/).filter(Boolean).pop() ?? parent);

  const counts = $derived.by(() => {
    const c = { new: 0, update: 0, duplicate: 0, ambiguous: 0, ignored: 0 };
    for (const e of entries) {
      if (e.ignored) c.ignored++;
      for (const m of e.mods) c[m.status]++;
    }
    return c;
  });

  const ambiguousMods = $derived(entries.flatMap((e) => e.mods.filter((m) => m.status === "ambiguous")));

  const toImport = $derived.by(() => {
    let n = 0;
    for (const e of entries) {
      if (e.ignored) continue;
      for (const m of e.mods) {
        if (m.status === "duplicate" && skipDuplicates) continue;
        n++;
      }
    }
    return n;
  });

  onMount(async () => {
    try {
      entries = await analyzeBulkImport(parent);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  });

  function setAllAmbiguous(action: "keep_both" | "replace") {
    const d: Record<string, "keep_both" | "replace"> = {};
    for (const m of ambiguousMods) d[m.id] = action;
    decisions = d;
  }

  function buildItems(): BulkExecItem[] {
    return entries
      .filter((e) => !e.ignored)
      .map((e) => ({
        path: e.path,
        skip_ids: skipDuplicates ? e.mods.filter((m) => m.status === "duplicate").map((m) => m.id) : [],
        replace_ids: e.mods.filter((m) => m.status === "ambiguous" && decisions[m.id] === "replace").map((m) => m.id),
      }));
  }

  async function execute() {
    if (running) return;
    running = true;
    error = "";
    try {
      const report = await executeBulkImport(buildItems(), copy);
      ondone(report);
    } catch (e) {
      error = String(e);
      running = false;
    }
  }

  const statusLabel: Record<string, string> = {
    new: "Nouveau",
    update: "Mise à jour",
    duplicate: "Doublon",
    ambiguous: "Ambigu",
  };
</script>

<div class="backdrop">
  <div class="modal">
    <header>
      <div>
        <h2>Import en masse</h2>
        <div class="sub mono">{parentName}</div>
      </div>
      <button class="btn-ghost close" type="button" onclick={onclose}>✕</button>
    </header>

    {#if loading}
      <div class="state">Analyse du dossier…</div>
    {:else if error && !entries.length}
      <div class="err">{error}</div>
    {:else}
      <!-- Récapitulatif -->
      <div class="counts">
        <span class="ct new">{counts.new} nouveau(x)</span>
        <span class="ct upd">{counts.update} màj</span>
        <span class="ct dup">{counts.duplicate} doublon(s)</span>
        <span class="ct amb">{counts.ambiguous} ambigu(s)</span>
        <span class="ct ign">{counts.ignored} ignoré(s)</span>
      </div>

      <!-- Arbitrage groupé -->
      <div class="controls">
        <label class="chk">
          <input type="checkbox" bind:checked={skipDuplicates} />
          <span>Ignorer les doublons</span>
        </label>
        {#if ambiguousMods.length}
          <div class="amb-actions">
            <span class="amb-lbl">Cas ambigus :</span>
            <button class="btn-sm" type="button" onclick={() => setAllAmbiguous("keep_both")}>Tout garder</button>
            <button class="btn-sm" type="button" onclick={() => setAllAmbiguous("replace")}>Tout écraser</button>
          </div>
        {/if}
      </div>

      <!-- Liste -->
      <div class="list">
        {#each entries as e (e.path)}
          <div class="entry" class:ignored={e.ignored}>
            <div class="e-name">{e.subfolder}</div>
            {#if e.ignored}
              <span class="badge ign">ignoré — pas de mod AC</span>
            {:else}
              <div class="e-mods">
                {#each e.mods as m (m.id)}
                  <div class="mod">
                    <span class="badge {m.status}">{statusLabel[m.status]}</span>
                    <span class="m-name">{m.name ?? m.id}</span>
                    {#if m.status === "ambiguous"}
                      <span class="m-conflict">≈ {m.existing_name ?? m.existing_id}</span>
                      <span class="seg-mini">
                        <button class:on={(decisions[m.id] ?? "keep_both") === "keep_both"} onclick={() => (decisions[m.id] = "keep_both")}>Garder les 2</button>
                        <button class:on={decisions[m.id] === "replace"} onclick={() => (decisions[m.id] = "replace")}>Écraser</button>
                      </span>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      </div>

      {#if error}<div class="err">{error}</div>{/if}

      <footer>
        <span class="mode mono">{copy ? "Copier" : "Déplacer"}</span>
        <div class="f-actions">
          <button class="btn" type="button" onclick={onclose} disabled={running}>Annuler</button>
          <button class="btn btn-primary" type="button" onclick={execute} disabled={running || toImport === 0}>
            {running ? "Import en cours…" : `Importer ${toImport} mod(s)`}
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
    max-height: 90vh;
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
  .sub {
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
  .seg-mini {
    display: flex;
    margin-left: auto;
    border: 1px solid var(--line);
  }
  .seg-mini button {
    background: var(--panel2);
    color: var(--muted);
    font-size: 10px;
    padding: 3px 7px;
    border-right: 1px solid var(--line);
  }
  .seg-mini button:last-child {
    border-right: none;
  }
  .seg-mini button.on {
    background: var(--raised);
    color: var(--rosso-bright);
  }
  .err {
    margin: 12px 18px;
    padding: 9px 11px;
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    font-size: 12px;
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
