<script lang="ts">
  // Écran Maintenance (§9.3) : détection assistée des mods cassés et des
  // junctions orphelines, suppression sur confirmation.
  import {
    maintenanceScan,
    deleteBrokenMod,
    removeOrphanJunction,
    reindexLibrary,
    repairAll,
    purgeOrphanSubs,
    type MaintenanceReport,
  } from "$lib/maintenance";
  import { indexStockContent } from "$lib/submods";
  import { t } from "$lib/i18n/index.svelte";

  import { errorText } from "$lib/errors";
  let report = $state<MaintenanceReport | null>(null);
  let scanning = $state(false);
  let busy = $state<string | null>(null);
  let error = $state("");
  let indexing = $state(false);
  let indexMsg = $state("");
  let reindexing = $state(false);
  let reindexMsg = $state("");
  let recalcSize = $state(false);
  let repairing = $state(false);
  let repairMsg = $state("");
  let reinstallBroken = $state(false);
  let reinstallFailures = $state<{ id: string; name: string; reason: string }[]>([]);
  let projectionFailures = $state<string[]>([]);

  async function doReindex() {
    reindexing = true;
    error = "";
    reindexMsg = "";
    try {
      const n = await reindexLibrary(recalcSize);
      reindexMsg = t("maintenance.reindexDone", { count: n });
    } catch (e) {
      error = errorText(e);
    } finally {
      reindexing = false;
    }
  }

  async function doIndexStock() {
    indexing = true;
    error = "";
    indexMsg = "";
    try {
      const n = await indexStockContent();
      indexMsg = n > 0 ? t("maintenance.stockIndexed", { count: n }) : t("maintenance.stockNoneNew");
    } catch (e) {
      error = errorText(e);
    } finally {
      indexing = false;
    }
  }

  // Nettoyage des sous-éléments sans parent (§9.3) : jamais automatique — ils
  // sont conservés à la suppression d'un mod pour qu'un réimport du même id les
  // retrouve, ce qui n'a plus d'intérêt une fois le parent définitivement parti.
  async function doPurgeOrphanSubs() {
    busy = "orphan-subs";
    error = "";
    try {
      await purgeOrphanSubs();
      await scan();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = "";
    }
  }

  async function doRepair() {
    repairing = true;
    error = "";
    repairMsg = "";
    reinstallFailures = [];
    projectionFailures = [];
    try {
      const r = await repairAll(reinstallBroken);
      const parts = [
        t("maintenance.repairProjectionsDone", { repaired: r.projections.repaired, alreadyOk: r.projections.already_ok }),
      ];
      if (r.projections.failed.length) {
        parts.push(t("maintenance.repairProjectionsFailed", { count: r.projections.failed.length }));
        projectionFailures = r.projections.failed;
      }
      parts.push(t("maintenance.repairRedeployedDone", { count: r.redeployed }));
      if (r.redeploy_errors.length) {
        parts.push(t("maintenance.repairRedeployFailed", { count: r.redeploy_errors.length }));
      }
      if (reinstallBroken) {
        parts.push(t("maintenance.repairReinstalledDone", { count: r.reinstalled.length }));
        if (r.reinstall_errors.length) {
          parts.push(t("maintenance.repairReinstallFailed", { count: r.reinstall_errors.length }));
        }
      }
      repairMsg = parts.join(" ");
      // Rafraîchit toujours après une réparation : une réinstallation réussie
      // change l'état des mods cassés, et sert aussi à retrouver le nom des
      // mods en échec ci-dessous (le rapport de repairAll ne connaît que leur id).
      await scan();
      if (reinstallBroken && r.reinstall_errors.length) {
        const names = new Map((report?.broken ?? []).map((b) => [b.id, b.name ?? b.id]));
        reinstallFailures = r.reinstall_errors.map((e) => ({
          id: e.id,
          name: names.get(e.id) ?? e.id,
          reason: e.error,
        }));
      }
    } catch (e) {
      error = errorText(e);
    } finally {
      repairing = false;
    }
  }

  async function scan() {
    scanning = true;
    error = "";
    try {
      report = await maintenanceScan();
    } catch (e) {
      error = errorText(e);
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
      error = errorText(e);
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
      error = errorText(e);
    } finally {
      busy = null;
    }
  }

  const isClean = $derived(
    report && report.broken.length === 0 && report.orphans.length === 0 && report.orphan_subs.length === 0,
  );
</script>

