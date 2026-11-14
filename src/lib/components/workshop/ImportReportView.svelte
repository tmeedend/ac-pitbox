<script lang="ts">
  // The report of an import (EXPORT§7.3, step 5): everything it did, heading
  // by heading, each with the mods it concerns. A merge touches a library the
  // user already had, and this window is the one place they learn what
  // changed in it — so nothing is summed up into "done".
  import { t } from "$lib/i18n/index.svelte";
  import { GROUP_LABEL, GROUPS, type ImportReport, type Named } from "$lib/workshop/transfer";

  interface Props {
    report: ImportReport;
  }
  let { report }: Props = $props();

  /** The classification files by what they hold, as the user knows them. */
  const CLASSIFICATION: Record<string, string> = {
    "taxonomy.json": "transfer.classTaxonomy",
    "rules-overlay.json": "transfer.classRules",
    "tag-rules.json": "transfer.classRulesLegacy",
    "brand_logos.json": "transfer.classLogos",
  };

  type Section = { label: string; items: Named[] };

  const sections = $derived.by((): Section[] => {
    const r = report;
    const out: Section[] = [
      { label: t("transfer.reportAdded", { count: r.added.length }), items: r.added },
      { label: t("transfer.reportAddedAddons", { count: r.added_addons.length }), items: r.added_addons },
      { label: t("transfer.reportKept", { count: r.kept.length }), items: r.kept },
      { label: t("transfer.reportFilled", { count: r.filled.length }), items: r.filled },
    ];
    for (const group of GROUPS) {
      const d = r.decided.find((x) => x.group === group);
      if (!d) continue;
      const key = d.side === "export" ? "transfer.reportDecidedExport" : "transfer.reportDecidedHere";
      out.push({ label: t(key, { group: t(GROUP_LABEL[group]), count: d.items.length }), items: d.items });
    }
    out.push(
      { label: t("transfer.reportNotes", { count: r.notes_joined.length }), items: r.notes_joined },
      { label: t("transfer.reportBlocked", { count: r.blocked.length }), items: r.blocked },
    );
    return out.filter((s) => s.items.length > 0);
  });

  const renamed = $derived([
    ...report.profiles_renamed.map((n) => t("transfer.reportProfileRenamed", { name: n })),
    ...report.grids_renamed.map((n) => t("transfer.reportGridRenamed", { name: n })),
  ]);
</script>

<div class="report">
  {#if report.mods}
    <p class="line">{t("transfer.importedMods", { count: report.mods })}</p>
  {/if}

  {#each sections as s (s.label)}
    <details class="section">
      <summary>{s.label}</summary>
      <ul class="names">
        <!-- Not keyed: an app and an "other" mod can share an id. -->
        {#each s.items as item}
          <li>{item.name}{#if item.name !== item.id}<span class="mono id"> {item.id}</span>{/if}</li>
        {/each}
      </ul>
    </details>
  {/each}

  {#if report.stock_applied}
    <p class="line">{t("transfer.stockApplied", { count: report.stock_applied })}</p>
  {/if}
  {#if report.thumbs_adopted}
    <p class="line">{t("transfer.reportThumbs", { count: report.thumbs_adopted })}</p>
  {/if}
  {#if report.attached_skipped}
    <p class="line muted">{t("transfer.reportAttachedSkipped", { count: report.attached_skipped })}</p>
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
  {#each renamed as line (line)}
    <p class="line">{line}</p>
  {/each}
  {#if report.classification_kept.length}
    <p class="line">
      {t("transfer.reportClassificationKept")}
      <span class="ids">
        {report.classification_kept.map((f) => (CLASSIFICATION[f] ? t(CLASSIFICATION[f]) : f)).join(", ")}
      </span>
    </p>
  {/if}
  {#if report.active_at_export}
    <p class="line muted">{t("transfer.activeAtExport", { count: report.active_at_export })}</p>
  {/if}
</div>

<style>
  .report {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: calc(60 * var(--vh));
    overflow-y: auto;
  }
  .line {
    color: var(--txt2);
    font-size: 12px;
    line-height: 1.45;
  }
  .line.muted {
    color: var(--muted);
    font-size: 11.5px;
  }
  .section {
    background: var(--panel2);
    border: 1px solid var(--line);
    padding: 6px 10px;
  }
  summary {
    color: var(--txt2);
    font-size: 12px;
    cursor: pointer;
  }
  .names {
    list-style: none;
    margin: 6px 0 2px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .names li {
    color: var(--muted);
    font-size: 11.5px;
  }
  .id {
    color: var(--faint);
    font-size: 10.5px;
  }
  .ids {
    display: block;
    color: var(--muted);
    font-size: 11px;
    overflow-wrap: anywhere;
  }
</style>
