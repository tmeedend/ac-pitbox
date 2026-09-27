// What reading the cars' files changed in the library (FICHE§9.3), as the
// notification shows it: kept on disk until closed (`techsheet-report.json`),
// since the reading may end before the screen listens.
import { invoke } from "@tauri-apps/api/core";
import { invokeSafe } from "$lib/invokeSafe";
import type { TechSheetReport } from "$lib/detail/techSheet";

export const techSheetReport = $state<{ report: TechSheetReport | null }>({ report: null });

/** A READ: `invokeSafe`, a missing report is better than a frozen shell. */
export async function loadTechSheetReport(): Promise<void> {
  techSheetReport.report = await invokeSafe<TechSheetReport | null>("get_techsheet_report", undefined, null);
}

/** "Close". On failure the report reloads and stays on screen, which is what
 * says the close did not happen. */
export async function dismissTechSheetReport(): Promise<void> {
  try {
    await invoke("dismiss_techsheet_report");
  } catch (e) {
    console.error("dismiss_techsheet_report", e);
  }
  await loadTechSheetReport();
}