<div class="maint">
  <header class="head">
    <div>
      <h2 class="lbl-screen">{t("nav.maintenance")}</h2>
      <p class="sub">{t("maintenance.subtitle")}</p>
    </div>
    <button class="btn btn-primary" type="button" onclick={scan} disabled={scanning}>
      {scanning ? t("maintenance.scanning") : t("maintenance.scan")}
    </button>
  </header>

  {#if error}<div class="err">{error}</div>{/if}

  <section class="stock-sec">
    <h3>{t("maintenance.stockTitle")}</h3>
    <p class="hint">{t("maintenance.stockHint")}</p>
    <div class="stock-row">
      <button class="btn" type="button" onclick={doIndexStock} disabled={indexing}>
        {indexing ? t("maintenance.indexing") : t("maintenance.indexStock")}
      </button>
      {#if indexMsg}<span class="stock-msg">{indexMsg}</span>{/if}
    </div>
  </section>

  <section class="stock-sec">
    <h3>{t("maintenance.reindexTitle")}</h3>
    <p class="hint">{t("maintenance.reindexHint")}</p>
    <label class="recalc-check">
      <input type="checkbox" bind:checked={recalcSize} />
      <span>{t("maintenance.recalcSize")}</span>
    </label>
    <div class="stock-row">
      <button class="btn" type="button" onclick={doReindex} disabled={reindexing}>
        {reindexing ? t("maintenance.reindexing") : t("maintenance.reindex")}
      </button>
      {#if reindexMsg}<span class="stock-msg">{reindexMsg}</span>{/if}
    </div>
  </section>

  <section class="stock-sec">
    <h3>{t("maintenance.repairTitle")}</h3>
    <p class="hint">{t("maintenance.repairHint")}</p>
    <label class="recalc-check">
      <input type="checkbox" bind:checked={reinstallBroken} />
      <span>{t("maintenance.repairReinstallOption")}</span>
    </label>
    <div class="stock-row">
      <button class="btn" type="button" onclick={doRepair} disabled={repairing}>
        {repairing ? t("maintenance.repairing") : t("maintenance.repair")}
      </button>
      {#if repairMsg}<span class="stock-msg">{repairMsg}</span>{/if}
    </div>
    {#if projectionFailures.length}
      <ul class="list repair-fail-list">
        {#each projectionFailures as f (f)}
          <li>
            <div class="l-main">
              <span class="l-name">{f.split(": ")[0]}</span>
              <span class="l-reason">{errorText(f.split(": ").slice(1).join(": "))}</span>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
    {#if reinstallFailures.length}
      <ul class="list repair-fail-list">
        {#each reinstallFailures as f (f.id)}
          <li>
            <div class="l-main">
              <span class="l-name">{f.name}</span>
              <span class="l-reason">{errorText(f.reason)}</span>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  {#if report}
    {#if isClean}
      <div class="ok">✓ {t("maintenance.clean")}</div>
    {/if}

    {#if report.broken.length}
      <section>
        <h3>{t("maintenance.brokenTitle")} <span class="count mono">{report.broken.length}</span></h3>
        <p class="hint">{t("maintenance.brokenHint")}</p>
        <ul class="list">
          {#each report.broken as b (b.id)}
            <li>
              <div class="l-main">
                <span class="l-name">{b.name ?? b.id}</span>
                <span class="l-kind mono">{b.kind === "Track" ? t("library.typeTrack") : t("library.typeCar")}</span>
                <span class="l-reason">{t(b.reason)}</span>
              </div>
              <button class="btn danger" type="button" onclick={() => removeBroken(b.id)} disabled={busy === b.id}>
                {busy === b.id ? t("common.working") : t("common.delete")}
              </button>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    {#if report.orphan_subs.length}
      <section>
        <h3>{t("maintenance.orphanSubsTitle")} <span class="count mono">{report.orphan_subs.length}</span></h3>
        <p class="hint">{t("maintenance.orphanSubsHint")}</p>
        <ul class="list">
          {#each report.orphan_subs as o (o.id)}
            <li>
              <div class="l-main">
                <span class="l-name mono">{o.name}</span>
                <span class="l-kind mono">{o.sub_type}</span>
                <span class="l-path mono">{o.parent_id}</span>
              </div>
            </li>
          {/each}
        </ul>
        <div class="row">
          <button class="btn danger" type="button" onclick={doPurgeOrphanSubs} disabled={busy === "orphan-subs"}>
            {busy === "orphan-subs" ? t("common.working") : t("maintenance.purgeOrphanSubs")}
          </button>
        </div>
      </section>
    {/if}

    {#if report.orphans.length}
      <section>
        <h3>{t("maintenance.orphansTitle")} <span class="count mono">{report.orphans.length}</span></h3>
        <p class="hint">{t("maintenance.orphansHint")}</p>
        <ul class="list">
          {#each report.orphans as o (o.path)}
            <li>
              <div class="l-main">
                <span class="l-name mono">{o.id}</span>
                <span class="l-kind mono">{o.kind === "Track" ? t("library.typeTrack") : t("library.typeCar")}</span>
                <span class="l-path mono">{o.path}</span>
              </div>
              <button class="btn danger" type="button" onclick={() => removeOrphan(o.kind, o.id)} disabled={busy === `${o.kind}/${o.id}`}>
                {busy === `${o.kind}/${o.id}` ? t("common.working") : t("common.remove")}
              </button>
            </li>
          {/each}
        </ul>
      </section>
    {/if}
  {:else if !scanning}
    <div class="empty">{t("maintenance.emptyHint")}</div>
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
  /* Taille/graisse viennent de `.lbl-screen` (global, §chantier libellés). */
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
  .recalc-check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--txt2);
    cursor: pointer;
    margin-bottom: 10px;
  }
  .stock-msg {
    color: var(--green);
    font-size: 12px;
  }
  .repair-fail-list {
    margin-top: 10px;
  }
</style>
