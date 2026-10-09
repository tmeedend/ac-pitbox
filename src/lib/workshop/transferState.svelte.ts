// The library export being written (EXPORT§7.2), and its outcome.
//
// Here and not in the Maintenance screen, for the reason the survey lives in
// `surveyState`: an export walks every folder of the library, nothing obliges
// one to stay on Fichiers meanwhile, and the notification stack is what keeps
// it — and the "open the folder" that ends it — visible across pages.
import { errorText } from "$lib/errors";
import { exportLibrary, type ExportReport, type Part } from "./transfer";

export const exportState = $state<{
  running: boolean;
  /** What the last one produced, until closed. */
  result: ExportReport | null;
  error: string;
}>({ running: false, result: null, error: "" });

/** One at a time: two exports would read the same library twice. */
export async function runExport(path: string, parts: Part[]): Promise<void> {
  if (exportState.running) return;
  exportState.running = true;
  exportState.result = null;
  exportState.error = "";
  try {
    exportState.result = await exportLibrary(path, parts);
  } catch (e) {
    console.error("library export", e);
    exportState.error = errorText(e);
  } finally {
    exportState.running = false;
  }
}

export function dismissExport(): void {
  exportState.result = null;
  exportState.error = "";
}
