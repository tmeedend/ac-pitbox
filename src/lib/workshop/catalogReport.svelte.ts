// The last update of the rules catalogue, as the notification and the
// Workshop banner show it (REGLES§6.2): applied automatically at startup,
// reported after, undone in one click.
//
// One state for both: the notification and the banner say the same thing and
// act on the same file (`catalog-state.json`), so closing one closes both.
import { invoke } from "@tauri-apps/api/core";
import { invokeSafe } from "$lib/invokeSafe";
import { errorText } from "$lib/errors";
import { bumpLibraryVersion } from "$lib/library/libraryVersion.svelte";

export interface CatalogChange {
  /** `brand_fix`, `tag_merge`, `family`, `country_alias`… */
  list: string;
  key: string;
  /** What the entry does, in the rules' own words - data, not prose. */
  label: string;
  /** Reclassified mods this rule acted on; absent for the taxonomy tables,
   * which the engine does not trace. */
  mods?: number;
}

export interface CatalogReport {
  from_version: string;
  to_version: string;
  changes: { added: CatalogChange[]; corrected: CatalogChange[]; retired: CatalogChange[] };
  /** Mods the update classified differently - the measured effect. */
  reclassified: string[];
}

export interface CatalogReportView {
  report: CatalogReport | null;
  can_revert: boolean;
  reverted: boolean;
  previous_version: string | null;
  current_version: string | null;
  /** Rule ids (and `#track-category` names) the user switched off. */
  disabled: string[];
}

export const catalogReport = $state<{ view: CatalogReportView | null; busy: boolean; error: string }>({
  view: null,
  busy: false,
  error: "",
});

/** A READ: `invokeSafe`, a missing report is better than a frozen shell. */
export async function loadCatalogReport(): Promise<void> {
  catalogReport.view = await invokeSafe<CatalogReportView | null>("get_catalog_report", undefined, null);
}

/**
 * "Go back to the previous catalogue", or return to the current one. A WRITE,
 * and a slow one (the library is re-classified): plain `invoke`, the error
 * shown and logged rather than swallowed (CLAUDE.md rule 6).
 */
export async function setCatalogReverted(reverted: boolean): Promise<void> {
  if (catalogReport.busy) return;
  catalogReport.busy = true;
  catalogReport.error = "";
  try {
    await invoke<number>("set_catalog_reverted", { reverted });
    bumpLibraryVersion();
  } catch (e) {
    console.error("set_catalog_reverted", e);
    catalogReport.error = errorText(e);
  } finally {
    catalogReport.busy = false;
    await loadCatalogReport();
  }
}

/** "Disable" on a report line (REGLES§6.3): the Rules screen's switch, then
 * the library re-applied. A WRITE: shown and logged on failure. */
export async function setRuleEnabled(list: string, key: string, enabled: boolean): Promise<void> {
  if (catalogReport.busy) return;
  catalogReport.busy = true;
  catalogReport.error = "";
  try {
    await invoke<number>("set_rule_enabled", { list, key, enabled });
    bumpLibraryVersion();
  } catch (e) {
    console.error("set_rule_enabled", e);
    catalogReport.error = errorText(e);
  } finally {
    catalogReport.busy = false;
    await loadCatalogReport();
  }
}

export async function dismissCatalogReport(): Promise<void> {
  try {
    await invoke("dismiss_catalog_report");
  } catch (e) {
    console.error("dismiss_catalog_report", e);
  }
  await loadCatalogReport();
}

/** The title's versions: the catalogue is embedded, so the application
 * version names it. A development build changes the catalogue without a new
 * version - no arrow between two equal names then. */
export function sameVersion(r: CatalogReport): boolean {
  return r.from_version === r.to_version;
}
