<script lang="ts">
  // Écran Maintenance (§9.3) : détection assistée des mods cassés et des
  // junctions orphelines, suppression sur confirmation.
  import {
    maintenanceScan,
    deleteBrokenMod,
    removeOrphanJunction,
    type MaintenanceReport,
  } from "$lib/maintenance";
  import { indexStockContent } from "$lib/submods";

  let report = $state<MaintenanceReport | null>(null);
  let scanning = $state(false);
  let busy = $state<string | null>(null);
  let error = $state("");
  let indexing = $state(false);
  let indexMsg = $state("");

  async function doIndexStock() {
    indexing = true;
    error = "";
    indexMsg = "";
    try {
      const n = await indexStockContent();
      indexMsg = n > 0
        ? `${n} élément(s) de contenu de base indexé(s).`
        : "Aucun nouveau contenu de base à indexer.";
    } catch (e) {
      error = String(e);
    } finally {
      indexing = false;
    }
  }

  async function scan() {
    scanning = true;
    error = "";
    try {
      report = await maintenanceScan();
    } catch (e) {
      error = String(e);
    } finally {
      scanning = false;
    }
  }

  async function removeBroken(id: string) {
    busy = id;
    error = "";
    try {
      await deleteBrokenMod(id);
      await scan();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  async function removeOrphan(kind: string, id: string) {
    busy = `${kind}/${id}`;
    error = "";
    try {
      await removeOrphanJunction(kind, id);
      await scan();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  const isClean = $derived(report && report.broken.length === 0 && report.orphans.length === 0);
</script>

<div class="maint">
  <header class="head">
    <div>
      <h2>Maintenance</h2>
      <p class="sub">Détecte les mods cassés (fichiers manquants) et les junctions orphelines (§9.3). Rien n'est supprimé sans ton action.</p>
    </div>
    <button class="btn btn-primary" type="button" onclick={scan} disabled={scanning}>
      {scanning ? "Analyse…" : "Analyser"}
    </button>
  </header>

  {#if error}<div class="err">{error}</div>{/if}

  <section class="stock-sec">
    <h3>Contenu de base (Kunos)</h3>
    <p class="hint">Indexe les voitures/circuits de base présents dans content/ (§12bis.1) — lecture seule, non désactivables. Permet aux skins et sons de s'y rattacher.</p>
    <div class="stock-row">
      <button class="btn" type="button" onclick={doIndexStock} disabled={indexing}>
        {indexing ? "Indexation…" : "Indexer le contenu de base"}
      </button>
      {#if indexMsg}<span class="stock-msg">{indexMsg}</span>{/if}
    </div>
  </section>

  {#if report}
    {#if isClean}
      <div class="ok">✓ Aucun problème détecté. Bibliothèque et junctions cohérentes.</div>
    {/if}

    {#if report.broken.length}
      <section>
        <h3>Mods cassés <span class="count mono">{report.broken.length}</span></h3>
        <p class="hint">Fichiers de la version active introuvables ou structure invalide. Supprimer retire le mod de l'overlay et ses fichiers de bibliothèque.</p>
        <ul class="list">
          {#each report.broken as b (b.id)}
            <li>
              <div class="l-main">
                <span class="l-name">{b.name ?? b.id}</span>
                <span class="l-kind mono">{b.kind === "Track" ? "Circuit" : "Voiture"}</span>
                <span class="l-reason">{b.reason}</span>
              </div>
              <button class="btn danger" type="button" onclick={() => removeBroken(b.id)} disabled={busy === b.id}>
                {busy === b.id ? "…" : "Supprimer"}
              </button>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    {#if report.orphans.length}
      <section>
        <h3>Junctions orphelines <span class="count mono">{report.orphans.length}</span></h3>
        <p class="hint">Liens dans content/ pointant vers une version supprimée. Les retirer ne touche qu'au lien (garde-fou : jamais un vrai dossier).</p>
        <ul class="list">
          {#each report.orphans as o (o.path)}
            <li>
              <div class="l-main">
                <span class="l-name mono">{o.id}</span>
                <span class="l-kind mono">{o.kind === "Track" ? "Circuit" : "Voiture"}</span>
                <span class="l-path mono">{o.path}</span>
              </div>
              <button class="btn danger" type="button" onclick={() => removeOrphan(o.kind, o.id)} disabled={busy === `${o.kind}/${o.id}`}>
                {busy === `${o.kind}/${o.id}` ? "…" : "Retirer"}
              </button>
            </li>
          {/each}
        </ul>
      </section>
    {/if}
  {:else if !scanning}
    <div class="empty">Lance une analyse pour détecter les problèmes.</div>
  {/if}
</div>

<style>
  .maint {
    max-width: 820px;
  }
  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 20px;
    margin-bottom: 20px;
  }
  h2 {
    font-size: 18px;
    font-weight: 600;
  }
  .sub {
    color: var(--muted);
    font-size: 12px;
    margin-top: 6px;
    line-height: 1.5;
    max-width: 560px;
  }
  .err {
    background: var(--rosso-dim);
    border: 1px solid var(--rosso-border);
    color: var(--rosso-bright);
    padding: 10px 12px;
    font-size: 12px;
    margin-bottom: 16px;
  }
  .ok {
    background: var(--green-dim);
    border: 1px solid var(--green-border);
    color: var(--green);
    padding: 12px 14px;
    font-size: 13px;
  }
  section {
    margin-bottom: 24px;
  }
  h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--muted);
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
  }
  .count {
    color: var(--rosso-bright);
  }
  .hint {
    font-size: 11.5px;
    color: var(--faint);
    margin-bottom: 10px;
    line-height: 1.5;
  }
  .list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .list li {
    display: flex;
    align-items: center;
    gap: 12px;
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 8px 12px;
  }
  .l-main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .l-name {
    font-weight: 600;
    color: var(--txt);
    font-size: 12.5px;
  }
  .l-kind {
    color: var(--muted);
    font-size: 10px;
  }
  .l-reason {
    color: var(--yellow);
    font-size: 11.5px;
  }
  .l-path {
    color: var(--muted2);
    font-size: 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }
  .btn {
    background: var(--raised);
    color: var(--txt2);
    border: 1px solid var(--line);
    font-size: 11px;
    padding: 6px 12px;
    flex: none;
  }
  .btn.btn-primary {
    background: var(--rosso);
    color: #fff;
    border-color: var(--rosso);
  }
  .btn.danger:hover {
    border-color: var(--rosso-border);
    color: var(--rosso-bright);
  }
  .btn:disabled {
    opacity: 0.5;
  }
  .empty {
    color: var(--muted);
    padding: 40px 0;
    text-align: center;
  }
  .stock-sec {
    border: 1px solid var(--line);
    background: var(--panel2);
    padding: 12px 14px;
    margin-bottom: 20px;
  }
  .stock-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .stock-msg {
    color: var(--green);
    font-size: 12px;
  }
</style>
