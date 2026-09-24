<script lang="ts">
  // The last catalogue update at the head of the Workshop's rule tabs
  // (REGLES§6.2, REGLES§6.3): what was added, corrected and retired, each line
  // in the rules' own words, and the mods it reclassified - the measured
  // effect, which is what lets one judge. Also the state "you went back to
  // the previous catalogue", with the way back to the current one.
  //
  // Folded by default: the counts are the news, the detail is for who wants
  // it. Not remembered - it is a report, not a preference.
  import { t } from "$lib/i18n/index.svelte";
  import {
    catalogReport,
    dismissCatalogReport,
    sameVersion,
    setCatalogReverted,
    type CatalogChange,
  } from "$lib/workshop/catalogReport.svelte";

  let open = $state(false);
  const view = $derived(catalogReport.view);
  const report = $derived(view?.report ?? null);

  const groups = $derived(
    report
      ? ([
          ["catalogReport.added", report.changes.added],
          ["catalogReport.corrected", report.changes.corrected],
          ["catalogReport.retired", report.changes.retired],
        ] as [string, CatalogChange[]][]).filter(([, l]) => l.length)
      : [],
  );
</script>

{#if view?.reverted}
  <div class="banner">
    <div class="head">
      <span class="title">{t("catalogReport.reverted", { version: view.previous_version ?? "" })}</span>
      <button type="button" class="btn" disabled={catalogReport.busy} onclick={() => void setCatalogReverted(false)}>
        {catalogReport.busy ? t("catalogReport.applying") : t("catalogReport.reapply", { version: view.current_version ?? "" })}
      </button>
    </div>
    {#if catalogReport.error}<div class="err">{catalogReport.error}</div>{/if}
  </div>
{:else if report && view}
  {@const c = report.changes}
  <div class="banner">
    <div class="head">
      <span class="title">
        ⚑ {sameVersion(report)
          ? t("catalogReport.titleSameVersion")
          : t("catalogReport.title", { from: report.from_version, to: report.to_version })}
      </span>
      <span class="n">
        {t("catalogReport.counts", { added: c.added.length, corrected: c.corrected.length, retired: c.retired.length })}
        · {t("catalogReport.reclassified", { count: report.reclassified.length })}
      </span>
      <span class="acts">
        <button type="button" class="btn" aria-expanded={open} onclick={() => (open = !open)}>
          {open ? t("catalogReport.hideDetails") : t("catalogReport.details")}
        </button>
        {#if view.can_revert}
          <button type="button" class="btn" disabled={catalogReport.busy} onclick={() => void setCatalogReverted(true)}>
            {catalogReport.busy ? t("catalogReport.applying") : t("catalogReport.revert", { version: report.from_version })}
          </button>
        {/if}
        <button type="button" class="x" title={t("catalogReport.close")} onclick={() => void dismissCatalogReport()}
          >×</button
        >
      </span>
    </div>
    {#if catalogReport.error}<div class="err">{catalogReport.error}</div>{/if}
    {#if open}
      <div class="detail">
        {#each groups as [key, list] (key)}
          <div class="group">
            <span class="lbl-key">{t(key, { count: list.length })}</span>
            <ul>
              {#each list as ch (`${ch.list}:${ch.key}`)}
                <li><span class="kind">{t(`catalogReport.list.${ch.list}`)}</span><span class="mono">{ch.label}</span></li>
              {/each}
            </ul>
          </div>
        {/each}
        {#if report.reclassified.length}
          <div class="group">
            <span class="lbl-key">{t("catalogReport.reclassified", { count: report.reclassified.length })}</span>
            <p class="mono mods">{report.reclassified.join(", ")}</p>
          </div>
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  /* A report, not a warning: the neutral surface of the app, with the orange
     hairline the rules spec gives this banner (REGLES§12, `#c88a2a` there,
     `--orange` here). No red: nothing in it belongs to the session. */
  .banner {
    background: var(--panel2);
    border: 1px solid var(--line);
    border-left: 2px solid var(--orange);
    padding: 8px 12px;
    margin-bottom: 14px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  .title {
    font-size: 12.5px;
    color: var(--txt);
  }
  .n {
    font-size: 11.5px;
    color: var(--muted);
  }
  .acts {
    margin-left: auto;
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .x {
    width: 20px;
    height: 20px;
    background: none;
    color: var(--muted2);
  }
  .x:hover {
    color: var(--txt);
  }
  .err {
    margin-top: 6px;
    font-size: 11.5px;
    color: var(--rosso-bright);
  }
  .detail {
    margin-top: 10px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .group ul {
    list-style: none;
    margin: 4px 0 0;
    padding: 0;
  }
  .group li {
    display: flex;
    gap: 10px;
    font-size: 12px;
    color: var(--txt2);
    padding: 1px 0;
  }
  .kind {
    min-width: 170px;
    color: var(--muted);
  }
  .mods {
    margin: 4px 0 0;
    font-size: 11.5px;
    color: var(--txt2);
    line-height: 1.5;
  }
</style>
